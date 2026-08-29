//! Virtual HID device backed by the DoppelPad driver.

use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, WriteFile,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::core::PCWSTR;

use crate::error::{Error, Result};
use crate::ioctl;

const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;

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

    /// Build, create, and start the device. On success the virtual HID device is
    /// live until the returned handle is dropped.
    pub fn build(self) -> Result<VirtualHidDevice> {
        if self.report_descriptor.is_empty() || self.report_descriptor.len() > u16::MAX as usize {
            return Err(Error::InvalidReportDescriptor);
        }

        let handle = open_control_device()?;
        let device = VirtualHidDevice { handle };

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
        Ok(device)
    }
}

/// A live virtual HID device. Dropping it removes the device from the system.
pub struct VirtualHidDevice {
    handle: HANDLE,
}

// The handle is owned exclusively and all operations on it are OS-serialized, so
// it is safe to move between threads. Concurrent submission is left to the caller.
unsafe impl Send for VirtualHidDevice {}

impl VirtualHidDevice {
    /// Submit a HID input report. For a numbered-report device the first byte
    /// must be the report ID. The report must match the descriptor's size.
    pub fn submit_input_report(&self, report: &[u8]) -> Result<()> {
        let mut written = 0u32;
        unsafe { WriteFile(self.handle, Some(report), Some(&mut written), None) }
            .map_err(Error::SubmitReport)
    }

    /// Query the driver's control-interface version. Useful as a liveness check.
    pub fn interface_version(&self) -> Result<u32> {
        let mut version = 0u32;
        let mut returned = 0u32;
        unsafe {
            DeviceIoControl(
                self.handle,
                ioctl::IOCTL_GET_INTERFACE_VERSION,
                None,
                0,
                Some(&mut version as *mut u32 as *mut c_void),
                size_of::<u32>() as u32,
                Some(&mut returned),
                None,
            )
        }
        .map_err(|source| Error::Ioctl {
            op: "get_interface_version",
            source,
        })?;
        if (returned as usize) < size_of::<u32>() {
            return Err(Error::ShortResponse {
                op: "get_interface_version",
            });
        }
        Ok(version)
    }

    fn ioctl_in(&self, op: &'static str, code: u32, input: &[u8]) -> Result<()> {
        let (ptr, len) = if input.is_empty() {
            (None, 0)
        } else {
            (Some(input.as_ptr() as *const c_void), input.len() as u32)
        };
        let mut returned = 0u32;
        unsafe {
            DeviceIoControl(
                self.handle,
                code,
                ptr,
                len,
                None,
                0,
                Some(&mut returned),
                None,
            )
        }
        .map_err(|source| Error::Ioctl { op, source })
    }
}

impl Drop for VirtualHidDevice {
    fn drop(&mut self) {
        // Closing the control handle tears down the VHF child device.
        unsafe {
            let _ = CloseHandle(self.handle);
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
            FILE_ATTRIBUTE_NORMAL,
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
    // SAFETY: T is a packed POD struct; we read size_of::<T> bytes from it.
    unsafe { std::slice::from_raw_parts(value as *const T as *const u8, size_of::<T>()) }
}
