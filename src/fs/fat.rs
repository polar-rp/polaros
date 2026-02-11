use alloc::string::String;
use alloc::vec::Vec;
use crate::drivers::ata;

// FAT32 Layout:
// [ Reserved (Boot Sector...) ] [ FAT 1 ] [ FAT 2 ] [ Data Region (Clusters) ]

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct BPB {
    jmp: [u8; 3],
    oem: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fats: u8,
    root_entries: u16,
    total_sectors_16: u16,
    media: u8,
    sectors_per_fat_16: u16,
    sectors_per_track: u16,
    heads: u16,
    hidden_sectors: u32,
    total_sectors_32: u32,
    
    // FAT32 Extended
    sectors_per_fat_32: u32,
    ext_flags: u16,
    fs_ver: u16,
    root_cluster: u32,
    fs_info: u16,
    bk_boot_sec: u16,
    reserved: [u8; 12],
    drive_num: u8,
    reserved2: u8,
    boot_sig: u8,
    vol_id: u32,
    vol_label: [u8; 11],
    fs_type: [u8; 8],
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct DirEntry {
    name: [u8; 8],
    ext: [u8; 3],
    attr: u8,
    reserved: u8,
    creation_ms: u8,
    creation_time: u16,
    creation_date: u16,
    last_access_date: u16,
    cluster_high: u16,
    time: u16,
    date: u16,
    cluster_low: u16,
    size: u32,
}

impl DirEntry {
    fn is_free(&self) -> bool {
        self.name[0] == 0xE5
    }
    fn is_end(&self) -> bool {
        self.name[0] == 0x00
    }
    fn is_long_name(&self) -> bool {
        self.attr == 0x0F
    }
    fn is_dir(&self) -> bool {
        (self.attr & 0x10) != 0
    }
    fn filename(&self) -> String {
        let mut name = String::new();
        for &b in &self.name {
            if b != 0x20 { name.push(b as char); }
        }
        if self.ext[0] != 0x20 {
            name.push('.');
            for &b in &self.ext {
                if b != 0x20 { name.push(b as char); }
            }
        }
        name
    }
    fn cluster(&self) -> u32 {
        ((self.cluster_high as u32) << 16) | (self.cluster_low as u32)
    }
}

// Global cached FS info could be stored here, but for simplicity we read BPB every time or assume LBA 0 (Partition 0) or LBA 2048 (if MBR present).
// Let's assume MBR and Partition 1 is FAT32. Or just try LBA 0.
const PARTITION_OFFSET: u32 = 0; // Try 0 (superfloppy) or 2048?
// Standard MBR often puts first partition at 2048.
// We will try to read LBA 0, check signature. If MBR, read partition table.

pub fn list_root_files() -> Vec<String> {
    let mut files = Vec::new();
    
    // Read Boot Sector
    let mut sector = [0u8; 512];
    if !ata::read_sector(PARTITION_OFFSET, &mut sector) {
        files.push("ATA Read Error".into());
        return files;
    }

    // Basic check for BPB
    // We cast to BPB. 
    // Note: This is unsafe casting of packed struct.
    let bpb = unsafe { &*(sector.as_ptr() as *const BPB) };

    if bpb.bytes_per_sector != 512 {
        // Might be MBR?
        files.push("Not valid FAT32 (invalid sector size)".into());
        return files;
    }
    
    // Calculate offsets
    let fat_start = PARTITION_OFFSET + bpb.reserved_sectors as u32;
    let fat_size = bpb.sectors_per_fat_32;
    let data_start = fat_start + (bpb.fats as u32 * fat_size);
    let root_cluster = bpb.root_cluster;
    
    // Read Root Directory (which is a cluster chain)
    // For MVP, read just the first cluster of Root Dir.
    let root_lba = cluster_to_lba(root_cluster, data_start, bpb.sectors_per_cluster);
    
    if !ata::read_sector(root_lba, &mut sector) {
        files.push("Failed to read Root Dir".into());
        return files;
    }

    // Parse entries
    for i in 0..16 { // 512 / 32 = 16 entries per sector
        let ptr = unsafe { sector.as_ptr().add(i * 32) };
        let entry = unsafe { &*(ptr as *const DirEntry) };

        if entry.is_end() { break; }
        if entry.is_free() || entry.is_long_name() { continue; }
        
        let mut name = entry.filename();
        if entry.is_dir() {
            name.push('/');
        } else {
            let size = entry.size;
            name.push_str(&alloc::format!(" ({} b)", size));
        }
        files.push(name);
    }

    files
}

fn cluster_to_lba(cluster: u32, data_start: u32, sectors_per_cluster: u8) -> u32 {
    data_start + ((cluster - 2) * sectors_per_cluster as u32)
}

pub fn read_file(name: &str) -> Option<Vec<u8>> {
    // Re-read BPB logic (should be cached)
    let mut sector = [0u8; 512];
    ata::read_sector(PARTITION_OFFSET, &mut sector);
    let bpb = unsafe { &*(sector.as_ptr() as *const BPB) };
    
    let fat_start = PARTITION_OFFSET + bpb.reserved_sectors as u32;
    let fat_size = bpb.sectors_per_fat_32;
    let data_start = fat_start + (bpb.fats as u32 * fat_size);
    
    // Find file in root dir
    let root_lba = cluster_to_lba(bpb.root_cluster, data_start, bpb.sectors_per_cluster);
    ata::read_sector(root_lba, &mut sector);

    let mut found_entry: Option<DirEntry> = None;
    
    for i in 0..16 {
        let ptr = unsafe { sector.as_ptr().add(i * 32) };
        let entry = unsafe { &*(ptr as *const DirEntry) };
        if entry.is_end() { break; }
        if entry.is_free() || entry.is_long_name() { continue; }
        
        if entry.filename() == name {
            found_entry = Some(*entry);
            break;
        }
    }

    if let Some(entry) = found_entry {
        // Read file content (single cluster for now)
        let start_cluster = entry.cluster();
        let lba = cluster_to_lba(start_cluster, data_start, bpb.sectors_per_cluster);
        
        let mut data = Vec::with_capacity(entry.size as usize);
        let mut buf = [0u8; 512];
        
        // Read just one sector/cluster for demo
        // Todo: Follow FAT chain
        ata::read_sector(lba, &mut buf);
        data.extend_from_slice(&buf[..entry.size.min(512) as usize]);
        
        return Some(data);
    }

    None
}