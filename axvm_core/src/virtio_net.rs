// src/virtio_net.rs
use crate::tap::TapInterface;
use std::sync::Mutex;
use std::mem::size_of;

// Constantes de Registradores MMIO (Spec v2)
const MMIO_MAGIC_VALUE: u64 = 0x000;
const MMIO_VERSION: u64 = 0x004;
const MMIO_DEVICE_ID: u64 = 0x008;
const MMIO_VENDOR_ID: u64 = 0x00c;
const MMIO_DEVICE_FEATURES: u64 = 0x010;
const MMIO_DEVICE_FEATURES_SEL: u64 = 0x014;
const MMIO_DRIVER_FEATURES: u64 = 0x020;
const MMIO_DRIVER_FEATURES_SEL: u64 = 0x024;
const MMIO_QUEUE_SEL: u64 = 0x030;
const MMIO_QUEUE_NUM_MAX: u64 = 0x034;
const MMIO_QUEUE_NUM: u64 = 0x038;
const MMIO_QUEUE_READY: u64 = 0x044;
const MMIO_INTERRUPT_STATUS: u64 = 0x060;
const MMIO_INTERRUPT_ACK: u64 = 0x064;
const MMIO_STATUS: u64 = 0x070;
const MMIO_QUEUE_DESC_LOW: u64 = 0x080;
const MMIO_QUEUE_DESC_HIGH: u64 = 0x084;
const MMIO_QUEUE_AVAIL_LOW: u64 = 0x090;
const MMIO_QUEUE_AVAIL_HIGH: u64 = 0x094;
const MMIO_QUEUE_USED_LOW: u64 = 0x0a0;
const MMIO_QUEUE_USED_HIGH: u64 = 0x0a4;
const MMIO_CONFIG_SPACE: u64 = 0x100;

// VirtIO Net Feature Bits
const VIRTIO_NET_F_MAC: u64 = 1 << 5;
const VIRTIO_F_VERSION_1: u64 = 1 << 32;

// VirtIO Ring Buffer Structures
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

// VirtIO Net Header (must precede every packet)
#[repr(C, packed)]
#[derive(Default, Debug, Clone, Copy)]
struct VirtioNetHdr {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
    num_buffers: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct VirtQueue {
    pub desc_addr: u64,
    pub avail_addr: u64,
    pub used_addr: u64,
    pub queue_size: u16,
    pub ready: bool,
    pub last_avail_idx: u16,
}

impl VirtQueue {
    fn new() -> Self {
        VirtQueue {
            desc_addr: 0,
            avail_addr: 0,
            used_addr: 0,
            queue_size: 0,
            ready: false,
            last_avail_idx: 0,
        }
    }
    
    fn available_idx(&self, mem: &[u8]) -> u16 {
        let idx_addr = self.avail_addr + 2;
        if idx_addr as usize + 2 > mem.len() {
            return 0;
        }
        let b = &mem[idx_addr as usize..idx_addr as usize + 2];
        u16::from_le_bytes([b[0], b[1]])
    }
    
    fn get_avail_desc_idx(&self, mem: &[u8]) -> Option<u16> {
        let guest_idx = self.available_idx(mem);
        
        if self.last_avail_idx == guest_idx {
            return None;
        }
        
        let ring_offset = 4 + (self.last_avail_idx % self.queue_size) as u64 * 2;
        let addr = self.avail_addr + ring_offset;
        
        if addr as usize + 2 > mem.len() {
            return None;
        }
        
        let b = &mem[addr as usize..addr as usize + 2];
        let desc_idx = u16::from_le_bytes([b[0], b[1]]);
        
        Some(desc_idx)
    }
    
    fn read_desc(&self, mem: &[u8], idx: u16) -> Option<VirtqDesc> {
        let offset = self.desc_addr + (idx as u64 * size_of::<VirtqDesc>() as u64);
        
        if offset as usize + size_of::<VirtqDesc>() > mem.len() {
            return None;
        }
        
        let b = &mem[offset as usize..offset as usize + size_of::<VirtqDesc>()];
        Some(unsafe { std::ptr::read(b.as_ptr() as *const VirtqDesc) })
    }
    
