// src/virtio.rs
//!
//! VirtIO-MMIO Block Device Driver (Data Plane)
//! Handles Virtqueues, Descriptors, and Disk I/O.
//!

#![allow(dead_code)]

use std::sync::Mutex;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use crate::memory::GuestMemory;

// Register Offsets
pub const VIRTIO_MMIO_MAGIC_VALUE: u64 = 0x000;
pub const VIRTIO_MMIO_VERSION: u64 = 0x004;
pub const VIRTIO_MMIO_DEVICE_ID: u64 = 0x008;
pub const VIRTIO_MMIO_VENDOR_ID: u64 = 0x00c;
pub const VIRTIO_MMIO_DEVICE_FEATURES: u64 = 0x010;
pub const VIRTIO_MMIO_DEVICE_FEATURES_SEL: u64 = 0x014;
pub const VIRTIO_MMIO_DRIVER_FEATURES: u64 = 0x020;
pub const VIRTIO_MMIO_DRIVER_FEATURES_SEL: u64 = 0x024;
pub const VIRTIO_MMIO_QUEUE_SEL: u64 = 0x030;
pub const VIRTIO_MMIO_QUEUE_NUM_MAX: u64 = 0x034;
pub const VIRTIO_MMIO_QUEUE_NUM: u64 = 0x038;
pub const VIRTIO_MMIO_QUEUE_READY: u64 = 0x044;
pub const VIRTIO_MMIO_QUEUE_NOTIFY: u64 = 0x050;
pub const VIRTIO_MMIO_INTERRUPT_STATUS: u64 = 0x060;
pub const VIRTIO_MMIO_INTERRUPT_ACK: u64 = 0x064;
pub const VIRTIO_MMIO_STATUS: u64 = 0x070;
pub const VIRTIO_MMIO_QUEUE_DESC_LOW: u64 = 0x080;
pub const VIRTIO_MMIO_QUEUE_DESC_HIGH: u64 = 0x084;
pub const VIRTIO_MMIO_QUEUE_AVAIL_LOW: u64 = 0x090;
pub const VIRTIO_MMIO_QUEUE_AVAIL_HIGH: u64 = 0x094;
pub const VIRTIO_MMIO_QUEUE_USED_LOW: u64 = 0x0a0;
pub const VIRTIO_MMIO_QUEUE_USED_HIGH: u64 = 0x0a4;
pub const VIRTIO_MMIO_CONFIG: u64 = 0x100;

// Constants
const MAGIC_VALUE: u32 = 0x74726976;
const VERSION: u32 = 2;
const DEVICE_ID_BLOCK: u32 = 2;
const VENDOR_ID: u32 = 0x554d4551;

// Features
const VIRTIO_BLK_F_SIZE_MAX: u64 = 1 << 1;
const VIRTIO_BLK_F_SEG_MAX: u64 = 1 << 2;
const VIRTIO_BLK_F_GEOMETRY: u64 = 1 << 4;
const VIRTIO_BLK_F_BLK_SIZE: u64 = 1 << 6;
const VIRTIO_F_VERSION_1: u64 = 1 << 32;

// Disk Config
const DISK_SIZE_SECTORS: u64 = 204800; // 100MB / 512
const SECTOR_SIZE: u32 = 512;

// Request Types
const VIRTIO_BLK_T_IN: u32 = 0;  // Read
const VIRTIO_BLK_T_OUT: u32 = 1; // Write

// Status
const VIRTIO_BLK_S_OK: u8 = 0;
const VIRTIO_BLK_S_IOERR: u8 = 1;

// Descriptor Flags
const VRING_DESC_F_NEXT: u16 = 1;
const VRING_DESC_F_WRITE: u16 = 2;

pub struct VirtioBlock {
    status: Mutex<u32>,
    features_sel: Mutex<u32>,
    driver_features: Mutex<u64>,
    interrupt_status: Mutex<u32>,
    
    queue_sel: Mutex<u32>,
    queue_num: Mutex<u32>,
    queue_ready: Mutex<u32>,
    queue_desc: Mutex<u64>,
    queue_avail: Mutex<u64>,
    queue_used: Mutex<u64>,
    
    last_avail_idx: Mutex<u16>,
    disk: Mutex<Option<File>>,
}

impl VirtioBlock {
    pub fn new() -> Self {
        println!(">>> [VirtIO] Initializing block device...");
        
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("disk.img")
            .ok();
        
        if file.is_some() {
            println!(">>> [VirtIO] disk.img opened successfully");
        } else {
            println!(">>> [VirtIO] Warning: disk.img not found - disk will be empty");
        }

        Self {
            status: Mutex::new(0),
            features_sel: Mutex::new(0),
            driver_features: Mutex::new(0),
            interrupt_status: Mutex::new(0),
            queue_sel: Mutex::new(0),
            queue_num: Mutex::new(0),
            queue_ready: Mutex::new(0),
            queue_desc: Mutex::new(0),
            queue_avail: Mutex::new(0),
            queue_used: Mutex::new(0),
            last_avail_idx: Mutex::new(0),
            disk: Mutex::new(file),
        }
    }

