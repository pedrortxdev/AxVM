# AxVM - Educational SMP Hypervisor in Rust

Um Hypervisor KVM minimalista, escrito em Rust, focado em alta performance para arquiteturas Intel Ivy Bridge (Xeon E5 v2).

## 🚀 Features

### 🧠 Compute (vCPU)
* **SMP (Symmetric Multi-Processing):** Suporte a até 20 vCPUs simultâneas.
* **Modo de Operação:** Inicialização em 32-bit Protected Mode e transição para 64-bit Long Mode.
* **Otimização de Memória:** Alocação via `mmap` com **Transparent Huge Pages (2MB)** para redução de TLB Misses.

### 💾 Storage (VirtIO)
* **VirtIO-MMIO:** Implementação completa do transporte MMIO.
* **VirtIO-Block:** Driver de disco funcional (`/dev/vda`).
* **Data Plane:** Processamento manual de Virtqueues (Avail/Used rings).
* **Interrupts:** Injeção de IRQ assíncrona via KVM ioctl (IRQ 5).

### ⚙️ System Internals
* **ACPI:** Geração dinâmica de tabelas RSDP, RSDT e MADT para topologia multicore.
* **Timer:** Emulação de PIT (i8254) e suporte a TSC Deadline Timer.
* **Loader:** Carregador de Kernel Linux bzImage compatível com Boot Protocol 2.15.
* **Serial:** Emulação UART 8250 para console kernel.

## 🛠️ Como Rodar

### 1. Prepare o Disco
```bash
# Criar disco de 100MB
dd if=/dev/zero of=disk.img bs=1M count=100
# Formatar (opcional, mas recomendado para teste de montagem)
mkfs.ext4 disk.img
```

### 2. Obtenha um Kernel
Copie um kernel compatível (bzImage) para a pasta raiz:
```bash
cp /boot/vmlinuz-$(uname -r) bzImage
```

### 3. Execute
```bash
cd axvm_core
cargo run --release
```

## 📋 Status Atual

O Hypervisor boota o Linux Kernel 6.8, detecta todos os cores, inicializa o driver VirtIO, monta o sistema de arquivos via `/dev/vda` e entrega o controle para o userspace (tentativa de execução do `/sbin/init`).

```
[    2.074862] EXT4-fs (vda): mounted filesystem ...
[    2.280010] Kernel panic - not syncing: No working init found.
```

---

### 🧠 A Arquitetura

1.  **Hardware:** Xeon E5-2680 v2 (Ivy Bridge).
2.  **Host OS:** Linux (via WSL2 ou Nativo).
3.  **Virtualization Layer:** KVM (Kernel-based Virtual Machine).
4.  **Userspace (Rust):** **AxVM** (Gerencia memória, vCPUs, I/O, VirtIO).
5.  **Guest OS:** Linux 6.8 (Acredita que está num hardware real).

---

## Licença

**Uso Restrito** — Este software é propriedade exclusiva de Daniel Rodrigues e Axion.
Não é permitido usar, copiar, modificar ou distribuir este software sem autorização.

© 2024-2025 Daniel Rodrigues / Axion. Todos os direitos reservados.
