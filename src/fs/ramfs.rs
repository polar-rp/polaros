use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use super::{DirEntry, RemoveResult, FileSystem};

enum FsEntry {
    File(Vec<u8>),
    Dir(BTreeMap<String, FsEntry>),
}

pub struct RamFs {
    root: BTreeMap<String, FsEntry>,
}

impl RamFs {
    pub fn new() -> Self {
        RamFs {
            root: BTreeMap::new(),
        }
    }

    fn get_dir(&self, path: &[String]) -> Option<&BTreeMap<String, FsEntry>> {
        let mut current = &self.root;
        for component in path {
            match current.get(component.as_str()) {
                Some(FsEntry::Dir(dir)) => current = dir,
                _ => return None,
            }
        }
        Some(current)
    }

    fn get_dir_mut(&mut self, path: &[String]) -> Option<&mut BTreeMap<String, FsEntry>> {
        let mut current = &mut self.root;
        for component in path {
            let next = match current.get_mut(component.as_str()) {
                Some(FsEntry::Dir(dir)) => dir,
                _ => return None,
            };
            current = next;
        }
        Some(current)
    }

    pub fn replace(&mut self, other: RamFs) {
        self.root = other.root;
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        Self::serialize_dir(&self.root, &mut data);
        data
    }

    fn serialize_dir(dir: &BTreeMap<String, FsEntry>, out: &mut Vec<u8>) {
        let count = dir.len() as u16;
        out.extend_from_slice(&count.to_le_bytes());
        for (name, entry) in dir {
            let name_bytes = name.as_bytes();
            let name_len = name_bytes.len().min(255) as u8;
            match entry {
                FsEntry::File(data) => {
                    out.push(0);
                    out.push(name_len);
                    out.extend_from_slice(&name_bytes[..name_len as usize]);
                    let data_len = data.len() as u32;
                    out.extend_from_slice(&data_len.to_le_bytes());
                    out.extend_from_slice(data);
                }
                FsEntry::Dir(children) => {
                    out.push(1);
                    out.push(name_len);
                    out.extend_from_slice(&name_bytes[..name_len as usize]);
                    Self::serialize_dir(children, out);
                }
            }
        }
    }

    pub fn load_from(data: &[u8]) -> Option<Self> {
        let mut pos = 0;
        let root = Self::deserialize_dir(data, &mut pos)?;
        Some(RamFs { root })
    }

    fn deserialize_dir(data: &[u8], pos: &mut usize) -> Option<BTreeMap<String, FsEntry>> {
        if *pos + 2 > data.len() { return None; }
        let count = u16::from_le_bytes([data[*pos], data[*pos + 1]]) as usize;
        *pos += 2;

        let mut dir = BTreeMap::new();
        for _ in 0..count {
            if *pos + 2 > data.len() { return None; }
            let entry_type = data[*pos];
            let name_len = data[*pos + 1] as usize;
            *pos += 2;

            if *pos + name_len > data.len() { return None; }
            let name = String::from(core::str::from_utf8(&data[*pos..*pos + name_len]).ok()?);
            *pos += name_len;

            match entry_type {
                0 => {
                    if *pos + 4 > data.len() { return None; }
                    let data_len = u32::from_le_bytes([
                        data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3],
                    ]) as usize;
                    *pos += 4;
                    if *pos + data_len > data.len() { return None; }
                    let file_data = Vec::from(&data[*pos..*pos + data_len]);
                    *pos += data_len;
                    dir.insert(name, FsEntry::File(file_data));
                }
                1 => {
                    let children = Self::deserialize_dir(data, pos)?;
                    dir.insert(name, FsEntry::Dir(children));
                }
                _ => return None,
            }
        }
        Some(dir)
    }
}

impl FileSystem for RamFs {
    fn list(&self, path: &[String]) -> Option<Vec<DirEntry>> {
        let dir = self.get_dir(path)?;
        let entries = dir.iter().map(|(name, entry)| {
            let (is_dir, size) = match entry {
                FsEntry::File(data) => (false, data.len()),
                FsEntry::Dir(_) => (true, 0),
            };
            DirEntry { name: name.clone(), is_dir, size }
        }).collect();
        Some(entries)
    }

    fn read(&self, path: &[String], name: &str) -> Option<&[u8]> {
        let dir = self.get_dir(path)?;
        match dir.get(name) {
            Some(FsEntry::File(data)) => Some(data.as_slice()),
            _ => None,
        }
    }

    fn write(&mut self, path: &[String], name: &str, data: &[u8]) -> bool {
        let dir = match self.get_dir_mut(path) {
            Some(d) => d,
            None => return false,
        };
        if matches!(dir.get(name), Some(FsEntry::Dir(_))) {
            return false;
        }
        dir.insert(String::from(name), FsEntry::File(Vec::from(data)));
        true
    }

    fn create(&mut self, path: &[String], name: &str) -> bool {
        let dir = match self.get_dir_mut(path) {
            Some(d) => d,
            None => return false,
        };
        if dir.contains_key(name) {
            return false;
        }
        dir.insert(String::from(name), FsEntry::File(Vec::new()));
        true
    }

    fn remove(&mut self, path: &[String], name: &str) -> RemoveResult {
        let dir = match self.get_dir_mut(path) {
            Some(d) => d,
            None => return RemoveResult::NotFound,
        };
        match dir.get(name) {
            None => return RemoveResult::NotFound,
            Some(FsEntry::Dir(contents)) if !contents.is_empty() => return RemoveResult::DirNotEmpty,
            _ => {}
        }
        dir.remove(name);
        RemoveResult::Ok
    }

    fn mkdir(&mut self, path: &[String], name: &str) -> bool {
        let dir = match self.get_dir_mut(path) {
            Some(d) => d,
            None => return false,
        };
        if dir.contains_key(name) {
            return false;
        }
        dir.insert(String::from(name), FsEntry::Dir(BTreeMap::new()));
        true
    }

    fn exists(&self, path: &[String], name: &str) -> bool {
        match self.get_dir(path) {
            Some(dir) => dir.contains_key(name),
            None => false,
        }
    }

    fn is_dir(&self, path: &[String], name: &str) -> bool {
        match self.get_dir(path) {
            Some(dir) => matches!(dir.get(name), Some(FsEntry::Dir(_))),
            None => false,
        }
    }

    fn names(&self, path: &[String]) -> Vec<String> {
        match self.get_dir(path) {
            Some(dir) => dir.keys().cloned().collect(),
            None => Vec::new(),
        }
    }
}
