// loader.rs
//!
//! Linux Kernel Loader.
//! Loads a bzImage kernel into guest memory following the Linux boot protocol.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::mem;
use std::ptr;
use std::slice;

use crate::memory::GuestMemory;
use crate::linux::{
    BootParams, SetupHeader, E820Entry,
    ZERO_PAGE_START, CMDLINE_START, KERNEL_START,
    E820_RAM, HDRS_MAGIC,
};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Offset of the setup header within the kernel image
const SETUP_HEADER_OFFSET: u64 = 0x1F1;

/// Default number of setup sectors if setup_sects is 0
const DEFAULT_SETUP_SECTS: u8 = 4;

/// Sector size in bytes
const SECTOR_SIZE: u64 = 512;

// ============================================================================
// HELPER MACROS FOR PACKED STRUCT ACCESS
// ============================================================================

/// Safely reads a field from a packed struct
macro_rules! read_packed {
    ($struct:expr, $field:ident) => {
        unsafe { ptr::addr_of!(($struct).$field).read_unaligned() }
    };
}

/// Safely writes a field to a packed struct
macro_rules! write_packed {
    ($struct:expr, $field:ident, $value:expr) => {
        unsafe { ptr::addr_of_mut!(($struct).$field).write_unaligned($value) }
    };
}

// ============================================================================
// KERNEL LOADER
// ============================================================================

