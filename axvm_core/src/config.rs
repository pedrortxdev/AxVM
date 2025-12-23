use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "AxVM")]
#[command(version = "0.7.0")]
#[command(about = "Lightweight KVM-based hypervisor", long_about = None)]
pub struct VmConfig {
    /// Memory size in MB (must be multiple of 2MB for HugePages)
    #[arg(short, long, default_value = "1024")]
    pub memory: usize,
    
    /// Number of vCPUs
    #[arg(short = 'c', long, default_value = "1")]
    pub vcpus: u8,
    
    /// Path to kernel image
    #[arg(short, long, default_value = "bzImage")]
    pub kernel: PathBuf,
    
    /// Path to disk image (optional)
    #[arg(short, long)]
    pub disk: Option<PathBuf>,
    
    /// Kernel command line arguments
    #[arg(long, default_value = "console=ttyS0 earlyprintk=serial reboot=k panic=1 nokaslr noapic virtio_mmio.device=4K@0xFEB00000:5 virtio_mmio.device=4K@0xFEB10000:6 root=/dev/vda rw")]
    pub cmdline: String,
    
    /// Increase verbosity (-v: info, -vv: debug, -vvv: trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
    
    /// Disable metrics collection
    #[arg(long)]
    pub no_metrics: bool,
}

impl VmConfig {
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        // Validate memory alignment (must be multiple of 2MB for HugePages)
        if self.memory % 2 != 0 {
            return Err(format!(
                "Memory size must be a multiple of 2MB for HugePages optimization. Got: {} MB",
                self.memory
            ));
        }
        
        // Validate minimum memory
        if self.memory < 128 {
            return Err(format!(
                "Memory size too small. Minimum: 128 MB, Got: {} MB",
                self.memory
            ));
        }
        
        // Validate maximum memory (safety limit: 64GB)
        if self.memory > 64 * 1024 {
            return Err(format!(
                "Memory size too large. Maximum: 64 GB, Got: {} MB",
                self.memory
            ));
        }
        
        // Validate vCPUs count
        if self.vcpus == 0 {
            return Err("vCPU count must be at least 1".to_string());
        }
        
        // Check against host CPU count
        let host_cpus = num_cpus::get();
        if self.vcpus as usize > host_cpus * 2 {
            return Err(format!(
                "vCPU count ({}) exceeds 2x host CPUs ({}). This may cause performance issues.",
                self.vcpus, host_cpus
            ));
        }
        
        // Validate kernel file exists
        if !self.kernel.exists() {
            return Err(format!(
                "Kernel image not found: {}",
                self.kernel.display()
            ));
        }
        
        // Validate disk file exists (if specified)
        if let Some(ref disk) = self.disk {
            if !disk.exists() {
                return Err(format!(
                    "Disk image not found: {}",
                    disk.display()
                ));
            }
        }
        
        Ok(())
    }
    
    /// Get tracing log level based on verbosity
    pub fn log_level(&self) -> &str {
        match self.verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    }
    
    /// Get memory size in bytes
    pub fn memory_bytes(&self) -> usize {
        self.memory * 1024 * 1024
    }
    
    /// Get kernel path as string
    pub fn kernel_path(&self) -> String {
        self.kernel.to_string_lossy().to_string()
    }
    
    /// Get disk path as optional string
    pub fn disk_path(&self) -> Option<String> {
        self.disk.as_ref().map(|p| p.to_string_lossy().to_string())
    }
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            memory: 1024,
            vcpus: 1,
            kernel: PathBuf::from("bzImage"),
            disk: None,
            cmdline: String::from(
                "console=ttyS0 earlyprintk=serial reboot=k panic=1 nokaslr noapic \
                 virtio_mmio.device=4K@0xFEB00000:5 virtio_mmio.device=4K@0xFEB10000:6 root=/dev/vda rw"
            ),
            verbose: 1,
            no_metrics: false,
        }
    }
}
