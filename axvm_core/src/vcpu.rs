




use kvm_ioctls::VcpuFd;
use kvm_bindings::kvm_segment;
use crate::memory::GuestMemory;






const CR0_PE: u64 = 1 << 0;


const CR0_PG: u64 = 1 << 31;


const CR4_PAE: u64 = 1 << 5;


const EFER_LME: u64 = 1 << 8;


const EFER_LMA: u64 = 1 << 10;









#[allow(dead_code)]
pub fn setup_long_mode(
    vcpu: &mut VcpuFd, 
    mem: &mut GuestMemory,
    entry_point: u64,
    boot_params: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    setup_protected_mode_32bit(vcpu, mem, entry_point, boot_params)
}














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







fn setup_gdt_32bit(mem: &mut GuestMemory) -> Result<(), String> {
    
    mem.write_u64(0x4000, 0)?;

    
    
    
    
    mem.write_u64(0x4008, 0x00CF9A000000FFFF)?;

    
    
    
    mem.write_u64(0x4010, 0x00CF92000000FFFF)?;

    Ok(())
}


fn setup_registers_32bit(
    vcpu: &mut VcpuFd,
    entry_point: u64,
    boot_params_addr: u64,
) -> Result<(), kvm_ioctls::Error> {
    let mut sregs = vcpu.get_sregs()?;

    
    sregs.cr0 = CR0_PE;
    sregs.cr3 = 0;
    sregs.cr4 = 0;
    sregs.efer = 0; 

    
    let code_seg = kvm_segment {
        base: 0,
        limit: 0xFFFFFFFF,
        selector: 0x08,
        type_: 0x0B,     
        present: 1,
        dpl: 0,
        db: 1,           
        s: 1,            
        l: 0,            
        g: 1,            
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    
    let data_seg = kvm_segment {
        base: 0,
        limit: 0xFFFFFFFF,
        selector: 0x10,
        type_: 0x03,     
        present: 1,
        dpl: 0,
        db: 1,           
        s: 1,
        l: 0,
        g: 1,
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    
    let tr_seg = kvm_segment {
        base: 0,
        limit: 0,
        selector: 0,
        type_: 0x0B,     
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

    
    sregs.cs = code_seg;
    sregs.ds = data_seg;
    sregs.es = data_seg;
    sregs.fs = data_seg;
    sregs.gs = data_seg;
    sregs.ss = data_seg;
    sregs.tr = tr_seg;
    sregs.ldt = ldt_seg;

    
    sregs.gdt.base = 0x4000;
    sregs.gdt.limit = 23;

    
    sregs.idt.base = 0;
    sregs.idt.limit = 0;

    vcpu.set_sregs(&sregs)?;

    
    let mut regs = vcpu.get_regs()?;
    regs.rflags = 2;               
    regs.rip = entry_point;        
    regs.rsi = boot_params_addr;   
    regs.rax = 0;
    regs.rbx = 0;
    regs.rcx = 0;
    regs.rdx = 0;
    regs.rdi = 0;
    regs.rbp = 0;
    regs.rsp = 0x90000;            
    vcpu.set_regs(&regs)?;

    Ok(())
}


fn setup_page_tables_extended(mem: &mut GuestMemory) -> Result<(), String> {
    
    mem.write_u64(0x1000, 0x2000 | 0x3)?;

    
    mem.write_u64(0x2000, 0x3000 | 0x3)?;

    
    
    for i in 0u64..512 {
        let physical_addr = i * 0x200000; 
        let pd_entry = physical_addr | 0x83; 
        mem.write_u64(0x3000 + (i * 8) as usize, pd_entry)?;
    }

    Ok(())
}











fn setup_gdt(mem: &mut GuestMemory) -> Result<(), String> {
    
    mem.write_u64(0x4000, 0)?;

    
    
    
    mem.write_u64(0x4008, 0x00209A0000000000)?;

    
    
    
    mem.write_u64(0x4010, 0x0000920000000000)?;

    Ok(())
}














fn setup_registers_with_entry(vcpu: &mut VcpuFd, entry_point: u64) -> Result<(), kvm_ioctls::Error> {
    let mut sregs = vcpu.get_sregs()?;

    
    sregs.cr3 = 0x1000;

    
    sregs.cr4 |= CR4_PAE;

    
    
    sregs.efer |= EFER_LME | EFER_LMA;

    
    sregs.cr0 |= CR0_PE | CR0_PG;

    
    
    

    
    let code_seg = kvm_segment {
        base: 0,
        limit: 0xFFFFFFFF,
        selector: 8,
        type_: 11,       
        present: 1,
        dpl: 0,
        db: 0,           
        s: 1,            
        l: 1,            
        g: 1,            
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    
    let data_seg = kvm_segment {
        base: 0,
        limit: 0xFFFFFFFF,
        selector: 16,
        type_: 3,        
        present: 1,
        dpl: 0,
        db: 1,           
        s: 1,
        l: 0,
        g: 1,
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    
    
    let tr_seg = kvm_segment {
        base: 0,
        limit: 0,
        selector: 0,
        type_: 11,       
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
        unusable: 1,     
        padding: 0,
    };

    
    sregs.cs = code_seg;
    sregs.ds = data_seg;
    sregs.es = data_seg;
    sregs.fs = data_seg;
    sregs.gs = data_seg;
    sregs.ss = data_seg;
    sregs.tr = tr_seg;
    sregs.ldt = ldt_seg;

    
    sregs.gdt.base = 0x4000;
    sregs.gdt.limit = 23; 

    
    sregs.idt.base = 0;
    sregs.idt.limit = 0;

    vcpu.set_sregs(&sregs)?;

    
    let mut regs = vcpu.get_regs()?;
    regs.rflags = 2;           
    regs.rip = entry_point;    
    regs.rax = 0;
    regs.rbx = 0;
    
    regs.rsi = crate::linux::ZERO_PAGE_START as u64;
    vcpu.set_regs(&regs)?;

    Ok(())
}
