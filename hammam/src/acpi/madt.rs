use core::slice;

const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";
const MADT_SIGNATURE: &[u8; 4] = b"APIC";
const RSDT_SIGNATURE: &[u8; 4] = b"RSDT";
const XSDT_SIGNATURE: &[u8; 4] = b"XSDT";
const SDT_HEADER_SIZE: usize = 36;
const MADT_FIXED_HEADER_SIZE: usize = SDT_HEADER_SIZE + 8;

#[repr(C)]
#[derive(Clone, Copy)]
struct RsdpV1 {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RsdpV2 {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,
    length: u32,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oemid: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

/// Параметры локального APIC из записи MADT типа 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalApic {
    pub processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

/// Параметры I/O APIC из записи MADT типа 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoApic {
    pub io_apic_id: u8,
    pub address: u32,
    pub global_system_interrupt_base: u32,
}

/// Представление таблицы MADT (APIC) в физической памяти.
pub struct Madt {
    pub local_apic_address: u32,
    pub flags: u32,
    entries_ptr: *const u8,
    entries_len: usize,
}

impl Madt {
    /// Ищет MADT по переданному физическому адресу RSDP.
    pub fn from_rsdp(rsdp_addr: u64) -> Option<Self> {
        let rsdp = unsafe { read_rsdp(rsdp_addr)? };

        if let Some(xsdt_addr) = rsdp.xsdt_address {
            if let Some(madt) = unsafe { find_madt_in_sdt(xsdt_addr, 8, XSDT_SIGNATURE) } {
                return Some(madt);
            }
        }

        if rsdp.rsdt_address != 0 {
            return unsafe { find_madt_in_sdt(rsdp.rsdt_address, 4, RSDT_SIGNATURE) };
        }

        None
    }

    /// Итератор по записям Local APIC (тип 0).
    pub fn local_apics(&self) -> LocalApicIter {
        LocalApicIter {
            entries_ptr: self.entries_ptr,
            offset: 0,
            len: self.entries_len,
        }
    }

    /// Итератор по записям I/O APIC (тип 1).
    pub fn io_apics(&self) -> IoApicIter {
        IoApicIter {
            entries_ptr: self.entries_ptr,
            offset: 0,
            len: self.entries_len,
        }
    }
}

pub struct LocalApicIter {
    entries_ptr: *const u8,
    offset: usize,
    len: usize,
}

impl Iterator for LocalApicIter {
    type Item = LocalApic;

    fn next(&mut self) -> Option<Self::Item> {
        while self.offset + 2 <= self.len {
            let entry = unsafe { self.entries_ptr.add(self.offset) };
            let entry_type = unsafe { read_u8(entry) };
            let entry_len = unsafe { read_u8(entry.add(1)) } as usize;

            if entry_len < 2 || self.offset + entry_len > self.len {
                return None;
            }

            self.offset += entry_len;

            if entry_type != 0 {
                continue;
            }

            if entry_len < 8 {
                continue;
            }

            let processor_id = unsafe { read_u8(entry.add(2)) };
            let apic_id = unsafe { read_u8(entry.add(3)) };
            let flags = unsafe { read_u32(entry.add(4)) };

            return Some(LocalApic {
                processor_id,
                apic_id,
                flags,
            });
        }

        None
    }
}

pub struct IoApicIter {
    entries_ptr: *const u8,
    offset: usize,
    len: usize,
}

impl Iterator for IoApicIter {
    type Item = IoApic;