/// Loads a Linux kernel (bzImage) into guest memory.
///
/// This function follows the Linux x86 boot protocol:
/// 1. Reads and validates the setup header
/// 2. Configures the E820 memory map
/// 3. Sets up the kernel command line
/// 4. Loads the protected mode kernel code
/// 5. Writes the boot parameters (Zero Page)
///
/// # Arguments
/// * `guest_mem` - Mutable reference to guest memory
/// * `kernel_path` - Path to the bzImage kernel file
/// * `mem_size` - Total guest memory size in bytes
/// * `cmdline` - Kernel command line string
///
/// # Returns
/// * `Ok(entry_point)` - The 32-bit entry point address (code32_start)
/// * `Err(String)` with error description on failure
pub fn load_linux(
    guest_mem: &mut GuestMemory,
    kernel_path: &str,
    mem_size: usize,
    cmdline: &str,
) -> Result<u64, String> {
    let mut file = File::open(kernel_path)
        .map_err(|e| format!("Failed to open kernel file '{}': {}", kernel_path, e))?;

    // ========================================================================
    // 1. Read Setup Header
    // ========================================================================

    let mut boot_params = BootParams::default();

    file.seek(SeekFrom::Start(SETUP_HEADER_OFFSET))
        .map_err(|e| format!("Failed to seek to kernel header: {}", e))?;

    // SAFETY: SetupHeader is a packed repr(C) struct
    unsafe {
        let header_slice = slice::from_raw_parts_mut(
            ptr::addr_of_mut!(boot_params.hdr) as *mut u8,
            mem::size_of::<SetupHeader>(),
        );
        file.read_exact(header_slice)
            .map_err(|e| format!("Failed to read kernel header: {}", e))?;
    }

    // Validate magic number (read unaligned from packed struct)
    let header_magic = read_packed!(boot_params.hdr, header);
    if header_magic != HDRS_MAGIC {
        return Err(format!(
            "Invalid kernel header magic: {:#x} (expected {:#x}). Not a valid bzImage?",
            header_magic, HDRS_MAGIC
        ));
    }

    let version = read_packed!(boot_params.hdr, version);
    log_loader(&format!(
        "Kernel header valid. Protocol version: {}.{}",
        version >> 8,
        version & 0xFF
    ));

    // ========================================================================
    // 2. Configure E820 Memory Map (Split Layout for Linux)
    // ========================================================================
    // Linux expects a hole at 0xA0000 (640KB) to 0x100000 (1MB) for VGA/BIOS.
    // Without this split, Linux may reject memory or fail to allocate low pages.

    write_packed!(boot_params, e820_entries, 2u8);

    // Entry 1: Low Memory (0 - 639KB) - Conventional memory
    boot_params.e820_table[0] = E820Entry {
        addr: 0,
        size: 0x9FC00,  // 639KB (just under 640KB)
        type_: E820_RAM,
    };

    // Entry 2: High Memory (1MB - End) - Where kernel lives
    boot_params.e820_table[1] = E820Entry {
        addr: 0x100000,  // Start at 1MB
        size: (mem_size - 0x100000) as u64,
        type_: E820_RAM,
    };

    log_loader(&format!("E820: Low RAM 0x0 - 0x9FC00 (639 KB)"));
    log_loader(&format!("E820: High RAM 0x100000 - {:#x} ({} MB)", 
        mem_size, (mem_size - 0x100000) / (1024 * 1024)));

    // ========================================================================
    // 3. Configure Kernel Command Line
    // ========================================================================

    if !cmdline.is_empty() {
        let cmdline_bytes = cmdline.as_bytes();

        guest_mem.write_slice(CMDLINE_START, cmdline_bytes)
            .map_err(|e| format!("Failed to write cmdline: {}", e))?;

        // Null terminator
        guest_mem.write_u8(CMDLINE_START + cmdline_bytes.len(), 0)
            .map_err(|e| format!("Failed to write cmdline terminator: {}", e))?;

        write_packed!(boot_params.hdr, cmd_line_ptr, CMDLINE_START as u32);
        write_packed!(boot_params.hdr, cmdline_size, (cmdline_bytes.len() + 1) as u32);

        log_loader(&format!("Cmdline: '{}'", cmdline));
    }

    // ========================================================================
    // 4. Configure Loader Identity
    // ========================================================================

    // 0xFF = undefined/custom bootloader
    write_packed!(boot_params.hdr, type_of_loader, 0xFFu8);

    // Set CAN_USE_HEAP flag if supported
    if version >= 0x0200 {
        let loadflags = read_packed!(boot_params.hdr, loadflags);
        write_packed!(boot_params.hdr, loadflags, loadflags | 0x80);
    }

    // ========================================================================
    // 5. Load Protected Mode Kernel
    // ========================================================================

    let setup_sects_raw = read_packed!(boot_params.hdr, setup_sects);
    let setup_sects = if setup_sects_raw == 0 {
        DEFAULT_SETUP_SECTS
    } else {
        setup_sects_raw
    };

    let kernel_offset = (setup_sects as u64 + 1) * SECTOR_SIZE;

    file.seek(SeekFrom::Start(kernel_offset))
        .map_err(|e| format!("Failed to seek to kernel code: {}", e))?;

    let mut kernel_code = Vec::new();
    file.read_to_end(&mut kernel_code)
        .map_err(|e| format!("Failed to read kernel code: {}", e))?;

    guest_mem.write_slice(KERNEL_START, &kernel_code)
        .map_err(|e| format!("Failed to write kernel to memory: {}", e))?;

    log_loader(&format!(
        "Kernel loaded at {:#x}. Size: {} bytes ({} KB)",
        KERNEL_START,
        kernel_code.len(),
        kernel_code.len() / 1024
    ));

    // ========================================================================
    // 6. Write Boot Parameters (Zero Page)
    // ========================================================================

    // SAFETY: BootParams is a packed repr(C) struct
    unsafe {
        let params_slice = slice::from_raw_parts(
            ptr::addr_of!(boot_params) as *const u8,
            mem::size_of::<BootParams>(),
        );
        guest_mem.write_slice(ZERO_PAGE_START, params_slice)
            .map_err(|e| format!("Failed to write Zero Page: {}", e))?;
    }

    log_loader(&format!("Zero Page written at {:#x}", ZERO_PAGE_START));

    // Get the actual 32-bit entry point from the kernel header
    let code32_start = read_packed!(boot_params.hdr, code32_start);
    let entry_point = if code32_start != 0 {
        code32_start as u64
    } else {
        KERNEL_START as u64
    };

    log_loader(&format!("Entry point (code32_start): {:#x}", entry_point));

    // Debug: Verify first bytes of kernel in RAM
    let first_bytes = guest_mem.read_slice(KERNEL_START, 16)
        .map_err(|e| format!("Debug read failed: {}", e))?;
    log_loader(&format!("Kernel first 16 bytes at {:#x}: {:02x?}", KERNEL_START, first_bytes));

    Ok(entry_point)
}

// ============================================================================
// LOGGING
// ============================================================================

fn log_loader(msg: &str) {
    println!(">>> [Loader] {}", msg);
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(SETUP_HEADER_OFFSET, 0x1F1);
        assert_eq!(SECTOR_SIZE, 512);
        assert_eq!(KERNEL_START, 0x100000);
    }
}