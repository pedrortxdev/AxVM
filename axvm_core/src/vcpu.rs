use kvm_ioctls::{Vcpu};
use kvm_bindings::{kvm_segment};
use crate::memory::GuestMemory;

const CR0_PE: u64 = 1 << 0;
const CR0_PG: u64 = 1 << 31;
const CR4_PAE: u64 = 1 << 5;
const EFER_LME: u64 = 1 << 8;

pub fn setup_long_mode(vcpu: &mut Vcpu, mem: &mut GuestMemory) -> Result<(), Box<dyn std::error::Error>> {
    setup_page_tables(mem);
    setup_gdt(mem);
    setup_registers(vcpu)?;
    Ok(())
}

fn setup_page_tables(mem: &mut GuestMemory) {
    // PML4[0] -> PDPT
    mem.write_u64(0x1000, 0x2000 | 0x3);
    // PDPT[0] -> PD
    mem.write_u64(0x2000, 0x3000 | 0x3);
    // PD[0] -> 2MB HugePage Identity Map
    mem.write_u64(0x3000, 0x0 | 0x83); 
}

fn setup_gdt(mem: &mut GuestMemory) {
    // Entry 0: Null
    mem.write_u64(0x4000, 0);
    
    // Entry 1: Code Segment (64-bit Kernel)
    // 0x00209A0000000000 -> P=1, L=1, D=0, Type=Code/Exec/Read
    mem.write_u64(0x4008, 0x00209A0000000000);

    // Entry 2: Data Segment (Flat Data)
    // 0x0000920000000000 -> P=1, Type=Data/Read/Write
    mem.write_u64(0x4010, 0x0000920000000000);
}

fn setup_registers(vcpu: &mut Vcpu) -> Result<(), kvm_ioctls::Error> {
    let mut sregs = vcpu.get_sregs()?;

    // 1. Configurar Page Tables (CR3)
    sregs.cr3 = 0x1000;

    // 2. Habilitar PAE
    sregs.cr4 |= CR4_PAE;

    // 3. EFER.LME *antes* de Paging (Critical Fix)
    sregs.efer |= EFER_LME; 

    // 4. Ativar Protected Mode (PE) e Paging (PG) -> Transição para Long Mode
    sregs.cr0 |= CR0_PE | CR0_PG;

    // 5. Configurar Segmentos
    let code_seg = kvm_segment {
        base: 0, limit: 0xFFFFFFFF, selector: 8, type_: 11,
        present: 1, dpl: 0, db: 0, s: 1, l: 1, g: 1, avl: 0, ..Default::default()
    };

    let data_seg = kvm_segment {
        base: 0, limit: 0xFFFFFFFF, selector: 16, type_: 3,
        present: 1, dpl: 0, db: 1, s: 1, l: 0, g: 1, avl: 0, ..Default::default()
    };

    sregs.cs = code_seg;
    sregs.ds = data_seg;
    sregs.es = data_seg;
    sregs.ss = data_seg; 
    sregs.fs = data_seg;
    sregs.gs = data_seg;

    sregs.gdt.base = 0x4000;
    sregs.gdt.limit = 23; 

    vcpu.set_sregs(&sregs)?;

    // Configurar Entry Point
    let mut regs = vcpu.get_regs()?;
    regs.rflags = 2;
    regs.rip = 0x0;
    regs.rax = 0; regs.rbx = 0;
    
    vcpu.set_regs(&regs)?;

    Ok(())
}
