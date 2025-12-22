// serial.rs
//!
//! Serial Console Emulator (UART 8250 subset).
//! Handles I/O traps for port 0x3F8 and outputs to host stdout.

use std::io::{self, Write};
use std::sync::Mutex;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Standard COM1 base address
pub const COM1_BASE: u16 = 0x3F8;

/// Register offsets from base address
pub const DATA_REGISTER: u16 = 0;        // THR (Write) / RBR (Read)
#[allow(dead_code)]
pub const LINE_STATUS_REGISTER: u16 = 5; // LSR

// ============================================================================
// SERIAL CONSOLE
// ============================================================================

/// Minimal UART 8250 emulator for guest serial output.
///
/// This provides a simple serial console that:
/// - Captures writes to COM1 data register
/// - Outputs characters to host stdout in real-time
/// - Reports transmitter ready status on LSR reads
///
/// Advanced UART features (baud rate, interrupts, FIFO) are not emulated.
pub struct SerialConsole {
    buffer: Mutex<Vec<u8>>,
}

impl SerialConsole {
    /// Creates a new serial console instance
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::new()),
        }
    }

    /// Handles a write operation to a serial port register
    ///
    /// # Arguments
    /// * `port` - The I/O port address (should be COM1_BASE + offset)
    /// * `data` - The data being written
    pub fn write(&self, port: u16, data: &[u8]) {
        let offset = port - COM1_BASE;

        match offset {
            DATA_REGISTER => {
                // Transmitter Holding Register (THR)
                // Guest is sending a character
                if let Ok(mut handle) = self.buffer.lock() {
                    // Write to host stdout
                    print!("{}", data[0] as char);
                    // Flush immediately for real-time feedback
                    let _ = io::stdout().flush();

                    // Store in buffer for persistent logging
                    handle.push(data[0]);
                }
            }
            _ => {
                // Ignore baud rate, interrupt, and other configuration writes.
                // AxVM assumes a "magic" perfect serial cable.
            }
        }
    }

    /// Handles a read operation from a serial port register
    ///
    /// # Arguments
    /// * `port` - The I/O port address (should be COM1_BASE + offset)
    ///
    /// # Returns
    /// The value to return to the guest
    #[allow(dead_code)]
    pub fn read(&self, port: u16) -> u8 {
        let offset = port - COM1_BASE;

        match offset {
            LINE_STATUS_REGISTER => {
                // LSR: Report transmitter empty and ready
                // Bit 5 = Transmitter Holding Register Empty (THRE)
                // Bit 6 = Transmitter Empty (TEMT)
                0x20 | 0x40
            }
            _ => 0,
        }
    }

    /// Returns the current buffer contents as a string
    #[allow(dead_code)]
    pub fn get_output(&self) -> String {
        if let Ok(handle) = self.buffer.lock() {
            String::from_utf8_lossy(&handle).to_string()
        } else {
            String::new()
        }
    }

    /// Clears the internal buffer
    #[allow(dead_code)]
    pub fn clear(&self) {
        if let Ok(mut handle) = self.buffer.lock() {
            handle.clear();
        }
    }
}

impl Default for SerialConsole {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_write() {
        let serial = SerialConsole::new();
        serial.write(COM1_BASE, &[b'H']);
        serial.write(COM1_BASE, &[b'i']);

        assert_eq!(serial.get_output(), "Hi");
    }

    #[test]
    fn test_lsr_ready() {
        let serial = SerialConsole::new();
        let lsr = serial.read(COM1_BASE + LINE_STATUS_REGISTER);

        // Both THRE and TEMT should be set
        assert_eq!(lsr & 0x60, 0x60);
    }

    #[test]
    fn test_clear_buffer() {
        let serial = SerialConsole::new();
        serial.write(COM1_BASE, &[b'X']);
        assert!(!serial.get_output().is_empty());

        serial.clear();
        assert!(serial.get_output().is_empty());
    }
}
