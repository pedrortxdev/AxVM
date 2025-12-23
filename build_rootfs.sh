#!/bin/bash
set -e

# Configuração
ALPINE_URL="https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/x86_64/alpine-minirootfs-3.19.1-x86_64.tar.gz"
DISK_NAME="rootfs.ext4"
DISK_SIZE="512M"
MOUNT_DIR="/mnt/axvm_root"

echo "=== AxVM RootFS Builder ==="
echo "=== 1. Criando imagem de disco vazia ($DISK_SIZE) ==="
dd if=/dev/zero of=$DISK_NAME bs=1M count=512 status=progress

echo ""
echo "=== 2. Formatando como EXT4 ==="
mkfs.ext4 $DISK_NAME

echo ""
echo "=== 3. Montando disco (Requer SUDO) ==="
sudo mkdir -p $MOUNT_DIR
sudo mount -o loop $DISK_NAME $MOUNT_DIR

echo ""
echo "=== 4. Baixando e extraindo Alpine Linux ==="
wget -qO- $ALPINE_URL | sudo tar -xz -C $MOUNT_DIR

echo ""
echo "=== 5. Configurando Serial (O PULO DO GATO) ==="
# Isso é CRUCIAL. Sem isso, o init sobe mas não joga o login na serial ttyS0
# O AxVM usa UART 8250, que o kernel vê como ttyS0.
echo "ttyS0::respawn:/sbin/getty -L ttyS0 115200 vt100" | sudo tee -a $MOUNT_DIR/etc/inittab

echo ""
echo "=== 6. Ajustes Finais (DNS, Hostname) ==="
echo "nameserver 8.8.8.8" | sudo tee $MOUNT_DIR/etc/resolv.conf
echo "axvm-guest" | sudo tee $MOUNT_DIR/etc/hostname

echo ""
echo "=== 7. Desmontando ==="
sudo umount $MOUNT_DIR
sudo rmdir $MOUNT_DIR

echo ""
echo "✅ Sucesso! Imagem '$DISK_NAME' criada."
echo ""
echo "Para testar, execute:"
echo "./target/release/axvm_core \\"
echo "  --memory 1024 \\"
echo "  --vcpus 2 \\"
echo "  --kernel ./bzImage \\"
echo "  --disk ./rootfs.ext4 \\"
echo "  --cmdline \"console=ttyS0 root=/dev/vda rw earlyprintk=serial\" \\"
echo "  -vv"
