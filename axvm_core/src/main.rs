// main.rs
mod memory;
mod vcpu;
mod error;
mod metrics;

use kvm_ioctls::{Kvm, VmFd, VcpuFd, Cap}; // Adicionado Cap
use kvm_bindings::KVM_MAX_CPUID_ENTRIES;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;

use crate::memory::GuestMemory;
use crate::error::{AxvmError, AxvmResult};
use crate::metrics::VmMetrics;

// ============================================================================
// CONSTANTS
// ============================================================================

const GUEST_CODE: &[u8] = &[
    0x48, 0xB8, 0xEF, 0xBE, 0xAD, 0xDE, 0xBE, 0xBA, 0xFE, 0xCA, // MOVABS RAX, 0xCAFEBABEDEADBEEF
    0x66, 0xBA, 0xF8, 0x03,                                     // MOV DX, 0x3F8
    0xEE,                                                       // OUT DX, AL
    0xF4,                                                       // HLT
];

const DEFAULT_MEM_SIZE: usize = 0x40000; // 256KB
const SERIAL_PORT: u16 = 0x3F8;
const EXPECTED_RAX: u64 = 0xCAFEBABEDEADBEEF;
const MAX_ITERATIONS: u64 = 1_000_000;
const VCPU_TIMEOUT: Duration = Duration::from_secs(5);

// ============================================================================
// CONFIGURATION
// ============================================================================

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub mem_size: usize,
    pub vcpu_count: u32,
    pub enable_metrics: bool,
    pub max_iterations: u64,
    pub timeout: Duration,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            mem_size: DEFAULT_MEM_SIZE,
            vcpu_count: 1,
            enable_metrics: true,
            max_iterations: MAX_ITERATIONS,
            timeout: VCPU_TIMEOUT,
        }
    }
}

// ============================================================================
// VM STATE MACHINE
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmState {
    Created,
    Initialized,
    Running,
    Paused,
    Stopped,
    Failed,
}

// ============================================================================
// OWNED VM EXIT (Fixes Borrow Checker Issues)
// ============================================================================

// Esta struct é crucial. Ela "copia" os dados do VcpuExit (que tem referência)
// para uma estrutura que é dona dos dados (Owned).
// Isso permite liberar o empréstimo do vCPU antes de chamar handle_exit.
#[derive(Debug)]
pub enum OwnedVmExit {
    IoIn(u16, Vec<u8>),
    IoOut(u16, Vec<u8>),
    MmioRead(u64, Vec<u8>),
    MmioWrite(u64, Vec<u8>),
    Hlt,
    Shutdown,
    FailEntry(u64, u32),
    InternalError,
    SystemEvent(u32, Vec<u64>),
    Unknown,
}

// ============================================================================
// VIRTUAL MACHINE
// ============================================================================

pub struct VirtualMachine {
    #[allow(dead_code)] // Retained for VM lifecycle management
    kvm: Kvm,
    #[allow(dead_code)] // Retained for VM-level ioctls
    vm: VmFd,
    vcpu: VcpuFd,
    #[allow(dead_code)] // Retained for memory access operations
    guest_mem: GuestMemory,
    config: VmConfig,
    state: VmState,
    metrics: Arc<VmMetrics>,
    should_stop: Arc<AtomicBool>,
    iteration_count: AtomicU64,
}

