




use std::io::{self, Write};

pub const COM1_BASE: u16 = 0x3F8;
pub const DATA_REGISTER: u16 = 0;
pub const LINE_STATUS_REGISTER: u16 = 5;

pub struct SerialConsole;

impl SerialConsole {
    pub fn new() -> Self {
        Self
    }

    pub fn write(&self, port: u16, data: &[u8]) {
        let offset = port - COM1_BASE;
        
        if offset == DATA_REGISTER {
            if let Some(&byte) = data.first() {
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                
                
                if byte == b'\n' {
                    let _ = handle.write_all(b"\r\n");
                } else {
                    let _ = handle.write_all(&[byte]);
                }
                let _ = handle.flush();
            }
        }
    }

    pub fn read(&self, port: u16) -> u8 {
        let offset = port - COM1_BASE;
        match offset {
            
            
            LINE_STATUS_REGISTER => 0x20 | 0x40,
            _ => 0,
        }
    }
}

impl Default for SerialConsole {
    fn default() -> Self {
        Self::new()
    }
}
