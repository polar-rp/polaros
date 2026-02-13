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

const PARTITION_OFFSET: u32 = 0;
const ENTRIES_PER_SECTOR: usize = 16; // 512 / 32

struct FatLayout {
    data_start: u32,
    root_cluster: u32,
    sectors_per_cluster: u8,
}

/// Read and validate the FAT32 BPB, returning computed layout info.
fn read_fat_layout() -> Result<FatLayout, &'static str> {
    let mut sector = [0u8; 512];
    if !ata::read_sector(PARTITION_OFFSET, &mut sector) {
        return Err("ATA Read Error");
    }
    let bpb = unsafe { &*(sector.as_ptr() as *const BPB) };
    if bpb.bytes_per_sector != 512 {
        return Err("Not valid FAT32 (invalid sector size)");
    }
    let fat_start = PARTITION_OFFSET + bpb.reserved_sectors as u32;
    let fat_size = bpb.sectors_per_fat_32;
    let data_start = fat_start + (bpb.fats as u32 * fat_size);
    Ok(FatLayout {
        data_start,
        root_cluster: bpb.root_cluster,
        sectors_per_cluster: bpb.sectors_per_cluster,
    })
}

fn cluster_to_lba(cluster: u32, data_start: u32, sectors_per_cluster: u8) -> u32 {
    data_start + ((cluster - 2) * sectors_per_cluster as u32)
}

/// Iterate directory entries in the first sector of a cluster, calling `f` for each valid entry.
/// Returns `Some(result)` if `f` returns `Some`, otherwise `None`.
fn find_in_root_dir<T>(layout: &FatLayout, f: impl Fn(&DirEntry) -> Option<T>) -> Option<T> {
    let root_lba = cluster_to_lba(layout.root_cluster, layout.data_start, layout.sectors_per_cluster);
    let mut sector = [0u8; 512];
    if !ata::read_sector(root_lba, &mut sector) {
        return None;
    }
    for i in 0..ENTRIES_PER_SECTOR {
        let ptr = unsafe { sector.as_ptr().add(i * 32) };
        let entry = unsafe { &*(ptr as *const DirEntry) };
        if entry.is_end() { break; }
        if entry.is_free() || entry.is_long_name() { continue; }
        if let Some(result) = f(entry) {
            return Some(result);
        }
    }
    None
}

pub fn list_root_files() -> Vec<String> {
    let layout = match read_fat_layout() {
        Ok(l) => l,
        Err(e) => return alloc::vec![String::from(e)],
    };

    let root_lba = cluster_to_lba(layout.root_cluster, layout.data_start, layout.sectors_per_cluster);
    let mut sector = [0u8; 512];
    if !ata::read_sector(root_lba, &mut sector) {
        return alloc::vec![String::from("Failed to read Root Dir")];
    }

    let mut files = Vec::new();
    for i in 0..ENTRIES_PER_SECTOR {
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

pub fn read_file(name: &str) -> Option<Vec<u8>> {
    let layout = read_fat_layout().ok()?;

    let found_entry = find_in_root_dir(&layout, |entry| {
        if entry.filename() == name { Some(*entry) } else { None }
    })?;

    let start_cluster = found_entry.cluster();
    let lba = cluster_to_lba(start_cluster, layout.data_start, layout.sectors_per_cluster);

    let mut data = Vec::with_capacity(found_entry.size as usize);
    let mut buf = [0u8; 512];
    ata::read_sector(lba, &mut buf);
    data.extend_from_slice(&buf[..found_entry.size.min(512) as usize]);

    Some(data)
}