# AxVM — Ivy Bridge Parallel Hypervisor

AxVM is a **microarchitecture-specialized virtual machine monitor** designed to extract maximum efficiency from specific CPU families instead of chasing generic compatibility.

This repository contains the **AxVM-Xv2 profile**, optimized specifically for **Intel Xeon E5 v2 (Ivy Bridge-EP)** processors, with a focus on **high parallelism, predictable scheduling, and low overhead**.

> AxVM does not virtualize everything.  
> It dominates the hardware it was designed for.

---

## Current Status

**✅ Functional x86-64 Long Mode Hypervisor**

```
╔════════════════════════════════════════════════════════════════╗
║                  AxVM Hypervisor v0.3                          ║
║              Production-Grade KVM Virtualization               ║
╚════════════════════════════════════════════════════════════════╝

>>> [✓] Validation PASSED: RAX=0xcafebabedeadbeef
>>> [✓] ✓ 64-bit Long Mode confirmed
```

The hypervisor successfully:
- Initializes KVM with capability verification
- Allocates and maps guest memory via mmap
- Sets up 4-level page tables (PML4 → PDPT → PD)
- Configures GDT with 64-bit code/data segments
- Bootstraps x86-64 long mode (CR0, CR4, EFER)
- Executes guest code and handles VM exits
- Validates 64-bit register operations

---

## Project Structure

```
axvm_core/
├── Cargo.toml          # Rust dependencies (kvm-ioctls, kvm-bindings, libc, ctrlc)
└── src/
    ├── main.rs         # VM lifecycle, exit handling, main loop
    ├── memory.rs       # Guest memory management (mmap, huge pages, protection)
    ├── vcpu.rs         # vCPU setup (long mode, page tables, GDT, registers)
    ├── error.rs        # Error types with severity levels
    └── metrics.rs      # Performance metrics collection
```

### Core Components

| Module | Description |
|--------|-------------|
| `VirtualMachine` | Main VM struct with state machine, metrics, and graceful shutdown |
| `GuestMemory` | Safe mmap wrapper with bounds checking, huge pages, mlock support |
| `setup_long_mode` | x86-64 long mode bootstrap (CR0.PG, CR4.PAE, EFER.LME/LMA) |
| `AxvmError` | Comprehensive error types with severity and recoverability hints |
| `VmMetrics` | Atomic counters for vCPU runs, IO exits, errors |

---

## Building & Running

```bash
# Build
cd axvm_core
cargo build --release

# Run (requires /dev/kvm access)
cargo run

# Run with debug output
AXVM_DEBUG=1 cargo run
```

### Requirements

- Linux with KVM support (`/dev/kvm`)
- Rust 1.70+ (2021 edition)
- Intel VT-x or AMD-V enabled in BIOS

---

## Why Ivy Bridge Xeon v2?

The Xeon E5-2680 v2 represents a class of CPUs that are still widely deployed:
- Many cores / threads
- Lower base clocks
- Moderate IPC
- Strong memory bandwidth
- Stable NUMA topology

Generic hypervisors often underutilize this class of hardware due to assumptions optimized for high-clock consumer CPUs.

AxVM-Xv2 embraces Ivy Bridge's strengths instead of fighting its limitations.

---

## Design Goals

- **Maximize throughput per socket**
- **Favor parallelism over single-core latency**
- **Minimize VM-exits and IPIs**
- **Deterministic scheduling**
- **NUMA-aware by default**
- **No legacy device emulation**

The objective is to make a system like a Xeon E5-2680 v2 behave, in aggregate, as efficiently as a much smaller set of high-clock cores.

---

## Architecture Overview

- Hardware-assisted virtualization only (Intel VT-x + EPT)
- No software CPU emulation
- One host thread per vCPU
- Static CPU pinning
- Batched I/O handling
- VirtIO-only devices
- Direct Linux kernel boot (no legacy BIOS)

```
Axion Control Plane
        |
        v
    AxVM-Xv2
        |
        v
    /dev/kvm
```

AxVM is a **runtime engine**, not a scheduler or orchestrator. Those responsibilities belong to Axion.

---

## CPU Requirements

This AxVM profile **will refuse to run** unless the host CPU meets all requirements.

Minimum requirements:
- Intel CPU
- Ivy Bridge-EP (Xeon E5 v2)
- VT-x
- EPT (Extended Page Tables)
- Invariant TSC
- x2APIC

Optional features (used when available):
- TSC scaling
- APIC virtualization
- Large pages (1G)

---

## What AxVM-Xv2 Does Differently

- Assumes **low per-core performance**
- Optimizes for **many runnable vCPUs**
- Reduces context switching overhead
- Prefers throughput over latency
- Uses aggressive batching strategies
- Treats NUMA boundaries as first-class constraints

This profile intentionally avoids optimizations meant for high-frequency CPUs.

---

## Non-Goals

- No cross-generation CPU support
- No legacy device models (IDE, VGA, USB, etc.)
- No live migration (for this profile)
- No emulation fallback
- No attempt to be a drop-in replacement for QEMU

---

## Relationship with Axion

AxVM is designed to be launched and managed exclusively by **Axion**, which:
- Detects host hardware
- Selects the appropriate AxVM profile
- Manages VM lifecycle and scheduling policies

AxVM exposes a stable control interface. Its internal implementation is profile-specific.

---

## Roadmap

- [x] KVM integration and capability detection
- [x] Guest memory allocation with mmap
- [x] x86-64 long mode bootstrap
- [x] Basic VM exit handling (IO, HLT, Shutdown)
- [x] Graceful shutdown (Ctrl+C signal handling)
- [x] Performance metrics collection
- [ ] Multi-vCPU support
- [ ] VirtIO device emulation
- [ ] EPT configuration
- [ ] NUMA-aware memory allocation
- [ ] Integration with Axion control plane

---

## Philosophy

> Hardware diversity is not a problem to abstract away.  
> It is a reality to be embraced deliberately.

AxVM exists to make old and modern hardware equally *respected*, not equally *generic*.

---

## License

MIT