    fn next(&mut self) -> Option<Self::Item> {
        while self.offset + 2 <= self.len {
            let entry = unsafe { self.entries_ptr.add(self.offset) };
            let entry_type = unsafe { read_u8(entry) };
            let entry_len = unsafe { read_u8(entry.add(1)) } as usize;

            if entry_len < 2 || self.offset + entry_len > self.len {
                return None;
            }

            self.offset += entry_len;

            if entry_type != 1 {
                continue;
            }

            if entry_len < 12 {
                continue;
            }

            let io_apic_id = unsafe { read_u8(entry.add(2)) };
            let address = unsafe { read_u32(entry.add(4)) };
            let global_system_interrupt_base = unsafe { read_u32(entry.add(8)) };

            return Some(IoApic {
                io_apic_id,
                address,
                global_system_interrupt_base,
            });
        }

        None
    }
}

struct RsdpInfo {
    rsdt_address: u64,
    xsdt_address: Option<u64>,
}

unsafe fn read_rsdp(addr: u64) -> Option<RsdpInfo> {
    let signature = unsafe { read_bytes(addr, 8) };
    if signature != RSDP_SIGNATURE {
        return None;
    }

    let revision = unsafe { read_u8(phys_to_ptr::<u8>(addr + 15)) };
    if revision >= 2 {
        let rsdp: RsdpV2 = unsafe { read_struct(addr) };
        if (rsdp.length as usize) < core::mem::size_of::<RsdpV2>() {
            return None;
        }
        if !checksum(addr, rsdp.length as usize) {
            return None;
        }
        return Some(RsdpInfo {
            rsdt_address: rsdp.rsdt_address as u64,
            xsdt_address: if rsdp.xsdt_address != 0 {
                Some(rsdp.xsdt_address)
            } else {
                None
            },
        });
    }

    let rsdp: RsdpV1 = unsafe { read_struct(addr) };
    if !checksum(addr, core::mem::size_of::<RsdpV1>()) {
        return None;
    }

    Some(RsdpInfo {
        rsdt_address: rsdp.rsdt_address as u64,
        xsdt_address: None,
    })
}

unsafe fn find_madt_in_sdt(sdt_addr: u64, entry_size: usize, expected_signature: &[u8; 4]) -> Option<Madt> {
    let header: SdtHeader = unsafe { read_struct(sdt_addr) };
    if &header.signature != expected_signature {
        return None;
    }

    let total_length = header.length as usize;
    if total_length < SDT_HEADER_SIZE {
        return None;
    }

    let entries_count = (total_length - SDT_HEADER_SIZE) / entry_size;
    let entries_base = sdt_addr + SDT_HEADER_SIZE as u64;

    for idx in 0..entries_count {
        let entry_addr = entries_base + (idx * entry_size) as u64;
        let table_addr = if entry_size == 8 {
            unsafe { read_u64(entry_addr) }
        } else {
            unsafe { read_u32(phys_to_ptr::<u8>(entry_addr)) as u64 }
        };

        if let Some(madt) = unsafe { parse_madt(table_addr) } {
            return Some(madt);
        }
    }

    None
}

unsafe fn parse_madt(addr: u64) -> Option<Madt> {
    let header: SdtHeader = unsafe { read_struct(addr) };
    if &header.signature != MADT_SIGNATURE {
        return None;
    }

    let length = header.length as usize;
    if length < MADT_FIXED_HEADER_SIZE {
        return None;
    }

    let local_apic_address = unsafe { read_u32(phys_to_ptr::<u8>(addr + SDT_HEADER_SIZE as u64)) };
    let flags = unsafe { read_u32(phys_to_ptr::<u8>(addr + SDT_HEADER_SIZE as u64 + 4)) };
    let entries_ptr = phys_to_ptr::<u8>(addr + MADT_FIXED_HEADER_SIZE as u64);
    let entries_len = length - MADT_FIXED_HEADER_SIZE;

    Some(Madt {
        local_apic_address,
        flags,
        entries_ptr,
        entries_len,
    })
}

unsafe fn read_struct<T: Copy>(addr: u64) -> T {
    unsafe { core::ptr::read_unaligned(phys_to_ptr::<T>(addr)) }
}

unsafe fn read_bytes(addr: u64, length: usize) -> &'static [u8] {
    if length == 0 {
        return &[];
    }
    let ptr = phys_to_ptr::<u8>(addr);
    unsafe { slice::from_raw_parts(ptr, length) }
}

unsafe fn read_u8(addr: *const u8) -> u8 {
    unsafe { core::ptr::read_unaligned(addr) }
}

unsafe fn read_u32(addr: *const u8) -> u32 {
    unsafe { core::ptr::read_unaligned(addr as *const u32) }
}

unsafe fn read_u64(addr: u64) -> u64 {
    unsafe { core::ptr::read_unaligned(phys_to_ptr::<u64>(addr)) }
}

fn checksum(addr: u64, length: usize) -> bool {
    let bytes = unsafe { read_bytes(addr, length) };
    let sum = bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    sum == 0
}

#[inline]
fn phys_to_ptr<T>(addr: u64) -> *const T {
    addr as usize as *const T
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsdp_signature_check() {
        let mut buffer = [0u8; 36];
        buffer[..8].copy_from_slice(RSDP_SIGNATURE);
        buffer[15] = 1;
        buffer[20..24].copy_from_slice(&0u32.to_le_bytes());
        buffer[24..28].copy_from_slice(&(36u32).to_le_bytes());
        buffer[28..36].copy_from_slice(&0u64.to_le_bytes());
        buffer[8] = (!buffer[..20].iter().fold(0u8, |acc, &b| acc.wrapping_add(b))).wrapping_add(0);
        assert_eq!(&buffer[..8], RSDP_SIGNATURE);
    }
}
