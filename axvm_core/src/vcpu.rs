// vcpu.rs
//!
//! vCPU setup module for AxVM.
//! Handles x86-64 long mode initialization including page tables, GDT, and registers.

use kvm_ioctls::VcpuFd;
use kvm_bindings::kvm_segment;
use crate::memory::GuestMemory;

// ============================================================================
// CONSTANTS - CONTROL REGISTERS
// ============================================================================

/// Protected Mode Enable (CR0.PE)
const CR0_PE: u64 = 1 << 0;

/// Paging Enable (CR0.PG)
const CR0_PG: u64 = 1 << 31;

/// Physical Address Extension (CR4.PAE)
const CR4_PAE: u64 = 1 << 5;

/// Long Mode Enable (EFER.LME)
const EFER_LME: u64 = 1 << 8;

/// Long Mode Active (EFER.LMA) - Set by CPU when PG=1 and LME=1
const EFER_LMA: u64 = 1 << 10;

// ============================================================================
// LONG MODE SETUP
// ============================================================================

/// Initializes the vCPU for x86-64 long mode operation.
///
/// For embedded payloads, use entry_point=0x0, boot_params=0x0.
/// For Linux kernel, use entry_point=0x100000, boot_params=0x7000.
#[allow(dead_code)]
pub fn setup_long_mode(
    vcpu: &mut VcpuFd, 
    mem: &mut GuestMemory,
    entry_point: u64,
    boot_params: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    setup_protected_mode_32bit(vcpu, mem, entry_point, boot_params)
}

/// Initializes the vCPU for x86-64 long mode with a custom entry point.
///
/// This function sets up:
/// - 4-level page tables with identity mapping for guest memory
/// - A minimal GDT with null, code (64-bit), and data segments
/// - Control registers (CR0, CR3, CR4, EFER) for paging and long mode
/// - Segment registers pointing to the GDT entries
/// - Initial RIP at the specified entry point
///
/// # Arguments
/// * `vcpu` - The vCPU file descriptor
/// * `mem` - Guest memory to write page tables and GDT
/// * `entry_point` - The initial RIP value (e.g., 0x0 or 0x100000 for Linux)
#[allow(dead_code)]
pub fn setup_long_mode_with_entry(
    vcpu: &mut VcpuFd, 
    mem: &mut GuestMemory,
    entry_point: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    setup_page_tables_extended(mem)?;
    setup_gdt(mem)?;
    setup_registers_with_entry(vcpu, entry_point)?;
    Ok(())
}

// ============================================================================
// 32-BIT PROTECTED MODE SETUP (FOR LINUX BZIMAGE BOOT)
// ============================================================================

/// Initializes the vCPU for 32-bit protected mode (Linux boot protocol).
///
/// The Linux bzImage is a compressed kernel that expects to be started in:
/// - 32-bit protected mode
/// - Paging DISABLED
/// - Flat memory model (base=0, limit=4GB)
/// - Interrupts disabled
///
/// The decompressor stub will then:
/// 1. Decompress the kernel
/// 2. Setup its own page tables
/// 3. Enable long mode
/// 4. Jump to the 64-bit kernel entry
///
/// # Arguments
/// * `vcpu` - The vCPU file descriptor
/// * `mem` - Guest memory for GDT
/// * `entry_point` - The bzImage entry point (typically 0x100000)
/// * `boot_params_addr` - Address of the Zero Page (boot_params struct)
pub fn setup_protected_mode_32bit(
    vcpu: &mut VcpuFd,
    mem: &mut GuestMemory,
    entry_point: u64,
    boot_params_addr: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    setup_gdt_32bit(mem)?;
    setup_registers_32bit(vcpu, entry_point, boot_params_addr)?;
    Ok(())
}

/// Sets up a 32-bit GDT for protected mode.
///
/// Memory layout at 0x4000:
/// - Entry 0 (0x4000): Null descriptor
/// - Entry 1 (0x4008): 32-bit code segment (selector 0x08) - flat, execute/read
/// - Entry 2 (0x4010): 32-bit data segment (selector 0x10) - flat, read/write
fn setup_gdt_32bit(mem: &mut GuestMemory) -> Result<(), String> {
    // Entry 0: Null Descriptor (mandatory)
    mem.write_u64(0x4000, 0)?;

    // Entry 1: 32-bit Code Segment (Flat, Execute/Read)
    // Base=0, Limit=0xFFFFF, G=1 (4KB), D=1 (32-bit), P=1, DPL=0, Type=0xA
    // Layout: [Limit 15:0] [Base 15:0] [Base 23:16] [Type|S|DPL|P] [Limit 19:16|AVL|L|D|G] [Base 31:24]
    // 0x00CF9A000000FFFF
    mem.write_u64(0x4008, 0x00CF9A000000FFFF)?;

    // Entry 2: 32-bit Data Segment (Flat, Read/Write)
    // Base=0, Limit=0xFFFFF, G=1, D=1, P=1, DPL=0, Type=0x2
    // 0x00CF92000000FFFF
    mem.write_u64(0x4010, 0x00CF92000000FFFF)?;

    Ok(())
}