    fn add_used(&mut self, mem: &mut [u8], desc_idx: u16, len: u32) {
        let used_elem_offset = 4 + (self.last_avail_idx % self.queue_size) as u64 * size_of::<VirtqUsedElem>() as u64;
        let addr = self.used_addr + used_elem_offset;
        
        if addr as usize + size_of::<VirtqUsedElem>() > mem.len() {
            return;
        }
        
        let elem = VirtqUsedElem { id: desc_idx as u32, len };
        
        unsafe {
            let ptr = mem.as_mut_ptr().add(addr as usize) as *mut VirtqUsedElem;
            *ptr = elem;
        }
        
        self.last_avail_idx = self.last_avail_idx.wrapping_add(1);
        
        let idx_addr = self.used_addr + 2;
        if idx_addr as usize + 2 <= mem.len() {
            unsafe {
                let idx_ptr = mem.as_mut_ptr().add(idx_addr as usize) as *mut u16;
                *idx_ptr = self.last_avail_idx;
            }
        }
    }
}

pub struct VirtioNet {
    tap: Mutex<Option<TapInterface>>,
    mac: [u8; 6],
    
    status: Mutex<u32>,
    driver_features_sel: Mutex<u32>,
    device_features_sel: Mutex<u32>,
    driver_features: Mutex<u64>,
    queue_sel: Mutex<u32>,
    
    queues: Mutex<[VirtQueue; 2]>,
    interrupt_status: Mutex<u32>,
}

impl VirtioNet {
    pub fn new(tap: Option<TapInterface>) -> Self {
        if tap.is_some() {
            println!(">>> [Net] VirtIO-Net device initialized with TAP");
            tracing::info!("VirtIO-Net device initialized with TAP interface");
        } else {
            println!(">>> [Net] VirtIO-Net device initialized WITHOUT TAP (link down)");
            tracing::warn!("VirtIO-Net device initialized without TAP interface");
        }
        
        VirtioNet {
            tap: Mutex::new(tap),
            mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            status: Mutex::new(0),
            driver_features_sel: Mutex::new(0),
            device_features_sel: Mutex::new(0),
            driver_features: Mutex::new(0),
            queue_sel: Mutex::new(0),
            queues: Mutex::new([VirtQueue::new(), VirtQueue::new()]),
            interrupt_status: Mutex::new(0),
        }
    }

    pub fn read(&self, offset: u64, data: &mut [u8]) {
        let val: u64 = match offset {
            MMIO_MAGIC_VALUE => 0x74726976,
            MMIO_VERSION => 2,
            MMIO_DEVICE_ID => 1,
            MMIO_VENDOR_ID => 0x1AF4,
            
            MMIO_DEVICE_FEATURES => {
                let sel = *self.device_features_sel.lock().unwrap();
                if sel == 0 {
                    (VIRTIO_NET_F_MAC | (VIRTIO_F_VERSION_1 & 0xFFFFFFFF)) as u64
                } else if sel == 1 {
                    (VIRTIO_F_VERSION_1 >> 32) as u64
                } else {
                    0
                }
            },
            
            MMIO_QUEUE_NUM_MAX => 256,
            
            MMIO_QUEUE_READY => {
                let sel = *self.queue_sel.lock().unwrap();
                let queues = self.queues.lock().unwrap();
                if (sel as usize) < 2 {
                    queues[sel as usize].ready as u64
                } else {
                    0
                }
            },
            
            MMIO_INTERRUPT_STATUS => *self.interrupt_status.lock().unwrap() as u64,
            MMIO_STATUS => *self.status.lock().unwrap() as u64,
            
            off if off >= MMIO_CONFIG_SPACE && off < MMIO_CONFIG_SPACE + 6 => {
                let idx = (off - MMIO_CONFIG_SPACE) as usize;
                let mut val: u64 = 0;
                for i in 0..data.len().min(6 - idx) {
                    val |= (self.mac[idx + i] as u64) << (i * 8);
                }
                val
            },
            
            _ => 0,
        };

        let bytes = val.to_le_bytes();
        let len = data.len().min(8);
        data[..len].copy_from_slice(&bytes[..len]);
    }

