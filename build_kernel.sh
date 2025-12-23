#!/bin/bash
set -e

echo "=== AxVM Kernel Builder ==="
echo "Este script compila um kernel Linux mínimo com VirtIO MMIO built-in"
echo ""

KERNEL_VERSION="6.1.112"
KERNEL_DIR="linux-${KERNEL_VERSION}"

# Verificar se já baixou
if [ ! -f "linux-${KERNEL_VERSION}.tar.xz" ]; then
    echo "[1/5] Baixando kernel ${KERNEL_VERSION}..."
    wget https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-${KERNEL_VERSION}.tar.xz
else
    echo "[1/5] Kernel já baixado, pulando..."
fi

# Extrair
if [ ! -d "$KERNEL_DIR" ]; then
    echo "[2/5] Extraindo kernel..."
    tar xf linux-${KERNEL_VERSION}.tar.xz
else
    echo "[2/5] Kernel já extraído, pulando..."
fi

cd $KERNEL_DIR

echo "[3/5] Configurando kernel (MicroVM config)..."
make defconfig

# Habilitar VirtIO MMIO e Block (built-in, não módulo!)
echo "CONFIG_VIRTIO=y" >> .config
echo "CONFIG_VIRTIO_MMIO=y" >> .config
echo "CONFIG_VIRTIO_MMIO_CMDLINE_DEVICES=y" >> .config
echo "CONFIG_VIRTIO_BLK=y" >> .config
echo "CONFIG_VIRTIO_CONSOLE=y" >> .config
echo "CONFIG_HW_RANDOM_VIRTIO=y" >> .config
echo "CONFIG_SERIAL_8250=y" >> .config
echo "CONFIG_SERIAL_8250_CONSOLE=y" >> .config
echo "CONFIG_EXT4_FS=y" >> .config
echo "CONFIG_TMPFS=y" >> .config
echo "CONFIG_DEVTMPFS=y" >> .config
echo "CONFIG_DEVTMPFS_MOUNT=y" >> .config

# Desabilitar coisas desnecessárias para reduzir tamanho
echo "CONFIG_MODULES=n" >> .config
echo "CONFIG_SOUND=n" >> .config
echo "CONFIG_USB=n" >> .config
echo "CONFIG_WIRELESS=n" >> .config
echo "CONFIG_WLAN=n" >> .config
echo "CONFIG_BT=n" >> .config

# Reprocessar config
make olddefconfig

echo "[4/5] Compilando kernel (isso pode demorar ~10 minutos)..."
make -j$(nproc) bzImage

echo "[5/5] Copiando kernel compilado..."
cp arch/x86/boot/bzImage ../bzImage-microvm

cd ..

echo ""
echo "✅ Sucesso! Kernel criado: bzImage-microvm"
echo ""
echo "Para testar:"
echo "cd axvm_core"
echo "sudo ./target/release/axvm_core \\"
echo "  --kernel ../bzImage-microvm \\"
echo "  --disk ../rootfs.ext4 \\"
echo "  -vv"
