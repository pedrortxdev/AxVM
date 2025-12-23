#!/bin/bash
# Quick test to verify TAP interface can be created

echo "=== Testing TAP Interface Creation ==="
echo ""
echo "Creating TAP interface 'test-tap'..."

# Try to create a TAP interface using ip command
sudo ip tuntap add mode tap name test-tap 2>&1

if [ $? -eq 0 ]; then
    echo "✅ SUCCESS! TAP interface created"
    echo ""
    echo "Interface details:"
    ip link show test-tap
    echo ""
    echo "Cleaning up..."
    sudo ip tuntap del mode tap name test-tap
    echo "✅ Test complete"
else
    echo "❌ FAILED - TAP interface creation failed"
    echo "This might be because:"
    echo "  - Not running as root"
    echo "  - TUN/TAP kernel module not loaded"
    echo ""
    echo "Try: sudo modprobe tun"
fi