    pub fn write(&self, offset: u64, data: &[u8]) -> Result<bool, String> {
        let val = match data.len() {
            1 => data[0] as u32,
            2 => u16::from_le_bytes([data[0], data[1]]) as u32,
            4 => u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            _ => return Err(format!("Invalid write size: {}", data.len())),
        };

        match offset {
            MMIO_DEVICE_FEATURES_SEL => {
                *self.device_features_sel.lock().unwrap() = val;
            },
            
            MMIO_DRIVER_FEATURES_SEL => {
                *self.driver_features_sel.lock().unwrap() = val;
            },
            
            MMIO_DRIVER_FEATURES => {
                let sel = *self.driver_features_sel.lock().unwrap();
                let mut features = self.driver_features.lock().unwrap();
                if sel == 0 {
                    *features = (*features & 0xFFFFFFFF00000000) | (val as u64);
                } else {
                    *features = (*features & 0x00000000FFFFFFFF) | ((val as u64) << 32);
                }
                tracing::debug!(features = *features, "Driver features negotiated");
            },
            
            MMIO_QUEUE_SEL => {
                *self.queue_sel.lock().unwrap() = val;
            },
            
            MMIO_QUEUE_NUM => {
                let sel = *self.queue_sel.lock().unwrap();
                if (sel as usize) < 2 {
                    self.queues.lock().unwrap()[sel as usize].queue_size = val as u16;
                }
            },
            
            MMIO_QUEUE_READY => {
                let sel = *self.queue_sel.lock().unwrap();
                if (sel as usize) < 2 {
                    let mut queues = self.queues.lock().unwrap();
                    queues[sel as usize].ready = (val & 1) == 1;
                    
                    if val == 1 {
                        let q = &queues[sel as usize];
                        println!(">>> [Net] Queue {} Configured: size={}, desc=0x{:x}, avail=0x{:x}, used=0x{:x}",
                            sel, q.queue_size, q.desc_addr, q.avail_addr, q.used_addr);
                        tracing::info!(
                            queue = sel,
                            size = q.queue_size,
                            desc = format!("0x{:x}", q.desc_addr),
                            avail = format!("0x{:x}", q.avail_addr),
                            used = format!("0x{:x}", q.used_addr),
                            "VirtIO-Net queue configured"
                        );
                    }
                }
            },
            
            MMIO_QUEUE_DESC_LOW => {
                let sel = *self.queue_sel.lock().unwrap();
                if (sel as usize) < 2 {
                    let mut queues = self.queues.lock().unwrap();
                    let addr = &mut queues[sel as usize].desc_addr;
                    *addr = (*addr & 0xFFFFFFFF00000000) | (val as u64);
                }
            },
            
            MMIO_QUEUE_DESC_HIGH => {
                let sel = *self.queue_sel.lock().unwrap();
                if (sel as usize) < 2 {
                    let mut queues = self.queues.lock().unwrap();
                    let addr = &mut queues[sel as usize].desc_addr;
                    *addr = (*addr & 0x00000000FFFFFFFF) | ((val as u64) << 32);
                }
            },
            
            MMIO_QUEUE_AVAIL_LOW => {
                let sel = *self.queue_sel.lock().unwrap();
                if (sel as usize) < 2 {
                    let mut queues = self.queues.lock().unwrap();
                    let addr = &mut queues[sel as usize].avail_addr;
                    *addr = (*addr & 0xFFFFFFFF00000000) | (val as u64);
                }
            },
            
            MMIO_QUEUE_AVAIL_HIGH => {
                let sel = *self.queue_sel.lock().unwrap();
                if (sel as usize) < 2 {
                    let mut queues = self.queues.lock().unwrap();
                    let addr = &mut queues[sel as usize].avail_addr;
                    *addr = (*addr & 0x00000000FFFFFFFF) | ((val as u64) << 32);
                }
            },
            
            MMIO_QUEUE_USED_LOW => {
                let sel = *self.queue_sel.lock().unwrap();
                if (sel as usize) < 2 {
                    let mut queues = self.queues.lock().unwrap();
                    let addr = &mut queues[sel as usize].used_addr;
                    *addr = (*addr & 0xFFFFFFFF00000000) | (val as u64);
                }
            },
            
            MMIO_QUEUE_USED_HIGH => {
                let sel = *self.queue_sel.lock().unwrap();
                if (sel as usize) < 2 {
                    let mut queues = self.queues.lock().unwrap();
                    let addr = &mut queues[sel as usize].used_addr;
                    *addr = (*addr & 0x00000000FFFFFFFF) | ((val as u64) << 32);
                }
            },
            
            MMIO_STATUS => {
                *self.status.lock().unwrap() = val;
                tracing::debug!(status = val, "VirtIO-Net status updated");
                
                if val == 0 {
                    self.reset();
                }
            },
            
            MMIO_INTERRUPT_ACK => {
                let mut int_status = self.interrupt_status.lock().unwrap();
                *int_status &= !val;
            },
            
            _ => {
                tracing::debug!(offset = offset, val = val, "Unknown VirtIO-Net write");
            }
        }

        Ok(false)
    }
    
