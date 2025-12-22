// memory.rs
//!
//! Guest memory management module for AxVM.
//! Provides safe abstractions over raw memory operations with bounds checking,
//! huge page support, and memory protection primitives.

use libc;
use std::ptr;
use std::slice;
use std::fmt;

// ============================================================================
// CONSTANTS
// ============================================================================

const PAGE_SIZE: usize = 4096;

/// 2MB huge page size (used in tests and HugePages2MB backend)
#[cfg(test)]
const HUGE_PAGE_SIZE_2MB: usize = 2 * 1024 * 1024;

// ============================================================================
// MEMORY CONFIGURATION
// ============================================================================

/// Backend type for memory allocation strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MemoryBackend {
    /// Standard anonymous mapping (default)
    Anonymous,
    /// Use 2MB huge pages for better TLB performance
    #[cfg(target_os = "linux")]
    HugePages2MB,
    /// Use 1GB huge pages (requires special kernel config)
    #[cfg(target_os = "linux")]
    HugePages1GB,
}

/// Configuration for guest memory allocation
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    pub size: usize,
    pub backend: MemoryBackend,
    pub zero_on_alloc: bool,
    pub mlock: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            size: 256 * 1024,
            backend: MemoryBackend::Anonymous,
            zero_on_alloc: true,
            mlock: false,
        }
    }
}

// ============================================================================
// GUEST MEMORY
// ============================================================================

/// Guest physical memory region backed by mmap
pub struct GuestMemory {
    ptr: *mut u8,
    len: usize,
    backend: MemoryBackend,
    locked: bool,
}

