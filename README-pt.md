# AxVM — Hypervisor Paralelo para Ivy Bridge

O AxVM é um **monitor de máquinas virtuais especializado por microarquitetura**, criado para extrair o máximo desempenho de CPUs específicas em vez de tentar ser compatível com tudo.

Este repositório contém o perfil **AxVM-Xv2**, otimizado especificamente para **Intel Xeon E5 v2 (Ivy Bridge-EP)**, com foco em **alto paralelismo, escalabilidade previsível e baixo overhead**.

> O AxVM não virtualiza qualquer coisa.  
> Ele domina o hardware para o qual foi projetado.

---

## Status Atual

**✅ Hypervisor Funcional com Boot Linux e VirtIO Storage**

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

O hypervisor atualmente:
- ✅ Inicializa o KVM com verificação de capacidades
- ✅ Aloca memória guest com Huge Pages (THP) para performance
- ✅ Configura page tables de 4 níveis (PML4 → PDPT → PD)
- ✅ Configura GDT com segmentos de código/dados 64-bit
- ✅ Inicializa x86-64 long mode (CR0, CR4, EFER)
- ✅ Gera tabelas ACPI (RSDP, RSDT, MADT) para SMP
- ✅ Executa kernel Linux 6.8.0 com boot protocol 2.15
- ✅ Emula UART 8250 serial console completo
- ✅ Implementa VirtIO-MMIO Block Device (100MB vda)
- ✅ Detecta e inicializa drivers VirtIO no Linux
- ✅ Shutdown gracioso com signal handling

---

## Estrutura do Projeto

```
axvm_core/
├── Cargo.toml          # Dependências Rust (kvm-ioctls, kvm-bindings, libc, ctrlc)
└── src/
    ├── main.rs         # Ciclo de vida da VM, tratamento de exits, loop principal
    ├── memory.rs       # Gerenciamento de memória guest (mmap, huge pages, proteção)
    ├── vcpu.rs         # Setup do vCPU (long mode, page tables, GDT, registradores)
    ├── loader.rs       # Boot do Linux (bzImage, Zero Page, E820, cmdline)
    ├── linux.rs        # Estruturas do protocolo boot Linux
    ├── acpi.rs         # Gerador de tabelas ACPI (RSDP, RSDT, MADT para SMP)
    ├── serial.rs       # Emulação UART 8250 serial console
    ├── virtio.rs       # VirtIO-MMIO Block Device (control + data plane)
    ├── error.rs        # Tipos de erro com níveis de severidade
    └── metrics.rs      # Coleta de métricas de desempenho
```

### Componentes Principais

| Módulo | Descrição |
|--------|-----------|
| `main.rs` | Struct principal com máquina de estados, métricas e shutdown gracioso |
| `GuestMemory` | Wrapper seguro para mmap com bounds checking, huge pages, mlock |
| `setup_long_mode` | Bootstrap do x86-64 long mode (CR0.PG, CR4.PAE, EFER.LME/LMA) |
| `load_linux` | Carrega bzImage, configura Zero Page, E820 memory map, cmdline |
| `setup_acpi` | Gera RSDP, RSDT, MADT para detecção de CPUs SMP pelo kernel |
| `VirtioBlock` | Device VirtIO-MMIO para storage com queue processing |
| `SerialConsole` | Emulador UART 8250 para output do console Linux |
| `AxvmError` | Tipos de erro abrangentes com severidade e hints de recuperação |

---

## Build e Execução

```bash
# Build
cd axvm_core
cargo build --release

# Criar disco virtual (100MB)
dd if=/dev/zero of=disk.img bs=1M count=100
mkfs.ext4 disk.img  # Opcional: formatar como EXT4

# Copiar kernel Linux
cp /boot/vmlinuz-$(uname -r) bzImage

# Executar (requer acesso a /dev/kvm)
cargo run
```

### Requisitos

- Linux com suporte a KVM (`/dev/kvm`)
- Rust 1.70+ (edição 2021)
- Intel VT-x ou AMD-V habilitado na BIOS
- Kernel Linux bzImage para boot

---

## Por que Ivy Bridge Xeon v2?

O Xeon E5-2680 v2 representa uma classe de CPUs ainda muito presente em produção:
- Muitos núcleos e threads (20 cores / 40 threads por máquina)
- Clock base baixo (2.8 GHz base / 3.6 GHz turbo)
- IPC moderado mas estável
- Boa largura de banda de memória
- Topologia NUMA estável

