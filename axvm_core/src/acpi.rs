// src/acpi.rs
//!
//! ACPI Table Generator for SMP Support
//! Generates RSDP, RSDT, and MADT tables to tell Linux about multiple CPUs
//!

use std::mem;
use std::slice;
use crate::memory::GuestMemory;

// Place tables in BIOS ROM area (0xE0000-0xFFFFF) where Linux scans for RSDP
pub const RSDP_START: usize = 0xE0000;

#[repr(C, packed)]
#[derive(Default, Clone, Copy)]
struct Rsdp {
    signature: [u8; 8],   // "RSD PTR "
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_addr: u32,
    length: u32,
    xsdt_addr: u64,
    ext_checksum: u8,
    reserved: [u8; 3],
}

#[repr(C, packed)]
#[derive(Default, Clone, Copy)]
struct SdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[repr(C, packed)]
#[derive(Default, Clone, Copy)]
struct Madt {
    header: SdtHeader,
    local_apic_addr: u32,
    flags: u32,
}

#[repr(C, packed)]
#[derive(Default, Clone, Copy)]
struct MadtLocalApic {
    type_: u8,
    length: u8,
    acpi_processor_id: u8,
    apic_id: u8,
    flags: u32,
}

fn calculate_checksum(data: &[u8]) -> u8 {
    0u8.wrapping_sub(data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x)))
}

/// Setup ACPI tables for SMP support
pub fn setup_acpi(mem: &mut GuestMemory, vcpu_count: u8) -> Result<(), String> {
    let rsdt_addr = RSDP_START + mem::size_of::<Rsdp>();
    let madt_addr = rsdt_addr + mem::size_of::<SdtHeader>() + 4;

    // 1. Build MADT (CPU List)
    let madt_len = mem::size_of::<Madt>() + (mem::size_of::<MadtLocalApic>() * vcpu_count as usize);
    let mut madt_data = vec![0u8; madt_len];

    unsafe {
        let madt = &mut *(madt_data.as_mut_ptr() as *mut Madt);
        madt.header.signature = *b"APIC";
        madt.header.length = madt_len as u32;
        madt.header.revision = 1;
        madt.header.oem_id = *b"AXVM  ";
        madt.header.oem_table_id = *b"AXVMCPU ";
        madt.header.oem_revision = 1;
        madt.header.creator_id = 0x4D5641; // "AVM"
        madt.header.creator_revision = 1;
        madt.local_apic_addr = 0xFEE00000;
        madt.flags = 1; // PCAT_COMPAT

        let entries_ptr = madt_data.as_mut_ptr().add(mem::size_of::<Madt>());
        for i in 0..vcpu_count {
            let entry = &mut *(entries_ptr.add(i as usize * mem::size_of::<MadtLocalApic>()) as *mut MadtLocalApic);
            entry.type_ = 0; // Local APIC
            entry.length = 8;
            entry.acpi_processor_id = i;
            entry.apic_id = i;
            entry.flags = 1; // Enabled
        }
        madt.header.checksum = calculate_checksum(&madt_data);
    }
    mem.write_slice(madt_addr, &madt_data)?;

    // 2. Build RSDT (Points to MADT)
    let rsdt_len = mem::size_of::<SdtHeader>() + 4;
    let mut rsdt_data = vec![0u8; rsdt_len];
    unsafe {
        let rsdt = &mut *(rsdt_data.as_mut_ptr() as *mut SdtHeader);
        rsdt.signature = *b"RSDT";
        rsdt.length = rsdt_len as u32;
        rsdt.revision = 1;
        rsdt.oem_id = *b"AXVM  ";
        rsdt.oem_table_id = *b"AXVMRSDT";
        rsdt.oem_revision = 1;
        rsdt.creator_id = 0x4D5641;
        rsdt.creator_revision = 1;
        
        let ptr_loc = rsdt_data.as_mut_ptr().add(mem::size_of::<SdtHeader>()) as *mut u32;
        *ptr_loc = madt_addr as u32;
        rsdt.checksum = calculate_checksum(&rsdt_data);
    }
    mem.write_slice(rsdt_addr, &rsdt_data)?;

    // 3. Build RSDP (Root Pointer)
    let mut rsdp = Rsdp::default();
    rsdp.signature = *b"RSD PTR ";
    rsdp.rsdt_addr = rsdt_addr as u32;
    rsdp.length = mem::size_of::<Rsdp>() as u32;
    rsdp.revision = 0;
    rsdp.oem_id = *b"AXVM  ";

    unsafe {
        let rsdp_slice = slice::from_raw_parts(
            &rsdp as *const _ as *const u8,
            mem::size_of::<Rsdp>()
        );
        // Calculate checksum for first 20 bytes only (ACPI 1.0 RSDP)
        let checksum = calculate_checksum(&rsdp_slice[..20]);
        
        let mut rsdp_vec = rsdp_slice.to_vec();
        rsdp_vec[8] = checksum; // checksum field offset
        
        mem.write_slice(RSDP_START, &rsdp_vec)?;
    }

    println!(">>> [ACPI] SMP Tables generated for {} CPUs at {:#x}", vcpu_count, RSDP_START);
    Ok(())
}
