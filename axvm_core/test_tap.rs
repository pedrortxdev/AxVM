// Test program for TAP interface
use std::io;

mod tap;

fn main() -> io::Result<()> {
    println!("=== AxVM TAP Interface Test ===\n");
    
    println!("[1] Attempting to create TAP interface 'axvm-tap0'...");
    match tap::TapInterface::new(Some("axvm-tap0")) {
        Ok(tap_if) => {
            println!("✅ SUCCESS! TAP interface created: {}", tap_if.name());
            println!("\nVerify with:");
            println!("  ip link show {}", tap_if.name());
            println!("  ip addr show {}", tap_if.name());
            
            // Keep the interface alive for a few seconds
            println!("\nInterface will remain active for 5 seconds...");
            std::thread::sleep(std::time::Duration::from_secs(5));
            
            println!("\nTest complete. Interface will be destroyed on exit.");
        },
        Err(e) => {
            eprintln!("❌ FAILED: {}", e);
            eprintln!("\nPossible causes:");
            eprintln!("  - Not running as root (try: sudo ./test_tap)");
            eprintln!("  - /dev/net/tun doesn't exist");
            eprintln!("  - Missing CAP_NET_ADMIN capability");
            return Err(e);
        }
    }
    
    Ok(())
}