Hypervisores genéricos tendem a subutilizar esse tipo de CPU por assumirem características de processadores modernos de alto clock.

O AxVM-Xv2 faz o oposto: **abraça o paralelismo do Ivy Bridge**.

---

## Objetivos de Projeto

- **Maximizar throughput por socket**
- **Priorizar paralelismo em vez de latência**
- **Reduzir VM-exits e IPIs**
- **Escalonamento determinístico**
- **NUMA como conceito central**
- **Sem emulação de dispositivos legados**
- **VirtIO como padrão para I/O**

O objetivo é fazer um sistema com Xeon E5-2680 v2 se comportar, no conjunto, como CPUs de clock muito mais alto.

---

## Visão Geral da Arquitetura

- Virtualização assistida por hardware (Intel VT-x + EPT)
- Nenhuma emulação de CPU em software
- Uma thread de host por vCPU
- Pinagem estática de CPU
- I/O tratado via VirtIO-MMIO
- Boot direto do kernel Linux (sem BIOS legado)
- Huge Pages (THP) para memória guest

```
Axion Control Plane
        |
        v
    AxVM-Xv2
        |
        v
    /dev/kvm ── VirtIO Block ── disk.img
```

O AxVM é apenas o **motor de execução**.  
Agendamento e orquestração são responsabilidade do Axion.

---

## Recursos Implementados

### CPU & Memória
- [x] Integração com KVM e detecção de capacidades
- [x] Alocação de memória guest com mmap + Huge Pages
- [x] Bootstrap do x86-64 long mode
- [x] Page tables de 4 níveis (1GB pages)
- [x] GDT 64-bit com segmentos corretos

### Boot Linux
- [x] Carregamento de bzImage (protocolo 2.15)
- [x] Zero Page configuration
- [x] E820 memory map (com BIOS hole)
- [x] Kernel command line

### ACPI & SMP
- [x] RSDP na região BIOS (0xE0000)
- [x] RSDT com ponteiro para MADT
- [x] MADT com Local APIC entries
- [x] Suporte a múltiplos vCPUs (até 20)

### Devices
- [x] UART 8250 serial console
- [x] PIT Timer (via KVM)
- [x] IRQ Chip (via KVM)
- [x] VirtIO-MMIO Block Device
  - Device detection e feature negotiation
  - Queue setup (descriptors, available, used rings)
  - Data plane (read/write to disk.img)

### Runtime
- [x] Tratamento de VM exits (IO, MMIO, HLT, Shutdown)
- [x] Shutdown gracioso (Ctrl+C signal handling)
- [x] Coleta de métricas de desempenho

---

## Roadmap

- [ ] IRQ injection para VirtIO (completar I/O assíncrono)
- [ ] IO-APIC emulation
- [ ] SMP Application Processor startup (SIPI handling)
- [ ] VirtIO-Net networking
- [ ] Configuração de EPT explícita
- [ ] Alocação de memória NUMA-aware
- [ ] Integração completa com control plane do Axion

---

## O que NÃO é objetivo

- Suporte a múltiplas gerações de CPU
- Emulação de dispositivos legados (IDE, VGA, USB)
- Migração ao vivo (neste perfil)
- Fallback de emulação
- Substituir o QEMU genericamente

---

## Relação com o Axion

O AxVM foi projetado para ser iniciado e gerenciado exclusivamente pelo **Axion**, que:
- Detecta o hardware do host
- Seleciona o perfil correto do AxVM
- Gerencia o ciclo de vida das VMs

O AxVM expõe uma interface de controle estável, enquanto sua implementação interna varia por perfil.

---

## Filosofia

> Diversidade de hardware não é um problema a ser escondido.  
> É uma realidade a ser explorada conscientemente.

O AxVM existe para fazer hardware antigo e moderno serem igualmente **respeitados**, não igualmente **genéricos**.

---

## Licença

**Uso Restrito** — Este software é propriedade exclusiva de Daniel Rodrigues e Axion.

Não é permitido usar, copiar, modificar ou distribuir este software sem autorização prévia por escrito.

© 2024-2025 Daniel Rodrigues / Axion. Todos os direitos reservados.
