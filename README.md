# AxVM — Ivy Bridge Parallel Hypervisor

AxVM is a **microarchitecture-specialized virtual machine monitor** designed to extract maximum efficiency from specific CPU families instead of chasing generic compatibility.

This repository contains the **AxVM-Xv2 profile**, optimized specifically for **Intel Xeon E5 v2 (Ivy Bridge-EP)** processors, with a focus on **high parallelism, predictable scheduling, and low overhead**.

> AxVM does not virtualize everything.  
> It dominates the hardware it was designed for.

---

## Current Status

**✅ Functional Hypervisor with Linux Boot and VirtIO Storage**

```
╔════════════════════════════════════════════════════════════════╗
║              AxVM Hypervisor v0.8                              ║
║          Storage Edition - VirtIO Block 💾                     ║
╚════════════════════════════════════════════════════════════════╝

>>> [VirtIO] disk.img opened successfully
>>> [Run] Spawning vCPU threads...

[    0.001073] virtio_blk virtio0: [vda] 204800 512-byte logical blocks (105 MB/100 MiB)
[    0.001073] smpboot: Total of 1 processors activated (5598.97 BogoMIPS)
```

The hypervisor successfully:
- ✅ Initializes KVM with capability verification
- ✅ Allocates guest memory with Huge Pages (THP) for performance
- ✅ Sets up 4-level page tables (PML4 → PDPT → PD)
- ✅ Configures GDT with 64-bit code/data segments
- ✅ Bootstraps x86-64 long mode (CR0, CR4, EFER)
- ✅ Generates ACPI tables (RSDP, RSDT, MADT) for SMP
- ✅ Executes Linux kernel 6.8.0 with boot protocol 2.15
- ✅ Emulates UART 8250 serial console
- ✅ Implements VirtIO-MMIO Block Device (100MB vda)
- ✅ Detects and initializes VirtIO drivers in Linux
- ✅ Graceful shutdown with signal handling

---

## Project Structure

```
axvm_core/
├── Cargo.toml          # Rust dependencies (kvm-ioctls, kvm-bindings, libc, ctrlc)
└── src/
    ├── main.rs         # VM lifecycle, exit handling, main loop
    ├── memory.rs       # Guest memory management (mmap, huge pages, protection)
    ├── vcpu.rs         # vCPU setup (long mode, page tables, GDT, registers)
    ├── loader.rs       # Linux boot (bzImage, Zero Page, E820, cmdline)
    ├── linux.rs        # Linux boot protocol structures
    ├── acpi.rs         # ACPI table generator (RSDP, RSDT, MADT for SMP)
    ├── serial.rs       # UART 8250 serial console emulation
    ├── virtio.rs       # VirtIO-MMIO Block Device (control + data plane)
    ├── error.rs        # Error types with severity levels
    └── metrics.rs      # Performance metrics collection
```

### Core Components

| Module | Description |
|--------|-------------|
| `main.rs` | Main VM struct with state machine, metrics, and graceful shutdown |
| `GuestMemory` | Safe mmap wrapper with bounds checking, huge pages, mlock support |
| `setup_long_mode` | x86-64 long mode bootstrap (CR0.PG, CR4.PAE, EFER.LME/LMA) |
| `load_linux` | Loads bzImage, configures Zero Page, E820 memory map, cmdline |
| `setup_acpi` | Generates RSDP, RSDT, MADT for SMP CPU detection by kernel |
| `VirtioBlock` | VirtIO-MMIO device for storage with queue processing |
| `SerialConsole` | UART 8250 emulator for Linux console output |
| `AxvmError` | Comprehensive error types with severity and recoverability hints |

---

## Building & Running

```bash
# Build
cd axvm_core
cargo build --release

# Create virtual disk (100MB)
dd if=/dev/zero of=disk.img bs=1M count=100
mkfs.ext4 disk.img  # Optional: format as EXT4

# Copy Linux kernel
cp /boot/vmlinuz-$(uname -r) bzImage

# Run (requires /dev/kvm access)
cargo run
```

### Requirements

- Linux with KVM support (`/dev/kvm`)
- Rust 1.70+ (2021 edition)
- Intel VT-x or AMD-V enabled in BIOS
- Linux kernel bzImage for boot

---

## Why Ivy Bridge Xeon v2?

The Xeon E5-2680 v2 represents a class of CPUs that are still widely deployed:
- Many cores / threads (20 cores / 40 threads per machine)
- Lower base clocks (2.8 GHz base / 3.6 GHz turbo)
- Moderate but stable IPC
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
- **VirtIO as the standard for I/O**

The objective is to make a system like a Xeon E5-2680 v2 behave, in aggregate, as efficiently as a much smaller set of high-clock cores.

---

## Architecture Overview

- Hardware-assisted virtualization only (Intel VT-x + EPT)
- No software CPU emulation
- One host thread per vCPU
- Static CPU pinning
- I/O handled via VirtIO-MMIO
- Direct Linux kernel boot (no legacy BIOS)
- Huge Pages (THP) for guest memory

```
Axion Control Plane
        |
        v
    AxVM-Xv2
        |
        v
    /dev/kvm ── VirtIO Block ── disk.img
```

AxVM is a **runtime engine**, not a scheduler or orchestrator. Those responsibilities belong to Axion.

---

## Implemented Features

### CPU & Memory
- [x] KVM integration and capability detection
- [x] Guest memory allocation with mmap + Huge Pages
- [x] x86-64 long mode bootstrap
- [x] 4-level page tables (1GB pages)
- [x] 64-bit GDT with correct segments

### Linux Boot
- [x] bzImage loading (protocol 2.15)
- [x] Zero Page configuration
- [x] E820 memory map (with BIOS hole)
- [x] Kernel command line

### ACPI & SMP
- [x] RSDP in BIOS region (0xE0000)
- [x] RSDT with MADT pointer
- [x] MADT with Local APIC entries
- [x] Multi-vCPU support (up to 20)

### Devices
- [x] UART 8250 serial console
- [x] PIT Timer (via KVM)
- [x] IRQ Chip (via KVM)
- [x] VirtIO-MMIO Block Device
  - Device detection and feature negotiation
  - Queue setup (descriptors, available, used rings)
  - Data plane (read/write to disk.img)

### Runtime
- [x] VM exit handling (IO, MMIO, HLT, Shutdown)
- [x] Graceful shutdown (Ctrl+C signal handling)
- [x] Performance metrics collection

---

## Roadmap

- [ ] IRQ injection for VirtIO (complete async I/O)
- [ ] IO-APIC emulation
- [ ] SMP Application Processor startup (SIPI handling)
- [ ] VirtIO-Net networking
- [ ] Explicit EPT configuration
- [ ] NUMA-aware memory allocation
- [ ] Full integration with Axion control plane

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

## Philosophy

> Hardware diversity is not a problem to abstract away.  
> It is a reality to be embraced deliberately.

AxVM exists to make old and modern hardware equally *respected*, not equally *generic*.

---

## License

**Restricted Use** — This software is the exclusive property of Daniel Rodrigues and Axion.

Use, copying, modification, or distribution of this software is not permitted without prior written authorization.

© 2024-2025 Daniel Rodrigues / Axion. All rights reserved.
