




mod memory;
mod vcpu;
mod error;
mod metrics;
mod serial;
mod linux;
mod loader;
mod acpi;
mod virtio;
mod config;
mod tap;
mod virtio_net;

use kvm_ioctls::{Kvm, VcpuFd};
use kvm_bindings::{KVM_MAX_CPUID_ENTRIES, kvm_pit_config, KVM_PIT_SPEAKER_DUMMY};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use clap::Parser;

use crate::memory::GuestMemory;
use crate::error::{AxvmError, AxvmResult};
use crate::metrics::VmMetrics;
use crate::serial::SerialConsole;
use crate::virtio::VirtioBlock;
use crate::virtio_net::VirtioNet;
use crate::config::VmConfig;



const VIRTIO_MMIO_BASE: u64 = 0xFEB00000; 
const VIRTIO_MMIO_SIZE: u64 = 0x1000;
const VIRTIO_NET_MMIO_BASE: u64 = 0xFEB10000;
const VIRTIO_NET_MMIO_SIZE: u64 = 0x1000;     





fn run_vcpu(
    vcpu: VcpuFd,
    vm_fd: Arc<std::sync::Mutex<kvm_ioctls::VmFd>>,
    cpu_id: u8,
    serial: Arc<SerialConsole>,
    virtio: Arc<VirtioBlock>,
    virtio_net: Arc<std::sync::Mutex<VirtioNet>>,
    should_stop: Arc<AtomicBool>,
    guest_mem: Arc<std::sync::Mutex<GuestMemory>>,
    metrics: Arc<VmMetrics>,
) {
    let mut vcpu = vcpu;
    
    tracing::info!(cpu_id = cpu_id, "vCPU thread started");
    
    loop {
        if should_stop.load(Ordering::Relaxed) { 
            tracing::debug!(cpu_id = cpu_id, "vCPU received stop signal");
            break; 
        }

        metrics.record_vcpu_run();

        // Process network packets (only on CPU 0 to avoid contention)
        if cpu_id == 0 {
            if let Ok(mut mem) = guest_mem.try_lock() {
                let mem_ptr = mem.as_ptr();
                let mem_len = mem.len();
                let mem_slice = unsafe { std::slice::from_raw_parts_mut(mem_ptr, mem_len) };
                
                if let Ok(net) = virtio_net.try_lock() {
                    let rx_work = net.process_rx(mem_slice);
                    let tx_work = net.process_tx(mem_slice);
                    
                    if (rx_work || tx_work) && net.should_interrupt() {
                        if let Ok(vm) = vm_fd.lock() {
                            let _ = vm.set_irq_line(6, true);
                            let _ = vm.set_irq_line(6, false);
                        }
                    }
                }
            }
        }

        match vcpu.run() {
            Ok(exit) => {
                metrics.record_vcpu_exit();
                
                match exit {
                    kvm_ioctls::VcpuExit::IoOut(port, data) => {
                        if port >= 0x3F8 && port < 0x3F8 + 8 {
                            serial.write(port, &data);
                            metrics.record_io_exit();
                        }
                    },
                    kvm_ioctls::VcpuExit::IoIn(port, data) => {
                        if port >= 0x3F8 && port < 0x3F8 + 8 {
                            let value = serial.read(port);
                            if !data.is_empty() {
                                data[0] = value;
                            }
                            metrics.record_io_exit();
                        }
                    },
                    
                    kvm_ioctls::VcpuExit::MmioRead(addr, data) => {
                        if addr >= VIRTIO_MMIO_BASE && addr < VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE {
                            virtio.read(addr - VIRTIO_MMIO_BASE, data);
                            metrics.record_mmio_exit();
                        } else if addr >= VIRTIO_NET_MMIO_BASE && addr < VIRTIO_NET_MMIO_BASE + VIRTIO_NET_MMIO_SIZE {
                            if let Ok(net) = virtio_net.lock() {
                                net.read(addr - VIRTIO_NET_MMIO_BASE, data);
                                metrics.record_mmio_exit();
                            }
                        }
                    },
                    kvm_ioctls::VcpuExit::MmioWrite(addr, data) => {
                        if addr >= VIRTIO_MMIO_BASE && addr < VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE {
                            let irq_needed = match guest_mem.lock() {
                                Ok(mut mem) => {
                                    match virtio.write(addr - VIRTIO_MMIO_BASE, data, &mut *mem) {
                                        Ok(needs_irq) => needs_irq,
                                        Err(e) => {
                                            tracing::warn!(cpu_id = cpu_id, error = %e, "VirtIO write error");
                                            false
                                        }
                                    }
                                },
                                Err(e) => {
                                    tracing::error!(cpu_id = cpu_id, error = %e, "Failed to lock guest memory");
                                    metrics.record_error();
                                    false
                                }
                            };
                            
                            if irq_needed {
                                match vm_fd.lock() {
                                    Ok(vm) => {
                                        if let Err(e) = vm.set_irq_line(5, true) {
                                            tracing::warn!(cpu_id = cpu_id, error = %e, "IRQ injection failed (set)");
                                            metrics.record_error();
                                        }
                                        if let Err(e) = vm.set_irq_line(5, false) {
                                            tracing::warn!(cpu_id = cpu_id, error = %e, "IRQ injection failed (clear)");
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!(cpu_id = cpu_id, error = %e, "Failed to lock VM fd for IRQ");
                                        metrics.record_error();
                                    }
                                }
                            }
                            metrics.record_mmio_exit();
                        } else if addr >= VIRTIO_NET_MMIO_BASE && addr < VIRTIO_NET_MMIO_BASE + VIRTIO_NET_MMIO_SIZE {
                            if let Ok(net) = virtio_net.lock() {
                                match net.write(addr - VIRTIO_NET_MMIO_BASE, data) {
                                    Ok(needs_irq) => {
                                        if needs_irq {
                                            if let Ok(vm) = vm_fd.lock() {
                                                if let Err(e) = vm.set_irq_line(6, true) {
                                                    tracing::warn!(cpu_id = cpu_id, error = %e, "Net IRQ injection failed (set)");
                                                }
                                                if let Err(e) = vm.set_irq_line(6, false) {
                                                    tracing::warn!(cpu_id = cpu_id, error = %e, "Net IRQ injection failed (clear)");
                                                }
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        tracing::warn!(cpu_id = cpu_id, error = %e, "VirtIO-Net write error");
                                    }
                                }
                                metrics.record_mmio_exit();
                            }
                        }
                    },
                    kvm_ioctls::VcpuExit::Hlt => {
                        if should_stop.load(Ordering::Relaxed) {
                            break;
                        }
                        metrics.record_hlt_exit();
                        thread::yield_now();
                    },
                    kvm_ioctls::VcpuExit::Shutdown => {
                        tracing::info!(cpu_id = cpu_id, "vCPU shutdown");
                        println!("\n>>> [CPU {}] SHUTDOWN!", cpu_id);
                        should_stop.store(true, Ordering::Relaxed);
                        break;
                    },
                    _ => {}
                }
            },
            Err(e) => {
                // Check for EAGAIN (errno 11) and EINTR (errno 4)
                let errno = e.errno();
                
                if errno == 11 {
                    // EAGAIN = vCPU not ready yet (normal during SMP boot)
                    tracing::trace!(cpu_id = cpu_id, "vCPU not ready (EAGAIN)");
                    thread::yield_now();
                    continue;
                } else if errno == 4 {
                    // EINTR = signal received
                    tracing::debug!(cpu_id = cpu_id, "vCPU interrupted by signal");
                    if should_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    continue;
                } else {
                    // Real error!
                    if should_stop.load(Ordering::Relaxed) {
                        break;
                    }
                    tracing::error!(cpu_id = cpu_id, error = %e, errno = errno, "Fatal vCPU error");
                    metrics.record_error();
                    should_stop.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    }
    
    tracing::info!(cpu_id = cpu_id, "vCPU thread exiting");
}





fn main() -> AxvmResult<()> {
    let config = VmConfig::parse();
    
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(config.log_level()))
        )
        .with_target(false)
        .init();

    tracing::info!("AxVM Hypervisor v0.7 starting");
    
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║              AxVM Hypervisor v0.7                              ║");
    println!("║          Storage Edition - VirtIO Block 💾                     ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    if let Err(e) = config.validate() {
        eprintln!("Configuration Error: {}", e);
        std::process::exit(1);
    }
    
    println!("Configuration:");
    println!("  Memory:   {} MB", config.memory);
    println!("  vCPUs:    {}", config.vcpus);
    println!("  Kernel:   {}", config.kernel.display());
    if let Some(ref disk) = config.disk {
        println!("  Disk:     {}", disk.display());
    }
    println!("  VirtIO:   Block @ {:#x}", VIRTIO_MMIO_BASE);
    println!("  Log:      {}", config.log_level());
    println!();

    
    let kvm = Kvm::new()
        .map_err(|e| AxvmError::KvmInit(e.to_string()))?;
    println!(">>> [INFO] KVM API Version: {}", kvm.get_api_version());
    
    let vm = kvm.create_vm()
        .map_err(|e| AxvmError::VmCreation(e.to_string()))?;

    
    vm.create_irq_chip()
        .map_err(|e| AxvmError::VmCreation(format!("IRQ Chip Error: {}", e)))?;
    println!(">>> [✓] IRQ Chip created");

    
    let pit_config = kvm_pit_config {
        flags: KVM_PIT_SPEAKER_DUMMY,
        ..Default::default()
    };
    vm.create_pit2(pit_config)
        .map_err(|e| AxvmError::VmCreation(format!("PIT Error: {}", e)))?;
    println!(">>> [✓] PIT Timer created");

    
    let mut guest_mem = GuestMemory::new(config.memory_bytes())
        .map_err(|e| AxvmError::MemoryAllocation(e.to_string()))?;

    let mem_region = kvm_bindings::kvm_userspace_memory_region {
        slot: 0,
        guest_phys_addr: 0,
        memory_size: config.memory_bytes() as u64,
        userspace_addr: guest_mem.as_ptr() as u64,
        flags: 0,
    };
    
    unsafe {
        vm.set_user_memory_region(mem_region)
            .map_err(|e| AxvmError::MemorySetup(e.to_string()))?;
    }
    println!(">>> [✓] Guest memory: {} MB", config.memory);

    
    acpi::setup_acpi(&mut guest_mem, config.vcpus)
        .map_err(|e| AxvmError::MemoryWrite(format!("ACPI Error: {}", e)))?;

    
    let entry_point = {
        let ep = loader::load_linux(
            &mut guest_mem, 
            &config.kernel_path(), 
            config.memory_bytes(), 
            &config.cmdline
        ).map_err(AxvmError::InternalError)?;
        
        println!(">>> [✓] Kernel loaded. Entry: {:#x}", ep);
        ep
    };

    let mut vcpus = Vec::new();
    for cpu_id in 0..config.vcpus {
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
    println!(">>> [✓] Created {} vCPUs", config.vcpus);

    let virtio_blk = Arc::new(VirtioBlock::new(config.disk_path().as_deref()));

    let virtio_net = match tap::TapInterface::new(Some("axvm-tap0")) {
        Ok(tap_iface) => {
            println!(">>> [Net] TAP interface '{}' created successfully", tap_iface.name());
            tracing::info!(name = tap_iface.name(), "TAP interface created");
            Arc::new(std::sync::Mutex::new(VirtioNet::new(Some(tap_iface))))
        },
        Err(e) => {
            eprintln!(">>> [Net] WARN: Failed to create TAP (run with sudo?): {}. Network disabled.", e);
            tracing::warn!(error = %e, "Failed to create TAP interface");
            Arc::new(std::sync::Mutex::new(VirtioNet::new(None)))
        }
    };

    let should_stop = Arc::new(AtomicBool::new(false));
    let serial = Arc::new(SerialConsole::new());
    let metrics = if config.no_metrics {
        Arc::new(VmMetrics::disabled())
    } else {
        Arc::new(VmMetrics::new())
    };

    println!(">>> [Run] Spawning {} vCPU threads...", config.vcpus);
    println!();

    let shared_mem = Arc::new(std::sync::Mutex::new(guest_mem));
    let shared_vm = Arc::new(std::sync::Mutex::new(vm));

    let mut handles = Vec::new();
    for (cpu_id, vcpu) in vcpus.into_iter().enumerate() {
        let serial = Arc::clone(&serial);
        let virtio = Arc::clone(&virtio_blk);
        let virtio_net = Arc::clone(&virtio_net);
        let should_stop = Arc::clone(&should_stop);
        let vm_fd = Arc::clone(&shared_vm);
        let guest_mem = Arc::clone(&shared_mem);
        let metrics = Arc::clone(&metrics);
        
        let handle = thread::spawn(move || {
            run_vcpu(vcpu, vm_fd, cpu_id as u8, serial, virtio, virtio_net, should_stop, guest_mem, metrics);
        });
        handles.push(handle);
    }

    let stop_handle = Arc::clone(&should_stop);
    let metrics_clone = Arc::clone(&metrics);
    ctrlc::set_handler(move || { 
        println!("\n>>> [Signal] Ctrl+C received, stopping...");
        stop_handle.store(true, Ordering::SeqCst);
        tracing::info!("Shutdown signal received");
    }).expect("Ctrl-C handler error");

    for h in handles {
        let _ = h.join();
    }

    println!("\n>>> [Exit] AxVM terminated.");
    println!("\n{}", metrics_clone);
    tracing::info!("AxVM shutdown complete");
    
    Ok(())
}