impl VirtualMachine {
    /// Creates a new VM instance
    pub fn new(config: VmConfig) -> AxvmResult<Self> {
        log_info("Initializing AxVM Hypervisor...");

        // Initialize KVM
        let kvm = Kvm::new()
            .map_err(|e| AxvmError::KvmInit(format!("Failed to open /dev/kvm: {}", e)))?;

        // Sanity checks
        Self::verify_kvm_capabilities(&kvm)?;

        // Create VM
        let vm = kvm.create_vm()
            .map_err(|e| AxvmError::VmCreation(format!("Failed to create VM: {}", e)))?;

        // Allocate guest memory
        let mut guest_mem = GuestMemory::new(config.mem_size)
            .map_err(|e| AxvmError::MemoryAllocation(format!("Failed to allocate memory: {}", e)))?;

        // Load guest code (Agora funciona porque write_slice retorna Result)
        guest_mem.write_slice(0x0, GUEST_CODE)
            .map_err(|e| AxvmError::MemoryWrite(format!("Failed to write guest code: {}", e)))?;

        // Setup memory region
        let mem_region = kvm_bindings::kvm_userspace_memory_region {
            slot: 0,
            guest_phys_addr: 0,
            memory_size: config.mem_size as u64,
            userspace_addr: guest_mem.as_ptr() as u64,
            flags: 0,
        };

        unsafe {
            vm.set_user_memory_region(mem_region)
                .map_err(|e| AxvmError::MemorySetup(format!("Failed to set memory region: {}", e)))?;
        }

        log_success(&format!("Guest memory: {} bytes at {:#x}", 
            config.mem_size, guest_mem.as_ptr() as u64));

        // Create vCPU
        let mut vcpu = vm.create_vcpu(0)
            .map_err(|e| AxvmError::VcpuCreation(format!("Failed to create vCPU: {}", e)))?;

        // Setup CPUID (CRITICAL for 64-bit support)
        Self::setup_cpuid(&kvm, &mut vcpu)?;

        // Bootstrap long mode
        vcpu::setup_long_mode(&mut vcpu, &mut guest_mem)
            .map_err(|e| AxvmError::LongModeSetup(format!("Failed to setup long mode: {}", e)))?;

        log_success("Long mode initialized");

        let metrics = if config.enable_metrics {
            Arc::new(VmMetrics::new())
        } else {
            Arc::new(VmMetrics::disabled())
        };

        Ok(Self {
            kvm,
            vm,
            vcpu,
            guest_mem,
            config,
            state: VmState::Initialized,
            metrics,
            should_stop: Arc::new(AtomicBool::new(false)),
            iteration_count: AtomicU64::new(0),
        })
    }

    /// Verifies KVM capabilities
    fn verify_kvm_capabilities(kvm: &Kvm) -> AxvmResult<()> {
        let api_version = kvm.get_api_version();
        
        if api_version != 12 {
            return Err(AxvmError::KvmVersion(
                format!("Unsupported KVM API version: {} (expected 12)", api_version)
            ));
        }

        log_info(&format!("KVM API Version: {}", api_version));

        // Check required capabilities - CORRIGIDO para usar Cap enum
        // KVM ioctls 0.19+ usa Cap enum e retorna bool
        if !kvm.check_extension(Cap::UserMemory) {
            return Err(AxvmError::MissingCapability("UserMemory".to_string()));
        }
        if !kvm.check_extension(Cap::SetTssAddr) {
            return Err(AxvmError::MissingCapability("SetTssAddr".to_string()));
        }

        log_success("KVM capabilities verified");
        Ok(())
    }

    /// Setup CPUID for 64-bit support
    fn setup_cpuid(kvm: &Kvm, vcpu: &mut VcpuFd) -> AxvmResult<()> {
        let kvm_cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)
            .map_err(|e| AxvmError::CpuidSetup(format!("Failed to get supported CPUID: {}", e)))?;

        vcpu.set_cpuid2(&kvm_cpuid)
            .map_err(|e| AxvmError::CpuidSetup(format!("Failed to set CPUID: {}", e)))?;

