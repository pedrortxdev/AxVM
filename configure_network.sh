#!/bin/bash
# Script to configure network inside rootfs.ext4

set -e

DISK_IMAGE="./rootfs.ext4"
MOUNT_POINT="/tmp/axvm_rootfs_mount"

echo "=== AxVM RootFS Network Configuration ==="
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
    echo "Error: Please run as root (sudo)"
    exit 1
fi

# Check if disk exists
if [ ! -f "$DISK_IMAGE" ]; then
    echo "Error: Disk image not found: $DISK_IMAGE"
    exit 1
fi

# Create mount point
mkdir -p "$MOUNT_POINT"

# Mount the disk
echo "[1] Mounting $DISK_IMAGE..."
mount -o loop "$DISK_IMAGE" "$MOUNT_POINT"

# Configure network
echo "[2] Configuring network (192.168.100.2/24)..."

# Create network interfaces file
cat > "$MOUNT_POINT/etc/network/interfaces" << 'EOF'
auto lo
iface lo inet loopback

auto eth0
iface eth0 inet static
    address 192.168.100.2
    netmask 255.255.255.0
    gateway 192.168.100.1
EOF

echo "[3] Network configuration written to /etc/network/interfaces"
cat "$MOUNT_POINT/etc/network/interfaces"

# Unmount
echo ""
echo "[4] Unmounting..."
umount "$MOUNT_POINT"
rmdir "$MOUNT_POINT"

echo ""
echo "✅ Done! Network configured in rootfs.ext4"
echo ""
echo "Next steps:"
echo "  1. Boot AxVM: sudo ./target/release/axvm_core --kernel ../bzImage-microvm --disk ../rootfs.ext4 -vv"
echo "  2. Inside guest, run: /etc/init.d/networking restart"
echo "  3. Test: ping 192.168.100.1"
