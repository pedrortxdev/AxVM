// src/memory.rs
//!
//! High-Performance Guest Memory Management
//! Uses mmap + MADV_HUGEPAGE for Ivy Bridge TLB Optimization.
//!

#![allow(dead_code)]

use std::ptr;
use libc::{
    c_void, mmap, munmap, madvise, 
    MAP_PRIVATE, MAP_ANONYMOUS, PROT_READ, PROT_WRITE, MAP_FAILED, 
    MADV_HUGEPAGE
};

/// Guest physical memory region backed by mmap with Huge Pages support
pub struct GuestMemory {
    ptr: *mut u8,
    len: usize,
    owned: bool, // True if we own this memory (should munmap on drop)
}

// Safe to send between threads - we manage memory manually
unsafe impl Send for GuestMemory {}
unsafe impl Sync for GuestMemory {}

impl GuestMemory {
    /// Allocates guest memory optimized for virtualization (Huge Pages)
    pub fn new(size: usize) -> Result<Self, String> {
        // Align to 2MB for Huge Pages
        let align_mask = (2 * 1024 * 1024) - 1;
        let aligned_size = (size + align_mask) & !align_mask;

        unsafe {
            // 1. mmap: Request raw memory from host kernel
            let ptr = mmap(
                ptr::null_mut(),
                aligned_size,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            );

            if ptr == MAP_FAILED {
                return Err(format!("mmap failed (Size: {} MB)", aligned_size / 1024 / 1024));
            }

            // 2. madvise: Request Huge Pages (2MB) - reduces TLB misses on Xeon
            if madvise(ptr, aligned_size, MADV_HUGEPAGE) != 0 {
                println!(">>> [WARN] Failed to enable Huge Pages (madvise error). Using 4KB pages.");
            } else {
                println!(">>> [Mem] Huge Pages (THP) hints enabled for guest RAM.");
            }

            // Zero-initialize (important for guest memory)
            ptr::write_bytes(ptr as *mut u8, 0, aligned_size);

            Ok(Self {
                ptr: ptr as *mut u8,
                len: size,
                owned: true,
            })
        }
    }

    /// Reconstruct from raw parts for thread access.
    /// 
    /// # Safety
    /// - The pointer must be valid and point to allocated memory
    /// - The memory must remain valid for the lifetime of this struct
    /// - This creates a non-owning view (won't munmap on drop)
    pub unsafe fn from_raw_parts(ptr: usize, len: usize) -> Self {
        Self {
            ptr: ptr as *mut u8,
            len,
            owned: false, // Don't munmap on drop - we don't own it
        }
    }

    // ========================================================================
    // ACCESSORS
    // ========================================================================

    #[inline]
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    // ========================================================================
    // MEMORY OPERATIONS
    // ========================================================================

    pub fn write_slice(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        if offset + data.len() > self.len {
            return Err(format!("Memory write overflow: addr={:#x}, len={}", offset, data.len()));
        }
        unsafe {
            let dest = self.ptr.add(offset);
            ptr::copy_nonoverlapping(data.as_ptr(), dest, data.len());
        }
        Ok(())
    }

    pub fn read_slice(&self, offset: usize, len: usize) -> Result<&[u8], String> {
        if offset + len > self.len {
            return Err(format!("Memory read overflow: addr={:#x}, len={}", offset, len));
        }
        unsafe {
            let src = self.ptr.add(offset);
            Ok(std::slice::from_raw_parts(src, len))
        }
    }

    // Helpers for primitive types (Little Endian for x86)
    
    pub fn write_u8(&mut self, offset: usize, val: u8) -> Result<(), String> {
        self.write_slice(offset, &[val])
    }

    pub fn write_u16(&mut self, offset: usize, val: u16) -> Result<(), String> {
        self.write_slice(offset, &val.to_le_bytes())
    }
    
    pub fn write_u32(&mut self, offset: usize, val: u32) -> Result<(), String> {
        self.write_slice(offset, &val.to_le_bytes())
    }
    
    pub fn write_u64(&mut self, offset: usize, val: u64) -> Result<(), String> {
        self.write_slice(offset, &val.to_le_bytes())
    }
}

impl Drop for GuestMemory {
    fn drop(&mut self) {
        // Only munmap if we own the memory (created via new())
        // Thread copies (from_raw_parts) don't own the memory
        if self.owned && !self.ptr.is_null() {
            unsafe {
                munmap(self.ptr as *mut c_void, self.len);
            }
        }
    }
}