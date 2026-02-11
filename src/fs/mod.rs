pub mod ramfs;

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;
use ramfs::RamFs;

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: usize,
}

pub enum RemoveResult {
    Ok,
    NotFound,
    DirNotEmpty,
}

pub trait FileSystem {
    fn list(&self, path: &[String]) -> Option<Vec<DirEntry>>;
    fn read(&self, path: &[String], name: &str) -> Option<&[u8]>;
    fn write(&mut self, path: &[String], name: &str, data: &[u8]) -> bool;
    fn create(&mut self, path: &[String], name: &str) -> bool;
    fn remove(&mut self, path: &[String], name: &str) -> RemoveResult;
    fn mkdir(&mut self, path: &[String], name: &str) -> bool;
    fn exists(&self, path: &[String], name: &str) -> bool;
    fn is_dir(&self, path: &[String], name: &str) -> bool;
    fn names(&self, path: &[String]) -> Vec<String>;
}

lazy_static::lazy_static! {
    pub static ref FS: Mutex<RamFs> = Mutex::new(RamFs::new());
}

pub fn init() {
    let mut fs = FS.lock();
    fs.write(&[], "readme.txt", b"Witaj w PolarOs v0.1.0!\nTo jest prosty system operacyjny napisany w Rust.\nUzyj 'help' aby zobaczyc dostepne komendy.");
    fs.write(&[], "hello.txt", b"Hello, World!");
    fs.write(&[], "version.txt", b"PolarOs v0.1.0\nArchitektura: x86_64\nJezyk: Rust");
    fs.mkdir(&[], "docs");
    let docs = [String::from("docs")];
    fs.write(&docs, "info.txt", b"Katalog z dokumentacja systemu.");
}