/// Configures registers for 32-bit protected mode (Linux boot).
fn setup_registers_32bit(
    vcpu: &mut VcpuFd,
    entry_point: u64,
    boot_params_addr: u64,
) -> Result<(), kvm_ioctls::Error> {
    let mut sregs = vcpu.get_sregs()?;

    // CR0: Only Protected Mode enabled, NO PAGING
    sregs.cr0 = CR0_PE;
    sregs.cr3 = 0;
    sregs.cr4 = 0;
    sregs.efer = 0; // No long mode

    // 32-bit Code Segment
    let code_seg = kvm_segment {
        base: 0,
        limit: 0xFFFFFFFF,
        selector: 0x08,
        type_: 0x0B,     // Execute + Read + Accessed
        present: 1,
        dpl: 0,
        db: 1,           // 32-bit segment
        s: 1,            // Code/Data segment
        l: 0,            // NOT 64-bit
        g: 1,            // 4KB granularity
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    // 32-bit Data Segment
    let data_seg = kvm_segment {
        base: 0,
        limit: 0xFFFFFFFF,
        selector: 0x10,
        type_: 0x03,     // Read + Write + Accessed
        present: 1,
        dpl: 0,
        db: 1,           // 32-bit segment
        s: 1,
        l: 0,
        g: 1,
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    // Task Register - minimal valid TSS
    let tr_seg = kvm_segment {
        base: 0,
        limit: 0,
        selector: 0,
        type_: 0x0B,     // 32-bit TSS (Busy)
        present: 1,
        dpl: 0,
        db: 0,
        s: 0,
        l: 0,
        g: 0,
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    // LDT - Explicitly disabled
    let ldt_seg = kvm_segment {
        base: 0,
        limit: 0,
        selector: 0,
        type_: 0x02,
        present: 0,
        dpl: 0,
        db: 0,
        s: 0,
        l: 0,
        g: 0,
        avl: 0,
        unusable: 1,
        padding: 0,
    };

    // Apply segment registers
    sregs.cs = code_seg;
    sregs.ds = data_seg;
    sregs.es = data_seg;
    sregs.fs = data_seg;
    sregs.gs = data_seg;
    sregs.ss = data_seg;
    sregs.tr = tr_seg;
    sregs.ldt = ldt_seg;

    // GDT register
    sregs.gdt.base = 0x4000;
    sregs.gdt.limit = 23;

    // Clear IDT
    sregs.idt.base = 0;
    sregs.idt.limit = 0;

    vcpu.set_sregs(&sregs)?;

    // General-purpose registers (Linux boot protocol)
    let mut regs = vcpu.get_regs()?;
    regs.rflags = 2;               // Reserved bit 1 must be set
    regs.rip = entry_point;        // bzImage entry (0x100000)
    regs.rsi = boot_params_addr;   // Pointer to boot_params (Zero Page)
    regs.rax = 0;
    regs.rbx = 0;
    regs.rcx = 0;
    regs.rdx = 0;
    regs.rdi = 0;
    regs.rbp = 0;
    regs.rsp = 0x90000;            // Valid stack in safe RAM area below 1MB
    vcpu.set_regs(&regs)?;

    Ok(())
}

// ============================================================================
// PAGE TABLE SETUP
// ============================================================================

/// Sets up a 4-level identity-mapped page table for up to 512GB.
///
/// Memory layout:
/// - 0x1000: PML4 (Page Map Level 4)
/// - 0x2000: PDPT (Page Directory Pointer Table)
/// - 0x3000+: PD entries (Page Directories) for 2MB huge pages
///
/// This identity-maps the first 1GB of memory using 2MB huge pages,
/// which is sufficient for loading and booting a Linux kernel.
fn setup_page_tables_extended(mem: &mut GuestMemory) -> Result<(), String> {
    // PML4[0] -> PDPT at 0x2000 (Present + Writable)
    mem.write_u64(0x1000, 0x2000 | 0x3)?;

    // PDPT[0] -> PD at 0x3000 (Present + Writable)
    mem.write_u64(0x2000, 0x3000 | 0x3)?;

    // Map first 1GB using 512 x 2MB pages
    // PD entries starting at 0x3000, each 8 bytes
    for i in 0u64..512 {
        let physical_addr = i * 0x200000; // 2MB per entry
        let pd_entry = physical_addr | 0x83; // Present + Writable + Huge (2MB)
        mem.write_u64(0x3000 + (i * 8) as usize, pd_entry)?;
    }

    Ok(())
}

// ============================================================================
// GDT SETUP
// ============================================================================

/// Sets up a minimal Global Descriptor Table for 64-bit mode.
///
/// Memory layout at 0x4000:
/// - Entry 0 (0x4000): Null descriptor
/// - Entry 1 (0x4008): 64-bit code segment (selector 0x08)
/// - Entry 2 (0x4010): Data segment (selector 0x10)
fn setup_gdt(mem: &mut GuestMemory) -> Result<(), String> {
    // Entry 0: Null Descriptor (mandatory)
    mem.write_u64(0x4000, 0)?;

    // Entry 1: 64-bit Code Segment
    // Bits: L=1, D=0, P=1, DPL=0, Type=0xA (Execute/Read)
    // Raw: 0x00209A0000000000
    mem.write_u64(0x4008, 0x00209A0000000000)?;

    // Entry 2: Data Segment (flat, writable)
    // Bits: P=1, DPL=0, Type=0x2 (Read/Write)
    // Raw: 0x0000920000000000
    mem.write_u64(0x4010, 0x0000920000000000)?;

    Ok(())
}

// ============================================================================
// REGISTER SETUP
// ============================================================================

/// Configures special and general-purpose registers for long mode.
///
/// Key fixes for KVM validation (EINVAL prevention):
/// - TR (Task Register) must be set to a valid 64-bit TSS descriptor
/// - LDT must be explicitly marked as unusable
/// - IDT base/limit cleared to prevent phantom interrupt faults
///
/// Linux boot requirements:
/// - RSI must point to the boot_params (Zero Page) address
fn setup_registers_with_entry(vcpu: &mut VcpuFd, entry_point: u64) -> Result<(), kvm_ioctls::Error> {
    let mut sregs = vcpu.get_sregs()?;

    // 1. Point CR3 to PML4 base address
    sregs.cr3 = 0x1000;

    // 2. Enable PAE (required for long mode)
    sregs.cr4 |= CR4_PAE;

    // 3. Set EFER.LME and EFER.LMA
    // Note: KVM expects LMA to be set when PG=1 and LME=1
    sregs.efer |= EFER_LME | EFER_LMA;

    // 4. Enable protected mode and paging (triggers long mode activation)
    sregs.cr0 |= CR0_PE | CR0_PG;

    // =========================================================================
    // SEGMENT DESCRIPTORS - CRITICAL FOR KVM VALIDATION
    // =========================================================================

    // Code Segment (64-bit, L=1, D=0)
    let code_seg = kvm_segment {
        base: 0,
        limit: 0xFFFFFFFF,
        selector: 8,
        type_: 11,       // Execute + Read + Accessed
        present: 1,
        dpl: 0,
        db: 0,           // Must be 0 for 64-bit code
        s: 1,            // Code/Data segment
        l: 1,            // 64-bit mode
        g: 1,            // 4KB granularity
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    // Data Segment (Flat, 32-bit compatible)
    let data_seg = kvm_segment {
        base: 0,
        limit: 0xFFFFFFFF,
        selector: 16,
        type_: 3,        // Read + Write + Accessed
        present: 1,
        dpl: 0,
        db: 1,           // 32-bit segment
        s: 1,
        l: 0,
        g: 1,
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    // Task Register (TR) - MANDATORY for 64-bit mode
    // Must be a valid 64-bit TSS (Busy) descriptor
    let tr_seg = kvm_segment {
        base: 0,
        limit: 0,
        selector: 0,
        type_: 11,       // 64-bit TSS (Busy)
        present: 1,
        dpl: 0,
        db: 0,
        s: 0,            // System segment
        l: 0,
        g: 0,
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    // LDT - Explicitly disabled
    let ldt_seg = kvm_segment {
        base: 0,
        limit: 0,
        selector: 0,
        type_: 2,
        present: 0,
        dpl: 0,
        db: 0,
        s: 0,
        l: 0,
        g: 0,
        avl: 0,
        unusable: 1,     // Marked as unusable
        padding: 0,
    };

    // Apply segment registers
    sregs.cs = code_seg;
    sregs.ds = data_seg;
    sregs.es = data_seg;
    sregs.fs = data_seg;
    sregs.gs = data_seg;
    sregs.ss = data_seg;
    sregs.tr = tr_seg;
    sregs.ldt = ldt_seg;

    // Configure GDT register
    sregs.gdt.base = 0x4000;
    sregs.gdt.limit = 23; // 3 entries * 8 bytes - 1

    // Clear IDT to prevent phantom interrupt faults
    sregs.idt.base = 0;
    sregs.idt.limit = 0;

    vcpu.set_sregs(&sregs)?;

    // Configure general-purpose registers
    let mut regs = vcpu.get_regs()?;
    regs.rflags = 2;           // Reserved bit 1 must be set
    regs.rip = entry_point;    // Custom entry point
    regs.rax = 0;
    regs.rbx = 0;
    // Linux boot protocol: RSI must point to boot_params (Zero Page)
    regs.rsi = crate::linux::ZERO_PAGE_START as u64;
    vcpu.set_regs(&regs)?;

    Ok(())
}