impl GuestMemory {
    /// Allocates guest memory using mmap with default configuration
    pub fn new(size: usize) -> Result<Self, String> {
        let config = MemoryConfig {
            size,
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Allocates guest memory with custom configuration
    pub fn with_config(config: MemoryConfig) -> Result<Self, String> {
        if config.size == 0 {
            return Err("Memory size cannot be zero".to_string());
        }

        let aligned_size = Self::align_to_page(config.size);
        let flags = Self::build_mmap_flags(&config.backend);

        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                aligned_size,
                libc::PROT_READ | libc::PROT_WRITE,
                flags,
                -1,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(format!(
                "mmap failed for {} bytes with backend {:?}: {}",
                aligned_size,
                config.backend,
                Self::get_errno_string()
            ));
        }

        let ptr = ptr as *mut u8;

        if config.zero_on_alloc {
            unsafe {
                ptr::write_bytes(ptr, 0, aligned_size);
            }
        }

        let mut memory = GuestMemory {
            ptr,
            len: aligned_size,
            backend: config.backend,
            locked: false,
        };

        if config.mlock {
            memory.lock()?;
        }

        Ok(memory)
    }

    /// Builds mmap flags based on backend configuration
    #[inline]
    fn build_mmap_flags(backend: &MemoryBackend) -> i32 {
        let mut flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;

        match backend {
            MemoryBackend::Anonymous => {}
            #[cfg(target_os = "linux")]
            MemoryBackend::HugePages2MB => {
                flags |= libc::MAP_HUGETLB | (21 << libc::MAP_HUGE_SHIFT);
            }
            #[cfg(target_os = "linux")]
            MemoryBackend::HugePages1GB => {
                flags |= libc::MAP_HUGETLB | (30 << libc::MAP_HUGE_SHIFT);
            }
        }

        flags
    }

    // ========================================================================
    // MEMORY ACCESS - RAW POINTERS
    // ========================================================================

    /// Returns raw pointer to guest memory base address
    #[inline]
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// Returns the size of the memory region in bytes
    #[inline]
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the memory region has zero size
    #[inline]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns a read-only slice view of the entire memory region
    #[inline]
    #[allow(dead_code)]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Returns a mutable slice view of the entire memory region
    #[inline]
    #[allow(dead_code)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }

    // ========================================================================
    // MEMORY ACCESS - BOUNDS CHECKED
    // ========================================================================

    /// Validates that an access at [offset, offset+len) is within bounds
    #[inline]
    fn check_bounds(&self, offset: usize, len: usize) -> Result<(), String> {
        if offset.checked_add(len).map_or(true, |end| end > self.len) {
            return Err(format!(
                "Memory access out of bounds: offset={:#x}, len={:#x}, total={:#x}",
                offset, len, self.len
            ));
        }
        Ok(())
    }

    /// Reads a single byte at the specified offset
    #[allow(dead_code)]
    pub fn read_u8(&self, offset: usize) -> Result<u8, String> {
        self.check_bounds(offset, 1)?;
        unsafe { Ok(*self.ptr.add(offset)) }
    }

    /// Reads a u16 at the specified offset (unaligned access supported)
    #[allow(dead_code)]
    pub fn read_u16(&self, offset: usize) -> Result<u16, String> {
        self.check_bounds(offset, 2)?;
        unsafe { Ok((self.ptr.add(offset) as *const u16).read_unaligned()) }
    }

    /// Reads a u32 at the specified offset (unaligned access supported)
    #[allow(dead_code)]
    pub fn read_u32(&self, offset: usize) -> Result<u32, String> {
        self.check_bounds(offset, 4)?;
        unsafe { Ok((self.ptr.add(offset) as *const u32).read_unaligned()) }
    }

    /// Reads a u64 at the specified offset (unaligned access supported)
    #[allow(dead_code)]
    pub fn read_u64(&self, offset: usize) -> Result<u64, String> {
        self.check_bounds(offset, 8)?;
        unsafe { Ok((self.ptr.add(offset) as *const u64).read_unaligned()) }
    }

    /// Returns a slice reference to memory at [offset, offset+len)
    #[allow(dead_code)]
    pub fn read_slice(&self, offset: usize, len: usize) -> Result<&[u8], String> {
        self.check_bounds(offset, len)?;
        unsafe { Ok(slice::from_raw_parts(self.ptr.add(offset), len)) }
    }

    /// Writes a single byte at the specified offset
    #[allow(dead_code)]
    pub fn write_u8(&mut self, offset: usize, value: u8) -> Result<(), String> {
        self.check_bounds(offset, 1)?;
        unsafe { *self.ptr.add(offset) = value; }
        Ok(())
    }

    /// Writes a u16 at the specified offset (unaligned access supported)
    #[allow(dead_code)]
    pub fn write_u16(&mut self, offset: usize, value: u16) -> Result<(), String> {
        self.check_bounds(offset, 2)?;
        unsafe { (self.ptr.add(offset) as *mut u16).write_unaligned(value); }
        Ok(())
    }

    /// Writes a u32 at the specified offset (unaligned access supported)
    #[allow(dead_code)]
    pub fn write_u32(&mut self, offset: usize, value: u32) -> Result<(), String> {
        self.check_bounds(offset, 4)?;
        unsafe { (self.ptr.add(offset) as *mut u32).write_unaligned(value); }
        Ok(())
    }

    /// Writes a u64 at the specified offset (unaligned access supported)
    pub fn write_u64(&mut self, offset: usize, value: u64) -> Result<(), String> {
        self.check_bounds(offset, 8)?;
        unsafe { (self.ptr.add(offset) as *mut u64).write_unaligned(value); }
        Ok(())
    }

    /// Writes a byte slice at the specified offset
    pub fn write_slice(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        self.check_bounds(offset, data.len())?;
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), self.ptr.add(offset), data.len());
        }
        Ok(())
    }

    /// Fills a memory region with a constant byte value
    #[allow(dead_code)]
    pub fn fill(&mut self, offset: usize, len: usize, value: u8) -> Result<(), String> {
        self.check_bounds(offset, len)?;
        unsafe { ptr::write_bytes(self.ptr.add(offset), value, len); }
        Ok(())
    }

    /// Zeros a memory region
    #[allow(dead_code)]
    pub fn zero(&mut self, offset: usize, len: usize) -> Result<(), String> {
        self.fill(offset, len, 0)
    }

    /// Copies data within the same memory region (handles overlapping)
    #[allow(dead_code)]
    pub fn copy_within(&mut self, src_offset: usize, dst_offset: usize, len: usize) -> Result<(), String> {
        self.check_bounds(src_offset, len)?;
        self.check_bounds(dst_offset, len)?;
        unsafe {
            ptr::copy(self.ptr.add(src_offset), self.ptr.add(dst_offset), len);
        }
        Ok(())
    }

    // ========================================================================
    // MEMORY PROTECTION
    // ========================================================================

    /// Locks memory pages in RAM to prevent swapping (mlock)
    pub fn lock(&mut self) -> Result<(), String> {
        if self.locked {
            return Ok(());
        }

        if unsafe { libc::mlock(self.ptr as *const libc::c_void, self.len) } != 0 {
            return Err(format!("mlock failed: {}", Self::get_errno_string()));
        }

        self.locked = true;
        Ok(())
    }

    /// Unlocks memory pages to allow swapping (munlock)
    pub fn unlock(&mut self) -> Result<(), String> {
        if !self.locked {
            return Ok(());
        }

        if unsafe { libc::munlock(self.ptr as *const libc::c_void, self.len) } != 0 {
            return Err(format!("munlock failed: {}", Self::get_errno_string()));
        }

        self.locked = false;
        Ok(())
    }

    /// Sets memory protection flags (mprotect wrapper)
    #[allow(dead_code)]
    pub fn protect(&mut self, read: bool, write: bool, exec: bool) -> Result<(), String> {
        let mut prot = 0;
        if read { prot |= libc::PROT_READ; }
        if write { prot |= libc::PROT_WRITE; }
        if exec { prot |= libc::PROT_EXEC; }

        if unsafe { libc::mprotect(self.ptr as *mut libc::c_void, self.len, prot) } != 0 {
            return Err(format!("mprotect failed: {}", Self::get_errno_string()));
        }

        Ok(())
    }

    /// Makes memory read-only
    #[allow(dead_code)]
    pub fn make_readonly(&mut self) -> Result<(), String> {
        self.protect(true, false, false)
    }

    /// Makes memory read-write
    #[allow(dead_code)]
    pub fn make_readwrite(&mut self) -> Result<(), String> {
        self.protect(true, true, false)
    }

    // ========================================================================
    // UTILITY METHODS
    // ========================================================================

    /// Aligns size up to the nearest page boundary
    #[inline]
    fn align_to_page(size: usize) -> usize {
        (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
    }

    /// Returns a human-readable string for the current errno
    fn get_errno_string() -> String {
        unsafe {
            let errno = *libc::__errno_location();
            let msg = libc::strerror(errno);
            if msg.is_null() {
                format!("errno {}", errno)
            } else {
                std::ffi::CStr::from_ptr(msg).to_string_lossy().into_owned()
            }
        }
    }

    /// Returns memory statistics for diagnostic purposes
    #[allow(dead_code)]
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            allocated: self.len,
            backend: self.backend,
            locked: self.locked,
            page_aligned: self.len % PAGE_SIZE == 0,
        }
    }
}

