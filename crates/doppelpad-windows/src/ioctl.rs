//! Wire format for the DoppelPad driver's control interface.
//!
//! These constants and structures mirror `driver/windows/Public.h` and
//! `WinUHidApi.h`. They are the source of truth shared with the C driver; keep
//! them in sync. The event structs are `#[repr(C, packed)]` to match the
//! driver's `#pragma pack(1)` blocks; `DeviceInfo` is plain `#[repr(C)]`
//! because its C definition is not packed (see its doc comment).

#![allow(dead_code)]

/// Win32 path of the driver's control device (`\\.\DoppelPad`).
pub const CONTROL_DEVICE_PATH: &str = r"\\.\DoppelPad";

/// Latest control-interface version understood by this client.
pub const INTERFACE_VERSION_LATEST: u32 = 1;

// --- CTL_CODE construction -------------------------------------------------
// See <winioctl.h>: CTL_CODE(DeviceType, Function, Method, Access).

const FILE_DEVICE_UNKNOWN: u32 = 0x0000_0022;
const METHOD_BUFFERED: u32 = 0;
const FILE_WRITE_DATA: u32 = 0x0002;
const FILE_ANY_ACCESS: u32 = 0;

const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

const fn winuhid_ctl_code(x: u32) -> u32 {
    ctl_code(
        FILE_DEVICE_UNKNOWN,
        0x800 + x,
        METHOD_BUFFERED,
        FILE_WRITE_DATA,
    )
}

const fn winuhid_na_ctl_code(x: u32) -> u32 {
    ctl_code(
        FILE_DEVICE_UNKNOWN,
        0x800 + x,
        METHOD_BUFFERED,
        FILE_ANY_ACCESS,
    )
}

pub const IOCTL_SET_DEVICE_INFO: u32 = winuhid_ctl_code(0);
pub const IOCTL_SET_REPORT_DESCRIPTOR: u32 = winuhid_ctl_code(1);
pub const IOCTL_SET_INSTANCE_ID: u32 = winuhid_ctl_code(2);
pub const IOCTL_SET_HARDWARE_IDS: u32 = winuhid_ctl_code(3);
pub const IOCTL_CREATE_DEVICE: u32 = winuhid_ctl_code(4);
pub const IOCTL_START_DEVICE: u32 = winuhid_ctl_code(5);
pub const IOCTL_GET_NEXT_EVENT: u32 = winuhid_ctl_code(6);
pub const IOCTL_COMPLETE_WRITE_EVENT: u32 = winuhid_ctl_code(7);
pub const IOCTL_COMPLETE_READ_EVENT: u32 = winuhid_ctl_code(8);
pub const IOCTL_GET_INTERFACE_VERSION: u32 = winuhid_na_ctl_code(9);

// --- Event type flags (WINUHID_EVENT_TYPE) ---------------------------------

pub const EVENT_NONE: u32 = 0x0;
pub const EVENT_GET_FEATURE: u32 = 0x1;
pub const EVENT_SET_FEATURE: u32 = 0x2;
pub const EVENT_WRITE_REPORT: u32 = 0x4;
pub const EVENT_READ_REPORT: u32 = 0x8;

/// A 16-byte GUID, matching the Win32 `GUID` layout.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

/// Input for `IOCTL_SET_DEVICE_INFO` (`WINUHID_DEVICE_INFO`).
///
/// Unlike the event structs, this one is not under `#pragma pack(1)` in the
/// driver header, so it uses default C alignment: the `GUID` is 4-aligned,
/// padding the struct to 32 bytes. Must be `repr(C)`, not packed.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DeviceInfo {
    pub interface_version: u32,
    pub supported_events: u32,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version_number: u16,
    pub container_id: Guid,
}

/// Input for `IOCTL_COMPLETE_WRITE_EVENT` (`WINUHID_WRITE_EVENT_COMPLETE`).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct WriteEventComplete {
    pub request_id: u32,
    /// NTSTATUS.
    pub status: i32,
}

/// Header of `IOCTL_COMPLETE_READ_EVENT` input (`WINUHID_READ_EVENT_COMPLETE`
/// without its trailing `Data[]`). The report bytes follow this header.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ReadEventCompleteHeader {
    pub request_id: u32,
    /// NTSTATUS.
    pub status: i32,
    pub data_length: u32,
}

/// Fixed-size prefix of a `WINUHID_EVENT` as returned by `IOCTL_GET_NEXT_EVENT`.
///
/// The full struct is a packed union; this prefix covers the common fields plus
/// the leading `DataLength` of both union arms (identical offset). For write
/// events, `data_length` report bytes follow at offset `size_of::<Self>()`.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EventHeader {
    pub event_type: u32,
    pub request_id: u32,
    pub report_id: u8,
    pub data_length: u32,
}

// NTSTATUS success sentinel.
pub const STATUS_SUCCESS: i32 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctl_codes_match_public_h() {
        // Values computed from winioctl.h CTL_CODE with the driver's parameters:
        // buffered method, FILE_WRITE_DATA access (0x2) for the numbered ioctls,
        // FILE_ANY_ACCESS for the version query.
        assert_eq!(IOCTL_SET_DEVICE_INFO, 0x0022_A000);
        assert_eq!(IOCTL_CREATE_DEVICE, 0x0022_A010);
        assert_eq!(IOCTL_GET_INTERFACE_VERSION, 0x0022_2024);
    }

    #[test]
    fn struct_layouts_are_packed() {
        // Default C alignment pads the GUID to offset 16, so 32 bytes, not 30.
        assert_eq!(core::mem::size_of::<DeviceInfo>(), 32);
        assert_eq!(core::mem::size_of::<WriteEventComplete>(), 8);
        assert_eq!(core::mem::size_of::<ReadEventCompleteHeader>(), 12);
        assert_eq!(core::mem::size_of::<EventHeader>(), 13);
    }
}