    fn reset(&self) {
        *self.status.lock().unwrap() = 0;
        let mut queues = self.queues.lock().unwrap();
        queues[0] = VirtQueue::new();
        queues[1] = VirtQueue::new();
        *self.queue_sel.lock().unwrap() = 0;
        tracing::info!("VirtIO-Net device reset");
        println!(">>> [Net] Device RESET");
    }
    
    pub fn process_rx(&self, mem: &mut [u8]) -> bool {
        let mut tap_guard = self.tap.lock().unwrap();
        if tap_guard.is_none() {
            return false;
        }
        
        let mut queues = self.queues.lock().unwrap();
        let queue = &mut queues[0]; // RX Queue
        
        if !queue.ready {
            return false;
        }
        
        if let Some(desc_idx) = queue.get_avail_desc_idx(mem) {
            if let Some(desc) = queue.read_desc(mem, desc_idx) {
                let addr = desc.addr as usize;
                let desc_len = desc.len; // Copy to avoid packed field reference
                let mut packet_buf = [0u8; 1514];
                
                if let Some(tap) = tap_guard.as_mut() {
                    match tap.read(&mut packet_buf) {
                        Ok(n) if n > 0 => {
                            let hdr = VirtioNetHdr::default();
                            let hdr_len = size_of::<VirtioNetHdr>();
                            
                            if (n + hdr_len) as u32 > desc_len {
                                tracing::warn!(packet_size = n, buffer_size = desc_len, "Packet too big for buffer");
                                return false;
                            }
                            
                            if addr + hdr_len + n > mem.len() {
                                tracing::error!("Buffer address out of bounds");
                                return false;
                            }
                            
                            unsafe {
                                let dest_ptr = mem.as_mut_ptr().add(addr);
                                std::ptr::copy_nonoverlapping(
                                    &hdr as *const _ as *const u8,
                                    dest_ptr,
                                    hdr_len
                                );
                                std::ptr::copy_nonoverlapping(
                                    packet_buf.as_ptr(),
                                    dest_ptr.add(hdr_len),
                                    n
                                );
                            }
                            
                            queue.add_used(mem, desc_idx, (n + hdr_len) as u32);
                            
                            let mut int_status = self.interrupt_status.lock().unwrap();
                            *int_status |= 1;
                            
                            tracing::debug!(bytes = n, "RX packet processed");
                            return true;
                        },
                        _ => {}
                    }
                }
            }
        }
        
        false
    }
    
    pub fn should_interrupt(&self) -> bool {
        *self.interrupt_status.lock().unwrap() != 0
    }
    
    pub fn process_tx(&self, mem: &mut [u8]) -> bool {
        let mut tap_guard = self.tap.lock().unwrap();
        if tap_guard.is_none() {
            return false;
        }
        
        let mut queues = self.queues.lock().unwrap();
        let queue = &mut queues[1]; // TX Queue
        
        if !queue.ready {
            return false;
        }
        
        let mut work_done = false;
        
        while let Some(desc_idx) = queue.get_avail_desc_idx(mem) {
            if let Some(desc) = queue.read_desc(mem, desc_idx) {
                let addr = desc.addr as usize;
                let desc_len = desc.len as usize;
                let hdr_len = size_of::<VirtioNetHdr>();
                
                if desc_len > hdr_len && addr + desc_len <= mem.len() {
                    let packet_slice = &mem[addr + hdr_len..addr + desc_len];
                    
                    if let Some(tap) = tap_guard.as_mut() {
                        match tap.write(packet_slice) {
                            Ok(n) => {
                                tracing::debug!(bytes = n, "TX packet sent");
                                work_done = true;
                            },
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to write to TAP");
                            }
                        }
                    }
                }
                
                queue.add_used(mem, desc_idx, 0);
                
                let mut int_status = self.interrupt_status.lock().unwrap();
                *int_status |= 1;
            } else {
                break;
            }
        }
        
        work_done
    }
}

impl Default for VirtioNet {
    fn default() -> Self {
        Self::new(None)
    }
}
