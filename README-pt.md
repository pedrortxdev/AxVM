# AxVM — Hypervisor Paralelo para Ivy Bridge

O AxVM é um **monitor de máquinas virtuais especializado por microarquitetura**, criado para extrair o máximo desempenho de CPUs específicas em vez de tentar ser compatível com tudo.

Este repositório contém o perfil **AxVM-Xv2**, otimizado especificamente para **Intel Xeon E5 v2 (Ivy Bridge-EP)**, com foco em **alto paralelismo, escalabilidade previsível e baixo overhead**.

> O AxVM não virtualiza qualquer coisa.  
> Ele domina o hardware para o qual foi projetado.

---

## Status Atual

**✅ Hypervisor x86-64 Long Mode Funcional**

```
╔════════════════════════════════════════════════════════════════╗
║                  AxVM Hypervisor v0.3                          ║
║              Production-Grade KVM Virtualization               ║
╚════════════════════════════════════════════════════════════════╝

>>> [✓] Validation PASSED: RAX=0xcafebabedeadbeef
>>> [✓] ✓ 64-bit Long Mode confirmed
```

O hypervisor atualmente:
- Inicializa o KVM com verificação de capacidades
- Aloca e mapeia memória guest via mmap
- Configura page tables de 4 níveis (PML4 → PDPT → PD)
- Configura GDT com segmentos de código/dados 64-bit
- Inicializa x86-64 long mode (CR0, CR4, EFER)
- Executa código guest e trata VM exits
- Valida operações de registradores 64-bit

---

## Estrutura do Projeto

```
axvm_core/
├── Cargo.toml          # Dependências Rust (kvm-ioctls, kvm-bindings, libc, ctrlc)
└── src/
    ├── main.rs         # Ciclo de vida da VM, tratamento de exits, loop principal
    ├── memory.rs       # Gerenciamento de memória guest (mmap, huge pages, proteção)
    ├── vcpu.rs         # Setup do vCPU (long mode, page tables, GDT, registradores)
    ├── error.rs        # Tipos de erro com níveis de severidade
    └── metrics.rs      # Coleta de métricas de desempenho
```

### Componentes Principais

| Módulo | Descrição |
|--------|-----------|
| `VirtualMachine` | Struct principal com máquina de estados, métricas e shutdown gracioso |
| `GuestMemory` | Wrapper seguro para mmap com bounds checking, huge pages, mlock |
| `setup_long_mode` | Bootstrap do x86-64 long mode (CR0.PG, CR4.PAE, EFER.LME/LMA) |
| `AxvmError` | Tipos de erro abrangentes com severidade e hints de recuperação |
| `VmMetrics` | Contadores atômicos para runs do vCPU, IO exits, erros |

---

## Build e Execução

```bash
# Build
cd axvm_core
cargo build --release

# Executar (requer acesso a /dev/kvm)
cargo run

# Executar com debug output
AXVM_DEBUG=1 cargo run
```

### Requisitos

- Linux com suporte a KVM (`/dev/kvm`)
- Rust 1.70+ (edição 2021)
- Intel VT-x ou AMD-V habilitado na BIOS

---

## Por que Ivy Bridge Xeon v2?

O Xeon E5-2680 v2 representa uma classe de CPUs ainda muito presente em produção:
- Muitos núcleos e threads
- Clock base baixo
- IPC moderado
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

O objetivo é fazer um sistema com Xeon E5-2680 v2 se comportar, no conjunto, como CPUs de clock muito mais alto.

---

## Visão Geral da Arquitetura

- Virtualização assistida por hardware (Intel VT-x + EPT)
- Nenhuma emulação de CPU em software
- Uma thread de host por vCPU
- Pinagem estática de CPU
- I/O tratado em lote
- Apenas dispositivos VirtIO
- Boot direto do kernel Linux (sem BIOS legado)

```
Axion Control Plane
        |
        v
    AxVM-Xv2
        |
        v
    /dev/kvm
```

O AxVM é apenas o **motor de execução**.  
Agendamento e orquestração são responsabilidade do Axion.

---

## Requisitos de CPU

Este perfil do AxVM **se recusa a iniciar** caso o hardware não atenda aos requisitos.

Requisitos mínimos:
- CPU Intel
- Ivy Bridge-EP (Xeon E5 v2)
- VT-x
- EPT (Extended Page Tables)
- TSC invariante
- x2APIC

Recursos opcionais (usados quando disponíveis):
- Escalonamento de TSC
- Virtualização de APIC
- Huge pages (1G)

---

## O que o AxVM-Xv2 faz diferente

- Assume **baixo desempenho por núcleo**
- Otimiza para **grande número de vCPUs**
- Reduz overhead de troca de contexto
- Prioriza throughput sobre latência
- Usa estratégias agressivas de batching
- Trata limites NUMA como restrições reais

Este perfil evita propositalmente otimizações voltadas a CPUs de alto clock.

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

## Roadmap

- [x] Integração com KVM e detecção de capacidades
- [x] Alocação de memória guest com mmap
- [x] Bootstrap do x86-64 long mode
- [x] Tratamento básico de VM exits (IO, HLT, Shutdown)
- [x] Shutdown gracioso (Ctrl+C signal handling)
- [x] Coleta de métricas de desempenho
- [ ] Suporte a múltiplos vCPUs
- [ ] Emulação de dispositivos VirtIO
- [ ] Configuração de EPT
- [ ] Alocação de memória NUMA-aware
- [ ] Integração com control plane do Axion

---

## Filosofia

> Diversidade de hardware não é um problema a ser escondido.  
> É uma realidade a ser explorada conscientemente.

O AxVM existe para fazer hardware antigo e moderno serem igualmente **respeitados**, não igualmente **genéricos**.

---

## Licença

MIT
