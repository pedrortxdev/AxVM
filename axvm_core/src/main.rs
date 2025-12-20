mod memory;
mod vcpu;

use kvm_ioctls::{Kvm, VcpuExit};
use crate::memory::GuestMemory;

// Payload Inalterado: O detector de Long Mode (MOVABS)
const GUEST_CODE: &[u8] = &[
    0x48, 0xB8, 0xEF, 0xBE, 0xAD, 0xDE, 0xBE, 0xBA, 0xFE, 0xCA, // MOVABS RAX, 0xCAFEBABEDEADBEEF
    0x66, 0xBA, 0xF8, 0x03,                                     // MOV DX, 0x3F8
    0xEE,                                                       // OUT DX, AL
    0xF4,                                                       // HLT
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kvm = Kvm::new()?;
    let vm = kvm.create_vm()?;

    // 1GB Virtual Space (usando 256KB reais alocados)
    let mem_size = 0x40000; 
    let mut guest_mem = GuestMemory::new(mem_size)?;

    guest_mem.write_slice(0x0, GUEST_CODE);

    let mem_region = kvm_bindings::kvm_userspace_memory_region {
        slot: 0,
        guest_phys_addr: 0,
        memory_size: mem_size as u64,
        userspace_addr: guest_mem.as_ptr() as u64,
        flags: 0,
    };
    unsafe { vm.set_user_memory_region(mem_region)? };

    let mut vcpu = vm.create_vcpu(0)?;

    // Bootstrapping
    vcpu::setup_long_mode(&mut vcpu, &mut guest_mem)?;

    println!(">>> AxVM v0.2-stable: Bootstrapping Long Mode...");

    loop {
        match vcpu.run() {
            Ok(reason) => match reason {
                VcpuExit::IoOut(port, _data) => {
                    if port == 0x3F8 {
                        let regs = vcpu.get_regs()?;
                        if regs.rax == 0xCAFEBABEDEADBEEF {
                            println!(">>> [PASS] RAX: {:#X}", regs.rax);
                            println!(">>> [INFO] CPU state: 64-bit Long Mode confirmed.");
                            break;
                        } else {
                            println!(">>> [FAIL] RAX mismatch: {:#X}", regs.rax);
                            break;
                        }
                    }
                }
                VcpuExit::Hlt => { println!(">>> [INFO] Guest Halted."); break; }
                VcpuExit::FailEntry(reason, _) => {
                    println!(">>> [CRITICAL] Hardware FailEntry: {}", reason);
                    break;
                }
                _ => {}
            },
            Err(e) => { println!(">>> [ERROR] vCPU Fault: {}", e); break; }
        }
    }

    Ok(())
  }