// ============================================================================
// MEMORY STATISTICS
// ============================================================================

/// Diagnostic information about a GuestMemory allocation
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MemoryStats {
    pub allocated: usize,
    pub backend: MemoryBackend,
    pub locked: bool,
    pub page_aligned: bool,
}

impl fmt::Display for MemoryStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Memory Statistics:")?;
        writeln!(f, "  Allocated:    {} bytes ({} KB)", self.allocated, self.allocated / 1024)?;
        writeln!(f, "  Backend:      {:?}", self.backend)?;
        writeln!(f, "  Locked:       {}", self.locked)?;
        writeln!(f, "  Page Aligned: {}", self.page_aligned)
    }
}

// ============================================================================
// DROP IMPLEMENTATION
// ============================================================================

impl Drop for GuestMemory {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            if self.locked {
                let _ = self.unlock();
            }
            unsafe {
                libc::munmap(self.ptr as *mut libc::c_void, self.len);
            }
        }
    }
}

// ============================================================================
// SEND/SYNC SAFETY
// ============================================================================

// SAFETY: GuestMemory owns its allocation exclusively.
// The pointer is valid for the lifetime of the struct.
unsafe impl Send for GuestMemory {}

// SAFETY: Immutable access (&self) is safe from multiple threads.
// Mutable access requires &mut self, enforced by Rust's borrow checker.
unsafe impl Sync for GuestMemory {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_allocation() {
        let mem = GuestMemory::new(4096).unwrap();
        assert_eq!(mem.len(), 4096);
        assert!(!mem.is_empty());
    }

    #[test]
    fn test_write_read_u64() {
        let mut mem = GuestMemory::new(4096).unwrap();
        mem.write_u64(0, 0xDEADBEEF).unwrap();
        assert_eq!(mem.read_u64(0).unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn test_write_read_slice() {
        let mut mem = GuestMemory::new(4096).unwrap();
        let data = b"Hello, World!";
        mem.write_slice(0, data).unwrap();
        assert_eq!(mem.read_slice(0, data.len()).unwrap(), data);
    }

    #[test]
    fn test_bounds_checking() {
        let mut mem = GuestMemory::new(4096).unwrap();
        assert!(mem.write_u64(4090, 0xFF).is_err());
        assert!(mem.read_u64(4090).is_err());
    }

    #[test]
    fn test_fill_and_zero() {
        let mut mem = GuestMemory::new(4096).unwrap();
        mem.fill(0, 100, 0xFF).unwrap();
        assert_eq!(mem.read_u8(50).unwrap(), 0xFF);
        mem.zero(0, 100).unwrap();
        assert_eq!(mem.read_u8(50).unwrap(), 0);
    }

    #[test]
    fn test_copy_within() {
        let mut mem = GuestMemory::new(4096).unwrap();
        let data = b"Test Data";
        mem.write_slice(0, data).unwrap();
        mem.copy_within(0, 100, data.len()).unwrap();
        assert_eq!(mem.read_slice(100, data.len()).unwrap(), data);
    }

    #[test]
    fn test_memory_protection() {
        let mut mem = GuestMemory::new(4096).unwrap();
        mem.lock().unwrap();
        assert!(mem.locked);
        mem.unlock().unwrap();
        assert!(!mem.locked);
    }

    #[test]
    fn test_huge_pages() {
        #[cfg(target_os = "linux")]
        {
            let config = MemoryConfig {
                size: HUGE_PAGE_SIZE_2MB,
                backend: MemoryBackend::HugePages2MB,
                zero_on_alloc: true,
                mlock: false,
            };
            let _ = GuestMemory::with_config(config);
        }
    }

    #[test]
    fn test_unaligned_access() {
        let mut mem = GuestMemory::new(4096).unwrap();
        mem.write_u64(5, 0x123456789ABCDEF0).unwrap();
        assert_eq!(mem.read_u64(5).unwrap(), 0x123456789ABCDEF0);
    }

    #[test]
    fn test_memory_stats() {
        let mem = GuestMemory::new(4096).unwrap();
        let stats = mem.stats();
        assert_eq!(stats.allocated, 4096);
        assert_eq!(stats.backend, MemoryBackend::Anonymous);
        assert!(!stats.locked);
        assert!(stats.page_aligned);
    }
}