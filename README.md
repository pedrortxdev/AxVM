# AxVM - Educational SMP Hypervisor in Rust

A minimalist KVM Hypervisor written in Rust, focused on high performance for Intel Ivy Bridge architectures (Xeon E5 v2).

## 🚀 Features

### 🧠 Compute (vCPU)
* **SMP (Symmetric Multi-Processing):** Support for up to 20 simultaneous vCPUs.
* **Operating Mode:** Boot in 32-bit Protected Mode and transition to 64-bit Long Mode.
* **Memory Optimization:** Allocation via `mmap` with **Transparent Huge Pages (2MB)** to reduce TLB Misses.

### 💾 Storage (VirtIO)
* **VirtIO-MMIO:** Full implementation of the MMIO transport.
* **VirtIO-Block:** Functional disk driver (`/dev/vda`).
* **Data Plane:** Manual processing of Virtqueues (Avail/Used rings).
* **Interrupts:** Asynchronous IRQ injection via KVM ioctl (IRQ 5).

### ⚙️ System Internals
* **ACPI:** Dynamic generation of RSDP, RSDT, and MADT tables for multicore topology.
* **Timer:** PIT (i8254) emulation and TSC Deadline Timer support.
* **Loader:** Linux Kernel bzImage loader compatible with Boot Protocol 2.15.
* **Serial:** UART 8250 emulation for kernel console.

## 🛠️ How to Run

### 1. Prepare the Disk
```bash
# Create 100MB disk
dd if=/dev/zero of=disk.img bs=1M count=100
# Format (optional, but recommended for mount testing)
mkfs.ext4 disk.img
```

### 2. Get a Kernel
Copy a compatible kernel (bzImage) to the root folder:
```bash
cp /boot/vmlinuz-$(uname -r) bzImage
```

### 3. Run
```bash
cd axvm_core
cargo run --release
```

## 📋 Current Status

The Hypervisor boots Linux Kernel 6.8, detects all cores, initializes the VirtIO driver, mounts the filesystem via `/dev/vda`, and hands over control to userspace (attempting to execute `/sbin/init`).

```
[    2.074862] EXT4-fs (vda): mounted filesystem ...
[    2.280010] Kernel panic - not syncing: No working init found.
```

---

### 🧠 The Architecture

1.  **Hardware:** Xeon E5-2680 v2 (Ivy Bridge).
2.  **Host OS:** Linux (via WSL2 or Native).
3.  **Virtualization Layer:** KVM (Kernel-based Virtual Machine).
4.  **Userspace (Rust):** **AxVM** (Manages memory, vCPUs, I/O, VirtIO).
5.  **Guest OS:** Linux 6.8 (Believes it is on real hardware).

---

## License

**Restricted Use** — This software is the exclusive property of Daniel Rodrigues and Axion.
Use, copying, modification, or distribution is not permitted without authorization.

© 2024-2025 Daniel Rodrigues / Axion. All rights reserved.
