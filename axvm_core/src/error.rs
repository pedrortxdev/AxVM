




#![allow(dead_code)]

use std::fmt;
use std::io;






#[derive(Debug)]
pub enum AxvmError {
    
    KvmInit(String),
    KvmVersion(String),
    MissingCapability(String),

    
    VmCreation(String),
    VcpuCreation(String),
    InvalidState(String),

    
    MemoryAllocation(String),
    MemoryWrite(String),
    MemoryRead(String),
    MemorySetup(String),
    MemoryAlignment(String),

    
    CpuidSetup(String),
    RegisterAccess(String),
    LongModeSetup(String),
    SregSetup(String),

    
    VcpuRuntime(String),
    Timeout(String),
    MaxIterations(String),
    HardwareFailure(String),
    InternalError(String),

    
    IoError(io::Error),
    DeviceNotFound(String),

    
    InvalidConfiguration(String),
    UnsupportedFeature(String),
    
    IrqInjection(String),
    LockPoisoned(String),
}





impl fmt::Display for AxvmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KvmInit(msg) => write!(f, "KVM initialization failed: {}", msg),
            Self::KvmVersion(msg) => write!(f, "KVM version mismatch: {}", msg),
            Self::MissingCapability(msg) => write!(f, "Required KVM capability missing: {}", msg),

            Self::VmCreation(msg) => write!(f, "VM creation failed: {}", msg),
            Self::VcpuCreation(msg) => write!(f, "vCPU creation failed: {}", msg),
            Self::InvalidState(msg) => write!(f, "Invalid VM state transition: {}", msg),

            Self::MemoryAllocation(msg) => write!(f, "Memory allocation failed: {}", msg),
            Self::MemoryWrite(msg) => write!(f, "Memory write failed: {}", msg),
            Self::MemoryRead(msg) => write!(f, "Memory read failed: {}", msg),
            Self::MemorySetup(msg) => write!(f, "Memory region setup failed: {}", msg),
            Self::MemoryAlignment(msg) => write!(f, "Memory alignment error: {}", msg),

            Self::CpuidSetup(msg) => write!(f, "CPUID configuration failed: {}", msg),
            Self::RegisterAccess(msg) => write!(f, "Register access failed: {}", msg),
            Self::LongModeSetup(msg) => write!(f, "Long mode setup failed: {}", msg),
            Self::SregSetup(msg) => write!(f, "Segment register setup failed: {}", msg),

            Self::VcpuRuntime(msg) => write!(f, "vCPU runtime error: {}", msg),
            Self::Timeout(msg) => write!(f, "Operation timed out: {}", msg),
            Self::MaxIterations(msg) => write!(f, "Maximum iterations exceeded: {}", msg),
            Self::HardwareFailure(msg) => write!(f, "Hardware virtualization failure: {}", msg),
            Self::InternalError(msg) => write!(f, "Internal hypervisor error: {}", msg),

            Self::IoError(err) => write!(f, "I/O error: {}", err),
            Self::DeviceNotFound(msg) => write!(f, "Device not found: {}", msg),

            Self::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
            Self::UnsupportedFeature(msg) => write!(f, "Unsupported feature: {}", msg),
            
            Self::IrqInjection(msg) => write!(f, "IRQ injection failed: {}", msg),
            Self::LockPoisoned(msg) => write!(f, "Lock poisoned: {}", msg),
        }
    }
}

// ============================================================================
// ERROR TRAIT IMPLEMENTATION
// ============================================================================

impl std::error::Error for AxvmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(err) => Some(err),
            _ => None,
        }
    }
}





impl From<io::Error> for AxvmError {
    fn from(err: io::Error) -> Self {
        Self::IoError(err)
    }
}






pub type AxvmResult<T> = Result<T, AxvmError>;






#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
    Fatal,
}

impl AxvmError {
    
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            
            Self::KvmInit(_)
            | Self::KvmVersion(_)
            | Self::MissingCapability(_)
            | Self::HardwareFailure(_) => ErrorSeverity::Fatal,

            
            Self::VmCreation(_)
            | Self::VcpuCreation(_)
            | Self::MemoryAllocation(_)
            | Self::MemorySetup(_)
            | Self::LongModeSetup(_) => ErrorSeverity::Critical,

            
            Self::InvalidState(_)
            | Self::Timeout(_)
            | Self::MaxIterations(_)
            | Self::VcpuRuntime(_) => ErrorSeverity::Error,

            
            Self::InvalidConfiguration(_) | Self::UnsupportedFeature(_) => ErrorSeverity::Warning,

            
            Self::DeviceNotFound(_) => ErrorSeverity::Info,

            Self::IrqInjection(_) => ErrorSeverity::Warning,
            Self::LockPoisoned(_) => ErrorSeverity::Critical,

            _ => ErrorSeverity::Error,
        }
    }

    
    pub fn is_recoverable(&self) -> bool {
        self.severity() < ErrorSeverity::Critical
    }

    
    pub fn requires_shutdown(&self) -> bool {
        self.severity() >= ErrorSeverity::Critical
    }
}





#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AxvmError::KvmInit("test error".to_string());
        assert_eq!(err.to_string(), "KVM initialization failed: test error");
    }

    #[test]
    fn test_error_severity() {
        assert_eq!(AxvmError::KvmInit(String::new()).severity(), ErrorSeverity::Fatal);
        assert_eq!(AxvmError::Timeout(String::new()).severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_error_recoverability() {
        assert!(!AxvmError::KvmInit(String::new()).is_recoverable());
        assert!(AxvmError::Timeout(String::new()).is_recoverable());
    }

    #[test]
    fn test_error_shutdown_requirement() {
        assert!(AxvmError::VmCreation(String::new()).requires_shutdown());
        assert!(!AxvmError::Timeout(String::new()).requires_shutdown());
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let axvm_err: AxvmError = io_err.into();
        assert!(matches!(axvm_err, AxvmError::IoError(_)));
    }
}