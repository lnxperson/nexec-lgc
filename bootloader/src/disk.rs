use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use crate::ports::{PortU8, PortU16};

pub struct Disk {
    data_port: PortU16,
    error_port: PortU8,
    sector_count_port: PortU8,
    lba_lo_port: PortU8,
    lba_mid_port: PortU8,
    lba_hi_port: PortU8,
    drive_port: PortU8,
    command_port: PortU8,
    control_port: PortU8,
}

#[derive(Debug, Clone, Copy)]
pub struct Partition {
    pub start_lba: u32,
    pub sector_count: u32,
    pub partition_type: u8,
}

impl Disk {
    pub fn open() -> Option<Self> {
        Some(Disk {
            data_port: PortU16::new(0x1F0),
            error_port: PortU8::new(0x1F1),
            sector_count_port: PortU8::new(0x1F2),
            lba_lo_port: PortU8::new(0x1F3),
            lba_mid_port: PortU8::new(0x1F4),
            lba_hi_port: PortU8::new(0x1F5),
            drive_port: PortU8::new(0x1F6),
            command_port: PortU8::new(0x1F7),
            control_port: PortU8::new(0x3F6),
        })
    }

    fn wait_bsy(&mut self) {
        loop {
            let status: u8 = unsafe { self.command_port.read() };
            if status & 0x80 == 0 { break; }
        }
    }

    fn wait_drq(&mut self) -> bool {
        loop {
            let status: u8 = unsafe { self.command_port.read() };
            if status & 0x08 != 0 { return true; }
            if status & 0x01 != 0 { return false; }
        }
    }

    fn read_sectors_lba28(&mut self, lba: u32, count: u8, buffer: &mut [u16]) -> bool {
        if buffer.len() < count as usize * 256 { return false; }
        self.wait_bsy();
        unsafe {
            self.drive_port.write(0xE0 | ((lba >> 24) & 0x0F) as u8);
            self.sector_count_port.write(count);
            self.lba_lo_port.write((lba & 0xFF) as u8);
            self.lba_mid_port.write(((lba >> 8) & 0xFF) as u8);
            self.lba_hi_port.write(((lba >> 16) & 0xFF) as u8);
            self.command_port.write(0x20);
        }
        if !self.wait_drq() { return false; }
        for i in 0..count as usize {
            for j in 0..256 {
                buffer[i * 256 + j] = unsafe { self.data_port.read() };
            }
        }
        true
    }

    pub fn read_sectors(&mut self, lba: u32, count: u8) -> Option<Vec<u8>> {
        let mut buf = vec![0u16; count as usize * 256];
        if !self.read_sectors_lba28(lba, count, &mut buf) { return None; }
        let mut result = Vec::with_capacity(count as usize * 512);
        for word in &buf {
            result.push((word & 0xFF) as u8);
            result.push((word >> 8) as u8);
        }
        Some(result)
    }

    fn write_sectors_lba28(&mut self, lba: u32, count: u8, buffer: &[u16]) -> bool {
        if buffer.len() < count as usize * 256 { return false; }
        self.wait_bsy();
        unsafe {
            self.drive_port.write(0xE0 | ((lba >> 24) & 0x0F) as u8);
            self.sector_count_port.write(count);
            self.lba_lo_port.write((lba & 0xFF) as u8);
            self.lba_mid_port.write(((lba >> 8) & 0xFF) as u8);
            self.lba_hi_port.write(((lba >> 16) & 0xFF) as u8);
            self.command_port.write(0x30);
        }
        if !self.wait_drq() { return false; }
        for i in 0..count as usize {
            for j in 0..256 {
                unsafe { self.data_port.write(buffer[i * 256 + j]); }
            }
            if i < count as usize - 1 { self.wait_drq(); }
        }
        unsafe { self.command_port.write(0xE7); }
        self.wait_bsy();
        true
    }

    pub fn write_sectors(&mut self, lba: u32, data: &[u8]) -> bool {
        let count = (data.len() + 511) / 512;
        let mut buf = vec![0u16; count * 256];
        for (i, byte) in data.iter().enumerate() {
            let word_idx = i / 2;
            if i % 2 == 0 { buf[word_idx] = *byte as u16; }
            else { buf[word_idx] |= (*byte as u16) << 8; }
        }
        self.write_sectors_lba28(lba, count as u8, &buf)
    }

