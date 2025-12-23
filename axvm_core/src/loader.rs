




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






const SETUP_HEADER_OFFSET: u64 = 0x1F1;


const DEFAULT_SETUP_SECTS: u8 = 4;


const SECTOR_SIZE: u64 = 512;






macro_rules! read_packed {
    ($struct:expr, $field:ident) => {
        unsafe { ptr::addr_of!(($struct).$field).read_unaligned() }
    };
}


macro_rules! write_packed {
    ($struct:expr, $field:ident, $value:expr) => {
        unsafe { ptr::addr_of_mut!(($struct).$field).write_unaligned($value) }
    };
}























pub fn load_linux(
    guest_mem: &mut GuestMemory,
    kernel_path: &str,
    mem_size: usize,
    cmdline: &str,
) -> Result<u64, String> {
    let mut file = File::open(kernel_path)
        .map_err(|e| format!("Failed to open kernel file '{}': {}", kernel_path, e))?;

    
    
    

    let mut boot_params = BootParams::default();

    file.seek(SeekFrom::Start(SETUP_HEADER_OFFSET))
        .map_err(|e| format!("Failed to seek to kernel header: {}", e))?;

    
    unsafe {
        let header_slice = slice::from_raw_parts_mut(
            ptr::addr_of_mut!(boot_params.hdr) as *mut u8,
            mem::size_of::<SetupHeader>(),
        );
        file.read_exact(header_slice)
            .map_err(|e| format!("Failed to read kernel header: {}", e))?;
    }

    
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

    
    
    
    
    

    write_packed!(boot_params, e820_entries, 2u8);

    
    boot_params.e820_table[0] = E820Entry {
        addr: 0,
        size: 0x9FC00,  
        type_: E820_RAM,
    };

    
    boot_params.e820_table[1] = E820Entry {
        addr: 0x100000,  
        size: (mem_size - 0x100000) as u64,
        type_: E820_RAM,
    };

    log_loader(&format!("E820: Low RAM 0x0 - 0x9FC00 (639 KB)"));
    log_loader(&format!("E820: High RAM 0x100000 - {:#x} ({} MB)", 
        mem_size, (mem_size - 0x100000) / (1024 * 1024)));

    
    
    

    if !cmdline.is_empty() {
        let cmdline_bytes = cmdline.as_bytes();

        guest_mem.write_slice(CMDLINE_START, cmdline_bytes)
            .map_err(|e| format!("Failed to write cmdline: {}", e))?;

        
        guest_mem.write_u8(CMDLINE_START + cmdline_bytes.len(), 0)
            .map_err(|e| format!("Failed to write cmdline terminator: {}", e))?;

        write_packed!(boot_params.hdr, cmd_line_ptr, CMDLINE_START as u32);
        write_packed!(boot_params.hdr, cmdline_size, (cmdline_bytes.len() + 1) as u32);

        log_loader(&format!("Cmdline: '{}'", cmdline));
    }

    
    
    

    
    write_packed!(boot_params.hdr, type_of_loader, 0xFFu8);

    
    if version >= 0x0200 {
        let loadflags = read_packed!(boot_params.hdr, loadflags);
        write_packed!(boot_params.hdr, loadflags, loadflags | 0x80);
    }

    
    
    

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

    
    
    

    
    unsafe {
        let params_slice = slice::from_raw_parts(
            ptr::addr_of!(boot_params) as *const u8,
            mem::size_of::<BootParams>(),
        );
        guest_mem.write_slice(ZERO_PAGE_START, params_slice)
            .map_err(|e| format!("Failed to write Zero Page: {}", e))?;
    }

    log_loader(&format!("Zero Page written at {:#x}", ZERO_PAGE_START));

    
    let code32_start = read_packed!(boot_params.hdr, code32_start);
    let entry_point = if code32_start != 0 {
        code32_start as u64
    } else {
        KERNEL_START as u64
    };

    log_loader(&format!("Entry point (code32_start): {:#x}", entry_point));

    
    let first_bytes = guest_mem.read_slice(KERNEL_START, 16)
        .map_err(|e| format!("Debug read failed: {}", e))?;
    log_loader(&format!("Kernel first 16 bytes at {:#x}: {:02x?}", KERNEL_START, first_bytes));

    Ok(entry_point)
}





fn log_loader(msg: &str) {
    println!(">>> [Loader] {}", msg);
}





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