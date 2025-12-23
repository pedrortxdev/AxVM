# AxVM - Building RootFS

The `rootfs.ext4` file is not included in the repository due to its size (512 MB).

## Quick Build

```bash
cd /home/daniel/AxVM
sudo ./build_rootfs.sh
```

This will create a minimal Alpine Linux rootfs with:
- Size: 512 MB
- Filesystem: ext4
- Init: OpenRC
- Network: eth0 configured via `/etc/network/interfaces`

## Manual Build

See [build_rootfs.sh](build_rootfs.sh) for the complete build process.

## Network Configuration

After building, configure the network:

```bash
sudo ./configure_network.sh
```

This sets up:
- IP: 192.168.100.2/24
- Gateway: 192.168.100.1
