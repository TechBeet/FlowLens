//! Rust interface to the FlowLens WFP kernel driver.

use std::fs::OpenOptions;
use std::io;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DriverTrafficEvent {
    pub timestamp: u64,
    pub pid: u32,
    pub process_name: [u8; 260],
    pub src_addr: u32,
    pub src_port: u16,
    pub dst_addr: u32,
    pub dst_port: u16,
    pub protocol: u8,
    pub direction: u8,
    pub data_size: u32,
}

impl Default for DriverTrafficEvent {
    fn default() -> Self {
        Self {
            timestamp: 0, pid: 0, process_name: [0; 260],
            src_addr: 0, src_port: 0, dst_addr: 0, dst_port: 0,
            protocol: 0, direction: 0, data_size: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DriverFilter {
    pub src_addr: u32,
    pub src_mask: u32,
    pub dst_addr: u32,
    pub dst_mask: u32,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
    pub direction: u8,
    pub enabled: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DriverStatus {
    pub capturing: u8,
    pub events_dropped: u32,
    pub events_written: u32,
    pub active_filters: u32,
    pub uptime_100ns: u64,
}

pub const IOCTL_START_CAPTURE: u32 = 0x0022_2000;
pub const IOCTL_STOP_CAPTURE: u32 = 0x0022_2004;
pub const IOCTL_ADD_FILTER: u32 = 0x0022_2008;
pub const IOCTL_REMOVE_FILTER: u32 = 0x0022_200C;
pub const IOCTL_READ_EVENTS: u32 = 0x0022_2010;
pub const IOCTL_GET_STATUS: u32 = 0x0022_2014;

pub struct DriverHandle {
    handle: std::fs::File,
}

impl DriverHandle {
    pub fn open() -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(r"\\.\FlowLens")?;
        Ok(Self { handle: file })
    }

    pub fn is_available() -> bool {
        OpenOptions::new().read(true).write(true).open(r"\\.\FlowLens").is_ok()
    }

    fn device_io_control(&self, code: u32, in_buf: Option<&[u8]>, out_buf: Option<&mut [u8]>) -> io::Result<u32> {
        #[link(name = "kernel32")]
        extern "C" {
            fn DeviceIoControl(
                h: *mut std::ffi::c_void, code: u32,
                in_buf: *const std::ffi::c_void, in_size: u32,
                out_buf: *mut std::ffi::c_void, out_size: u32,
                bytes_returned: *mut u32, overlapped: *mut std::ffi::c_void,
            ) -> i32;
        }
        use std::os::windows::io::AsRawHandle;
        let h = self.handle.as_raw_handle();
        let in_ptr = in_buf.map(|b| b.as_ptr()).unwrap_or(std::ptr::null());
        let in_size = in_buf.map(|b| b.len() as u32).unwrap_or(0);
        let (out_ptr, out_size) = match out_buf {
            Some(b) => (b.as_mut_ptr(), b.len() as u32),
            None => (std::ptr::null_mut(), 0),
        };
        let mut bytes_returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(h, code, in_ptr as *const _, in_size, out_ptr as *mut _, out_size, &mut bytes_returned, std::ptr::null_mut())
        };
        if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(bytes_returned) }
    }

    pub fn start_capture(&self) -> io::Result<()> {
        self.device_io_control(IOCTL_START_CAPTURE, None, None)?;
        Ok(())
    }

    pub fn stop_capture(&self) -> io::Result<()> {
        self.device_io_control(IOCTL_STOP_CAPTURE, None, None)?;
        Ok(())
    }

    pub fn add_filter(&self, filter: &DriverFilter) -> io::Result<()> {
        let bytes = unsafe { std::slice::from_raw_parts(filter as *const _ as *const u8, std::mem::size_of::<DriverFilter>()) };
        self.device_io_control(IOCTL_ADD_FILTER, Some(bytes), None)?;
        Ok(())
    }

    pub fn remove_filter(&self, index: u32) -> io::Result<()> {
        let bytes = index.to_ne_bytes();
        self.device_io_control(IOCTL_REMOVE_FILTER, Some(&bytes), None)?;
        Ok(())
    }

    pub fn read_events(&self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.device_io_control(IOCTL_READ_EVENTS, None, Some(buf))?;
        Ok(n as usize)
    }

    pub fn get_status(&self) -> io::Result<DriverStatus> {
        let mut buf = [0u8; std::mem::size_of::<DriverStatus>()];
        let n = self.device_io_control(IOCTL_GET_STATUS, None, Some(&mut buf))?;
        if n as usize != std::mem::size_of::<DriverStatus>() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "short status read"));
        }
        Ok(unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const DriverStatus) })
    }
}

pub struct DriverCaptureEngine {
    handle: DriverHandle,
    pub running: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl DriverCaptureEngine {
    pub fn try_open() -> Option<Self> {
        DriverHandle::open().ok().map(|h| Self {
            handle: h,
            running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    pub fn is_available() -> bool {
        DriverHandle::is_available()
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        self.handle.start_capture()?;
        self.running.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.handle.stop_capture()?;
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    pub fn read_events(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.handle.read_events(buf)
    }

    pub fn get_status(&self) -> io::Result<DriverStatus> {
        self.handle.get_status()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_ioctl_constants() {
        assert_eq!(IOCTL_START_CAPTURE, 0x0022_2000);
        assert_eq!(IOCTL_READ_EVENTS, 0x0022_2010);
    }
    #[test]
    fn test_driver_handle_is_available_callable() {
        let _ = DriverHandle::is_available();
    }
}
