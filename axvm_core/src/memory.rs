use libc;
use std::ptr;

pub struct GuestMemory {
    ptr: *mut u8,
    len: usize,
}

impl GuestMemory {
    pub fn new(size: usize) -> Result<Self, String> {
        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err("AxVM Panic: Falha crítica no mmap (Host OOM?).".to_string());
        }

        Ok(GuestMemory { ptr: ptr as *mut u8, len: size })
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    pub fn write_u64(&mut self, offset: usize, value: u64) {
        if offset + 8 > self.len { return; }
        unsafe {
            let addr = self.ptr.add(offset) as *mut u64;
            *addr = value;
        }
    }

    pub fn write_slice(&mut self, offset: usize, data: &[u8]) {
        if offset + data.len() > self.len { return; }
        unsafe {
            let dest = self.ptr.add(offset);
            ptr::copy_nonoverlapping(data.as_ptr(), dest, data.len());
        }
    }
}

impl Drop for GuestMemory {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.len) };
        }
    }
}