        log_success("CPUID configured (64-bit support enabled)");
        Ok(())
    }

    /// Runs the VM
    pub fn run(&mut self) -> AxvmResult<VmExitReason> {
        if self.state != VmState::Initialized && self.state != VmState::Paused {
            return Err(AxvmError::InvalidState(
                format!("Cannot run VM in state: {:?}", self.state)
            ));
        }

        self.state = VmState::Running;
        log_info("Starting VM execution...");

        let start_time = Instant::now();
        let timeout = self.config.timeout;

        loop {
            // Check timeout
            if start_time.elapsed() > timeout {
                self.state = VmState::Failed;
                return Err(AxvmError::Timeout(
                    format!("VM execution timeout after {:?}", timeout)
                ));
            }

            // Check stop signal
            if self.should_stop.load(Ordering::Relaxed) {
                self.state = VmState::Stopped;
                return Ok(VmExitReason::Stopped);
            }

            // Check iteration limit
            let iterations = self.iteration_count.fetch_add(1, Ordering::Relaxed);
            if iterations >= self.config.max_iterations {
                self.state = VmState::Failed;
                return Err(AxvmError::MaxIterations(
                    format!("Exceeded maximum iterations: {}", self.config.max_iterations)
                ));
            }

            // Run vCPU
            self.metrics.record_vcpu_run();
            
            // CORREÇÃO CRÍTICA DE LIFETIMES
            // 1. Executamos o vCPU
            // 2. Convertemos a saída para OwnedVmExit (copiando dados)
            // 3. O escopo do borrow do vCPU termina
            // 4. Chamamos handle_exit (que pede &mut self)
            
            let exit_reason = match self.vcpu.run() {
                Ok(exit) => {
                    match exit {
                        kvm_ioctls::VcpuExit::IoIn(port, data) => OwnedVmExit::IoIn(port, data.to_vec()),
                        kvm_ioctls::VcpuExit::IoOut(port, data) => OwnedVmExit::IoOut(port, data.to_vec()),
                        kvm_ioctls::VcpuExit::Hlt => OwnedVmExit::Hlt,
                        kvm_ioctls::VcpuExit::Shutdown => OwnedVmExit::Shutdown,
                        kvm_ioctls::VcpuExit::FailEntry(r, c) => OwnedVmExit::FailEntry(r, c),
                        kvm_ioctls::VcpuExit::InternalError => OwnedVmExit::InternalError,
                        kvm_ioctls::VcpuExit::SystemEvent(t, f) => OwnedVmExit::SystemEvent(t, f.to_vec()),
                        _ => OwnedVmExit::Unknown,
                    }
                },
                Err(e) => {
                    self.metrics.record_error();
                    self.state = VmState::Failed;
                    return Err(AxvmError::VcpuRuntime(
                        format!("vCPU execution error: {:?}", e)
                    ));
                }
            };

            // Agora podemos chamar handle_exit porque self.vcpu não está mais "emprestado"
            match self.handle_exit(exit_reason)? {
                ExitAction::Continue => continue,
                ExitAction::Stop(reason) => {
                    self.state = VmState::Stopped;
                    return Ok(reason);
                }
            }
        }
    }

    /// Handles VM exit reasons
    fn handle_exit(&mut self, exit: OwnedVmExit) -> AxvmResult<ExitAction> {
        match exit {
            OwnedVmExit::IoOut(port, data) => {
                self.metrics.record_io_exit();
                self.handle_io_out(port, &data)
            }

            OwnedVmExit::IoIn(port, data) => {
                self.metrics.record_io_exit();
                log_debug(&format!("IO IN: port={:#x}, size={}", port, data.len()));
                Ok(ExitAction::Continue)
            }

            OwnedVmExit::Hlt => {
                log_success("Guest executed HLT instruction");
                Ok(ExitAction::Stop(VmExitReason::Halted))
            }

            OwnedVmExit::Shutdown => {
                log_info("Guest initiated shutdown");
                Ok(ExitAction::Stop(VmExitReason::Shutdown))
            }

            OwnedVmExit::FailEntry(reason, cpu) => {
                self.metrics.record_error();
                log_error(&format!("Hardware VM-Entry failure: reason={}, cpu={}", reason, cpu));
                
                // Try to dump CPU state for debugging
                if let Ok(regs) = self.vcpu.get_regs() {
                    log_error(&format!("CPU State: RIP={:#x}, RSP={:#x}, RFLAGS={:#x}",
                        regs.rip, regs.rsp, regs.rflags));
                }
                
                Err(AxvmError::HardwareFailure(
                    format!("VM-Entry failed: reason={}, cpu={}", reason, cpu)
                ))
            }

            OwnedVmExit::InternalError => {
                self.metrics.record_error();
                Err(AxvmError::InternalError(
                    "KVM internal error occurred".to_string()
                ))
            }

            OwnedVmExit::SystemEvent(event_type, flags) => {
                // CORRIGIDO: flags agora é Vec<u64>, usamos {:?} para formatar
                log_info(&format!("System event: type={}, flags={:?}", event_type, flags));
                Ok(ExitAction::Stop(VmExitReason::SystemEvent))
            }

            _ => {
                log_debug("Unhandled exit");
                Ok(ExitAction::Continue)
            }
        }
    }

    /// Handles IO OUT operations
    fn handle_io_out(&mut self, port: u16, data: &[u8]) -> AxvmResult<ExitAction> {
        if port == SERIAL_PORT {
            let regs = self.vcpu.get_regs()
                .map_err(|e| AxvmError::RegisterAccess(
                    format!("Failed to read registers: {}", e)
                ))?;

            log_info(&format!("Serial output: port={:#x}, RAX={:#x}", port, regs.rax));

            if regs.rax == EXPECTED_RAX {
                log_success(&format!("✓ Validation PASSED: RAX={:#x}", regs.rax));
                log_success("✓ 64-bit Long Mode confirmed");
                return Ok(ExitAction::Stop(VmExitReason::Success));
            } else {
                log_error(&format!("✗ Validation FAILED: RAX={:#x} (expected {:#x})",
                    regs.rax, EXPECTED_RAX));
                return Ok(ExitAction::Stop(VmExitReason::ValidationFailed));
            }
        }

        log_debug(&format!("IO OUT: port={:#x}, data={:?}", port, data));
        Ok(ExitAction::Continue)
    }

    /// Signals VM to stop
    pub fn stop(&self) {
        self.should_stop.store(true, Ordering::Relaxed);
    }

    /// Returns current VM state
    pub fn state(&self) -> VmState {
        self.state
    }

    /// Returns VM metrics
    pub fn metrics(&self) -> &VmMetrics {
        &self.metrics
    }

    /// Returns iteration count
    pub fn iterations(&self) -> u64 {
        self.iteration_count.load(Ordering::Relaxed)
    }

    /// Returns a cloneable handle to signal VM stop
    pub fn stop_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.should_stop)
    }
}