    /// Handle MMIO read
    pub fn read(&self, offset: u64, data: &mut [u8]) {
        let val: u32 = match offset {
            VIRTIO_MMIO_MAGIC_VALUE => MAGIC_VALUE,
            VIRTIO_MMIO_VERSION => VERSION,
            VIRTIO_MMIO_DEVICE_ID => DEVICE_ID_BLOCK,
            VIRTIO_MMIO_VENDOR_ID => VENDOR_ID,
            VIRTIO_MMIO_DEVICE_FEATURES => {
                let sel = *self.features_sel.lock().unwrap();
                if sel == 0 {
                    (VIRTIO_BLK_F_SIZE_MAX | VIRTIO_BLK_F_SEG_MAX | 
                     VIRTIO_BLK_F_GEOMETRY | VIRTIO_BLK_F_BLK_SIZE) as u32
                } else {
                    (VIRTIO_F_VERSION_1 >> 32) as u32
                }
            },
            VIRTIO_MMIO_QUEUE_NUM_MAX => 256,
            VIRTIO_MMIO_QUEUE_READY => *self.queue_ready.lock().unwrap(),
            VIRTIO_MMIO_INTERRUPT_STATUS => *self.interrupt_status.lock().unwrap(),
            VIRTIO_MMIO_STATUS => *self.status.lock().unwrap(),
            VIRTIO_MMIO_CONFIG => (DISK_SIZE_SECTORS & 0xFFFFFFFF) as u32,
            0x104 => (DISK_SIZE_SECTORS >> 32) as u32,
            0x114 => SECTOR_SIZE,
            _ => 0,
        };

        let bytes = val.to_le_bytes();
        let len = data.len().min(4);
        data[..len].copy_from_slice(&bytes[..len]);
    }

    /// Handle MMIO write - returns true if IRQ needed
    pub fn write(&self, offset: u64, data: &[u8], mem: &mut GuestMemory) -> bool {
        if data.len() < 4 { return false; }
        let val = u32::from_le_bytes(data[0..4].try_into().unwrap_or([0; 4]));
        let mut trigger_irq = false;

        match offset {
            VIRTIO_MMIO_DEVICE_FEATURES_SEL => *self.features_sel.lock().unwrap() = val,
            VIRTIO_MMIO_DRIVER_FEATURES_SEL => *self.features_sel.lock().unwrap() = val,
            VIRTIO_MMIO_DRIVER_FEATURES => {
                let sel = *self.features_sel.lock().unwrap();
                let mut feat = self.driver_features.lock().unwrap();
                if sel == 0 { *feat = (*feat & !0xFFFFFFFF) | val as u64; }
                else { *feat = (*feat & 0xFFFFFFFF) | ((val as u64) << 32); }
            },
            VIRTIO_MMIO_QUEUE_SEL => *self.queue_sel.lock().unwrap() = val,
            VIRTIO_MMIO_QUEUE_NUM => *self.queue_num.lock().unwrap() = val,
            VIRTIO_MMIO_QUEUE_READY => *self.queue_ready.lock().unwrap() = val,
            VIRTIO_MMIO_QUEUE_NOTIFY => {
                trigger_irq = self.process_queue(mem);
            },
            VIRTIO_MMIO_INTERRUPT_ACK => *self.interrupt_status.lock().unwrap() &= !val,
            VIRTIO_MMIO_STATUS => {
                let old = *self.status.lock().unwrap();
                *self.status.lock().unwrap() = val;
                if val == 0 && old != 0 { 
                    *self.queue_ready.lock().unwrap() = 0;
                    *self.last_avail_idx.lock().unwrap() = 0;
                }
            },
            VIRTIO_MMIO_QUEUE_DESC_LOW => self.set_low(&self.queue_desc, val),
            VIRTIO_MMIO_QUEUE_DESC_HIGH => self.set_high(&self.queue_desc, val),
            VIRTIO_MMIO_QUEUE_AVAIL_LOW => self.set_low(&self.queue_avail, val),
            VIRTIO_MMIO_QUEUE_AVAIL_HIGH => self.set_high(&self.queue_avail, val),
            VIRTIO_MMIO_QUEUE_USED_LOW => self.set_low(&self.queue_used, val),
            VIRTIO_MMIO_QUEUE_USED_HIGH => self.set_high(&self.queue_used, val),
            _ => {}
        }
        
        trigger_irq
    }

    fn set_low(&self, mutex: &Mutex<u64>, val: u32) {
        let mut g = mutex.lock().unwrap();
        *g = (*g & 0xFFFFFFFF00000000) | val as u64;
    }

    fn set_high(&self, mutex: &Mutex<u64>, val: u32) {
        let mut g = mutex.lock().unwrap();
        *g = (*g & 0x00000000FFFFFFFF) | ((val as u64) << 32);
    }

    // ========================================================================
    // DATA PLANE
    // ========================================================================
    
