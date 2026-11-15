//! Virtual HID device backed by the DoppelPad driver.
//!
//! The control handle is opened with `FILE_FLAG_OVERLAPPED`. This is essential:
//! a plain synchronous handle serializes all I/O, so the reader thread's
//! long-pending `GET_NEXT_EVENT` would block `submit_input_report` on the same
//! handle. Overlapped I/O lets input submission and event reading proceed
//! independently, and gives the reader a clean stop via an event object.

use std::ffi::c_void;
use std::thread::JoinHandle;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, WriteFile,
};
use windows::Win32::System::IO::{CancelIoEx, DeviceIoControl, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::{
    CreateEventW, INFINITE, ResetEvent, SetEvent, WaitForMultipleObjects,
};
use windows::core::PCWSTR;

use crate::error::{Error, Result};
use crate::ioctl;

const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
// HRESULT_FROM_WIN32(ERROR_IO_PENDING = 997).
const HRESULT_IO_PENDING: i32 = 0x8007_03E5u32 as i32;

/// Callback invoked (on an internal reader thread) for each output report, with
/// a borrowed slice: the report id, then the report body. Borrowing avoids a
/// per-event allocation.
pub type OutputCallback = Box<dyn FnMut(u8, &[u8]) + Send>;

/// Events a device wishes to handle from its HID clients. A plain input-only
/// device (mouse, keyboard, basic gamepad) needs none of these; output-capable
/// devices (rumble, feature reports) opt in.
#[derive(Clone, Copy, Default)]
pub struct SupportedEvents(u32);

impl SupportedEvents {
    pub const NONE: Self = Self(ioctl::EVENT_NONE);

    pub fn get_feature(mut self, on: bool) -> Self {
        self.set(ioctl::EVENT_GET_FEATURE, on);
        self
    }
    pub fn set_feature(mut self, on: bool) -> Self {
        self.set(ioctl::EVENT_SET_FEATURE, on);
        self
    }
    pub fn write_report(mut self, on: bool) -> Self {
        self.set(ioctl::EVENT_WRITE_REPORT, on);
        self
    }
    pub fn read_report(mut self, on: bool) -> Self {
        self.set(ioctl::EVENT_READ_REPORT, on);
        self
    }

    fn set(&mut self, bit: u32, on: bool) {
        if on {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }
}

/// Builder for a [`VirtualHidDevice`].
pub struct VirtualHidDeviceBuilder {
    vendor_id: u16,
    product_id: u16,
    version_number: u16,
    report_descriptor: Vec<u8>,
    supported_events: SupportedEvents,
    instance_id: Option<Vec<u16>>,
    hardware_ids: Option<Vec<u16>>,
    container_id: ioctl::Guid,
    on_output: Option<OutputCallback>,
}

impl VirtualHidDeviceBuilder {
    /// Start a device with the given USB identity and HID report descriptor.
    pub fn new(vendor_id: u16, product_id: u16, report_descriptor: impl Into<Vec<u8>>) -> Self {
        Self {
            vendor_id,
            product_id,
            version_number: 0,
            report_descriptor: report_descriptor.into(),
            supported_events: SupportedEvents::NONE,
            instance_id: None,
            hardware_ids: None,
            container_id: ioctl::Guid::default(),
            on_output: None,
        }
    }

    pub fn version_number(mut self, version: u16) -> Self {
        self.version_number = version;
        self
    }

    pub fn supported_events(mut self, events: SupportedEvents) -> Self {
        self.supported_events = events;
        self
    }

    /// Distinguish this instance among identical devices (a wide, NUL-terminated
    /// string is sent to the driver).
    pub fn instance_id(mut self, id: &str) -> Self {
        self.instance_id = Some(wide_nul(id));
        self
    }

    /// Additional PnP hardware IDs, as a REG_MULTI_SZ (double-NUL terminated).
    pub fn hardware_ids<'a>(mut self, ids: impl IntoIterator<Item = &'a str>) -> Self {
        self.hardware_ids = Some(multi_sz(ids));
        self
    }

    /// Set the device container ID. A non-null container groups the device as a
    /// distinct physical device; root-enumerated devices otherwise get a null
    /// sentinel container that some XInput code treats specially.
    pub fn container_id(mut self, data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        self.container_id = ioctl::Guid {
            data1,
            data2,
            data3,
            data4,
        };
        self
    }

    /// Register a callback for output reports (rumble, LEDs). Implies the
    /// `write_report` event.
    pub fn on_output(mut self, callback: OutputCallback) -> Self {
        self.supported_events = self.supported_events.write_report(true);
        self.on_output = Some(callback);
        self
    }

    /// Build, create, and start the device. On success the virtual HID device is
    /// live until the returned handle is dropped.
    pub fn build(self) -> Result<VirtualHidDevice> {
        if self.report_descriptor.is_empty() || self.report_descriptor.len() > u16::MAX as usize {
            return Err(Error::InvalidReportDescriptor);
        }

        // Created before the handle so a failure here cannot leak an unwrapped handle.
        let write_event = Event::new().map_err(|source| Error::Ioctl {
            op: "create_write_event",
            source,
        })?;
        let handle = open_control_device()?;
        let mut device = VirtualHidDevice {
            handle,
            write_event,
            reader: None,
        };

        let info = ioctl::DeviceInfo {
            interface_version: ioctl::INTERFACE_VERSION_LATEST,
            supported_events: self.supported_events.0,
            vendor_id: self.vendor_id,
            product_id: self.product_id,
            version_number: self.version_number,
            container_id: self.container_id,
        };
        device.ioctl_in(
            "set_device_info",
            ioctl::IOCTL_SET_DEVICE_INFO,
            as_bytes(&info),
        )?;
        device.ioctl_in(
            "set_report_descriptor",
            ioctl::IOCTL_SET_REPORT_DESCRIPTOR,
            &self.report_descriptor,
        )?;
        if let Some(id) = &self.instance_id {
            device.ioctl_in(
                "set_instance_id",
                ioctl::IOCTL_SET_INSTANCE_ID,
                wide_bytes(id),
            )?;
        }
        if let Some(ids) = &self.hardware_ids {
            device.ioctl_in(
                "set_hardware_ids",
                ioctl::IOCTL_SET_HARDWARE_IDS,
                wide_bytes(ids),
            )?;
        }
        device.ioctl_in("create_device", ioctl::IOCTL_CREATE_DEVICE, &[])?;
        device.ioctl_in("start_device", ioctl::IOCTL_START_DEVICE, &[])?;

        // Spawn the output-report reader if the caller registered a callback.
        if let Some(callback) = self.on_output {
            device.reader = Some(Reader::spawn(handle, callback)?);
        }
        Ok(device)
    }
}

/// A live virtual HID device. Dropping it removes the device from the system.
pub struct VirtualHidDevice {
    handle: HANDLE,
    // Reused by every submit_input_report. Safe only because this type is Send but
    // not Sync, so two submissions never overlap; do not add a Sync impl.
    write_event: Event,
    reader: Option<Reader>,
}

// The handle is owned exclusively and overlapped I/O on it is not serialized, so
// it is safe to move between threads. Concurrent submission is left to the caller.
unsafe impl Send for VirtualHidDevice {}

impl VirtualHidDevice {
    /// Submit a HID input report. For a numbered-report device the first byte
    /// must be the report ID. The report must match the descriptor's size.
    pub fn submit_input_report(&self, report: &[u8]) -> Result<()> {
        unsafe {
            let _ = ResetEvent(self.write_event.0);
        }
        let mut ov = OVERLAPPED {
            hEvent: self.write_event.0,
            ..Default::default()
        };
        let mut written = 0u32;
        let start =
            unsafe { WriteFile(self.handle, Some(report), Some(&mut written), Some(&mut ov)) };
        finish_overlapped(self.handle, &ov, start).map_err(Error::SubmitReport)?;
        Ok(())
    }

    /// Query the driver's control-interface version. Useful as a liveness check.
    pub fn interface_version(&self) -> Result<u32> {
        let mut version = 0u32;
        let n = self
            .ioctl(
                "get_interface_version",
                ioctl::IOCTL_GET_INTERFACE_VERSION,
                &[],
                as_bytes_mut(&mut version),
            )
            .map_err(|source| Error::Ioctl {
                op: "get_interface_version",
                source,
            })?;
        if (n as usize) < size_of::<u32>() {
            return Err(Error::ShortResponse {
                op: "get_interface_version",
            });
        }
        Ok(version)
    }

    fn ioctl_in(&self, op: &'static str, code: u32, input: &[u8]) -> Result<()> {
        self.ioctl(op, code, input, &mut [])
            .map(|_| ())
            .map_err(|source| Error::Ioctl { op, source })
    }

    fn ioctl(
        &self,
        _op: &'static str,
        code: u32,
        input: &[u8],
        output: &mut [u8],
    ) -> std::result::Result<u32, windows::core::Error> {
        device_io_control(self.handle, code, input, output)
    }
}

impl Drop for VirtualHidDevice {
    fn drop(&mut self) {
        // Stop the reader (if any) before closing the handle it shares.
        if let Some(reader) = self.reader.take() {
            reader.stop(self.handle);
        }
        // Closing the control handle tears down the VHF child device.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// Background thread that pumps output events and invokes the user callback.
struct Reader {
    stop_event: Event,
    thread: Option<JoinHandle<()>>,
}

impl Reader {
    fn spawn(handle: HANDLE, callback: OutputCallback) -> Result<Self> {
        let stop_event = Event::new().map_err(|source| Error::Ioctl {
            op: "create_stop_event",
            source,
        })?;
        let raw_handle = handle.0 as isize;
        let raw_stop = stop_event.0.0 as isize;
        let thread = std::thread::spawn(move || {
            reader_loop(
                HANDLE(raw_handle as *mut c_void),
                HANDLE(raw_stop as *mut c_void),
                callback,
            );
        });
        Ok(Reader {
            stop_event,
            thread: Some(thread),
        })
    }

    fn stop(mut self, handle: HANDLE) {
        // Signal the reader and cancel any pending event ioctl on the handle.
        unsafe {
            let _ = SetEvent(self.stop_event.0);
            let _ = CancelIoEx(handle, None);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn reader_loop(handle: HANDLE, stop_event: HANDLE, mut callback: OutputCallback) {
    let io_event = match Event::new() {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut buf = vec![0u8; 4096];
    loop {
        unsafe {
            let _ = ResetEvent(io_event.0);
        }
        let mut ov = OVERLAPPED {
            hEvent: io_event.0,
            ..Default::default()
        };
        let mut returned = 0u32;
        let start = unsafe {
            DeviceIoControl(
                handle,
                ioctl::IOCTL_GET_NEXT_EVENT,
                None,
                0,
                Some(buf.as_mut_ptr() as *mut c_void),
                buf.len() as u32,
                Some(&mut returned),
                Some(&mut ov),
            )
        };

        let n = match start {
            Ok(()) => returned,
            Err(e) if e.code().0 == HRESULT_IO_PENDING => {
                let objects = [io_event.0, stop_event];
                let waited = unsafe { WaitForMultipleObjects(&objects, false, INFINITE) };
                if waited != WAIT_OBJECT_0 {
                    // Stop requested (or wait failed): cancel, then wait for the request to
                    // finish, since cancellation is async and the kernel still owns ov/buf/io_event.
                    let mut got = 0u32;
                    unsafe {
                        let _ = CancelIoEx(handle, Some(&ov));
                        let _ = GetOverlappedResult(handle, &ov, &mut got, true);
                    }
                    break;
                }
                let mut got = 0u32;
                if unsafe { GetOverlappedResult(handle, &ov, &mut got, true) }.is_err() {
                    break;
                }
                got
            }
            Err(_) => break,
        };

        let n = n as usize;
        let header_len = size_of::<ioctl::EventHeader>();
        if n < header_len {
            continue;
        }
        // SAFETY: the driver wrote n >= header_len bytes; EventHeader is packed POD.
        let header = unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const ioctl::EventHeader) };
        // Copy out of the packed struct; comparing its fields in place would borrow them.
        let (event_type, request_id, report_id) =
            (header.event_type, header.request_id, header.report_id);
        let data_length = header.data_length as usize;

        if event_type == ioctl::EVENT_WRITE_REPORT || event_type == ioctl::EVENT_SET_FEATURE {
            let end = (header_len + data_length).min(n);
            callback(report_id, &buf[header_len..end]);
            complete_write(handle, request_id, ioctl::STATUS_SUCCESS);
        }
        // Read/GET events are not requested by output-only devices; ignore.
    }
}

fn complete_write(handle: HANDLE, request_id: u32, status: i32) {
    let complete = ioctl::WriteEventComplete { request_id, status };
    let _ = device_io_control(
        handle,
        ioctl::IOCTL_COMPLETE_WRITE_EVENT,
        as_bytes(&complete),
        &mut [],
    );
}

/// Issue an overlapped `DeviceIoControl` and wait for completion (synchronous
/// semantics without serializing the handle).
fn device_io_control(
    handle: HANDLE,
    code: u32,
    input: &[u8],
    output: &mut [u8],
) -> std::result::Result<u32, windows::core::Error> {
    let event = Event::new()?;
    let mut ov = OVERLAPPED {
        hEvent: event.0,
        ..Default::default()
    };
    let in_ptr = (!input.is_empty()).then_some(input.as_ptr() as *const c_void);
    let out_ptr = (!output.is_empty()).then_some(output.as_mut_ptr() as *mut c_void);
    let mut returned = 0u32;
    let start = unsafe {
        DeviceIoControl(
            handle,
            code,
            in_ptr,
            input.len() as u32,
            out_ptr,
            output.len() as u32,
            Some(&mut returned),
            Some(&mut ov),
        )
    };
    finish_overlapped(handle, &ov, start)
}

/// Complete an overlapped operation: return immediately on success, or wait for
/// the pending I/O and return the byte count.
fn finish_overlapped(
    handle: HANDLE,
    ov: &OVERLAPPED,
    start: windows::core::Result<()>,
) -> std::result::Result<u32, windows::core::Error> {
    match start {
        Ok(()) => Ok(ov.InternalHigh as u32),
        Err(e) if e.code().0 == HRESULT_IO_PENDING => {
            let mut got = 0u32;
            unsafe { GetOverlappedResult(handle, ov, &mut got, true) }?;
            Ok(got)
        }
        Err(e) => Err(e),
    }
}

/// A Win32 event handle, closed on drop.
struct Event(HANDLE);

impl Event {
    fn new() -> windows::core::Result<Self> {
        // Manual-reset, initially non-signaled.
        let h = unsafe { CreateEventW(None, true, false, None) }?;
        Ok(Event(h))
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn open_control_device() -> Result<HANDLE> {
    let path = wide_nul(ioctl::CONTROL_DEVICE_PATH);
    unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )
    }
    .map_err(Error::Open)
}

fn wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn multi_sz<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<u16> {
    let mut buf = Vec::new();
    for item in items {
        buf.extend(item.encode_utf16());
        buf.push(0);
    }
    buf.push(0); // final terminating NUL for the REG_MULTI_SZ
    buf
}

fn wide_bytes(w: &[u16]) -> &[u8] {
    // SAFETY: reinterpreting a u16 slice as bytes; length is exact.
    unsafe { std::slice::from_raw_parts(w.as_ptr() as *const u8, std::mem::size_of_val(w)) }
}

fn as_bytes<T>(value: &T) -> &[u8] {
    // SAFETY: T is a POD struct; we read size_of::<T> bytes from it.
    unsafe { std::slice::from_raw_parts(value as *const T as *const u8, size_of::<T>()) }
}

fn as_bytes_mut<T>(value: &mut T) -> &mut [u8] {
    // SAFETY: T is a POD struct; we expose size_of::<T> bytes of it.
    unsafe { std::slice::from_raw_parts_mut(value as *mut T as *mut u8, size_of::<T>()) }
}