// ============================================================================
// EXIT HANDLING
// ============================================================================

#[derive(Debug)]
enum ExitAction {
    Continue,
    Stop(VmExitReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmExitReason {
    Success,
    Halted,
    Shutdown,
    Stopped,
    SystemEvent,
    ValidationFailed,
}

// ============================================================================
// LOGGING UTILITIES
// ============================================================================

fn log_info(msg: &str) {
    println!(">>> [INFO] {}", msg);
}

fn log_success(msg: &str) {
    println!(">>> [✓] {}", msg);
}

fn log_error(msg: &str) {
    eprintln!(">>> [ERROR] {}", msg);
}

fn log_debug(msg: &str) {
    if std::env::var("AXVM_DEBUG").is_ok() {
        println!(">>> [DEBUG] {}", msg);
    }
}

// ============================================================================
// MAIN ENTRY POINT
// ============================================================================

fn main() -> AxvmResult<()> {
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                  AxVM Hypervisor v0.3                          ║");
    println!("║              Production-Grade KVM Virtualization               ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    let config = VmConfig::default();
    
    println!("Configuration:");
    println!("  Memory Size:     {} bytes ({} KB)", config.mem_size, config.mem_size / 1024);
    println!("  vCPU Count:      {}", config.vcpu_count);
    println!("  Max Iterations:  {}", config.max_iterations);
    println!("  Timeout:         {:?}", config.timeout);
    println!();

    let start_time = Instant::now();

    // CORREÇÃO: clone() na config
    let mut vm = VirtualMachine::new(config.clone())?;

    // Setup Ctrl+C handler for graceful shutdown
    let stop_handle = vm.stop_handle();
    ctrlc::set_handler(move || {
        println!();
        log_info("Received interrupt signal (Ctrl+C). Stopping VM gracefully...");
        stop_handle.store(true, Ordering::Relaxed);
    }).expect("Failed to set Ctrl+C handler");

    log_info("Press Ctrl+C to stop the VM gracefully");
    println!();
    
    let result = vm.run();
    
    let elapsed = start_time.elapsed();

    println!();
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                      Execution Summary                         ║");
    println!("╠════════════════════════════════════════════════════════════════╣");
    
    match result {
        Ok(reason) => {
            println!("║  Status:         SUCCESS                                      ║");
            println!("║  Exit Reason:    {:?}                                    ║", reason);
        }
        Err(ref e) => {
            println!("║  Status:         FAILED                                       ║");
            println!("║  Error:          {}                              ║", e);
        }
    }
    
    println!("║  State:          {:?}                                    ║", vm.state());
    println!("║  Iterations:     {}                                        ║", vm.iterations());
    println!("║  Elapsed Time:   {:?}                                    ║", elapsed);
    
    if config.enable_metrics {
        let metrics = vm.metrics();
        println!("╠════════════════════════════════════════════════════════════════╣");
        println!("║                          Metrics                               ║");
        println!("╠════════════════════════════════════════════════════════════════╣");
        println!("║  vCPU Runs:      {}                                        ║", metrics.vcpu_runs());
        println!("║  IO Exits:       {}                                         ║", metrics.io_exits());
        println!("║  Errors:         {}                                          ║", metrics.errors());
    }
    
    println!("╚════════════════════════════════════════════════════════════════╝");

    result.map(|_| ())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_creation() {
        let config = VmConfig::default();
        let vm = VirtualMachine::new(config);
        assert!(vm.is_ok());
    }

    #[test]
    fn test_vm_state_transitions() {
        let config = VmConfig::default();
        let mut vm = VirtualMachine::new(config).unwrap();
        assert_eq!(vm.state(), VmState::Initialized);
        
        let _ = vm.run();
        assert!(vm.state() == VmState::Stopped || vm.state() == VmState::Failed);
    }

    #[test]
    fn test_vm_stop_signal() {
        let config = VmConfig::default();
        let vm = VirtualMachine::new(config).unwrap();
        
        vm.stop();
        assert!(vm.should_stop.load(Ordering::Relaxed));
    }
}