    fn process_queue(&self, mem: &mut GuestMemory) -> bool {
        let queue_size = *self.queue_num.lock().unwrap() as u16;
        if queue_size == 0 || *self.queue_ready.lock().unwrap() == 0 { 
            return false; 
        }

        let desc_addr = *self.queue_desc.lock().unwrap();
        let avail_addr = *self.queue_avail.lock().unwrap();
        let used_addr = *self.queue_used.lock().unwrap();

        // Read avail->idx
        let avail_idx = match mem.read_slice(avail_addr as usize + 2, 2) {
            Ok(bytes) => u16::from_le_bytes([bytes[0], bytes[1]]),
            Err(_) => return false,
        };

        let mut last_idx = self.last_avail_idx.lock().unwrap();
        let mut work_done = false;

        // Process pending requests
        while *last_idx != avail_idx {
            let ring_offset = 4 + (*last_idx % queue_size) as usize * 2;
            let head_idx = match mem.read_slice(avail_addr as usize + ring_offset, 2) {
                Ok(bytes) => u16::from_le_bytes([bytes[0], bytes[1]]),
                Err(_) => break,
            };

            let written = self.process_descriptor_chain(mem, desc_addr, head_idx);

            // Update used ring
            let used_idx = match mem.read_slice(used_addr as usize + 2, 2) {
                Ok(bytes) => u16::from_le_bytes([bytes[0], bytes[1]]),
                Err(_) => 0,
            };
            
            let used_ring_offset = 4 + (used_idx % queue_size) as usize * 8;
            let _ = mem.write_u32(used_addr as usize + used_ring_offset, head_idx as u32);
            let _ = mem.write_u32(used_addr as usize + used_ring_offset + 4, written);
            let _ = mem.write_u16(used_addr as usize + 2, used_idx.wrapping_add(1));

            *last_idx = last_idx.wrapping_add(1);
            work_done = true;
        }

        if work_done {
            *self.interrupt_status.lock().unwrap() |= 1;
            return true;
        }
        false
    }

    fn process_descriptor_chain(&self, mem: &mut GuestMemory, desc_table: u64, head_idx: u16) -> u32 {
        let mut next_idx = head_idx;
        let mut total_written = 0u32;
        
        let mut sector = 0u64;
        let mut is_write = false;
        let mut data_addr = 0u64;
        let mut data_len = 0u32;
        let mut status_addr = 0u64;
        let mut phase = 0; // 0=header, 1=data, 2=status

        loop {
            let desc_offset = desc_table as usize + (next_idx as usize * 16);
            let desc_bytes = match mem.read_slice(desc_offset, 16) {
                Ok(b) => b,
                Err(_) => break,
            };
            
            let addr = u64::from_le_bytes(desc_bytes[0..8].try_into().unwrap());
            let len = u32::from_le_bytes(desc_bytes[8..12].try_into().unwrap());
            let flags = u16::from_le_bytes(desc_bytes[12..14].try_into().unwrap());
            let next = u16::from_le_bytes(desc_bytes[14..16].try_into().unwrap());

            match phase {
                0 => {
                    // Header: type(4), reserved(4), sector(8)
                    if let Ok(header) = mem.read_slice(addr as usize, 16.min(len as usize)) {
                        if header.len() >= 16 {
                            let type_ = u32::from_le_bytes(header[0..4].try_into().unwrap());
                            sector = u64::from_le_bytes(header[8..16].try_into().unwrap());
                            is_write = type_ == VIRTIO_BLK_T_OUT;
                        }
                    }
                    phase = 1;
                },
                1 => {
                    if (flags & VRING_DESC_F_NEXT) != 0 {
                        // Data descriptor
                        data_addr = addr;
                        data_len = len;
                    } else {
                        // Status descriptor (last one)
                        status_addr = addr;
                        phase = 2;
                    }
                },
                _ => {
                    status_addr = addr;
                }
            }

            if (flags & VRING_DESC_F_NEXT) == 0 { break; }
            next_idx = next;
        }

        // Perform I/O
        if data_addr != 0 && data_len > 0 {
            let offset = sector * 512;
            let mut disk = self.disk.lock().unwrap();
            
            if let Some(ref mut file) = *disk {
                if file.seek(SeekFrom::Start(offset)).is_ok() {
                    if is_write {
                        if let Ok(data) = mem.read_slice(data_addr as usize, data_len as usize) {
                            let _ = file.write_all(data);
                        }
                    } else {
                        let mut buf = vec![0u8; data_len as usize];
                        let bytes_read = file.read(&mut buf).unwrap_or(0);
                        if bytes_read > 0 {
                            let _ = mem.write_slice(data_addr as usize, &buf[..bytes_read]);
                            total_written += bytes_read as u32;
                        }
                    }
                }
            }
        }

        // Write status
        if status_addr != 0 {
            let _ = mem.write_u8(status_addr as usize, VIRTIO_BLK_S_OK);
            total_written += 1;
        }

        total_written
    }
}

impl Default for VirtioBlock {
    fn default() -> Self {
        Self::new()
    }
}
