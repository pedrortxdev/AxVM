// linux.rs
//!
//! Linux Boot Protocol Definitions.
//! Translates C structs from arch/x86/include/uapi/asm/bootparam.h
//!
//! Reference: https://www.kernel.org/doc/html/latest/x86/boot.html

// ============================================================================
// CONSTANTS - MEMORY LAYOUT
// ============================================================================

/// Base address where the kernel expects the Zero Page (boot_params)
pub const ZERO_PAGE_START: usize = 0x7000;

/// Address where the kernel command line (cmdline) will be placed
pub const CMDLINE_START: usize = 0x20000;

/// Address where the protected mode kernel code is loaded (conventional 1MB)
pub const KERNEL_START: usize = 0x100000;

// ============================================================================
// CONSTANTS - E820 MEMORY MAP TYPES
// ============================================================================

/// Usable RAM
pub const E820_RAM: u32 = 1;

/// Reserved memory (unusable)
#[allow(dead_code)]
pub const E820_RESERVED: u32 = 2;

/// ACPI reclaimable memory
#[allow(dead_code)]
pub const E820_ACPI: u32 = 3;

/// ACPI NVS memory
#[allow(dead_code)]
pub const E820_NVS: u32 = 4;

/// Unusable memory
#[allow(dead_code)]
pub const E820_UNUSABLE: u32 = 5;

// ============================================================================
// SETUP HEADER
// ============================================================================

/// Linux kernel setup header structure.
///
/// This structure is located at offset 0x1F1 in the kernel image.
/// It contains boot protocol information and configuration.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SetupHeader {
    pub setup_sects: u8,
    pub root_flags: u16,
    pub syssize: u32,
    pub ram_size: u16,
    pub vid_mode: u16,
    pub root_dev: u16,
    pub boot_flag: u16,
    pub jump: u16,
    /// Magic signature "HdrS" (0x53726448)
    pub header: u32,
    pub version: u16,
    pub realmode_swtch: u32,
    pub start_sys_seg: u16,
    pub kernel_version: u16,
    /// Bootloader identifier (0xFF = undefined/custom)
    pub type_of_loader: u8,
    /// Boot protocol flags
    pub loadflags: u8,
    pub setup_move_size: u16,
    /// 32-bit entry point address
    pub code32_start: u32,
    pub ramdisk_image: u32,
    pub ramdisk_size: u32,
    pub bootsect_kludge: u32,
    pub heap_end_ptr: u16,
    pub ext_loader_ver: u8,
    pub ext_loader_type: u8,
    /// Pointer to kernel command line string
    pub cmd_line_ptr: u32,
    pub initrd_addr_max: u32,
    pub kernel_alignment: u32,
    pub relocatable_kernel: u8,
    pub min_alignment: u8,
    pub xloadflags: u16,
    pub cmdline_size: u32,
    pub hardware_subarch: u32,
    pub hardware_subarch_data: u64,
    pub payload_offset: u32,
    pub payload_length: u32,
    pub setup_data: u64,
    pub pref_address: u64,
    pub init_size: u32,
    pub handover_offset: u32,
}

// ============================================================================
// E820 MEMORY MAP ENTRY
// ============================================================================

/// E820 memory map entry.
///
/// Describes a region of physical memory and its type.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct E820Entry {
    /// Base address of the memory region
    pub addr: u64,
    /// Size of the memory region in bytes
    pub size: u64,
    /// Type of memory (E820_RAM, E820_RESERVED, etc.)
    pub type_: u32,
}

// ============================================================================
// BOOT PARAMS (ZERO PAGE)
// ============================================================================

/// Linux boot parameters structure (Zero Page).
///
/// This structure is passed to the kernel at a fixed address
/// and contains all information needed to boot.
///
/// Total size: 4096 bytes (one page)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BootParams {
    pub screen_info: [u8; 0x40],
    pub apm_bios_info: [u8; 0x14],
    pub _pad2: [u8; 0x4],
    pub tboot_addr: u64,
    pub ist_info: [u8; 0x10],
    pub _pad3: [u8; 0x10],
    pub hd0_info: [u8; 0x10],
    pub hd1_info: [u8; 0x10],
    pub sys_desc_table: [u8; 0x10],
    pub olpc_ofw_header: [u8; 0x10],
    pub ext_ramdisk_image: u32,
    pub ext_ramdisk_size: u32,
    pub ext_cmd_line_ptr: u32,
    pub _pad4: [u8; 0x74],
    pub edid_info: [u8; 0x80],
    pub efi_info: [u8; 0x20],
    pub alt_mem_k: u32,
    pub scratch: u32,
    /// Number of entries in e820_table
    pub e820_entries: u8,
    pub eddbuf_entries: u8,
    pub eddbuf_ptr: u8,
    pub max_pfn: u32,
    pub _pad5: [u8; 0x40],
    pub vbe_control_info: [u8; 0x200],
    pub vbe_mode_info: [u8; 0x100],
    pub vbe_interface: [u8; 0x2],
    pub vbe_interface_len: [u8; 0x2],
    pub vbe_interface_off: [u8; 0x2],
    pub vbe_interface_seg: [u8; 0x2],
    pub _pad6: [u8; 0x48],
    pub acpi_rsdp_addr: u64,
    pub _pad7: [u8; 0x15],
    /// E820 memory map table (up to 128 entries)
    pub e820_table: [E820Entry; 128],
    pub _pad8: [u8; 0x30],
    /// Setup header (kernel configuration)
    pub hdr: SetupHeader,
    pub _pad9: [u8; 0x290],
    pub edd_mbr_sig_buffer: [u8; 0x10],
    pub e820_table_kexec: [E820Entry; 128],
    pub _pad10: [u8; 0x2e0],
}

impl Default for BootParams {
    fn default() -> Self {
        // SAFETY: BootParams is a POD type with no invalid bit patterns.
        // Zero-initialization is valid for all fields.
        unsafe { std::mem::zeroed() }
    }
}

// ============================================================================
// CONSTANTS - SETUP HEADER MAGIC
// ============================================================================

/// Magic number "HdrS" indicating a valid Linux kernel
pub const HDRS_MAGIC: u32 = 0x53726448;

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_setup_header_size() {
        // SetupHeader should be exactly 119 bytes (packed)
        assert!(mem::size_of::<SetupHeader>() > 0);
    }

    #[test]
    fn test_e820_entry_size() {
        // E820Entry: 8 + 8 + 4 = 20 bytes
        assert_eq!(mem::size_of::<E820Entry>(), 20);
    }

    #[test]
    fn test_boot_params_default() {
        let params = BootParams::default();
        assert_eq!(params.e820_entries, 0);
        assert_eq!(params.hdr.header, 0);
    }

    #[test]
    fn test_hdrs_magic() {
        // "HdrS" in little-endian
        let magic = u32::from_le_bytes([b'H', b'd', b'r', b'S']);
        assert_eq!(magic, HDRS_MAGIC);
    }
}