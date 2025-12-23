// src/main.rs - AxVM v0.7 Storage Edition
//!
//! AxVM Hypervisor - VirtIO Block Device Support
//!

mod memory;
mod vcpu;
mod error;
mod metrics;
mod serial;
mod linux;
mod loader;
mod acpi;
mod virtio;

use kvm_ioctls::{Kvm, VcpuFd};
use kvm_bindings::{KVM_MAX_CPUID_ENTRIES, kvm_pit_config, KVM_PIT_SPEAKER_DUMMY};
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crate::memory::GuestMemory;
use crate::error::{AxvmError, AxvmResult};
use crate::metrics::VmMetrics;
use crate::serial::SerialConsole;
use crate::virtio::VirtioBlock;

// ============================================================================
// CONSTANTS
// ============================================================================

const DEFAULT_MEM_SIZE: usize = 1024 * 1024 * 1024; // 1GB
const DEFAULT_VCPU_COUNT: u8 = 1; // Temporarily 1 for disk testing

// VirtIO-MMIO memory region (in device hole, outside guest RAM)
const VIRTIO_MMIO_BASE: u64 = 0xFEB00000; // Device memory hole
const VIRTIO_MMIO_SIZE: u64 = 0x1000;     // 4KB

// ============================================================================
// CONFIGURATION
// ============================================================================

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub mem_size: usize,
    pub vcpu_count: u8,
    pub kernel_path: Option<String>,
    pub kernel_cmdline: String,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            mem_size: DEFAULT_MEM_SIZE,
            vcpu_count: DEFAULT_VCPU_COUNT,
            kernel_path: Some("bzImage".to_string()),
            // Added: tsc=unstable clocksource=jiffies to bypass TSC issues
            kernel_cmdline: String::from(
                "console=ttyS0 earlyprintk=serial reboot=k panic=1 nokaslr nox2apic \
                 tsc=unstable clocksource=jiffies \
                 virtio_mmio.device=4K@0xFEB00000:5 root=/dev/vda"
            ),
        }
    }
}

// ============================================================================
// VCPU RUNNER
// ============================================================================

