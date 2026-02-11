use core::sync::atomic::{AtomicBool, Ordering};
use x86_64::instructions::port::Port;

const DATA: u16 = 0x1F0;
const SECTOR_COUNT: u16 = 0x1F2;
const LBA_LOW: u16 = 0x1F3;
const LBA_MID: u16 = 0x1F4;
const LBA_HIGH: u16 = 0x1F5;
const DRIVE_HEAD: u16 = 0x1F6;
const STATUS: u16 = 0x1F7;
const COMMAND: u16 = 0x1F7;

const BSY: u8 = 0x80;
const DRQ: u8 = 0x08;
const ERR: u8 = 0x01;

const CMD_READ: u8 = 0x20;
const CMD_WRITE: u8 = 0x30;
const CMD_FLUSH: u8 = 0xE7;
const CMD_IDENTIFY: u8 = 0xEC;

pub const DATA_START_SECTOR: u32 = 2048;

static AVAILABLE: AtomicBool = AtomicBool::new(false);

fn wait_bsy() {
    unsafe {
        let mut port = Port::<u8>::new(STATUS);
        while port.read() & BSY != 0 {}
    }
}

fn wait_drq() -> bool {
    unsafe {
        let mut port = Port::<u8>::new(STATUS);
        loop {
            let s = port.read();
            if s & ERR != 0 { return false; }
            if s & DRQ != 0 { return true; }
        }
    }
}

pub fn init() {
    unsafe {
        Port::<u8>::new(DRIVE_HEAD).write(0xE0);
        Port::<u8>::new(SECTOR_COUNT).write(0);
        Port::<u8>::new(LBA_LOW).write(0);
        Port::<u8>::new(LBA_MID).write(0);
        Port::<u8>::new(LBA_HIGH).write(0);
        Port::<u8>::new(COMMAND).write(CMD_IDENTIFY);

        let status = Port::<u8>::new(STATUS).read();
        if status == 0 {
            return;
        }

        wait_bsy();

        if Port::<u8>::new(LBA_MID).read() != 0 || Port::<u8>::new(LBA_HIGH).read() != 0 {
            return;
        }

        if !wait_drq() {
            return;
        }

        let mut data_port = Port::<u16>::new(DATA);
        for _ in 0..256 {
            data_port.read();
        }

        AVAILABLE.store(true, Ordering::Relaxed);
    }
}

pub fn is_available() -> bool {
    AVAILABLE.load(Ordering::Relaxed)
}

pub fn read_sector(lba: u32, buf: &mut [u8; 512]) -> bool {
    if !is_available() { return false; }
    unsafe {
        wait_bsy();
        Port::<u8>::new(DRIVE_HEAD).write(0xE0 | ((lba >> 24) & 0x0F) as u8);
        Port::<u8>::new(SECTOR_COUNT).write(1);
        Port::<u8>::new(LBA_LOW).write(lba as u8);
        Port::<u8>::new(LBA_MID).write((lba >> 8) as u8);
        Port::<u8>::new(LBA_HIGH).write((lba >> 16) as u8);
        Port::<u8>::new(COMMAND).write(CMD_READ);

        if !wait_drq() { return false; }

        let mut data_port = Port::<u16>::new(DATA);
        for i in 0..256 {
            let word = data_port.read();
            buf[i * 2] = word as u8;
            buf[i * 2 + 1] = (word >> 8) as u8;
        }
        true
    }
}

pub fn write_sector(lba: u32, buf: &[u8; 512]) -> bool {
    if !is_available() { return false; }
    unsafe {
        wait_bsy();
        Port::<u8>::new(DRIVE_HEAD).write(0xE0 | ((lba >> 24) & 0x0F) as u8);
        Port::<u8>::new(SECTOR_COUNT).write(1);
        Port::<u8>::new(LBA_LOW).write(lba as u8);
        Port::<u8>::new(LBA_MID).write((lba >> 8) as u8);
        Port::<u8>::new(LBA_HIGH).write((lba >> 16) as u8);
        Port::<u8>::new(COMMAND).write(CMD_WRITE);

        if !wait_drq() { return false; }

        let mut data_port = Port::<u16>::new(DATA);
        for i in 0..256 {
            let word = (buf[i * 2 + 1] as u16) << 8 | buf[i * 2] as u16;
            data_port.write(word);
        }

        Port::<u8>::new(COMMAND).write(CMD_FLUSH);
        wait_bsy();
        true
    }
}
