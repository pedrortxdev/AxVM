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
/// This function sets up:
/// - 4-level page tables with identity mapping for the first 2MB
/// - A minimal GDT with null, code (64-bit), and data segments
/// - Control registers (CR0, CR3, CR4, EFER) for paging and long mode
/// - Segment registers pointing to the GDT entries
/// - Initial RIP at address 0x0
pub fn setup_long_mode(vcpu: &mut VcpuFd, mem: &mut GuestMemory) -> Result<(), Box<dyn std::error::Error>> {
    setup_page_tables(mem)?;
    setup_gdt(mem)?;
    setup_registers(vcpu)?;
    Ok(())
}

// ============================================================================
// PAGE TABLE SETUP
// ============================================================================

/// Sets up a 4-level identity-mapped page table structure.
///
/// Memory layout:
/// - 0x1000: PML4 (Page Map Level 4)
/// - 0x2000: PDPT (Page Directory Pointer Table)
/// - 0x3000: PD   (Page Directory) with 2MB huge page mapping
///
/// This maps virtual address 0x0 to physical address 0x0 (first 2MB).
fn setup_page_tables(mem: &mut GuestMemory) -> Result<(), String> {
    // PML4[0] -> PDPT at 0x2000 (Present + Writable)
    mem.write_u64(0x1000, 0x2000 | 0x3)?;

    // PDPT[0] -> PD at 0x3000 (Present + Writable)
    mem.write_u64(0x2000, 0x3000 | 0x3)?;

    // PD[0] -> 2MB Identity Map (Present + Writable + Huge Page)
    mem.write_u64(0x3000, 0x0 | 0x83)?;

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
fn setup_registers(vcpu: &mut VcpuFd) -> Result<(), kvm_ioctls::Error> {
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
    regs.rflags = 2;  // Reserved bit 1 must be set
    regs.rip = 0x0;   // Entry point
    regs.rax = 0;
    regs.rbx = 0;
    vcpu.set_regs(&regs)?;

    Ok(())
}
