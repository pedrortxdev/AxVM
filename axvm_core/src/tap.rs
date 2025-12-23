// src/tap.rs
use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::mem;

// Constantes mágicas do Kernel Linux (if_tun.h)
const IFF_TAP: i16 = 0x0002;
const IFF_NO_PI: i16 = 0x1000;
const TUNSETIFF: u64 = 0x400454ca; // Macro _IOW('T', 202, int)

#[repr(C)]
struct IfReq {
    ifr_name: [u8; 16],
    ifr_flags: i16,
    _pad: [u8; 22], // Padding para completar sizeof(struct ifreq)
}

pub struct TapInterface {
    file: File,
    name: String,
}

impl TapInterface {
    /// Cria uma nova interface TAP.
    /// Se `name` for None, o Kernel escolhe (ex: tap0, tap1).
    pub fn new(dev_name: Option<&str>) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/net/tun")?;

        let mut ifr: IfReq = unsafe { mem::zeroed() };
        ifr.ifr_flags = IFF_TAP | IFF_NO_PI; // TAP mode, sem Packet Info header

        if let Some(name) = dev_name {
            let bytes = name.as_bytes();
            if bytes.len() > 15 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Nome muito longo"));
            }
            ifr.ifr_name[..bytes.len()].copy_from_slice(bytes);
        }

        // A mágica: IOCTL para configurar o dispositivo
        let ret = unsafe { libc::ioctl(file.as_raw_fd(), TUNSETIFF, &mut ifr) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        // Set non-blocking mode
        let fd = file.as_raw_fd();
        unsafe {
            let mut flags = libc::fcntl(fd, libc::F_GETFL);
            flags |= libc::O_NONBLOCK;
            libc::fcntl(fd, libc::F_SETFL, flags);
        }

        let actual_name = unsafe {
            CStr::from_ptr(ifr.ifr_name.as_ptr() as *const i8)
                .to_string_lossy()
                .into_owned()
        };

        tracing::info!(name = %actual_name, "TAP interface created");

        Ok(TapInterface {
            file,
            name: actual_name,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    // Encaminha leitura para o arquivo
    pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }

    // Encaminha escrita para o arquivo
    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }
}
