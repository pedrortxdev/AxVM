# VirtIO-Net Testing Guide

## Quick Test: Ping Between Host and Guest

### 1. Configure Host TAP Interface

```bash
# Add IP to TAP interface
sudo ip addr add 192.168.100.1/24 dev axvm-tap0
sudo ip link set axvm-tap0 up

# Verify
ip addr show axvm-tap0
```

### 2. Boot AxVM

```bash
cd /home/daniel/AxVM/axvm_core
sudo ./target/release/axvm_core --kernel ../bzImage-microvm --disk ../rootfs.ext4 -vv
```

### 3. Configure Guest Network (Inside AxVM)

```bash
# At the login prompt, login as root (no password)
ip addr add 192.168.100.2/24 dev eth0
ip link set eth0 up

# Verify
ip addr show eth0
ip link show eth0
```

### 4. Test Connectivity

```bash
# From guest, ping host
ping -c 4 192.168.100.1

# Expected output:
# 64 bytes from 192.168.100.1: icmp_seq=1 ttl=64 time=X ms
```

### 5. Test from Host (in another terminal)

```bash
# Ping guest
ping -c 4 192.168.100.2
```

---

## Expected AxVM Logs

With `-vv` you should see:
```
>>> [Net] Queue 0 Configured: size=256, desc=0x3358000, avail=0x3359000, used=0x335a000
>>> [Net] Queue 1 Configured: size=256, desc=0x335c000, avail=0x335d000, used=0x335e000
```

When packets are received:
```
2025-12-23T... DEBUG RX packet processed bytes=XX
```

---

## Troubleshooting

### No ping response

1. **Check TAP is up:**
   ```bash
   ip link show axvm-tap0
   # Should show: state UP
   ```

2. **Check guest interface:**
   ```bash
   # Inside guest
   ip link show eth0
   # Should show: state UP
   ```

3. **Check routing:**
   ```bash
   # Inside guest
   ip route
   # Should show: 192.168.100.0/24 dev eth0
   ```

4. **Enable packet capture:**
   ```bash
   # On host, in another terminal
   sudo tcpdump -i axvm-tap0 -n
   ```

### For internet access (optional)

Enable NAT on host:
```bash
# Enable IP forwarding
sudo sysctl -w net.ipv4.ip_forward=1

# Add NAT rule
sudo iptables -t nat -A POSTROUTING -s 192.168.100.0/24 -j MASQUERADE

# In guest, add default route
ip route add default via 192.168.100.1
```

Then test:
```bash
ping -c 4 8.8.8.8
```

---

## Success Criteria

✅ TAP interface created: `axvm-tap0`
✅ Guest sees `eth0` with MAC `52:54:00:12:34:56`
✅ Ping from guest to host works
✅ Ping from host to guest works
✅ AxVM logs show "RX packet processed"

🎉 **If all checks pass, you have a fully functional hypervisor with networking!**