fn run_vcpu(
    vcpu: VcpuFd,
    cpu_id: u8,
    serial: Arc<SerialConsole>,
    virtio: Arc<VirtioBlock>,
    should_stop: Arc<AtomicBool>,
    mem_ptr: usize,
    mem_len: usize,
) {
    let mut vcpu = vcpu;
    let mut local_mem = std::mem::ManuallyDrop::new(
        unsafe { GuestMemory::from_raw_parts(mem_ptr, mem_len) }
    );
    
    loop {
        if should_stop.load(Ordering::Relaxed) { 
            break; 
        }

        match vcpu.run() {
            Ok(exit) => match exit {
                kvm_ioctls::VcpuExit::IoOut(port, data) => {
                    if port >= 0x3F8 && port < 0x3F8 + 8 {
                        serial.write(port, &data);
                    }
                },
                kvm_ioctls::VcpuExit::IoIn(port, data) => {
                    if port >= 0x3F8 && port < 0x3F8 + 8 {
                        let value = serial.read(port);
                        if !data.is_empty() {
                            data[0] = value;
                        }
                    }
                },
                // VirtIO MMIO interception
                kvm_ioctls::VcpuExit::MmioRead(addr, data) => {
                    if addr >= VIRTIO_MMIO_BASE && addr < VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE {
                        virtio.read(addr - VIRTIO_MMIO_BASE, data);
                    }
                },
                kvm_ioctls::VcpuExit::MmioWrite(addr, data) => {
                    if addr >= VIRTIO_MMIO_BASE && addr < VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE {
                        let _ = virtio.write(addr - VIRTIO_MMIO_BASE, data, &mut *local_mem);
                        // IRQ injection disabled for now - Linux uses polling
                    }
                },
                kvm_ioctls::VcpuExit::Hlt => {
                    if should_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                },
                kvm_ioctls::VcpuExit::Shutdown => {
                    println!("\n>>> [CPU {}] SHUTDOWN!", cpu_id);
                    should_stop.store(true, Ordering::Relaxed);
                    break;
                },
                _ => {}
            },
            Err(_) => {
                if should_stop.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_micros(100));
            }
        }
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() -> AxvmResult<()> {
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              AxVM Hypervisor v0.7                              ║");
    println!("║          Storage Edition - VirtIO Block 💾                     ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    let config = VmConfig::default();
    
    println!("Configuration:");
    println!("  Memory:   {} MB", config.mem_size / (1024 * 1024));
    println!("  vCPUs:    {}", config.vcpu_count);
    println!("  Kernel:   {:?}", config.kernel_path);
    println!("  VirtIO:   Block @ {:#x}", VIRTIO_MMIO_BASE);
    println!();

    // Initialize KVM
    let kvm = Kvm::new()
        .map_err(|e| AxvmError::KvmInit(e.to_string()))?;
    println!(">>> [INFO] KVM API Version: {}", kvm.get_api_version());
    
    let vm = kvm.create_vm()
        .map_err(|e| AxvmError::VmCreation(e.to_string()))?;

    // Create IRQCHIP
    vm.create_irq_chip()
        .map_err(|e| AxvmError::VmCreation(format!("IRQ Chip Error: {}", e)))?;
    println!(">>> [✓] IRQ Chip created");

    // Create PIT Timer
    let pit_config = kvm_pit_config {
        flags: KVM_PIT_SPEAKER_DUMMY,
        ..Default::default()
    };
    vm.create_pit2(pit_config)
        .map_err(|e| AxvmError::VmCreation(format!("PIT Error: {}", e)))?;
    println!(">>> [✓] PIT Timer created");

    // Allocate Memory
    let mut guest_mem = GuestMemory::new(config.mem_size)
        .map_err(|e| AxvmError::MemoryAllocation(e.to_string()))?;

    let mem_region = kvm_bindings::kvm_userspace_memory_region {
        slot: 0,
        guest_phys_addr: 0,
        memory_size: config.mem_size as u64,
        userspace_addr: guest_mem.as_ptr() as u64,
        flags: 0,
    };
    
    unsafe {
        vm.set_user_memory_region(mem_region)
            .map_err(|e| AxvmError::MemorySetup(e.to_string()))?;
    }
    println!(">>> [✓] Guest memory: {} MB", config.mem_size / (1024 * 1024));

    // Generate ACPI Tables
    acpi::setup_acpi(&mut guest_mem, config.vcpu_count)
        .map_err(|e| AxvmError::MemoryWrite(format!("ACPI Error: {}", e)))?;

    // Load Kernel
    let entry_point = if let Some(ref path) = config.kernel_path {
        let ep = loader::load_linux(
            &mut guest_mem, 
            path, 
            config.mem_size, 
            &config.kernel_cmdline
        ).map_err(|e| AxvmError::InternalError(e))?;
        
        println!(">>> [✓] Kernel loaded. Entry: {:#x}", ep);
        ep
    } else {
        0x100000
    };

    // Create vCPUs
    let mut vcpus = Vec::new();
    for cpu_id in 0..config.vcpu_count {
        let mut vcpu = vm.create_vcpu(cpu_id as u64)
            .map_err(|e| AxvmError::VcpuCreation(e.to_string()))?;
        
        let kvm_cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)
            .map_err(|e| AxvmError::CpuidSetup(e.to_string()))?;
        vcpu.set_cpuid2(&kvm_cpuid)
            .map_err(|e| AxvmError::CpuidSetup(e.to_string()))?;
        
        vcpu::setup_long_mode(&mut vcpu, &mut guest_mem, entry_point, 0x7000)
            .map_err(|e| AxvmError::LongModeSetup(e.to_string()))?;
        
        vcpus.push(vcpu);
    }
    println!(">>> [✓] Created {} vCPUs", config.vcpu_count);

    // Initialize VirtIO Block Device
    let virtio_blk = Arc::new(VirtioBlock::new());

    // Shared state
    let should_stop = Arc::new(AtomicBool::new(false));
    let serial = Arc::new(SerialConsole::new());
    let _metrics = Arc::new(VmMetrics::new());

    println!(">>> [Run] Spawning {} vCPU threads...", config.vcpu_count);
    println!();

    // Spawn threads
    let mut handles = Vec::new();
    let thread_ids: Arc<std::sync::Mutex<Vec<libc::pthread_t>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    
    // Get memory pointers for threads
    let mem_ptr = guest_mem.as_ptr() as usize;
    let mem_len = guest_mem.len();

    for (cpu_id, vcpu) in vcpus.into_iter().enumerate() {
        let serial = Arc::clone(&serial);
        let virtio = Arc::clone(&virtio_blk);
        let should_stop = Arc::clone(&should_stop);
        let thread_ids_clone = Arc::clone(&thread_ids);
        
        let handle = thread::spawn(move || {
            {
                let mut ids = thread_ids_clone.lock().unwrap();
                ids.push(unsafe { libc::pthread_self() });
            }
            run_vcpu(vcpu, cpu_id as u8, serial, virtio, should_stop, mem_ptr, mem_len);
        });
        handles.push(handle);
    }

    // Wait for threads to register
    thread::sleep(Duration::from_millis(100));

    // Ctrl+C handler
    let stop_handle = Arc::clone(&should_stop);
    let thread_ids_for_signal = Arc::clone(&thread_ids);
    ctrlc::set_handler(move || { 
        println!("\n>>> [Signal] Ctrl+C received, stopping...");
        stop_handle.store(true, Ordering::SeqCst);
        
        if let Ok(ids) = thread_ids_for_signal.lock() {
            for &tid in ids.iter() {
                unsafe { libc::pthread_kill(tid, libc::SIGUSR1); }
            }
        }
    }).expect("Ctrl-C handler error");

    // Wait for all threads
    for h in handles {
        let _ = h.join();
    }

    println!("\n>>> [Exit] AxVM terminated.");
    Ok(())
}