    pub fn find_boot_partition(&mut self) -> Option<Partition> {
        let mbr = self.read_sectors(0, 1)?;
        for i in 0..4 {
            let offset = 446 + i * 16;
            let partition_type = mbr[offset + 4];
            let start_lba = u32::from_le_bytes([
                mbr[offset + 8], mbr[offset + 9], mbr[offset + 10], mbr[offset + 11],
            ]);
            let sector_count = u32::from_le_bytes([
                mbr[offset + 12], mbr[offset + 13], mbr[offset + 14], mbr[offset + 15],
            ]);
            if partition_type == 0x0B || partition_type == 0x0C {
                if start_lba > 0 && sector_count > 0 {
                    return Some(Partition { start_lba, sector_count, partition_type });
                }
            }
        }
        for i in 0..4 {
            let offset = 446 + i * 16;
            let partition_type = mbr[offset + 4];
            let start_lba = u32::from_le_bytes([
                mbr[offset + 8], mbr[offset + 9], mbr[offset + 10], mbr[offset + 11],
            ]);
            let sector_count = u32::from_le_bytes([
                mbr[offset + 12], mbr[offset + 13], mbr[offset + 14], mbr[offset + 15],
            ]);
            if partition_type != 0 && start_lba > 0 && sector_count > 0 {
                return Some(Partition { start_lba, sector_count, partition_type });
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// FAT32 implementation (read-only)
// ---------------------------------------------------------------------------

pub struct Fat32<'a> {
    disk: &'a mut Disk,
    partition_start: u32,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fat_count: u8,
    sectors_per_fat: u32,
    root_cluster: u32,
}

impl<'a> Fat32<'a> {
    pub fn new(disk: &'a mut Disk, partition: Partition) -> Option<Self> {
        let boot_data = disk.read_sectors(partition.start_lba, 1)?;
        let bytes_per_sector = u16::from_le_bytes([boot_data[11], boot_data[12]]);
        let sectors_per_cluster = boot_data[13];
        let reserved_sectors = u16::from_le_bytes([boot_data[14], boot_data[15]]);
        let fat_count = boot_data[16];
        let sectors_per_fat = u32::from_le_bytes([boot_data[36], boot_data[37], boot_data[38], boot_data[39]]);
        let root_cluster = u32::from_le_bytes([boot_data[44], boot_data[45], boot_data[46], boot_data[47]]);
        if bytes_per_sector != 512 { return None; }
        Some(Fat32 { disk, partition_start: partition.start_lba, bytes_per_sector, sectors_per_cluster, reserved_sectors, fat_count, sectors_per_fat, root_cluster })
    }

    fn fat_start_lba(&self) -> u32 {
        self.partition_start + self.reserved_sectors as u32
    }

    fn cluster_to_lba(&self, cluster: u32) -> u32 {
        let data_start = self.fat_start_lba() + self.fat_count as u32 * self.sectors_per_fat;
        data_start + (cluster - 2) * self.sectors_per_cluster as u32
    }

    fn read_fat_entry(&mut self, cluster: u32) -> Option<u32> {
        let fat_offset = cluster * 4;
        let fat_sector = fat_offset / self.bytes_per_sector as u32;
        let byte_offset = fat_offset % self.bytes_per_sector as u32;
        let lba = self.fat_start_lba() + fat_sector;
        let sector = self.disk.read_sectors(lba, 1)?;
        let val = u32::from_le_bytes([
            sector[byte_offset as usize], sector[byte_offset as usize + 1],
            sector[byte_offset as usize + 2], sector[byte_offset as usize + 3],
        ]);
        Some(val & 0x0FFFFFFF)
    }

    fn read_cluster(&mut self, cluster: u32) -> Option<Vec<u8>> {
        let lba = self.cluster_to_lba(cluster);
        self.disk.read_sectors(lba, self.sectors_per_cluster)
    }

    fn read_file_clusters(&mut self, first_cluster: u32) -> Option<Vec<u8>> {
        let mut data = Vec::new();
        let mut cluster = first_cluster;
        loop {
            if cluster < 2 || cluster >= 0x0FFFFFF8 { break; }
            let cluster_data = self.read_cluster(cluster)?;
            data.extend_from_slice(&cluster_data);
            cluster = self.read_fat_entry(cluster)?;
        }
        Some(data)
    }

    fn read_directory(&mut self, cluster: u32) -> Option<Vec<FatDirEntry>> {
        let data = self.read_file_clusters(cluster)?;
        let mut entries = Vec::new();
        for chunk in data.chunks(32) {
            if chunk.len() < 32 { break; }
            let first_byte = chunk[0];
            if first_byte == 0x00 { break; }
            if first_byte == 0xE5 { continue; }
            let attributes = chunk[11];
            if attributes == 0x0F || (attributes & 0x08) != 0 { continue; }
            let name = Self::parse_short_name(&chunk[0..11]);
            let cluster_high = u16::from_le_bytes([chunk[20], chunk[21]]);
            let cluster_low = u16::from_le_bytes([chunk[26], chunk[27]]);
            let file_cluster = ((cluster_high as u32) << 16) | cluster_low as u32;
            let file_size = u32::from_le_bytes([chunk[28], chunk[29], chunk[30], chunk[31]]);
            entries.push(FatDirEntry { name, attributes, cluster: file_cluster, size: file_size });
        }
        Some(entries)
    }

    fn parse_short_name(raw: &[u8]) -> String {
        let mut name = String::new();
        let mut i = 0;
        while i < 8 && raw[i] != b' ' { name.push(raw[i] as char); i += 1; }
        let mut has_ext = false;
        let mut j = 8;
        while j < 11 && raw[j] != b' ' { has_ext = true; j += 1; }
        if has_ext {
            name.push('.');
            for k in 8..11 {
                if raw[k] == b' ' { break; }
                name.push(raw[k] as char);
            }
        }
        name
    }

    pub fn read_file(&mut self, path: &str) -> Option<Vec<u8>> {
        let normalized = path.replace('/', "\\");
        let parts: Vec<&str> = normalized.split('\\').filter(|p| !p.is_empty()).collect();
        let mut current_cluster = self.root_cluster;
        for (i, part) in parts.iter().enumerate() {
            let entries = self.read_directory(current_cluster)?;
            let mut found = None;
            for e in &entries {
                if e.name.eq_ignore_ascii_case(part) { found = Some(e); break; }
            }
            match found {
                Some(e) => {
                    if i == parts.len() - 1 {
                        if (e.attributes & 0x10) != 0 { return None; }
                        return self.read_file_clusters(e.cluster);
                    }
                    current_cluster = e.cluster;
                }
                None => return None,
            }
        }
        None
    }

    pub fn read_dir(&mut self, path: &str) -> Option<Vec<(String, Vec<u8>)>> {
        let normalized = path.replace('/', "\\");
        let parts: Vec<&str> = normalized.split('\\').filter(|p| !p.is_empty()).collect();
        let mut current_cluster = self.root_cluster;
        for part in &parts {
            if part.is_empty() { continue; }
            let entries = self.read_directory(current_cluster)?;
            let mut found = None;
            for e in &entries { if e.name.eq_ignore_ascii_case(part) { found = Some(e); break; } }
            match found { Some(e) => current_cluster = e.cluster, None => return None }
        }
        let entries = self.read_directory(current_cluster)?;
        let mut result = Vec::new();
        for e in &entries {
            if (e.attributes & 0x10) != 0 { continue; }
            if let Some(data) = self.read_file_clusters(e.cluster) {
                result.push((e.name.clone(), data));
            }
        }
        Some(result)
    }

    pub fn write_file(&mut self, _path: &str, _data: &[u8]) -> Result<(), ()> { Err(()) }
    pub fn delete_file(&mut self, _path: &str) -> Result<(), ()> { Err(()) }
    pub fn create_dir(&mut self, _path: &str) -> Result<(), ()> { Err(()) }
}

struct FatDirEntry {
    name: String,
    attributes: u8,
    cluster: u32,
    size: u32,
}
