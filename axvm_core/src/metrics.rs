// metrics.rs
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::fmt;

// ============================================================================
// VM METRICS
// ============================================================================

#[derive(Debug)]
pub struct VmMetrics {
    enabled: AtomicBool,
    
    // Execution Metrics
    vcpu_runs: AtomicU64,
    vcpu_exits: AtomicU64,
    total_instructions: AtomicU64,
    
    // Exit Reason Counters
    io_exits: AtomicU64,
    mmio_exits: AtomicU64,
    hlt_exits: AtomicU64,
    interrupt_exits: AtomicU64,
    exception_exits: AtomicU64,
    
    // Error Tracking
    errors: AtomicU64,
    hardware_failures: AtomicU64,
    timeout_events: AtomicU64,
    
    // Memory Operations
    memory_reads: AtomicU64,
    memory_writes: AtomicU64,
    memory_faults: AtomicU64,
    
    // Performance Metrics
    total_cycles: AtomicU64,
    idle_cycles: AtomicU64,
    
    // Timing (stored as microseconds)
    total_runtime_us: AtomicU64,
    vcpu_active_time_us: AtomicU64,
}

impl VmMetrics {
    /// Creates a new enabled metrics instance
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            vcpu_runs: AtomicU64::new(0),
            vcpu_exits: AtomicU64::new(0),
            total_instructions: AtomicU64::new(0),
            io_exits: AtomicU64::new(0),
            mmio_exits: AtomicU64::new(0),
            hlt_exits: AtomicU64::new(0),
            interrupt_exits: AtomicU64::new(0),
            exception_exits: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            hardware_failures: AtomicU64::new(0),
            timeout_events: AtomicU64::new(0),
            memory_reads: AtomicU64::new(0),
            memory_writes: AtomicU64::new(0),
            memory_faults: AtomicU64::new(0),
            total_cycles: AtomicU64::new(0),
            idle_cycles: AtomicU64::new(0),
            total_runtime_us: AtomicU64::new(0),
            vcpu_active_time_us: AtomicU64::new(0),
        }
    }

    /// Creates a disabled metrics instance (zero overhead)
    pub fn disabled() -> Self {
        let metrics = Self::new();
        metrics.enabled.store(false, Ordering::Relaxed);
        metrics
    }

    /// Checks if metrics collection is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Enables metrics collection
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Release);
    }

    /// Disables metrics collection
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Release);
    }

    // ========================================================================
    // RECORDING METHODS
    // ========================================================================

    /// Records a vCPU run
    #[inline]
    pub fn record_vcpu_run(&self) {
        if self.is_enabled() {
            self.vcpu_runs.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records a vCPU exit
    #[inline]
    pub fn record_vcpu_exit(&self) {
        if self.is_enabled() {
            self.vcpu_exits.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records executed instructions
    #[inline]
    pub fn record_instructions(&self, count: u64) {
        if self.is_enabled() {
            self.total_instructions.fetch_add(count, Ordering::Relaxed);
        }
    }

    /// Records an I/O exit
    #[inline]
    pub fn record_io_exit(&self) {
        if self.is_enabled() {
            self.io_exits.fetch_add(1, Ordering::Relaxed);
            self.vcpu_exits.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records an MMIO exit
    #[inline]
    pub fn record_mmio_exit(&self) {
        if self.is_enabled() {
            self.mmio_exits.fetch_add(1, Ordering::Relaxed);
            self.vcpu_exits.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records a HLT exit
    #[inline]
    pub fn record_hlt_exit(&self) {
        if self.is_enabled() {
            self.hlt_exits.fetch_add(1, Ordering::Relaxed);
            self.vcpu_exits.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records an interrupt exit
    #[inline]
    pub fn record_interrupt_exit(&self) {
        if self.is_enabled() {
            self.interrupt_exits.fetch_add(1, Ordering::Relaxed);
            self.vcpu_exits.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records an exception exit
    #[inline]
    pub fn record_exception_exit(&self) {
        if self.is_enabled() {
            self.exception_exits.fetch_add(1, Ordering::Relaxed);
            self.vcpu_exits.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records a generic error
    #[inline]
    pub fn record_error(&self) {
        if self.is_enabled() {
            self.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records a hardware failure
    #[inline]
    pub fn record_hardware_failure(&self) {
        if self.is_enabled() {
            self.hardware_failures.fetch_add(1, Ordering::Relaxed);
            self.errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records a timeout event
    #[inline]
    pub fn record_timeout(&self) {
        if self.is_enabled() {
            self.timeout_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records a memory read operation
    #[inline]
    pub fn record_memory_read(&self) {
        if self.is_enabled() {
            self.memory_reads.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records a memory write operation
    #[inline]
    pub fn record_memory_write(&self) {
        if self.is_enabled() {
            self.memory_writes.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records a memory fault
    #[inline]
    pub fn record_memory_fault(&self) {
        if self.is_enabled() {
            self.memory_faults.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records CPU cycles
    #[inline]
    pub fn record_cycles(&self, cycles: u64) {
        if self.is_enabled() {
            self.total_cycles.fetch_add(cycles, Ordering::Relaxed);
        }
    }

    /// Records idle cycles
    #[inline]
    pub fn record_idle_cycles(&self, cycles: u64) {
        if self.is_enabled() {
            self.idle_cycles.fetch_add(cycles, Ordering::Relaxed);
        }
    }

    /// Records runtime duration
    #[inline]
    pub fn record_runtime(&self, duration: Duration) {
        if self.is_enabled() {
            self.total_runtime_us.fetch_add(
                duration.as_micros() as u64,
                Ordering::Relaxed
            );
        }
    }

    /// Records vCPU active time
    #[inline]
    pub fn record_vcpu_active_time(&self, duration: Duration) {
        if self.is_enabled() {
            self.vcpu_active_time_us.fetch_add(
                duration.as_micros() as u64,
                Ordering::Relaxed
            );
        }
    }

    // ========================================================================
    // ACCESSOR METHODS
    // ========================================================================

    pub fn vcpu_runs(&self) -> u64 {
        self.vcpu_runs.load(Ordering::Relaxed)
    }

    pub fn vcpu_exits(&self) -> u64 {
        self.vcpu_exits.load(Ordering::Relaxed)
    }

    pub fn total_instructions(&self) -> u64 {
        self.total_instructions.load(Ordering::Relaxed)
    }

    pub fn io_exits(&self) -> u64 {
        self.io_exits.load(Ordering::Relaxed)
    }

    pub fn mmio_exits(&self) -> u64 {
        self.mmio_exits.load(Ordering::Relaxed)
    }

    pub fn hlt_exits(&self) -> u64 {
        self.hlt_exits.load(Ordering::Relaxed)
    }

    pub fn interrupt_exits(&self) -> u64 {
        self.interrupt_exits.load(Ordering::Relaxed)
    }

    pub fn exception_exits(&self) -> u64 {
        self.exception_exits.load(Ordering::Relaxed)
    }

    pub fn errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    pub fn hardware_failures(&self) -> u64 {
        self.hardware_failures.load(Ordering::Relaxed)
    }

    pub fn timeout_events(&self) -> u64 {
        self.timeout_events.load(Ordering::Relaxed)
    }

    pub fn memory_reads(&self) -> u64 {
        self.memory_reads.load(Ordering::Relaxed)
    }

    pub fn memory_writes(&self) -> u64 {
        self.memory_writes.load(Ordering::Relaxed)
    }

    pub fn memory_faults(&self) -> u64 {
        self.memory_faults.load(Ordering::Relaxed)
    }

    pub fn total_cycles(&self) -> u64 {
        self.total_cycles.load(Ordering::Relaxed)
    }

    pub fn idle_cycles(&self) -> u64 {
        self.idle_cycles.load(Ordering::Relaxed)
    }

    pub fn total_runtime(&self) -> Duration {
        Duration::from_micros(self.total_runtime_us.load(Ordering::Relaxed))
    }

    pub fn vcpu_active_time(&self) -> Duration {
        Duration::from_micros(self.vcpu_active_time_us.load(Ordering::Relaxed))
    }

    // ========================================================================
    // COMPUTED METRICS
    // ========================================================================

    /// Calculates the average exits per run
    pub fn avg_exits_per_run(&self) -> f64 {
        let runs = self.vcpu_runs();
        if runs == 0 {
            0.0
        } else {
            self.vcpu_exits() as f64 / runs as f64
        }
    }

    /// Calculates the exit rate (exits per second)
    pub fn exit_rate(&self) -> f64 {
        let runtime_secs = self.total_runtime().as_secs_f64();
        if runtime_secs == 0.0 {
            0.0
        } else {
            self.vcpu_exits() as f64 / runtime_secs
        }
    }

    /// Calculates instructions per cycle (IPC)
    pub fn instructions_per_cycle(&self) -> f64 {
        let cycles = self.total_cycles();
        if cycles == 0 {
            0.0
        } else {
            self.total_instructions() as f64 / cycles as f64
        }
    }

    /// Calculates CPU utilization percentage
    pub fn cpu_utilization(&self) -> f64 {
        let total = self.total_cycles();
        if total == 0 {
            0.0
        } else {
            let active = total.saturating_sub(self.idle_cycles());
            (active as f64 / total as f64) * 100.0
        }
    }

    /// Calculates vCPU efficiency (active time / total runtime)
    pub fn vcpu_efficiency(&self) -> f64 {
        let total_us = self.total_runtime_us.load(Ordering::Relaxed);
        if total_us == 0 {
            0.0
        } else {
            let active_us = self.vcpu_active_time_us.load(Ordering::Relaxed);
            (active_us as f64 / total_us as f64) * 100.0
        }
    }

    /// Calculates error rate
    pub fn error_rate(&self) -> f64 {
        let runs = self.vcpu_runs();
        if runs == 0 {
            0.0
        } else {
            (self.errors() as f64 / runs as f64) * 100.0
        }
    }

    // ========================================================================
    // UTILITY METHODS
    // ========================================================================

    /// Resets all metrics to zero
    pub fn reset(&self) {
        if !self.is_enabled() {
            return;
        }

        self.vcpu_runs.store(0, Ordering::Relaxed);
        self.vcpu_exits.store(0, Ordering::Relaxed);
        self.total_instructions.store(0, Ordering::Relaxed);
        self.io_exits.store(0, Ordering::Relaxed);
        self.mmio_exits.store(0, Ordering::Relaxed);
        self.hlt_exits.store(0, Ordering::Relaxed);
        self.interrupt_exits.store(0, Ordering::Relaxed);
        self.exception_exits.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.hardware_failures.store(0, Ordering::Relaxed);
        self.timeout_events.store(0, Ordering::Relaxed);
        self.memory_reads.store(0, Ordering::Relaxed);
        self.memory_writes.store(0, Ordering::Relaxed);
        self.memory_faults.store(0, Ordering::Relaxed);
        self.total_cycles.store(0, Ordering::Relaxed);
        self.idle_cycles.store(0, Ordering::Relaxed);
        self.total_runtime_us.store(0, Ordering::Relaxed);
        self.vcpu_active_time_us.store(0, Ordering::Relaxed);
    }

    /// Creates a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            timestamp: Instant::now(),
            vcpu_runs: self.vcpu_runs(),
            vcpu_exits: self.vcpu_exits(),
            io_exits: self.io_exits(),
            errors: self.errors(),
            total_runtime: self.total_runtime(),
        }
    }
}

impl Default for VmMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// METRICS SNAPSHOT
// ============================================================================

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub timestamp: Instant,
    pub vcpu_runs: u64,
    pub vcpu_exits: u64,
    pub io_exits: u64,
    pub errors: u64,
    pub total_runtime: Duration,
}

impl MetricsSnapshot {
    /// Calculates the delta between two snapshots
    pub fn delta(&self, other: &MetricsSnapshot) -> MetricsDelta {
        MetricsDelta {
            duration: self.timestamp.duration_since(other.timestamp),
            vcpu_runs: self.vcpu_runs.saturating_sub(other.vcpu_runs),
            vcpu_exits: self.vcpu_exits.saturating_sub(other.vcpu_exits),
            io_exits: self.io_exits.saturating_sub(other.io_exits),
            errors: self.errors.saturating_sub(other.errors),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricsDelta {
    pub duration: Duration,
    pub vcpu_runs: u64,
    pub vcpu_exits: u64,
    pub io_exits: u64,
    pub errors: u64,
}

// ============================================================================
// DISPLAY IMPLEMENTATION
// ============================================================================

impl fmt::Display for VmMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "VM Metrics:")?;
        writeln!(f, "  Enabled:           {}", self.is_enabled())?;
        writeln!(f, "  vCPU Runs:         {}", self.vcpu_runs())?;
        writeln!(f, "  vCPU Exits:        {}", self.vcpu_exits())?;
        writeln!(f, "  - I/O Exits:       {}", self.io_exits())?;
        writeln!(f, "  - MMIO Exits:      {}", self.mmio_exits())?;
        writeln!(f, "  - HLT Exits:       {}", self.hlt_exits())?;
        writeln!(f, "  - Interrupts:      {}", self.interrupt_exits())?;
        writeln!(f, "  - Exceptions:      {}", self.exception_exits())?;
        writeln!(f, "  Errors:            {}", self.errors())?;
        writeln!(f, "  Hardware Failures: {}", self.hardware_failures())?;
        writeln!(f, "  Memory Ops:        {} reads, {} writes", 
            self.memory_reads(), self.memory_writes())?;
        writeln!(f, "  Total Runtime:     {:?}", self.total_runtime())?;
        writeln!(f, "  CPU Utilization:   {:.2}%", self.cpu_utilization())?;
        writeln!(f, "  vCPU Efficiency:   {:.2}%", self.vcpu_efficiency())?;
        writeln!(f, "  Error Rate:        {:.4}%", self.error_rate())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = VmMetrics::new();
        assert!(metrics.is_enabled());
        assert_eq!(metrics.vcpu_runs(), 0);
    }

    #[test]
    fn test_metrics_disabled() {
        let metrics = VmMetrics::disabled();
        assert!(!metrics.is_enabled());
        
        metrics.record_vcpu_run();
        assert_eq!(metrics.vcpu_runs(), 0); // Should not record
    }

    #[test]
    fn test_metrics_recording() {
        let metrics = VmMetrics::new();
        
        metrics.record_vcpu_run();
        metrics.record_vcpu_run();
        assert_eq!(metrics.vcpu_runs(), 2);
        
        metrics.record_io_exit();
        assert_eq!(metrics.io_exits(), 1);
        assert_eq!(metrics.vcpu_exits(), 1);
    }

    #[test]
    fn test_metrics_computed() {
        let metrics = VmMetrics::new();
        
        metrics.record_vcpu_run();
        metrics.record_vcpu_run();
        metrics.record_io_exit();
        metrics.record_io_exit();
        metrics.record_io_exit();
        
        assert_eq!(metrics.avg_exits_per_run(), 1.5);
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = VmMetrics::new();
        
        metrics.record_vcpu_run();
        metrics.record_error();
        assert_eq!(metrics.vcpu_runs(), 1);
        assert_eq!(metrics.errors(), 1);
        
        metrics.reset();
        assert_eq!(metrics.vcpu_runs(), 0);
        assert_eq!(metrics.errors(), 0);
    }

    #[test]
    fn test_metrics_snapshot() {
        let metrics = VmMetrics::new();
        
        let snap1 = metrics.snapshot();
        metrics.record_vcpu_run();
        metrics.record_vcpu_run();
        let snap2 = metrics.snapshot();
        
        let delta = snap2.delta(&snap1);
        assert_eq!(delta.vcpu_runs, 2);
    }

    #[test]
    fn test_cpu_utilization() {
        let metrics = VmMetrics::new();
        
        metrics.record_cycles(1000);
        metrics.record_idle_cycles(200);
        
        assert_eq!(metrics.cpu_utilization(), 80.0);
    }
}