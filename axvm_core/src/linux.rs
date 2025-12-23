





pub const ZERO_PAGE_START: usize = 0x7000;
pub const CMDLINE_START: usize = 0x20000;
pub const KERNEL_START: usize = 0x100000;
pub const E820_RAM: u32 = 1;
pub const HDRS_MAGIC: u32 = 0x53726448;

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
    pub header: u32,
    pub version: u16,
    pub realmode_swtch: u32,
    pub start_sys_seg: u16,
    pub kernel_version: u16,
    pub type_of_loader: u8,
    pub loadflags: u8,
    pub setup_move_size: u16,
    pub code32_start: u32,
    pub ramdisk_image: u32,
    pub ramdisk_size: u32,
    pub bootsect_kludge: u32,
    pub heap_end_ptr: u16,
    pub ext_loader_ver: u8,
    pub ext_loader_type: u8,
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

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Default)]
pub struct E820Entry {
    pub addr: u64,
    pub size: u64,
    pub type_: u32,
}

#[repr(C, packed)]
pub struct BootParams {
    
    pub _pad1: [u8; 0x1e8], 
    
    
    pub e820_entries: u8,
    
    
    pub eddbuf_entries: u8,
    pub eddbuf_ptr: u8,
    
    
    pub _pad2: u8,
    
    
    pub max_pfn: u32,
    
    
    pub _pad3: u8,
    
    
    pub hdr: SetupHeader, 
    
    
    
    
    
    
    
    pub _pad4: [u8; 0x2d0 - 0x1f1 - std::mem::size_of::<SetupHeader>()],
    
    
    pub e820_table: [E820Entry; 128],
    
    
    pub _pad5: [u8; 4096 - 0x2d0 - (128 * 20)],
}

impl Default for BootParams {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}