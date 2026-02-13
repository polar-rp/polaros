src/drivers/ata.rs
```rust
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

```

src/drivers/keyboard.rs
```rust
use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, KeyboardLayout,
                   Keyboard, Modifiers, ScancodeSet1};
use spin::Mutex;
use core::sync::atomic::{AtomicU8, Ordering};

const BUF_SIZE: usize = 128;

// ---- Layout selection ----

/// Available keyboard layouts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LayoutId {
    Us104 = 0,
    Uk105 = 1,
    De105 = 2,
    Azerty = 3,
    Dvorak = 4,
    Colemak = 5,
}

static CURRENT_LAYOUT: AtomicU8 = AtomicU8::new(0); // default: US

pub fn set_layout(layout: LayoutId) {
    CURRENT_LAYOUT.store(layout as u8, Ordering::Relaxed);
}

pub fn current_layout() -> LayoutId {
    match CURRENT_LAYOUT.load(Ordering::Relaxed) {
        0 => LayoutId::Us104,
        1 => LayoutId::Uk105,
        2 => LayoutId::De105,
        3 => LayoutId::Azerty,
        4 => LayoutId::Dvorak,
        5 => LayoutId::Colemak,
        _ => LayoutId::Us104,
    }
}

pub fn layout_name(id: LayoutId) -> &'static str {
    match id {
        LayoutId::Us104 => "us",
        LayoutId::Uk105 => "uk",
        LayoutId::De105 => "de",
        LayoutId::Azerty => "fr",
        LayoutId::Dvorak => "dvorak",
        LayoutId::Colemak => "colemak",
    }
}

pub fn layout_from_name(name: &str) -> Option<LayoutId> {
    match name {
        "us" => Some(LayoutId::Us104),
        "uk" => Some(LayoutId::Uk105),
        "de" => Some(LayoutId::De105),
        "fr" | "azerty" => Some(LayoutId::Azerty),
        "dvorak" => Some(LayoutId::Dvorak),
        "colemak" => Some(LayoutId::Colemak),
        _ => None,
    }
}

/// Dynamic keyboard layout that delegates to the selected layout at runtime.
/// Also fixes the Oem7 bug in US104 layout (scancode 0x2B not mapped).
pub struct PolarLayout;

impl KeyboardLayout for PolarLayout {
    fn map_keycode(
        &self,
        keycode: KeyCode,
        modifiers: &Modifiers,
        handle_ctrl: HandleControl,
    ) -> DecodedKey {
        match current_layout() {
            LayoutId::Us104 => {
                // Fix: Us104Key doesn't handle Oem7 (scancode 0x2B = the \| key
                // on ANSI keyboards). It only handles Oem5 (scancode 0x56, ISO extra key).
                if keycode == KeyCode::Oem7 {
                    if modifiers.is_shifted() {
                        DecodedKey::Unicode('|')
                    } else {
                        DecodedKey::Unicode('\\')
                    }
                } else {
                    layouts::Us104Key.map_keycode(keycode, modifiers, handle_ctrl)
                }
            }
            LayoutId::Uk105 => {
                layouts::Uk105Key.map_keycode(keycode, modifiers, handle_ctrl)
            }
            LayoutId::De105 => {
                layouts::De105Key.map_keycode(keycode, modifiers, handle_ctrl)
            }
            LayoutId::Azerty => {
                layouts::Azerty.map_keycode(keycode, modifiers, handle_ctrl)
            }
            LayoutId::Dvorak => {
                layouts::Dvorak104Key.map_keycode(keycode, modifiers, handle_ctrl)
            }
            LayoutId::Colemak => {
                layouts::Colemak.map_keycode(keycode, modifiers, handle_ctrl)
            }
        }
    }
}

// ---- Scancode buffer ----

struct ScancodeBuffer {
    buf: [u8; BUF_SIZE],
    read_pos: usize,
    write_pos: usize,
    count: usize,
}

impl ScancodeBuffer {
    const fn new() -> Self {
        ScancodeBuffer {
            buf: [0; BUF_SIZE],
            read_pos: 0,
            write_pos: 0,
            count: 0,
        }
    }

    fn push(&mut self, scancode: u8) {
        if self.count < BUF_SIZE {
            self.buf[self.write_pos] = scancode;
            self.write_pos = (self.write_pos + 1) % BUF_SIZE;
            self.count += 1;
        }
    }

    fn pop(&mut self) -> Option<u8> {
        if self.count == 0 {
            None
        } else {
            let scancode = self.buf[self.read_pos];
            self.read_pos = (self.read_pos + 1) % BUF_SIZE;
            self.count -= 1;
            Some(scancode)
        }
    }
}

static SCANCODE_QUEUE: Mutex<ScancodeBuffer> = Mutex::new(ScancodeBuffer::new());

lazy_static::lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard<PolarLayout, ScancodeSet1>> =
        Mutex::new(Keyboard::new(
            ScancodeSet1::new(),
            PolarLayout,
            HandleControl::Ignore,
        ));
}

// ---- Public API ----

pub fn add_scancode(scancode: u8) {
    SCANCODE_QUEUE.lock().push(scancode);
}

pub fn add_scancode_gui(scancode: u8) {
    use crate::gui::event::{Event, KeyCode as GuiKeyCode, EVENT_QUEUE};

    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            let event = match key {
                DecodedKey::Unicode('\n') => Some(Event::KeyPress(GuiKeyCode::Enter)),
                DecodedKey::Unicode('\u{8}') => Some(Event::KeyPress(GuiKeyCode::Backspace)),
                DecodedKey::Unicode('\t') => Some(Event::KeyPress(GuiKeyCode::Tab)),
                DecodedKey::Unicode('\x1B') => Some(Event::KeyPress(GuiKeyCode::Escape)),
                DecodedKey::Unicode(ch) => Some(Event::KeyPress(GuiKeyCode::Char(ch))),
                DecodedKey::RawKey(KeyCode::ArrowUp) => Some(Event::KeyPress(GuiKeyCode::ArrowUp)),
                DecodedKey::RawKey(KeyCode::ArrowDown) => Some(Event::KeyPress(GuiKeyCode::ArrowDown)),
                DecodedKey::RawKey(KeyCode::ArrowLeft) => Some(Event::KeyPress(GuiKeyCode::ArrowLeft)),
                DecodedKey::RawKey(KeyCode::ArrowRight) => Some(Event::KeyPress(GuiKeyCode::ArrowRight)),
                DecodedKey::RawKey(KeyCode::Escape) => Some(Event::KeyPress(GuiKeyCode::Escape)),
                DecodedKey::RawKey(KeyCode::F1) => Some(Event::KeyPress(GuiKeyCode::F1)),
                DecodedKey::RawKey(KeyCode::F2) => Some(Event::KeyPress(GuiKeyCode::F2)),
                DecodedKey::RawKey(KeyCode::F3) => Some(Event::KeyPress(GuiKeyCode::F3)),
                DecodedKey::RawKey(_) => None,
            };
            if let Some(evt) = event {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(evt);
                }
            }
        }
    }
}

pub enum KeyEvent {
    Char(char),
    ArrowUp,
    ArrowDown,
}

pub fn read_key() -> KeyEvent {
    loop {
        let scancode = loop {
            let maybe = SCANCODE_QUEUE.lock().pop();
            if let Some(sc) = maybe {
                break sc;
            }
            x86_64::instructions::hlt();
        };

        let mut keyboard = KEYBOARD.lock();
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(character) => return KeyEvent::Char(character),
                    DecodedKey::RawKey(KeyCode::ArrowUp) => return KeyEvent::ArrowUp,
                    DecodedKey::RawKey(KeyCode::ArrowDown) => return KeyEvent::ArrowDown,
                    DecodedKey::RawKey(_) => {}
                }
            }
        }
    }
}

```

src/drivers/mod.rs
```rust
pub mod vga;
pub mod serial;
pub mod keyboard;
pub mod ata;
pub mod mouse;

```

src/drivers/mouse.rs
```rust
use spin::Mutex;
use x86_64::instructions::port::Port;
use crate::gui::event::{Event, MouseButton, EVENT_QUEUE};
use crate::gui::framebuffer::{SCREEN_WIDTH, SCREEN_HEIGHT};

static MOUSE_STATE: Mutex<MouseState> = Mutex::new(MouseState::new());

struct MouseState {
    cycle: u8,
    bytes: [u8; 3],
    x: i16,
    y: i16,
    prev_buttons: u8,
}

impl MouseState {
    const fn new() -> Self {
        MouseState {
            cycle: 0,
            bytes: [0; 3],
            x: 160,
            y: 100,
            prev_buttons: 0,
        }
    }
}

fn wait_input() {
    for _ in 0..100_000 {
        let mut status_port = Port::<u8>::new(0x64);
        let status: u8 = unsafe { status_port.read() };
        if status & 0x02 == 0 {
            return;
        }
    }
}

fn wait_output() {
    for _ in 0..100_000 {
        let mut status_port = Port::<u8>::new(0x64);
        let status: u8 = unsafe { status_port.read() };
        if status & 0x01 != 0 {
            return;
        }
    }
}

fn command(cmd: u8) {
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(0x64);
        command_port.write(0xD4);
    }
    wait_input();
    unsafe {
        let mut data_port = Port::<u8>::new(0x60);
        data_port.write(cmd);
    }
    wait_output();
    unsafe {
        let mut data_port = Port::<u8>::new(0x60);
        let _ack = data_port.read();
    }
}

pub fn init() {
    // Enable auxiliary device (mouse)
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(0x64);
        command_port.write(0xA8);
    }

    // Enable IRQ12 - read config byte
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(0x64);
        command_port.write(0x20);
    }
    wait_output();
    let mut config: u8;
    unsafe {
        let mut data_port = Port::<u8>::new(0x60);
        config = data_port.read();
    }
    config |= 0x02;
    config &= !0x20;
    wait_input();
    unsafe {
        let mut command_port = Port::<u8>::new(0x64);
        command_port.write(0x60);
    }
    wait_input();
    unsafe {
        let mut data_port = Port::<u8>::new(0x60);
        data_port.write(config);
    }

    command(0xF6);
    command(0xF4);
}

pub fn handle_byte(byte: u8) {
    let mut state = MOUSE_STATE.lock();

    match state.cycle {
        0 => {
            if byte & 0x08 != 0 {
                state.bytes[0] = byte;
                state.cycle = 1;
            }
        }
        1 => {
            state.bytes[1] = byte;
            state.cycle = 2;
        }
        2 => {
            state.bytes[2] = byte;
            state.cycle = 0;

            let flags = state.bytes[0];
            let mut dx = state.bytes[1] as i16;
            let mut dy = state.bytes[2] as i16;

            if flags & 0x10 != 0 { dx -= 256; }
            if flags & 0x20 != 0 { dy -= 256; }
            if flags & 0xC0 != 0 { return; }

            // Y inverted in PS/2
            dy = -dy;

            let old_x = state.x;
            let old_y = state.y;

            state.x = (state.x + dx).max(0).min(SCREEN_WIDTH as i16 - 1);
            state.y = (state.y + dy).max(0).min(SCREEN_HEIGHT as i16 - 1);

            let new_x = state.x;
            let new_y = state.y;

            // Mouse move
            if new_x != old_x || new_y != old_y {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseMove { x: new_x, y: new_y });
                }
            }

            // Button state transitions
            let buttons = flags & 0x07;
            let prev = state.prev_buttons;
            state.prev_buttons = buttons;

            // Left button
            if buttons & 0x01 != 0 && prev & 0x01 == 0 {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseDown { x: new_x, y: new_y, button: MouseButton::Left });
                }
            }
            if buttons & 0x01 == 0 && prev & 0x01 != 0 {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseUp { x: new_x, y: new_y, button: MouseButton::Left });
                }
            }

            // Right button
            if buttons & 0x02 != 0 && prev & 0x02 == 0 {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseDown { x: new_x, y: new_y, button: MouseButton::Right });
                }
            }
            if buttons & 0x02 == 0 && prev & 0x02 != 0 {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.push(Event::MouseUp { x: new_x, y: new_y, button: MouseButton::Right });
                }
            }
        }
        _ => {
            state.cycle = 0;
        }
    }
}

```

src/drivers/serial.rs
```rust
use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::SerialPort;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::drivers::serial::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*
    ));
}

```

src/drivers/vga.rs
```rust
use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;
const TEXT_HEIGHT: usize = BUFFER_HEIGHT - 1; // row 24 reserved for status bar

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = TEXT_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                _ => self.write_byte(0xfe),
            }
        }
    }

    fn new_line(&mut self) {
        for row in 1..TEXT_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(TEXT_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }

    pub fn clear_screen(&mut self) {
        for row in 0..TEXT_HEIGHT {
            self.clear_row(row);
        }
        self.column_position = 0;
    }

    pub fn delete_last_char(&mut self) {
        if self.column_position > 0 {
            self.column_position -= 1;
            let row = TEXT_HEIGHT - 1;
            let col = self.column_position;
            let color_code = self.color_code;
            self.buffer.chars[row][col].write(ScreenChar {
                ascii_character: b' ',
                color_code,
            });
        }
    }

    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.color_code = ColorCode::new(foreground, background);
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::LightGreen, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::drivers::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

pub fn clear_screen() {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().clear_screen();
    });
}

pub fn delete_last_char() {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().delete_last_char();
    });
}

pub fn set_color(foreground: Color, background: Color) {
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().set_color(foreground, background);
    });
}

pub fn update_status_bar(left: &str, right: &str) {
    let vga = 0xb8000 as *mut ScreenChar;
    let row_offset = (BUFFER_HEIGHT - 1) * BUFFER_WIDTH;
    let color = ColorCode::new(Color::White, Color::DarkGray);
    let blank = ScreenChar { ascii_character: b' ', color_code: color };

    unsafe {
        for col in 0..BUFFER_WIDTH {
            core::ptr::write_volatile(vga.add(row_offset + col), blank);
        }
        for (col, byte) in left.bytes().enumerate() {
            if col >= BUFFER_WIDTH { break; }
            core::ptr::write_volatile(vga.add(row_offset + col), ScreenChar {
                ascii_character: byte,
                color_code: color,
            });
        }
        let right_start = BUFFER_WIDTH.saturating_sub(right.len());
        for (i, byte) in right.bytes().enumerate() {
            let col = right_start + i;
            if col >= BUFFER_WIDTH { break; }
            core::ptr::write_volatile(vga.add(row_offset + col), ScreenChar {
                ascii_character: byte,
                color_code: color,
            });
        }
    }
}

```

src/fs/fat.rs
```rust
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
```

src/fs/mod.rs
```rust
pub mod ramfs;
pub mod fat;

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

```

src/fs/ramfs.rs
```rust
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

```

src/gui/compositor.rs
```rust
use alloc::vec::Vec;
use alloc::boxed::Box;
use super::widget::{Widget, EventResponse};
use super::event::{Event, MouseButton};
use super::framebuffer::Framebuffer;
use super::theme::Theme;
use super::cursor::Cursor;

struct DragState {
    widget_id: u32,
    offset_x: i16,
    offset_y: i16,
}

pub struct Compositor {
    layers: Vec<Box<dyn Widget>>,
    focused_id: Option<u32>,
    drag_state: Option<DragState>,
}

impl Compositor {
    pub fn new() -> Self {
        Compositor {
            layers: Vec::new(),
            focused_id: None,
            drag_state: None,
        }
    }

    pub fn add_widget(&mut self, widget: Box<dyn Widget>) {
        self.layers.push(widget);
    }

    pub fn remove_widget(&mut self, id: u32) {
        self.layers.retain(|w| w.id() != id);
        if self.focused_id == Some(id) {
            self.focused_id = None;
        }
        if let Some(ref drag) = self.drag_state {
            if drag.widget_id == id {
                self.drag_state = None;
            }
        }
    }

    fn raise_to_top(&mut self, id: u32) {
        if let Some(pos) = self.layers.iter().position(|w| w.id() == id) {
            if pos < self.layers.len() - 1 {
                let widget = self.layers.remove(pos);
                self.layers.push(widget);
            }
        }
    }

    fn hit_test(&self, x: i16, y: i16) -> Option<u32> {
        for layer in self.layers.iter().rev() {
            if layer.bounds().contains(x, y) {
                return Some(layer.id());
            }
        }
        None
    }

    /// Returns true if a new terminal should be spawned
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MouseDown { x, y, button: MouseButton::Left } => {
                if let Some(hit_id) = self.hit_test(*x, *y) {
                    // Raise and focus
                    if hit_id != 0 {
                        self.raise_to_top(hit_id);
                        self.focused_id = Some(hit_id);
                    }

                    // Check if on title bar for dragging (skip desktop id=0)
                    if hit_id != 0 {
                        let theme = crate::gui::theme::default_dark_theme();
                        if let Some(widget) = self.layers.iter().find(|w| w.id() == hit_id) {
                            let bounds = widget.bounds();
                            let title_bar = super::widget::Rect::new(
                                bounds.x, bounds.y, bounds.w, theme.title_height,
                            );
                            let close_rect = super::widget::Rect::new(
                                bounds.x + bounds.w as i16 - theme.title_height as i16 + 1,
                                bounds.y + 1,
                                theme.title_height - 2,
                                theme.title_height - 2,
                            );
                            if title_bar.contains(*x, *y) && !close_rect.contains(*x, *y) {
                                self.drag_state = Some(DragState {
                                    widget_id: hit_id,
                                    offset_x: *x - bounds.x,
                                    offset_y: *y - bounds.y,
                                });
                            }
                        }
                    }

                    // Forward event to widget
                    let mut close_id = None;
                    let mut open_terminal = false;
                    if let Some(widget) = self.layers.iter_mut().find(|w| w.id() == hit_id) {
                        let resp = widget.handle_event(event);
                        match resp {
                            EventResponse::CloseRequest => { close_id = Some(hit_id); }
                            EventResponse::OpenTerminal => { open_terminal = true; }
                            _ => {}
                        }
                    }
                    if let Some(id) = close_id {
                        self.remove_widget(id);
                    }
                    return open_terminal;
                }
                false
            }
            Event::MouseDown { .. } => { false }
            Event::MouseMove { x, y } => {
                if let Some(ref drag) = self.drag_state {
                    let new_x = *x - drag.offset_x;
                    let new_y = *y - drag.offset_y;
                    let drag_id = drag.widget_id;
                    if let Some(widget) = self.layers.iter_mut().find(|w| w.id() == drag_id) {
                        widget.set_position(new_x, new_y);
                    }
                }
                for layer in self.layers.iter_mut() {
                    layer.handle_event(event);
                }
                false
            }
            Event::MouseUp { .. } => {
                self.drag_state = None;
                for layer in self.layers.iter_mut() {
                    layer.handle_event(event);
                }
                false
            }
            Event::KeyPress(_) => {
                if let Some(fid) = self.focused_id {
                    if let Some(widget) = self.layers.iter_mut().find(|w| w.id() == fid) {
                        widget.handle_event(event);
                    }
                }
                false
            }
            Event::Tick => { false }
        }
    }

    pub fn render(&mut self, fb: &mut Framebuffer, theme: &Theme, cursor: &Cursor) {
        for layer in &mut self.layers {
            layer.render(fb, theme);
        }
        cursor.render(fb);
    }
}

```

src/gui/cursor.rs
```rust
use super::framebuffer::Framebuffer;

// 8x12 arrow cursor bitmap (1 = draw, 0 = transparent)
const CURSOR_WIDTH: i16 = 8;
const CURSOR_HEIGHT: i16 = 12;

#[rustfmt::skip]
const CURSOR_BITMAP: [u8; 12] = [
    0b1000_0000,
    0b1100_0000,
    0b1110_0000,
    0b1111_0000,
    0b1111_1000,
    0b1111_1100,
    0b1111_1110,
    0b1111_0000,
    0b1101_1000,
    0b1000_1100,
    0b0000_0110,
    0b0000_0011,
];

#[rustfmt::skip]
const CURSOR_OUTLINE: [u8; 12] = [
    0b1000_0000,
    0b1100_0000,
    0b1010_0000,
    0b1001_0000,
    0b1000_1000,
    0b1000_0100,
    0b1000_0010,
    0b1001_0000,
    0b1101_1000,
    0b1000_1100,
    0b0000_0110,
    0b0000_0011,
];

pub struct Cursor {
    pub x: i16,
    pub y: i16,
    pub visible: bool,
}

impl Cursor {
    pub fn new(x: i16, y: i16) -> Self {
        Cursor { x, y, visible: true }
    }

    pub fn render(&self, fb: &mut Framebuffer) {
        if !self.visible {
            return;
        }
        for row in 0..CURSOR_HEIGHT {
            let fill_bits = CURSOR_BITMAP[row as usize];
            let outline_bits = CURSOR_OUTLINE[row as usize];
            for col in 0..CURSOR_WIDTH {
                let mask = 0x80u8 >> col;
                let px = self.x + col;
                let py = self.y + row;
                if outline_bits & mask != 0 {
                    // Outline: draw black
                    fb.set_pixel(px, py, 15); // BLACK
                } else if fill_bits & mask != 0 {
                    // Fill: draw white (TEXT_BRIGHT)
                    fb.set_pixel(px, py, 8);
                }
            }
        }
    }
}

```

src/gui/event.rs
```rust
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy)]
pub enum Event {
    KeyPress(KeyCode),
    MouseMove { x: i16, y: i16 },
    MouseDown { x: i16, y: i16, button: MouseButton },
    MouseUp { x: i16, y: i16, button: MouseButton },
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Enter,
    Backspace,
    Tab,
    Escape,
    F1,
    F2,
    F3,
}

const QUEUE_SIZE: usize = 256;

pub struct EventQueue {
    buf: [Option<Event>; QUEUE_SIZE],
    read_pos: usize,
    write_pos: usize,
    count: usize,
}

impl EventQueue {
    pub const fn new() -> Self {
        const NONE: Option<Event> = None;
        EventQueue {
            buf: [NONE; QUEUE_SIZE],
            read_pos: 0,
            write_pos: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, event: Event) {
        if self.count < QUEUE_SIZE {
            self.buf[self.write_pos] = Some(event);
            self.write_pos = (self.write_pos + 1) % QUEUE_SIZE;
            self.count += 1;
        }
    }

    pub fn pop(&mut self) -> Option<Event> {
        if self.count == 0 {
            return None;
        }
        let event = self.buf[self.read_pos].take();
        self.read_pos = (self.read_pos + 1) % QUEUE_SIZE;
        self.count -= 1;
        event
    }
}

pub static EVENT_QUEUE: Mutex<EventQueue> = Mutex::new(EventQueue::new());

```

src/gui/font.rs
```rust
use super::framebuffer::Framebuffer;
use font8x8::legacy::BASIC_LEGACY;

pub const CHAR_WIDTH: u16 = 8;
pub const CHAR_HEIGHT: u16 = 8;

pub fn draw_char(fb: &mut Framebuffer, x: i16, y: i16, ch: char, color: u8) {
    let idx = ch as usize;
    if idx >= BASIC_LEGACY.len() {
        return;
    }
    let glyph = BASIC_LEGACY[idx];
    for (row, &bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            if bits & (1 << col) != 0 {
                fb.set_pixel(x + col as i16, y + row as i16, color);
            }
        }
    }
}

pub fn draw_char_bg(fb: &mut Framebuffer, x: i16, y: i16, ch: char, fg: u8, bg: u8) {
    let idx = ch as usize;
    if idx >= BASIC_LEGACY.len() {
        return;
    }
    let glyph = BASIC_LEGACY[idx];
    for (row, &bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            let color = if bits & (1 << col) != 0 { fg } else { bg };
            fb.set_pixel(x + col as i16, y + row as i16, color);
        }
    }
}

pub fn draw_text(fb: &mut Framebuffer, x: i16, y: i16, text: &str, color: u8) {
    let mut cx = x;
    for ch in text.chars() {
        draw_char(fb, cx, y, ch, color);
        cx += CHAR_WIDTH as i16;
    }
}

pub fn draw_text_bg(fb: &mut Framebuffer, x: i16, y: i16, text: &str, fg: u8, bg: u8) {
    let mut cx = x;
    for ch in text.chars() {
        draw_char_bg(fb, cx, y, ch, fg, bg);
        cx += CHAR_WIDTH as i16;
    }
}

pub fn text_width(text: &str) -> u16 {
    text.len() as u16 * CHAR_WIDTH
}

```

src/gui/framebuffer.rs
```rust
use alloc::boxed::Box;
use x86_64::instructions::port::Port;

pub const SCREEN_WIDTH: u16 = 320;
pub const SCREEN_HEIGHT: u16 = 200;
const FB_SIZE: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize; // 64000

const VGA_FRAMEBUFFER: *mut u8 = 0xA0000 as *mut u8;

/// Standard VGA Mode 13h register values (320x200x256 linear)
/// Reference: FreeVGA, OSDev wiki
const MODE_13H_MISC: u8 = 0x63;

const MODE_13H_SEQ: [u8; 5] = [
    0x03, // Reset
    0x01, // Clocking Mode (8-dot)
    0x0F, // Map Mask (all planes)
    0x00, // Character Map Select
    0x0E, // Sequencer Memory Mode (chain-4)
];

const MODE_13H_CRTC: [u8; 25] = [
    0x5F, // Horizontal Total
    0x4F, // Horizontal Display End
    0x50, // Start Horizontal Blanking
    0x82, // End Horizontal Blanking
    0x54, // Start Horizontal Retrace
    0x80, // End Horizontal Retrace
    0xBF, // Vertical Total
    0x1F, // Overflow
    0x00, // Preset Row Scan
    0x41, // Maximum Scan Line
    0x00, // Cursor Start
    0x00, // Cursor End
    0x00, // Start Address High
    0x00, // Start Address Low
    0x00, // Cursor Location High
    0x00, // Cursor Location Low
    0x9C, // Start Vertical Retrace
    0x0E, // End Vertical Retrace (also unlocks CRTC)
    0x8F, // Vertical Display End
    0x28, // Offset (logical width / 8 = 320/8 = 40 = 0x28)
    0x40, // Underline Location
    0x96, // Start Vertical Blanking
    0xB9, // End Vertical Blanking
    0xA3, // Mode Control
    0xFF, // Line Compare
];

const MODE_13H_GC: [u8; 9] = [
    0x00, // Set/Reset
    0x00, // Enable Set/Reset
    0x00, // Color Compare
    0x00, // Data Rotate
    0x00, // Read Map Select
    0x40, // Graphics Mode (256-color)
    0x05, // Miscellaneous Graphics
    0x0F, // Color Don't Care
    0xFF, // Bit Mask
];

const MODE_13H_AC: [u8; 21] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    0x41, // Attribute Mode Control
    0x00, // Overscan Color
    0x0F, // Color Plane Enable
    0x00, // Horizontal Pixel Panning
    0x00, // Color Select
];

/// Switch VGA hardware to Mode 13h (320x200, 256 colors, linear framebuffer)
/// Must be called before any framebuffer writes.
pub fn set_mode_13h() {
    unsafe {
        // Write Miscellaneous Output Register
        Port::<u8>::new(0x3C2).write(MODE_13H_MISC);

        // Sequencer registers
        for (i, &val) in MODE_13H_SEQ.iter().enumerate() {
            Port::<u8>::new(0x3C4).write(i as u8);
            Port::<u8>::new(0x3C5).write(val);
        }

        // Unlock CRTC (clear protect bit in register 0x11)
        Port::<u8>::new(0x3D4).write(0x11);
        let val = Port::<u8>::new(0x3D5).read();
        Port::<u8>::new(0x3D4).write(0x11);
        Port::<u8>::new(0x3D5).write(val & 0x7F);

        // CRTC registers
        for (i, &val) in MODE_13H_CRTC.iter().enumerate() {
            Port::<u8>::new(0x3D4).write(i as u8);
            Port::<u8>::new(0x3D5).write(val);
        }

        // Graphics Controller registers
        for (i, &val) in MODE_13H_GC.iter().enumerate() {
            Port::<u8>::new(0x3CE).write(i as u8);
            Port::<u8>::new(0x3CF).write(val);
        }

        // Attribute Controller registers
        // Reading 0x3DA resets the AC flip-flop to index mode
        let _ = Port::<u8>::new(0x3DA).read();
        for (i, &val) in MODE_13H_AC.iter().enumerate() {
            Port::<u8>::new(0x3C0).write(i as u8);
            Port::<u8>::new(0x3C0).write(val);
        }
        // Re-enable video output (set bit 5)
        Port::<u8>::new(0x3C0).write(0x20);

        // Clear framebuffer to black
        core::ptr::write_bytes(VGA_FRAMEBUFFER, 0, FB_SIZE);
    }
}

pub struct Framebuffer {
    buffer: Box<[u8; FB_SIZE]>,
}

impl Framebuffer {
    pub fn new() -> Self {
        Framebuffer {
            buffer: Box::new([0u8; FB_SIZE]),
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: i16, y: i16, color: u8) {
        if x >= 0 && x < SCREEN_WIDTH as i16 && y >= 0 && y < SCREEN_HEIGHT as i16 {
            self.buffer[y as usize * SCREEN_WIDTH as usize + x as usize] = color;
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: i16, y: i16) -> u8 {
        if x >= 0 && x < SCREEN_WIDTH as i16 && y >= 0 && y < SCREEN_HEIGHT as i16 {
            self.buffer[y as usize * SCREEN_WIDTH as usize + x as usize]
        } else {
            0
        }
    }

    #[inline]
    pub fn xor_pixel(&mut self, x: i16, y: i16) {
        if x >= 0 && x < SCREEN_WIDTH as i16 && y >= 0 && y < SCREEN_HEIGHT as i16 {
            let idx = y as usize * SCREEN_WIDTH as usize + x as usize;
            self.buffer[idx] ^= 0xFF;
        }
    }

    pub fn clear(&mut self, color: u8) {
        for byte in self.buffer.iter_mut() {
            *byte = color;
        }
    }

    pub fn present(&self) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.buffer.as_ptr(),
                VGA_FRAMEBUFFER,
                FB_SIZE,
            );
        }
    }
}

```

src/gui/mod.rs
```rust
pub mod framebuffer;
pub mod primitives;
pub mod font;
pub mod palette;
pub mod event;
pub mod theme;
pub mod cursor;
pub mod widget;
pub mod compositor;
pub mod wallpaper;

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use alloc::boxed::Box;

use framebuffer::Framebuffer;
use theme::{default_dark_theme, Theme};
use cursor::Cursor;
use compositor::Compositor;
use event::{EVENT_QUEUE, Event};
use widget::desktop::Desktop;
use widget::window::Window;
use widget::terminal::TerminalWidget;

pub static GUI_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);
static NEXT_WIDGET_ID: AtomicU32 = AtomicU32::new(10);

pub fn next_id() -> u32 {
    NEXT_WIDGET_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn run() -> ! {
    crate::serial_println!("[GUI] Initializing...");

    // Switch to VGA Mode 13h (320x200x256)
    framebuffer::set_mode_13h();
    crate::serial_println!("[GUI] VGA Mode 13h set");

    // Load VGA palette
    palette::load_palette();
    crate::serial_println!("[GUI] Palette loaded");

    // Set GUI mode flag - this redirects keyboard events
    GUI_MODE_ACTIVE.store(true, Ordering::SeqCst);

    // Initialize mouse
    crate::drivers::mouse::init();
    crate::serial_println!("[GUI] Mouse initialized");

    let mut fb = Framebuffer::new();
    let theme = default_dark_theme();
    let mut compositor = Compositor::new();
    let mut cursor = Cursor::new(160, 100);

    // Add desktop with wallpaper (always layer 0)
    compositor.add_widget(Box::new(Desktop::new(0, &theme)));
    crate::serial_println!("[GUI] Desktop with wallpaper ready");

    // Add initial terminal window
    spawn_terminal_window(&mut compositor, &theme);

    crate::serial_println!("[GUI] Compositor ready, entering event loop");

    let mut frame_counter: u32 = 0;

    loop {
        // Process all pending events
        let mut need_new_terminal = false;
        loop {
            let event = {
                if let Some(mut queue) = EVENT_QUEUE.try_lock() {
                    queue.pop()
                } else {
                    None
                }
            };
            match event {
                Some(Event::MouseMove { x, y }) => {
                    cursor.x = x;
                    cursor.y = y;
                    compositor.handle_event(&Event::MouseMove { x, y });
                }
                Some(Event::MouseDown { x, y, button }) => {
                    if compositor.handle_event(&Event::MouseDown { x, y, button }) {
                        need_new_terminal = true;
                    }
                }
                Some(Event::MouseUp { x, y, button }) => {
                    compositor.handle_event(&Event::MouseUp { x, y, button });
                }
                Some(Event::KeyPress(key)) => {
                    compositor.handle_event(&Event::KeyPress(key));
                }
                Some(other) => {
                    compositor.handle_event(&other);
                }
                None => break,
            }
        }

        if need_new_terminal {
            spawn_terminal_window(&mut compositor, &theme);
        }

        // Render ~30fps: every 3 ticks at 100Hz timer
        frame_counter += 1;
        if frame_counter >= 3 {
            frame_counter = 0;

            // Send tick for cursor blink etc
            compositor.handle_event(&Event::Tick);

            // Render
            compositor.render(&mut fb, &theme, &cursor);
            fb.present();
        }

        x86_64::instructions::hlt();
    }
}

fn spawn_terminal_window(compositor: &mut Compositor, theme: &Theme) {
    let win_id = next_id();
    let term_id = next_id();

    // Offset each new window slightly
    let offset = ((win_id as i16 - 10) * 15) % 60;
    let x = 20 + offset;
    let y = 10 + offset;

    let mut win = Window::new(win_id, x, y, 280, 160, "Terminal");
    let cr = win.content_rect(theme);
    let terminal = TerminalWidget::new(term_id, cr.x, cr.y, cr.w, cr.h);
    win.set_content(Box::new(terminal));
    compositor.add_widget(Box::new(win));
}

```

src/gui/palette.rs
```rust
use x86_64::instructions::port::Port;

// Core UI colors (indices 0-15)
pub const BG_DARK: u8 = 0;
pub const BG_BASE: u8 = 1;
pub const BG_SURFACE: u8 = 2;
pub const BG_ELEVATED: u8 = 3;
pub const BG_HIGHLIGHT: u8 = 4;
pub const BORDER: u8 = 5;
pub const TEXT_MUTED: u8 = 6;
pub const TEXT_PRIMARY: u8 = 7;
pub const TEXT_BRIGHT: u8 = 8;
pub const ACCENT_BLUE: u8 = 9;
pub const ACCENT_HOVER: u8 = 10;
pub const SUCCESS: u8 = 11;
pub const WARNING: u8 = 12;
pub const ERROR: u8 = 13;
pub const ACCENT_PURPLE: u8 = 14;
pub const BLACK: u8 = 15;

// VGA DAC uses 6-bit color (0-63), so we convert 8-bit (0-255) by shifting right 2
fn rgb(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    (r >> 2, g >> 2, b >> 2)
}

fn set_dac_color(index: u8, r6: u8, g6: u8, b6: u8) {
    unsafe {
        let mut addr_port = Port::<u8>::new(0x3C8);
        let mut data_port = Port::<u8>::new(0x3C9);
        addr_port.write(index);
        data_port.write(r6);
        data_port.write(g6);
        data_port.write(b6);
    }
}

fn set_color(index: u8, r: u8, g: u8, b: u8) {
    let (r6, g6, b6) = rgb(r, g, b);
    set_dac_color(index, r6, g6, b6);
}

fn lerp(a: u8, b: u8, t: u8, steps: u8) -> u8 {
    let a = a as u16;
    let b = b as u16;
    let t = t as u16;
    let steps = steps as u16;
    (a + (b - a) * t / steps) as u8
}

fn set_ramp(start_index: u8, r0: u8, g0: u8, b0: u8, r1: u8, g1: u8, b1: u8, count: u8) {
    for i in 0..count {
        let r = lerp(r0, r1, i, count - 1);
        let g = lerp(g0, g1, i, count - 1);
        let b = lerp(b0, b1, i, count - 1);
        set_color(start_index + i, r, g, b);
    }
}

pub fn load_palette() {
    // Core UI colors (0-15)
    set_color(0,  0x14, 0x14, 0x20); // BG_DARK
    set_color(1,  0x20, 0x20, 0x30); // BG_BASE
    set_color(2,  0x2C, 0x2C, 0x40); // BG_SURFACE
    set_color(3,  0x38, 0x38, 0x50); // BG_ELEVATED
    set_color(4,  0x48, 0x48, 0x60); // BG_HIGHLIGHT
    set_color(5,  0x64, 0x64, 0x78); // BORDER
    set_color(6,  0x80, 0x80, 0x90); // TEXT_MUTED
    set_color(7,  0xC0, 0xC0, 0xC8); // TEXT_PRIMARY
    set_color(8,  0xF0, 0xF0, 0xFC); // TEXT_BRIGHT
    set_color(9,  0x38, 0x58, 0xC8); // ACCENT_BLUE
    set_color(10, 0x50, 0x78, 0xE8); // ACCENT_HOVER
    set_color(11, 0x20, 0xA0, 0x80); // SUCCESS
    set_color(12, 0xDC, 0x8C, 0x20); // WARNING
    set_color(13, 0xC8, 0x30, 0x30); // ERROR
    set_color(14, 0x70, 0x38, 0xC0); // ACCENT_PURPLE
    set_color(15, 0x00, 0x00, 0x00); // BLACK

    // Grayscale ramp (16-31)
    set_ramp(16, 0x08, 0x08, 0x08, 0xF8, 0xF8, 0xF8, 16);

    // Blue ramp (32-47)
    set_ramp(32, 0x08, 0x08, 0x30, 0x60, 0x90, 0xF0, 16);

    // Purple ramp (48-63)
    set_ramp(48, 0x20, 0x08, 0x30, 0xB0, 0x60, 0xF0, 16);

    // Teal ramp (64-79)
    set_ramp(64, 0x08, 0x20, 0x20, 0x40, 0xE0, 0xD0, 16);

    // Green ramp (80-95)
    set_ramp(80, 0x08, 0x20, 0x08, 0x50, 0xE0, 0x50, 16);

    // Orange ramp (96-111)
    set_ramp(96, 0x30, 0x18, 0x08, 0xF0, 0xA0, 0x30, 16);

    // Red ramp (112-127)
    set_ramp(112, 0x30, 0x08, 0x08, 0xF0, 0x40, 0x40, 16);

    // Reserved (128-255) - set to black
    for i in 128u8..=255 {
        set_color(i, 0, 0, 0);
    }
}

```

src/gui/primitives.rs
```rust
use super::framebuffer::Framebuffer;

pub fn fill_rect(fb: &mut Framebuffer, x: i16, y: i16, w: u16, h: u16, color: u8) {
    for dy in 0..h as i16 {
        for dx in 0..w as i16 {
            fb.set_pixel(x + dx, y + dy, color);
        }
    }
}

pub fn draw_rect(fb: &mut Framebuffer, x: i16, y: i16, w: u16, h: u16, color: u8) {
    draw_hline(fb, x, y, w, color);
    draw_hline(fb, x, y + h as i16 - 1, w, color);
    draw_vline(fb, x, y, h, color);
    draw_vline(fb, x + w as i16 - 1, y, h, color);
}

pub fn draw_hline(fb: &mut Framebuffer, x: i16, y: i16, w: u16, color: u8) {
    for dx in 0..w as i16 {
        fb.set_pixel(x + dx, y, color);
    }
}

pub fn draw_vline(fb: &mut Framebuffer, x: i16, y: i16, h: u16, color: u8) {
    for dy in 0..h as i16 {
        fb.set_pixel(x, y + dy, color);
    }
}

pub fn draw_line(fb: &mut Framebuffer, x0: i16, y0: i16, x1: i16, y1: i16, color: u8) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i16 = if x0 < x1 { 1 } else { -1 };
    let sy: i16 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        fb.set_pixel(x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

```

src/gui/theme.rs
```rust
use super::palette;

pub struct Theme {
    pub desktop_bg: u8,
    pub window_bg: u8,
    pub title_bg: u8,
    pub title_text: u8,
    pub button_bg: u8,
    pub button_hover: u8,
    pub button_pressed: u8,
    pub button_text: u8,
    pub text_primary: u8,
    pub text_muted: u8,
    pub text_bright: u8,
    pub border: u8,
    pub close_btn: u8,
    pub taskbar_bg: u8,
    pub taskbar_text: u8,
    pub title_height: u16,
    pub taskbar_height: u16,
    pub button_padding: u16,
    pub border_width: u16,
}

pub fn default_dark_theme() -> Theme {
    Theme {
        desktop_bg: palette::BG_DARK,
        window_bg: palette::BG_BASE,
        title_bg: palette::BG_SURFACE,
        title_text: palette::TEXT_BRIGHT,
        button_bg: palette::ACCENT_BLUE,
        button_hover: palette::ACCENT_HOVER,
        button_pressed: palette::BG_HIGHLIGHT,
        button_text: palette::TEXT_BRIGHT,
        text_primary: palette::TEXT_PRIMARY,
        text_muted: palette::TEXT_MUTED,
        text_bright: palette::TEXT_BRIGHT,
        border: palette::BORDER,
        close_btn: palette::ERROR,
        taskbar_bg: palette::BG_SURFACE,
        taskbar_text: palette::TEXT_PRIMARY,
        title_height: 12,
        taskbar_height: 14,
        button_padding: 4,
        border_width: 1,
    }
}

```

src/gui/wallpaper.rs
```rust
use alloc::vec::Vec;

const PATH1_D: &str = "M568.23,1442.47c-111.71,79.07-246.9,197.83-168.4,348.89-147.13-75.76-196.49-248.1-125.32-394.89,61.31-126.46,224.54-267.71,342.66-341.82l451.92-283.57c107.53-67.47,210.11-132.21,307.9-212.43,122.09-100.15,250.72-257.91,113.92-411.25,104.79,34.44,189.46,106.98,248.3,195.84,82.06,123.93,76.14,276.54-9.3,396.76-49.31,69.38-109.54,124.67-179.42,175.23-102.66,74.28-209.63,135.29-324.97,189.34l-352.45,165.18c-107.41,50.34-208.28,104.35-304.85,172.7Z";

const PATH2_D: &str = "M107.46,1483.1c-148.34-218.21-17.66-441.66,140.01-609.3,92.1-97.93,190.94-182.8,298.19-263.82l172.35-130.21c96.12-72.62,189.19-143.91,276.23-226.91,40.8-38.9,119.78-122.81,87.2-175.22-32.03-51.53-188.62-30.12-243.13-17.17C533.32,132.93,278.27,341.31,146.69,625.45c-30.63,66.15-54.03,130.08-72.21,200.52L5.9,1091.62C-37.76,699.99,164.24,326.37,502.93,132.98,715.67,11.52,966.51-30.18,1206.85,22.17c52.61,11.46,101.51,29.56,143.13,59.52,71.63,51.56,93.93,137.9,57.84,218.16-30.37,67.53-77.68,122.99-134.52,172.42-89.24,77.59-183.09,143.56-283.66,206.86l-328.28,206.61c-109.96,69.21-211.31,143.75-307.71,230.29-128.83,115.66-239.21,284.24-164.63,462.35-37.34-26.51-57.67-60.17-81.55-95.28Z";

const PATH3_D: &str = "M495.53,1582.27c8.16,96.48,127.55,166.89,207.35,197.62,193.88,74.65,406.81,73.11,603.92,5.02,349.82-120.84,588.65-443.5,614.7-812.28,7.93-112.22-2.33-218.31-26.03-328.2-7.52-34.85-16.3-69.48-15.61-104.25,136.12,242.62,155.4,527.88,63.78,787.39-138.34,391.9-512.22,659.4-927.6,669.11-144.9,3.39-282.22-24.41-411.27-86.01-57.64-27.51-107.05-63.61-145.01-114.12-52.98-70.5-34.97-166.8,35.76-214.27Z";

const SVG_W: f32 = 2000.0;
const SVG_H: f32 = 1997.0;
const BEZIER_STEPS: usize = 8;

fn parse_f32(bytes: &[u8]) -> f32 {
    let mut neg = false;
    let mut i = 0;
    if i < bytes.len() && bytes[i] == b'-' { neg = true; i += 1; }
    else if i < bytes.len() && bytes[i] == b'+' { i += 1; }

    let mut result: f32 = 0.0;
    while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
        result = result * 10.0 + (bytes[i] - b'0') as f32;
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let mut frac = 0.1f32;
        while i < bytes.len() && bytes[i] >= b'0' && bytes[i] <= b'9' {
            result += (bytes[i] - b'0') as f32 * frac;
            frac *= 0.1;
            i += 1;
        }
    }
    if neg { -result } else { result }
}

fn next_number(bytes: &[u8], pos: &mut usize) -> Option<f32> {
    while *pos < bytes.len() && (bytes[*pos] == b',' || bytes[*pos] == b' ') {
        *pos += 1;
    }
    if *pos >= bytes.len() { return None; }
    if bytes[*pos].is_ascii_alphabetic() { return None; }

    let start = *pos;
    if bytes[*pos] == b'-' || bytes[*pos] == b'+' {
        *pos += 1;
    }
    while *pos < bytes.len() && bytes[*pos] >= b'0' && bytes[*pos] <= b'9' {
        *pos += 1;
    }
    if *pos < bytes.len() && bytes[*pos] == b'.' {
        *pos += 1;
        while *pos < bytes.len() && bytes[*pos] >= b'0' && bytes[*pos] <= b'9' {
            *pos += 1;
        }
    }
    if *pos == start { return None; }
    Some(parse_f32(&bytes[start..*pos]))
}

fn read2(bytes: &[u8], pos: &mut usize) -> Option<(f32, f32)> {
    let a = next_number(bytes, pos)?;
    let b = next_number(bytes, pos)?;
    Some((a, b))
}

fn read6(bytes: &[u8], pos: &mut usize) -> Option<(f32, f32, f32, f32, f32, f32)> {
    let a = next_number(bytes, pos)?;
    let b = next_number(bytes, pos)?;
    let c = next_number(bytes, pos)?;
    let d = next_number(bytes, pos)?;
    let e = next_number(bytes, pos)?;
    let f = next_number(bytes, pos)?;
    Some((a, b, c, d, e, f))
}

fn cubic_bezier(t: f32, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) -> (f32, f32) {
    let u = 1.0 - t;
    let uu = u * u;
    let uuu = uu * u;
    let tt = t * t;
    let ttt = tt * t;
    (
        uuu * p0.0 + 3.0 * uu * t * p1.0 + 3.0 * u * tt * p2.0 + ttt * p3.0,
        uuu * p0.1 + 3.0 * uu * t * p1.1 + 3.0 * u * tt * p2.1 + ttt * p3.1,
    )
}

fn to_screen(p: (f32, f32), scale: f32, ox: f32, oy: f32) -> (i16, i16) {
    ((p.0 * scale + ox) as i16, (p.1 * scale + oy) as i16)
}

fn sample_bezier(
    verts: &mut Vec<(i16, i16)>,
    p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32),
    scale: f32, ox: f32, oy: f32,
) {
    for i in 1..=BEZIER_STEPS {
        let t = i as f32 / BEZIER_STEPS as f32;
        let p = cubic_bezier(t, p0, p1, p2, p3);
        verts.push(to_screen(p, scale, ox, oy));
    }
}

fn parse_path_to_polygon(d: &str, scale: f32, ox: f32, oy: f32) -> Vec<(i16, i16)> {
    let bytes = d.as_bytes();
    let mut pos = 0;
    let mut cursor = (0.0f32, 0.0f32);
    let mut _path_start = cursor;
    let mut verts: Vec<(i16, i16)> = Vec::new();
    let mut cmd = b'M';

    while pos < bytes.len() {
        let ch = bytes[pos];
        if ch.is_ascii_alphabetic() {
            cmd = ch;
            pos += 1;
            if cmd == b'Z' || cmd == b'z' {
                break;
            }
            continue;
        }

        match cmd {
            b'M' => {
                if let Some((x, y)) = read2(bytes, &mut pos) {
                    cursor = (x, y);
                    _path_start = cursor;
                    verts.push(to_screen(cursor, scale, ox, oy));
                    cmd = b'L';
                } else { break; }
            }
            b'm' => {
                if let Some((dx, dy)) = read2(bytes, &mut pos) {
                    cursor.0 += dx;
                    cursor.1 += dy;
                    _path_start = cursor;
                    verts.push(to_screen(cursor, scale, ox, oy));
                    cmd = b'l';
                } else { break; }
            }
            b'c' => {
                if let Some((dx1, dy1, dx2, dy2, dx, dy)) = read6(bytes, &mut pos) {
                    let cp1 = (cursor.0 + dx1, cursor.1 + dy1);
                    let cp2 = (cursor.0 + dx2, cursor.1 + dy2);
                    let end = (cursor.0 + dx, cursor.1 + dy);
                    sample_bezier(&mut verts, cursor, cp1, cp2, end, scale, ox, oy);
                    cursor = end;
                } else { break; }
            }
            b'C' => {
                if let Some((x1, y1, x2, y2, x, y)) = read6(bytes, &mut pos) {
                    sample_bezier(&mut verts, cursor, (x1, y1), (x2, y2), (x, y), scale, ox, oy);
                    cursor = (x, y);
                } else { break; }
            }
            b'l' => {
                if let Some((dx, dy)) = read2(bytes, &mut pos) {
                    cursor.0 += dx;
                    cursor.1 += dy;
                    verts.push(to_screen(cursor, scale, ox, oy));
                } else { break; }
            }
            b'L' => {
                if let Some((x, y)) = read2(bytes, &mut pos) {
                    cursor = (x, y);
                    verts.push(to_screen(cursor, scale, ox, oy));
                } else { break; }
            }
            _ => { pos += 1; }
        }
    }

    verts
}

fn fill_polygon(buffer: &mut [u8], width: usize, height: usize, vertices: &[(i16, i16)], color: u8) {
    if vertices.len() < 3 { return; }

    let min_y = vertices.iter().map(|v| v.1).min().unwrap().max(0);
    let max_y = vertices.iter().map(|v| v.1).max().unwrap().min(height as i16 - 1);

    let n = vertices.len();

    for y in min_y..=max_y {
        let mut intersections = [0i16; 64];
        let mut n_ix = 0usize;

        for i in 0..n {
            let (x0, y0) = vertices[i];
            let (x1, y1) = vertices[(i + 1) % n];

            if y0 == y1 { continue; }

            if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                let x = x0 as i32
                    + (y as i32 - y0 as i32) * (x1 as i32 - x0 as i32)
                        / (y1 as i32 - y0 as i32);
                if n_ix < 64 {
                    intersections[n_ix] = x as i16;
                    n_ix += 1;
                }
            }
        }

        // Insertion sort
        for i in 1..n_ix {
            let key = intersections[i];
            let mut j = i;
            while j > 0 && intersections[j - 1] > key {
                intersections[j] = intersections[j - 1];
                j -= 1;
            }
            intersections[j] = key;
        }

        // Fill between pairs
        let mut i = 0;
        while i + 1 < n_ix {
            let x_start = intersections[i].max(0) as usize;
            let x_end = (intersections[i + 1].max(0) as usize).min(width - 1);
            for x in x_start..=x_end {
                buffer[y as usize * width + x] = color;
            }
            i += 2;
        }
    }
}

pub fn render_wallpaper(width: u16, height: u16, bg_color: u8, logo_color: u8) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;

    let mut buffer = Vec::with_capacity(w * h);
    buffer.resize(w * h, bg_color);

    // Scale SVG to fit screen area
    let scale_x = width as f32 / SVG_W;
    let scale_y = height as f32 / SVG_H;
    let scale = if scale_x < scale_y { scale_x } else { scale_y };
    let ox = (width as f32 - SVG_W * scale) / 2.0;
    let oy = (height as f32 - SVG_H * scale) / 2.0;

    for path_d in &[PATH1_D, PATH2_D, PATH3_D] {
        let polygon = parse_path_to_polygon(path_d, scale, ox, oy);
        fill_polygon(&mut buffer, w, h, &polygon, logo_color);
    }

    buffer
}

```

src/gui/widget/button.rs
```rust
use alloc::string::String;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::{Event, MouseButton};
use crate::gui::framebuffer::Framebuffer;
use crate::gui::theme::Theme;
use crate::gui::primitives::fill_rect;
use crate::gui::font::{draw_text, text_width};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonState {
    Normal,
    Hovered,
    Pressed,
}

pub struct Button {
    id: u32,
    x: i16,
    y: i16,
    w: u16,
    h: u16,
    label: String,
    state: ButtonState,
    pub on_click: Option<fn()>,
}

impl Button {
    pub fn new(id: u32, x: i16, y: i16, w: u16, h: u16, label: &str) -> Self {
        Button {
            id,
            x, y, w, h,
            label: String::from(label),
            state: ButtonState::Normal,
            on_click: None,
        }
    }
}

impl Widget for Button {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    fn set_position(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        let bg = match self.state {
            ButtonState::Normal => theme.button_bg,
            ButtonState::Hovered => theme.button_hover,
            ButtonState::Pressed => theme.button_pressed,
        };
        fill_rect(fb, self.x, self.y, self.w, self.h, bg);

        // Center label
        let tw = text_width(&self.label);
        let tx = self.x + (self.w as i16 - tw as i16) / 2;
        let ty = self.y + (self.h as i16 - 8) / 2;
        draw_text(fb, tx, ty, &self.label, theme.button_text);
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseMove { x, y } => {
                if self.bounds().contains(*x, *y) {
                    if self.state != ButtonState::Pressed {
                        self.state = ButtonState::Hovered;
                    }
                } else {
                    self.state = ButtonState::Normal;
                }
                EventResponse::Ignored
            }
            Event::MouseDown { x, y, button: MouseButton::Left } => {
                if self.bounds().contains(*x, *y) {
                    self.state = ButtonState::Pressed;
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            Event::MouseUp { x, y, button: MouseButton::Left } => {
                if self.state == ButtonState::Pressed && self.bounds().contains(*x, *y) {
                    self.state = ButtonState::Hovered;
                    if let Some(callback) = self.on_click {
                        callback();
                    }
                    EventResponse::Consumed
                } else {
                    self.state = ButtonState::Normal;
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn focusable(&self) -> bool { true }
}

```

src/gui/widget/desktop.rs
```rust
use alloc::vec::Vec;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::{Event, MouseButton};
use crate::gui::framebuffer::{Framebuffer, SCREEN_WIDTH, SCREEN_HEIGHT};
use crate::gui::theme::Theme;
use crate::gui::primitives::fill_rect;
use crate::gui::font::{draw_text_bg, draw_text};
use crate::gui::wallpaper;
use crate::kernel::timer;

const ICON_X: i16 = 8;
const ICON_Y: i16 = 8;
const ICON_W: u16 = 32;
const ICON_H: u16 = 28;

pub struct Desktop {
    id: u32,
    wallpaper: Vec<u8>,
    wallpaper_w: u16,
    wallpaper_h: u16,
}

impl Desktop {
    pub fn new(id: u32, theme: &Theme) -> Self {
        let wp_h = SCREEN_HEIGHT - theme.taskbar_height;
        let bg_color = 17u8;   // dark gray
        let logo_color = 20u8; // slightly lighter gray
        let wp = wallpaper::render_wallpaper(SCREEN_WIDTH, wp_h, bg_color, logo_color);
        Desktop {
            id,
            wallpaper: wp,
            wallpaper_w: SCREEN_WIDTH,
            wallpaper_h: wp_h,
        }
    }

    fn icon_rect(&self) -> Rect {
        Rect::new(ICON_X, ICON_Y, ICON_W, ICON_H)
    }
}

impl Widget for Desktop {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(0, 0, SCREEN_WIDTH, SCREEN_HEIGHT)
    }

    fn set_position(&mut self, _x: i16, _y: i16) {}

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        // Blit pre-rendered wallpaper
        let w = self.wallpaper_w as usize;
        let h = self.wallpaper_h as usize;
        for y in 0..h {
            for x in 0..w {
                let color = self.wallpaper[y * w + x];
                fb.set_pixel(x as i16, y as i16, color);
            }
        }

        // Terminal icon
        let ir = self.icon_rect();
        fill_rect(fb, ir.x, ir.y, ir.w, ir.h, theme.title_bg);
        // Icon border
        crate::gui::primitives::draw_rect(fb, ir.x, ir.y, ir.w, ir.h, theme.border);
        // ">_" text
        draw_text(fb, ir.x + 4, ir.y + 4, ">_", theme.text_bright);
        // Label under icon
        draw_text(fb, ir.x - 4, ir.y + ir.h as i16 + 2, "Term", theme.text_primary);

        // Taskbar
        let tb_y = SCREEN_HEIGHT as i16 - theme.taskbar_height as i16;
        fill_rect(fb, 0, tb_y, SCREEN_WIDTH, theme.taskbar_height, theme.taskbar_bg);

        // "PolarOs" on the left
        draw_text_bg(fb, 4, tb_y + 3, "PolarOs", theme.text_bright, theme.taskbar_bg);

        // Clock on the right
        let ticks = timer::ticks();
        let secs = ticks / timer::TIMER_HZ as u64;
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;

        let mut buf = [0u8; 16];
        let time_str = format_time(&mut buf, h, m, s);
        let tw = time_str.len() as i16 * 8;
        draw_text_bg(fb, SCREEN_WIDTH as i16 - tw - 4, tb_y + 3, time_str, theme.taskbar_text, theme.taskbar_bg);
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseDown { x, y, button: MouseButton::Left } => {
                if self.icon_rect().contains(*x, *y) {
                    return EventResponse::OpenTerminal;
                }
                EventResponse::Ignored
            }
            _ => EventResponse::Ignored,
        }
    }
}

fn format_time<'a>(buf: &'a mut [u8; 16], h: u64, m: u64, s: u64) -> &'a str {
    use core::fmt::Write;
    struct BufWriter<'b> {
        buf: &'b mut [u8],
        pos: usize,
    }
    impl<'b> core::fmt::Write for BufWriter<'b> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for &byte in s.as_bytes() {
                if self.pos < self.buf.len() {
                    self.buf[self.pos] = byte;
                    self.pos += 1;
                }
            }
            Ok(())
        }
    }
    let mut w = BufWriter { buf, pos: 0 };
    let _ = write!(w, "{}:{:02}:{:02}", h, m, s);
    let len = w.pos;
    core::str::from_utf8(&buf[..len]).unwrap_or("")
}

```

src/gui/widget/label.rs
```rust
use alloc::string::String;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::Event;
use crate::gui::framebuffer::Framebuffer;
use crate::gui::theme::Theme;
use crate::gui::font::{draw_text, text_width, CHAR_HEIGHT};

pub struct Label {
    id: u32,
    x: i16,
    y: i16,
    text: String,
    color: Option<u8>,
}

impl Label {
    pub fn new(id: u32, x: i16, y: i16, text: &str) -> Self {
        Label {
            id,
            x, y,
            text: String::from(text),
            color: None,
        }
    }

    pub fn with_color(mut self, color: u8) -> Self {
        self.color = Some(color);
        self
    }
}

impl Widget for Label {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, text_width(&self.text), CHAR_HEIGHT)
    }

    fn set_position(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        let color = self.color.unwrap_or(theme.text_primary);
        draw_text(fb, self.x, self.y, &self.text, color);
    }

    fn handle_event(&mut self, _event: &Event) -> EventResponse {
        EventResponse::Ignored
    }
}

```

src/gui/widget/mod.rs
```rust
pub mod desktop;
pub mod window;
pub mod button;
pub mod label;
pub mod panel;
pub mod terminal;

use super::event::Event;
use super::framebuffer::Framebuffer;
use super::theme::Theme;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub fn new(x: i16, y: i16, w: u16, h: u16) -> Self {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, px: i16, py: i16) -> bool {
        px >= self.x && px < self.x + self.w as i16
            && py >= self.y && py < self.y + self.h as i16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResponse {
    Consumed,
    Ignored,
    CloseRequest,
    OpenTerminal,
}

pub trait Widget {
    fn id(&self) -> u32;
    fn bounds(&self) -> Rect;
    fn set_position(&mut self, x: i16, y: i16);
    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme);
    fn handle_event(&mut self, event: &Event) -> EventResponse;
    fn focusable(&self) -> bool { false }
    fn is_dirty(&self) -> bool { true }
    fn clear_dirty(&mut self) {}
    fn as_terminal(&mut self) -> Option<&mut terminal::TerminalWidget> { None }
}

```

src/gui/widget/panel.rs
```rust
use alloc::vec::Vec;
use alloc::boxed::Box;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::Event;
use crate::gui::framebuffer::Framebuffer;
use crate::gui::theme::Theme;
use crate::gui::primitives::{fill_rect, draw_rect};

pub struct Panel {
    id: u32,
    x: i16,
    y: i16,
    w: u16,
    h: u16,
    bg_color: Option<u8>,
    has_border: bool,
    children: Vec<Box<dyn Widget>>,
}

impl Panel {
    pub fn new(id: u32, x: i16, y: i16, w: u16, h: u16) -> Self {
        Panel {
            id,
            x, y, w, h,
            bg_color: None,
            has_border: false,
            children: Vec::new(),
        }
    }

    pub fn with_bg(mut self, color: u8) -> Self {
        self.bg_color = Some(color);
        self
    }

    pub fn with_border(mut self) -> Self {
        self.has_border = true;
        self
    }

    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }
}

impl Widget for Panel {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    fn set_position(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        if let Some(bg) = self.bg_color {
            fill_rect(fb, self.x, self.y, self.w, self.h, bg);
        }
        if self.has_border {
            draw_rect(fb, self.x, self.y, self.w, self.h, theme.border);
        }
        for child in &mut self.children {
            child.render(fb, theme);
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        for child in self.children.iter_mut().rev() {
            let resp = child.handle_event(event);
            if resp != EventResponse::Ignored {
                return resp;
            }
        }
        EventResponse::Ignored
    }
}

```

src/gui/widget/terminal.rs
```rust
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::{Event, KeyCode};
use crate::gui::framebuffer::Framebuffer;
use crate::gui::theme::Theme;
use crate::gui::font::{draw_char_bg, CHAR_WIDTH, CHAR_HEIGHT};
use crate::gui::primitives::fill_rect;

const MAX_SCROLLBACK: usize = 100;
const MAX_INPUT: usize = 256;

pub struct TerminalWidget {
    id: u32,
    x: i16,
    y: i16,
    pub w: u16,
    pub h: u16,
    lines: Vec<String>,
    input_buf: String,
    cursor_visible: bool,
    cursor_blink_tick: u32,
    cwd: Vec<String>,
}

impl TerminalWidget {
    pub fn new(id: u32, x: i16, y: i16, w: u16, h: u16) -> Self {
        let mut term = TerminalWidget {
            id,
            x, y, w, h,
            lines: Vec::new(),
            input_buf: String::new(),
            cursor_visible: true,
            cursor_blink_tick: 0,
            cwd: Vec::new(),
        };
        term.push_line("PolarOs Terminal v0.1.0");
        term.push_line("Wpisz 'help' aby zobaczyc komendy.");
        term.push_line("");
        term
    }

    pub fn push_line(&mut self, text: &str) {
        // Wrap long lines
        let cols = self.cols();
        if cols == 0 { return; }
        let mut remaining = text;
        loop {
            if remaining.len() <= cols {
                self.lines.push(String::from(remaining));
                break;
            }
            let (left, right) = remaining.split_at(cols);
            self.lines.push(String::from(left));
            remaining = right;
        }
        while self.lines.len() > MAX_SCROLLBACK {
            self.lines.remove(0);
        }
    }

    fn cols(&self) -> usize {
        self.w as usize / CHAR_WIDTH as usize
    }

    fn visible_rows(&self) -> usize {
        (self.h as usize / CHAR_HEIGHT as usize).saturating_sub(1) // -1 for input line
    }

    fn prompt(&self) -> String {
        if self.cwd.is_empty() {
            String::from("/> ")
        } else {
            let mut p = String::new();
            for c in &self.cwd {
                p.push('/');
                p.push_str(c);
            }
            p.push_str("> ");
            p
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.input_buf.clone();
        let prompt = self.prompt();
        self.push_line(&format!("{}{}", prompt, cmd));
        self.input_buf.clear();

        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return;
        }

        // Expand environment variables
        let expanded = crate::shell::commands::expand_env_vars(trimmed);
        let trimmed = expanded.trim();

        // Parse redirections
        let (pipeline_str, redir_out, redir_append, redir_in) = self.parse_redirections(trimmed);

        // Handle input redirection
        let mut pipe_data: Option<String> = None;
        if let Some(ref input_file) = redir_in {
            use crate::fs::{FS, FileSystem};
            let fs = FS.lock();
            match fs.read(&self.cwd, input_file) {
                Some(data) => {
                    pipe_data = Some(String::from(core::str::from_utf8(data).unwrap_or("")));
                }
                None => {
                    self.push_line(&format!("Plik '{}' nie istnieje.", input_file));
                    return;
                }
            }
        }

        // Split on pipe and chain commands
        for part in pipeline_str.split('|') {
            let part = part.trim();
            if part.is_empty() { continue; }

            let (cmd, args) = match part.split_once(' ') {
                Some((c, a)) => (c, a),
                None => (part, ""),
            };

            // Handle clear specially in GUI
            if cmd == "clear" {
                self.lines.clear();
                pipe_data = Some(String::new());
                continue;
            }

            let output = crate::shell::commands::run_command(
                cmd, args, &mut self.cwd, pipe_data.as_deref()
            );
            pipe_data = Some(output);
        }

        // Handle output
        if let Some(output) = pipe_data {
            if let Some(ref filename) = redir_out {
                use crate::fs::{FS, FileSystem};
                let mut fs = FS.lock();
                if fs.write(&self.cwd, filename, output.as_bytes()) {
                    self.push_line(&format!("Zapisano do '{}'.", filename));
                } else {
                    self.push_line(&format!("Nie mozna zapisac do '{}'.", filename));
                }
            } else if let Some(ref filename) = redir_append {
                use crate::fs::{FS, FileSystem};
                let mut fs = FS.lock();
                let mut existing = match fs.read(&self.cwd, filename) {
                    Some(data) => Vec::from(data),
                    None => Vec::new(),
                };
                if !existing.is_empty() && existing.last() != Some(&b'\n') {
                    existing.push(b'\n');
                }
                existing.extend_from_slice(output.as_bytes());
                if fs.write(&self.cwd, filename, &existing) {
                    self.push_line(&format!("Dopisano do '{}'.", filename));
                } else {
                    self.push_line(&format!("Nie mozna dopisac do '{}'.", filename));
                }
            } else if !output.is_empty() {
                for line in output.lines() {
                    self.push_line(line);
                }
            }
        }
    }

    fn parse_redirections<'a>(&self, line: &'a str) -> (&'a str, Option<String>, Option<String>, Option<String>) {
        // Returns (pipeline_str, write_file, append_file, input_file)
        let mut write_file = None;
        let mut append_file = None;
        let mut input_file = None;
        let mut pipeline_end = line.len();

        // Check for >>
        if let Some(pos) = line.rfind(">>") {
            let filename = line[pos + 2..].trim();
            if !filename.is_empty() && !filename.contains('|') {
                append_file = Some(String::from(filename));
                pipeline_end = pos;
            }
        } else if let Some(pos) = line.rfind('>') {
            let filename = line[pos + 1..].trim();
            if !filename.is_empty() && !filename.contains('|') {
                write_file = Some(String::from(filename));
                pipeline_end = pos;
            }
        }

        let remaining = &line[..pipeline_end];

        // Check for <
        if let Some(pos) = remaining.rfind('<') {
            let filename = remaining[pos + 1..].trim();
            if !filename.is_empty() {
                input_file = Some(String::from(filename));
                return (&remaining[..pos], write_file, append_file, input_file);
            }
        }

        (remaining, write_file, append_file, input_file)
    }
}

impl Widget for TerminalWidget {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    fn set_position(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        let bg = theme.window_bg;
        let fg = theme.text_primary;
        let prompt_color = 10; // ACCENT_HOVER = cyan-ish

        let cols = self.cols();
        let vis_rows = self.visible_rows();

        // Draw scrollback lines
        let start = if self.lines.len() > vis_rows {
            self.lines.len() - vis_rows
        } else {
            0
        };
        let visible_lines = &self.lines[start..];

        for (row, line) in visible_lines.iter().enumerate() {
            let py = self.y + row as i16 * CHAR_HEIGHT as i16;
            let mut px = self.x;
            for ch in line.chars().take(cols) {
                draw_char_bg(fb, px, py, ch, fg, bg);
                px += CHAR_WIDTH as i16;
            }
        }

        // Draw input line at bottom
        let input_y = self.y + vis_rows as i16 * CHAR_HEIGHT as i16;
        let prompt = self.prompt();
        let mut px = self.x;

        // Draw prompt
        for ch in prompt.chars() {
            draw_char_bg(fb, px, input_y, ch, prompt_color, bg);
            px += CHAR_WIDTH as i16;
        }

        // Draw input text
        for ch in self.input_buf.chars() {
            if px < self.x + self.w as i16 {
                draw_char_bg(fb, px, input_y, ch, fg, bg);
                px += CHAR_WIDTH as i16;
            }
        }

        // Draw cursor
        if self.cursor_visible {
            if px < self.x + self.w as i16 {
                fill_rect(fb, px, input_y, CHAR_WIDTH, CHAR_HEIGHT, fg);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::KeyPress(key) => {
                match key {
                    KeyCode::Char(ch) => {
                        if *ch >= ' ' && self.input_buf.len() < MAX_INPUT {
                            self.input_buf.push(*ch);
                        }
                        EventResponse::Consumed
                    }
                    KeyCode::Enter => {
                        self.execute_command();
                        EventResponse::Consumed
                    }
                    KeyCode::Backspace => {
                        self.input_buf.pop();
                        EventResponse::Consumed
                    }
                    _ => EventResponse::Ignored,
                }
            }
            Event::Tick => {
                self.cursor_blink_tick += 1;
                if self.cursor_blink_tick >= 25 {
                    self.cursor_blink_tick = 0;
                    self.cursor_visible = !self.cursor_visible;
                }
                EventResponse::Ignored
            }
            _ => EventResponse::Ignored,
        }
    }

    fn focusable(&self) -> bool { true }

    fn as_terminal(&mut self) -> Option<&mut TerminalWidget> {
        Some(self)
    }
}

```

src/gui/widget/window.rs
```rust
use alloc::string::String;
use alloc::boxed::Box;
use super::{Widget, Rect, EventResponse};
use crate::gui::event::{Event, MouseButton};
use crate::gui::framebuffer::Framebuffer;
use crate::gui::theme::Theme;
use crate::gui::primitives::{fill_rect, draw_rect};
use crate::gui::font::draw_text;

pub struct Window {
    id: u32,
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
    title: String,
    pub content: Option<Box<dyn Widget>>,
}

impl Window {
    pub fn new(id: u32, x: i16, y: i16, w: u16, h: u16, title: &str) -> Self {
        Window {
            id,
            x,
            y,
            w,
            h,
            title: String::from(title),
            content: None,
        }
    }

    pub fn set_content(&mut self, widget: Box<dyn Widget>) {
        self.content = Some(widget);
    }

    pub fn title_bar_rect(&self, theme: &Theme) -> Rect {
        Rect::new(self.x, self.y, self.w, theme.title_height)
    }

    pub fn close_button_rect(&self, theme: &Theme) -> Rect {
        let size = theme.title_height - 2;
        Rect::new(
            self.x + self.w as i16 - size as i16 - 1,
            self.y + 1,
            size,
            size,
        )
    }

    pub fn content_rect(&self, theme: &Theme) -> Rect {
        Rect::new(
            self.x + 1,
            self.y + theme.title_height as i16,
            self.w - 2,
            self.h - theme.title_height - 1,
        )
    }
}

impl Widget for Window {
    fn id(&self) -> u32 { self.id }

    fn bounds(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    fn set_position(&mut self, x: i16, y: i16) {
        self.x = x;
        self.y = y;
    }

    fn render(&mut self, fb: &mut Framebuffer, theme: &Theme) {
        // Window border
        draw_rect(fb, self.x, self.y, self.w, self.h, theme.border);

        // Title bar
        fill_rect(fb, self.x + 1, self.y + 1, self.w - 2, theme.title_height - 1, theme.title_bg);

        // Title text
        draw_text(fb, self.x + 4, self.y + 2, &self.title, theme.title_text);

        // Close button
        let cb = self.close_button_rect(theme);
        fill_rect(fb, cb.x, cb.y, cb.w, cb.h, theme.close_btn);
        // Draw X
        draw_text(fb, cb.x + 1, cb.y + 1, "x", theme.text_bright);

        // Content area background
        let cr = self.content_rect(theme);
        fill_rect(fb, cr.x, cr.y, cr.w, cr.h, theme.window_bg);

        // Render content widget (reposition to content area)
        if let Some(ref mut content) = self.content {
            content.set_position(cr.x, cr.y);
            // Update terminal size to match content area
            if let Some(term) = content.as_terminal() {
                term.w = cr.w;
                term.h = cr.h;
            }
            content.render(fb, theme);
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResponse {
        match event {
            Event::MouseDown { x, y, button: MouseButton::Left } => {
                let theme = crate::gui::theme::default_dark_theme();
                let cb = self.close_button_rect(&theme);
                if cb.contains(*x, *y) {
                    return EventResponse::CloseRequest;
                }
                if self.bounds().contains(*x, *y) {
                    // Forward to content
                    if let Some(ref mut content) = self.content {
                        let resp = content.handle_event(event);
                        if resp != EventResponse::Ignored {
                            return resp;
                        }
                    }
                    return EventResponse::Consumed;
                }
                EventResponse::Ignored
            }
            Event::KeyPress(_) => {
                if let Some(ref mut content) = self.content {
                    return content.handle_event(event);
                }
                EventResponse::Ignored
            }
            _ => {
                if let Some(ref mut content) = self.content {
                    let resp = content.handle_event(event);
                    if resp != EventResponse::Ignored {
                        return resp;
                    }
                }
                EventResponse::Ignored
            }
        }
    }

    fn focusable(&self) -> bool { true }

    fn as_terminal(&mut self) -> Option<&mut super::terminal::TerminalWidget> {
        if let Some(ref mut content) = self.content {
            content.as_terminal()
        } else {
            None
        }
    }
}

```

src/kernel/elf.rs
```rust
use xmas_elf::{ElfFile, program::Type};
use x86_64::{
    structures::paging::{Page, PageTableFlags, Mapper, Size4KiB, FrameAllocator},
    VirtAddr,
};
use crate::kernel::memory::MEMORY_MANAGER;

pub fn load_and_map_elf(data: &[u8]) -> Result<u64, &'static str> {
    let elf = ElfFile::new(data).map_err(|_| "Failed to parse ELF")?;

    if elf.header.pt1.class() != xmas_elf::header::Class::SixtyFour {
        return Err("Not 64-bit ELF");
    }
    if elf.header.pt2.machine().as_machine() != xmas_elf::header::Machine::X86_64 {
        return Err("Not x86_64 ELF");
    }

    let mut mm = MEMORY_MANAGER.lock();
    // Destructure to split borrows
    let mm_ref = &mut *mm;
    let mapper = mm_ref.mapper.as_mut().ok_or("Memory manager not initialized")?;
    let allocator = mm_ref.frame_allocator.as_mut().ok_or("Frame allocator not initialized")?;

    for ph in elf.program_iter() {
        if ph.get_type() == Ok(Type::Load) {
            let virt_start_addr = ph.virtual_addr();
            let mem_size = ph.mem_size();
            let file_size = ph.file_size();
            let file_offset = ph.offset();

            let start_page = Page::containing_address(VirtAddr::new(virt_start_addr));
            let end_page = Page::containing_address(VirtAddr::new(virt_start_addr + mem_size - 1));

            let mut flags = PageTableFlags::PRESENT;
            if !ph.flags().is_execute() {
                flags |= PageTableFlags::NO_EXECUTE;
            }
            if ph.flags().is_write() {
                flags |= PageTableFlags::WRITABLE;
            }

            for page in Page::<Size4KiB>::range_inclusive(start_page, end_page) {
                // Check if already mapped
                if mapper.translate_page(page).is_err() {
                    let frame = allocator.allocate_frame().ok_or("Out of memory")?;
                    unsafe {
                        mapper.map_to(page, frame, flags, allocator)
                            .map_err(|_| "Map failed")?
                            .flush();
                    }
                }
            }

            // Copy data
            let dest_ptr = virt_start_addr as *mut u8;
            unsafe {
                // Copy segment data
                if file_size > 0 {
                    let data_start = file_offset as usize;
                    let data_end = data_start + file_size as usize;
                    let src = &data[data_start..data_end];
                    core::ptr::copy_nonoverlapping(src.as_ptr(), dest_ptr, src.len());
                }
                // Zero out BSS
                if mem_size > file_size {
                    let bss_start = dest_ptr.add(file_size as usize);
                    core::ptr::write_bytes(bss_start, 0, (mem_size - file_size) as usize);
                }
            }
        }
    }

    Ok(elf.header.pt2.entry_point())
}

```

src/kernel/gdt.rs
```rust
use lazy_static::lazy_static;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        let user_data_selector = gdt.add_entry(Descriptor::user_data_segment());
        let user_code_selector = gdt.add_entry(Descriptor::user_code_segment());
        (
            gdt,
            Selectors {
                code_selector,
                data_selector,
                tss_selector,
                user_data_selector,
                user_code_selector,
            },
        )
    };
}

pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub data_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
    pub user_data_selector: SegmentSelector,
    pub user_code_selector: SegmentSelector,
}

pub fn selectors() -> &'static Selectors {
    &GDT.1
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}

```

src/kernel/idt.rs
```rust
use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use crate::kernel::gdt;
use crate::kernel::pic::InterruptIndex;
use crate::kernel::timer;
use crate::println;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer::timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);
        idt[InterruptIndex::Mouse.as_usize()].set_handler_fn(mouse_interrupt_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    crate::hlt_loop();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    use crate::kernel::pic::PICS;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    if crate::gui::GUI_MODE_ACTIVE.load(core::sync::atomic::Ordering::Relaxed) {
        crate::drivers::keyboard::add_scancode_gui(scancode);
    } else {
        crate::drivers::keyboard::add_scancode(scancode);
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    use crate::kernel::pic::PICS;

    let mut port = Port::new(0x60);
    let byte: u8 = unsafe { port.read() };

    crate::drivers::mouse::handle_byte(byte);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Mouse.as_u8());
    }
}

```

src/kernel/memory/frame_allocator.rs
```rust
use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
    PhysAddr,
};

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next_addr: u64,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next_addr: 0,
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        for region in self.memory_map.iter() {
            if region.region_type != MemoryRegionType::Usable {
                continue;
            }
            let region_start = region.range.start_addr();
            let region_end = region.range.end_addr();
            let addr = if self.next_addr >= region_start {
                self.next_addr
            } else {
                region_start
            };
            if addr < region_end {
                self.next_addr = addr + 4096;
                return Some(PhysFrame::containing_address(PhysAddr::new(addr)));
            }
        }
        None
    }
}

```

src/kernel/memory/heap.rs
```rust
use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 512 * 1024; // 512 KiB

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

pub fn heap_stats() -> (usize, usize) {
    let heap = ALLOCATOR.lock();
    (heap.used(), heap.free())
}

```

src/kernel/memory/mod.rs
```rust
pub mod paging;
pub mod frame_allocator;
pub mod heap;

use spin::Mutex;
use x86_64::structures::paging::OffsetPageTable;
use self::frame_allocator::BootInfoFrameAllocator;

pub struct MemoryManager {
    pub mapper: Option<OffsetPageTable<'static>>,
    pub frame_allocator: Option<BootInfoFrameAllocator>,
}

pub static MEMORY_MANAGER: Mutex<MemoryManager> = Mutex::new(MemoryManager {
    mapper: None,
    frame_allocator: None,
});

```

src/kernel/memory/paging.rs
```rust
use x86_64::{
    structures::paging::{OffsetPageTable, PageTable},
    VirtAddr,
};

pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

```

src/kernel/mod.rs
```rust
pub mod gdt;
pub mod idt;
pub mod pic;
pub mod timer;
pub mod memory;
pub mod task;
pub mod syscall;
pub mod elf;

```

src/kernel/pic.rs
```rust
use pic8259::ChainedPics;
use spin;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
    Mouse = PIC_1_OFFSET + 12,
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

```

src/kernel/syscall/handlers.rs
```rust
use alloc::string::String;
use alloc::vec::Vec;
use crate::kernel::task;
use crate::fs::{FS, FileSystem};

/// Syscall numbers
pub const SYS_EXIT: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_YIELD: u64 = 2;
pub const SYS_GETPID: u64 = 3;
pub const SYS_OPEN: u64 = 4;
pub const SYS_READ: u64 = 5;
pub const SYS_CLOSE: u64 = 6;
pub const SYS_STAT: u64 = 7;

/// Per-task file descriptor table.
/// fd 0 = stdin (not really usable yet), fd 1 = stdout, fd 2 = stderr.
/// fd 3+ = opened files.
const MAX_FDS: usize = 16;

struct OpenFile {
    path: Vec<String>,
    name: String,
    offset: usize,
}

static mut FD_TABLE: [Option<OpenFile>; MAX_FDS] = {
    // Can't use array init with non-Copy types, use a const block
    const NONE: Option<OpenFile> = None;
    [NONE; MAX_FDS]
};

fn alloc_fd() -> Option<usize> {
    unsafe {
        for i in 3..MAX_FDS {
            if FD_TABLE[i].is_none() {
                return Some(i);
            }
        }
    }
    None
}

/// Main syscall dispatcher. Called from assembly entry point.
/// Returns value in RAX.
#[no_mangle]
pub extern "C" fn syscall_dispatch(nr: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    match nr {
        SYS_EXIT => sys_exit(arg0),
        SYS_WRITE => sys_write(arg0, arg1, arg2),
        SYS_YIELD => sys_yield(),
        SYS_GETPID => sys_getpid(),
        SYS_OPEN => sys_open(arg0, arg1),
        SYS_READ => sys_read(arg0, arg1, arg2),
        SYS_CLOSE => sys_close(arg0),
        SYS_STAT => sys_stat(arg0, arg1),
        _ => {
            // Unknown syscall
            u64::MAX
        }
    }
}

/// sys_exit(code) - terminate current task
fn sys_exit(_code: u64) -> u64 {
    // Clean up all open FDs for this task
    unsafe {
        for i in 3..MAX_FDS {
            FD_TABLE[i] = None;
        }
    }
    task::exit_current_task();
}

/// sys_write(fd, buf_ptr, len) - write to screen (fd=1 or fd=2 -> VGA)
fn sys_write(fd: u64, buf_ptr: u64, len: u64) -> u64 {
    if fd != 1 && fd != 2 {
        return u64::MAX; // only stdout/stderr supported for writing
    }

    let len = len as usize;
    let slice = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, len) };

    if let Ok(s) = core::str::from_utf8(slice) {
        crate::print!("{}", s);
        len as u64
    } else {
        for &byte in slice {
            if byte >= 0x20 && byte <= 0x7e || byte == b'\n' {
                crate::print!("{}", byte as char);
            }
        }
        len as u64
    }
}

/// sys_yield() - cooperative yield
fn sys_yield() -> u64 {
    task::yield_now();
    0
}

/// sys_getpid() - return current task ID
fn sys_getpid() -> u64 {
    let sched = task::SCHEDULER.lock();
    let tasks = sched.task_list();
    for t in tasks {
        if t.state == task::TaskState::Running {
            return t.id.0;
        }
    }
    0
}

/// sys_open(path_ptr, path_len) -> fd or u64::MAX on error
/// Opens a file for reading. Path is relative to root.
fn sys_open(path_ptr: u64, path_len: u64) -> u64 {
    let len = path_len as usize;
    let slice = unsafe { core::slice::from_raw_parts(path_ptr as *const u8, len) };
    let path_str = match core::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return u64::MAX,
    };

    // Parse path: "/docs/info.txt" -> path=["docs"], name="info.txt"
    let (dir_path, filename) = parse_file_path(path_str);

    // Check if file exists
    {
        let fs = FS.lock();
        if !fs.exists(&dir_path, &filename) {
            return u64::MAX;
        }
        if fs.is_dir(&dir_path, &filename) {
            return u64::MAX; // can't open directories
        }
    }

    let fd = match alloc_fd() {
        Some(fd) => fd,
        None => return u64::MAX,
    };

    unsafe {
        FD_TABLE[fd] = Some(OpenFile {
            path: dir_path,
            name: filename,
            offset: 0,
        });
    }

    fd as u64
}

/// sys_read(fd, buf_ptr, len) -> bytes_read or u64::MAX on error
fn sys_read(fd: u64, buf_ptr: u64, len: u64) -> u64 {
    let fd = fd as usize;
    if fd >= MAX_FDS {
        return u64::MAX;
    }

    let (read_bytes, new_offset) = unsafe {
        let file = match &FD_TABLE[fd] {
            Some(f) => f,
            None => return u64::MAX,
        };

        let fs = FS.lock();
        match fs.read(&file.path, &file.name) {
            Some(data) => {
                let remaining = if file.offset < data.len() {
                    &data[file.offset..]
                } else {
                    &[]
                };
                let to_read = remaining.len().min(len as usize);
                let dest = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, to_read);
                dest.copy_from_slice(&remaining[..to_read]);
                (to_read, file.offset + to_read)
            }
            None => return u64::MAX,
        }
    };

    // Update offset
    unsafe {
        if let Some(ref mut file) = FD_TABLE[fd] {
            file.offset = new_offset;
        }
    }

    read_bytes as u64
}

/// sys_close(fd) -> 0 on success, u64::MAX on error
fn sys_close(fd: u64) -> u64 {
    let fd = fd as usize;
    if fd < 3 || fd >= MAX_FDS {
        return u64::MAX;
    }
    unsafe {
        if FD_TABLE[fd].is_some() {
            FD_TABLE[fd] = None;
            0
        } else {
            u64::MAX
        }
    }
}

/// sys_stat(path_ptr, path_len) -> file size or u64::MAX on error
fn sys_stat(path_ptr: u64, path_len: u64) -> u64 {
    let len = path_len as usize;
    let slice = unsafe { core::slice::from_raw_parts(path_ptr as *const u8, len) };
    let path_str = match core::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return u64::MAX,
    };

    let (dir_path, filename) = parse_file_path(path_str);

    let fs = FS.lock();
    match fs.read(&dir_path, &filename) {
        Some(data) => data.len() as u64,
        None => u64::MAX,
    }
}

/// Parse a path like "/docs/info.txt" into (dir_components, filename)
fn parse_file_path(path: &str) -> (Vec<String>, String) {
    let path = path.trim_start_matches('/');
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        return (Vec::new(), String::new());
    }

    let filename = String::from(*parts.last().unwrap());
    let dir: Vec<String> = parts[..parts.len() - 1].iter().map(|s| String::from(*s)).collect();
    (dir, filename)
}

```

src/kernel/syscall/mod.rs
```rust
pub mod handlers;
pub mod userprogs;

use x86_64::registers::model_specific::{Star, LStar, SFMask, Efer, EferFlags};
use x86_64::registers::rflags::RFlags;
use x86_64::VirtAddr;

use crate::kernel::gdt;

/// Initialize the syscall/sysret mechanism by programming MSRs.
pub fn init() {
    let selectors = gdt::selectors();

    unsafe {
        // Enable SCE (System Call Extensions) in EFER
        Efer::update(|flags| {
            *flags |= EferFlags::SYSTEM_CALL_EXTENSIONS;
        });

        // STAR: bits 47:32 = kernel CS selector, bits 63:48 = sysret base
        // For sysret (64-bit): user CS = STAR[63:48] + 16, user SS = STAR[63:48] + 8
        // For syscall: kernel CS = STAR[47:32], kernel SS = STAR[47:32] + 8
        // sysret base must be: user_data_selector - 8 (so +8 = user_data, +16 = user_code)
        let sysret_base = selectors.user_data_selector.0 - 8;
        Star::write_raw(sysret_base, selectors.code_selector.0);

        // LSTAR: target RIP for syscall instruction
        LStar::write(VirtAddr::new(syscall_entry_asm as u64));

        // SFMASK: clear IF (interrupt flag) on syscall entry for safety
        SFMask::write(RFlags::INTERRUPT_FLAG);
    }
}

/// The assembly entry point for the `syscall` instruction.
/// On entry:
///   RCX = user RIP (return address)
///   R11 = user RFLAGS
///   RAX = syscall number
///   RDI = arg0, RSI = arg1, RDX = arg2
#[naked]
unsafe extern "C" fn syscall_entry_asm() {
    core::arch::asm!(
        // We're now in ring 0 but still on the user stack.
        // For safety we should switch to the kernel stack,
        // but for simplicity in this minimal implementation
        // we use the user stack (which is mapped in kernel space too).

        // Save user registers
        "push rcx",   // user RIP
        "push r11",   // user RFLAGS

        // Call the Rust dispatcher: syscall_dispatch(nr, arg0, arg1, arg2)
        // RAX=nr is already in rdi position after we shuffle:
        // Current: RAX=nr, RDI=arg0, RSI=arg1, RDX=arg2
        // Need:    RDI=nr, RSI=arg0, RDX=arg1, RCX=arg2
        "mov rcx, rdx",  // arg2
        "mov rdx, rsi",  // arg1
        "mov rsi, rdi",  // arg0
        "mov rdi, rax",  // nr

        "call {dispatch}",

        // RAX now has the return value

        // Restore user registers
        "pop r11",    // user RFLAGS
        "pop rcx",    // user RIP

        "sysretq",
        dispatch = sym handlers::syscall_dispatch,
        options(noreturn)
    );
}

```

src/kernel/syscall/userprogs.rs
```rust
/// Pre-compiled user-mode programs (raw x86_64 machine code).
/// These use the syscall instruction to communicate with the kernel.
///
/// Syscall convention:
///   RAX = syscall number
///   RDI = arg0, RSI = arg1, RDX = arg2
///   syscall
///   RAX = return value

/// "hello" program: writes "Hello from user mode!\n" then exits.
///
/// Equivalent to:
///   mov rax, 1          ; SYS_WRITE
///   mov rdi, 1          ; fd = stdout
///   lea rsi, [rip+msg]  ; buf pointer
///   mov rdx, 22         ; length
///   syscall
///   mov rax, 0          ; SYS_EXIT
///   xor rdi, rdi        ; code = 0
///   syscall
///   msg: db "Hello from user mode!\n"
pub fn hello_program() -> &'static [u8] {
    // We generate this at runtime using a function that the task will call,
    // rather than raw bytes, since it's simpler and more maintainable.
    // See `run_user_hello` below.
    &[]
}

/// Run "hello" as a kernel-mode task that simulates a user syscall.
/// This demonstrates the syscall dispatch path without actual ring-3 transition.
pub fn run_user_hello() {
    let msg = "Hello from user mode!\n";
    // Call syscall dispatch directly (simulating what a real syscall would do)
    super::handlers::syscall_dispatch(
        super::handlers::SYS_WRITE,
        1, // stdout
        msg.as_ptr() as u64,
        msg.len() as u64,
    );

    let msg2 = "Syscall getpid returned: ";
    super::handlers::syscall_dispatch(
        super::handlers::SYS_WRITE,
        1,
        msg2.as_ptr() as u64,
        msg2.len() as u64,
    );

    // Get PID
    let pid = super::handlers::syscall_dispatch(
        super::handlers::SYS_GETPID,
        0, 0, 0,
    );

    // Print PID (simple decimal conversion)
    let mut buf = [0u8; 20];
    let mut pos = 0;
    if pid == 0 {
        buf[0] = b'0';
        pos = 1;
    } else {
        let mut n = pid;
        let mut digits = [0u8; 20];
        let mut dpos = 0;
        while n > 0 {
            digits[dpos] = b'0' + (n % 10) as u8;
            dpos += 1;
            n /= 10;
        }
        for i in (0..dpos).rev() {
            buf[pos] = digits[i];
            pos += 1;
        }
    }
    buf[pos] = b'\n';
    pos += 1;

    super::handlers::syscall_dispatch(
        super::handlers::SYS_WRITE,
        1,
        buf.as_ptr() as u64,
        pos as u64,
    );

    // Yield a few times to demonstrate cooperation
    for _ in 0..3 {
        super::handlers::syscall_dispatch(super::handlers::SYS_YIELD, 0, 0, 0);
    }

    // Exit
    super::handlers::syscall_dispatch(super::handlers::SYS_EXIT, 0, 0, 0);
}

/// A user program that counts using syscalls
pub fn run_user_counter() {
    for i in 1..=5u64 {
        let msg = "[usercount] Krok ";
        super::handlers::syscall_dispatch(
            super::handlers::SYS_WRITE, 1,
            msg.as_ptr() as u64, msg.len() as u64,
        );

        // Print number
        let mut buf = [0u8; 4];
        buf[0] = b'0' + (i % 10) as u8;
        buf[1] = b'\n';
        super::handlers::syscall_dispatch(
            super::handlers::SYS_WRITE, 1,
            buf.as_ptr() as u64, 2,
        );

        // Yield
        super::handlers::syscall_dispatch(super::handlers::SYS_YIELD, 0, 0, 0);
    }

    let done = "[usercount] Zakonczony.\n";
    super::handlers::syscall_dispatch(
        super::handlers::SYS_WRITE, 1,
        done.as_ptr() as u64, done.len() as u64,
    );
}

```

src/kernel/task/context.rs
```rust
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub rsp: u64,
}

impl Context {
    pub const fn empty() -> Self {
        Context {
            rsp: 0,
        }
    }
}

/// Switch between two tasks by saving/restoring callee-saved registers and RSP.
///
/// This saves rbp, rbx, r12-r15 on the old stack, saves the old RSP,
/// loads the new RSP, restores callee-saved regs, and returns to wherever
/// the new task's stack says (via `ret`).
///
/// # Safety
/// Both context pointers must be valid. The new context's RSP must point
/// to a valid stack with the correct layout.
#[naked]
pub unsafe extern "C" fn switch_context(old: *mut Context, new: *const Context) {
    core::arch::asm!(
        // Save callee-saved registers on old stack
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Save old RSP
        "mov [rdi], rsp",

        // Load new RSP
        "mov rsp, [rsi]",

        // Restore callee-saved registers from new stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",

        "ret",
        options(noreturn)
    );
}

```

src/kernel/task/mod.rs
```rust
pub mod context;
pub mod scheduler;

pub use scheduler::{yield_now, spawn, exit_current_task, TaskId, TaskState, SCHEDULER};

```

src/kernel/task/scheduler.rs
```rust
use alloc::vec::Vec;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::{Context, switch_context};

const TASK_STACK_SIZE: usize = 4096 * 4; // 16 KiB per task
const DEFAULT_QUANTUM: u32 = 10; // 10 ticks = 100ms at 100Hz

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Terminated,
}

pub struct Task {
    pub id: TaskId,
    pub state: TaskState,
    pub context: Context,
    pub name: &'static str,
    pub quantum_remaining: u32,
    _stack: Option<Box<[u8]>>,
}

impl Task {
    /// Create a new task with its own stack that will execute `entry_fn`.
    ///
    /// Stack layout (low address → high address):
    ///   [r15=0, r14=0, r13=0, r12=entry_fn, rbx=0, rbp=0, ret_addr=trampoline]
    ///
    /// This matches what `switch_context` expects: it pops r15..rbp then `ret`.
    pub fn new(name: &'static str, entry_fn: fn()) -> Self {
        let id = TaskId(NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed));

        // Allocate stack
        let stack = Box::new([0u8; TASK_STACK_SIZE]);
        let stack_top = stack.as_ptr() as u64 + TASK_STACK_SIZE as u64;

        // Build the initial stack frame for switch_context
        // switch_context does: pop r15, r14, r13, r12, rbx, rbp, ret
        // So we push in reverse order: ret_addr, rbp, rbx, r12, r13, r14, r15
        let mut rsp = stack_top;

        // Return address — where `ret` in switch_context will jump to
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = task_entry_trampoline as u64; }

        // rbp = 0
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        // rbx = 0
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        // r12 = entry_fn pointer (trampoline reads this)
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = entry_fn as u64; }

        // r13 = 0
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        // r14 = 0
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        // r15 = 0 (switch_context pops this first)
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        let mut ctx = Context::empty();
        ctx.rsp = rsp;

        Task {
            id,
            state: TaskState::Ready,
            context: ctx,
            name,
            quantum_remaining: DEFAULT_QUANTUM,
            _stack: Some(stack),
        }
    }

    /// Create a "virtual" task representing the currently running kernel thread (task 0).
    pub fn kernel_task() -> Self {
        Task {
            id: TaskId(0),
            state: TaskState::Running,
            context: Context::empty(),
            name: "kernel",
            quantum_remaining: DEFAULT_QUANTUM,
            _stack: None, // uses the kernel stack
        }
    }
}

/// Trampoline that reads the entry function from r12 and calls it.
/// When the function returns, it marks the task as terminated and yields.
extern "C" fn task_entry_trampoline() {
    // Re-enable interrupts — we may have been switched to from a timer handler
    // where interrupts were disabled.
    x86_64::instructions::interrupts::enable();

    // R12 holds the function pointer (set up by Task::new)
    let entry_fn: fn();
    unsafe {
        core::arch::asm!("mov {}, r12", out(reg) entry_fn);
    }
    entry_fn();
    exit_current_task();
}

pub struct Scheduler {
    tasks: Vec<Task>,
    current: usize,
}

impl Scheduler {
    pub fn new() -> Self {
        let mut sched = Scheduler {
            tasks: Vec::new(),
            current: 0,
        };
        // Task 0 is the kernel/shell task
        sched.tasks.push(Task::kernel_task());
        sched
    }

    pub fn spawn(&mut self, name: &'static str, entry_fn: fn()) -> TaskId {
        let task = Task::new(name, entry_fn);
        let id = task.id;
        self.tasks.push(task);
        id
    }

    /// Decide whether to switch tasks (preemptive path).
    /// Decrements the quantum; if expired, finds the next ready task.
    /// Returns context pointers for the switch, or None if no switch needed.
    pub fn schedule_preempt(&mut self) -> Option<(*mut Context, *const Context)> {
        if self.tasks.len() <= 1 {
            return None;
        }

        // Decrement quantum for current task
        if self.tasks[self.current].quantum_remaining > 0 {
            self.tasks[self.current].quantum_remaining -= 1;
            if self.tasks[self.current].quantum_remaining > 0 {
                return None; // still has time left
            }
        }

        self.find_and_switch()
    }

    /// Cooperative yield: always try to switch to next task.
    pub fn schedule_yield(&mut self) -> Option<(*mut Context, *const Context)> {
        if self.tasks.len() <= 1 {
            return None;
        }
        self.find_and_switch()
    }

    /// Find the next runnable task and prepare context pointers for switching.
    fn find_and_switch(&mut self) -> Option<(*mut Context, *const Context)> {
        let old_idx = self.current;

        // Find next ready task (round-robin)
        let mut next_idx = (old_idx + 1) % self.tasks.len();
        let start = next_idx;
        loop {
            if self.tasks[next_idx].state == TaskState::Ready
                || self.tasks[next_idx].state == TaskState::Running
            {
                break;
            }
            next_idx = (next_idx + 1) % self.tasks.len();
            if next_idx == start {
                // No other runnable task — reset quantum and stay
                self.tasks[old_idx].quantum_remaining = DEFAULT_QUANTUM;
                return None;
            }
        }

        if next_idx == old_idx {
            // Only one runnable task — reset quantum and stay
            self.tasks[old_idx].quantum_remaining = DEFAULT_QUANTUM;
            return None;
        }

        // Update states
        if self.tasks[old_idx].state == TaskState::Running {
            self.tasks[old_idx].state = TaskState::Ready;
        }
        self.tasks[next_idx].state = TaskState::Running;
        self.tasks[next_idx].quantum_remaining = DEFAULT_QUANTUM;
        self.current = next_idx;

        let old_ctx = &mut self.tasks[old_idx].context as *mut Context;
        let new_ctx = &self.tasks[next_idx].context as *const Context;

        Some((old_ctx, new_ctx))
    }

    /// Legacy cooperative schedule (calls switch_context internally).
    /// Used only by yield_now for backward compat.
    pub fn schedule(&mut self) {
        if let Some((old_ctx, new_ctx)) = self.schedule_yield() {
            unsafe {
                switch_context(old_ctx, new_ctx);
            }
        }
    }

    pub fn terminate_current(&mut self) {
        self.tasks[self.current].state = TaskState::Terminated;
    }

    pub fn kill_task(&mut self, id: TaskId) -> bool {
        for task in &mut self.tasks {
            if task.id == id && task.state != TaskState::Terminated {
                task.state = TaskState::Terminated;
                return true;
            }
        }
        false
    }

    pub fn task_list(&self) -> &[Task] {
        &self.tasks
    }

    pub fn cleanup_terminated(&mut self) {
        // Don't remove task 0 (kernel) or the current task
        self.tasks.retain(|t| {
            t.id == TaskId(0) || t.state != TaskState::Terminated
        });
        // Fix current index after cleanup
        for (i, task) in self.tasks.iter().enumerate() {
            if task.state == TaskState::Running {
                self.current = i;
                break;
            }
        }
    }
}

use spin::Mutex;

lazy_static::lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}

impl Scheduler {
    pub unsafe fn force_unlock() {
        SCHEDULER.force_unlock();
    }
}

/// Yield the CPU to the next ready task (cooperative).
pub fn yield_now() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let switch = {
            let mut sched = SCHEDULER.lock();
            sched.schedule_yield()
        }; // lock dropped here

        if let Some((old_ctx, new_ctx)) = switch {
            unsafe { switch_context(old_ctx, new_ctx); }
        }
    });
}

/// Mark the current task as terminated and yield.
pub fn exit_current_task() -> ! {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let switch = {
            let mut sched = SCHEDULER.lock();
            sched.terminate_current();
            sched.schedule_yield()
        }; // lock dropped

        if let Some((old_ctx, new_ctx)) = switch {
            unsafe { switch_context(old_ctx, new_ctx); }
        }
    });
    // Should never reach here, but just in case
    loop {
        x86_64::instructions::hlt();
    }
}

/// Spawn a new task.
pub fn spawn(name: &'static str, entry_fn: fn()) -> TaskId {
    x86_64::instructions::interrupts::without_interrupts(|| {
        SCHEDULER.lock().spawn(name, entry_fn)
    })
}

```

src/kernel/timer.rs
```rust
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::InterruptStackFrame;
use crate::kernel::pic::{InterruptIndex, PICS};
use crate::kernel::task::context::switch_context;

pub const TIMER_HZ: u32 = 100;
static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init_timer() {
    use x86_64::instructions::port::Port;
    let divisor: u16 = (1_193_182u32 / TIMER_HZ) as u16;
    unsafe {
        let mut cmd = Port::<u8>::new(0x43);
        let mut data = Port::<u8>::new(0x40);
        cmd.write(0x36);
        data.write((divisor & 0xFF) as u8);
        data.write((divisor >> 8) as u8);
    }
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

struct FmtBuf {
    buf: [u8; 40],
    pos: usize,
}

impl FmtBuf {
    fn new() -> Self {
        FmtBuf { buf: [0; 40], pos: 0 }
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.pos]).unwrap_or("")
    }
}

impl core::fmt::Write for FmtBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &byte in s.as_bytes() {
            if self.pos < self.buf.len() {
                self.buf[self.pos] = byte;
                self.pos += 1;
            }
        }
        Ok(())
    }
}

/// Naked interrupt handler for Timer.
/// Saves context, calls rust handler, schedules, restores context.
#[naked]
pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        core::arch::asm!(
            // 1. Save all GPRs (except RSP which is already saved by CPU)
            "push rax",
            "push rbx",
            "push rcx",
            "push rdx",
            "push rsi",
            "push rdi",
            "push rbp",
            "push r8",
            "push r9",
            "push r10",
            "push r11",
            "push r12",
            "push r13",
            "push r14",
            "push r15",

            // 2. Call Rust handler to update ticks and ACK PIC
            "call rust_timer_handler",

            // 3. Call Scheduler to switch tasks (preemptive)
            "call scheduler_schedule",

            // 4. Restore all GPRs
            "pop r15",
            "pop r14",
            "pop r13",
            "pop r12",
            "pop r11",
            "pop r10",
            "pop r9",
            "pop r8",
            "pop rbp",
            "pop rdi",
            "pop rsi",
            "pop rdx",
            "pop rcx",
            "pop rbx",
            "pop rax",

            // 5. Return from interrupt
            "iretq",
            options(noreturn)
        );
    }
}

#[no_mangle]
pub extern "C" fn rust_timer_handler() {
    let ticks = TICKS.fetch_add(1, Ordering::Relaxed) + 1;

    // Only update VGA text status bar when NOT in GUI mode
    if !crate::gui::GUI_MODE_ACTIVE.load(Ordering::Relaxed) {
        if ticks % TIMER_HZ as u64 == 0 {
            use core::fmt::Write;
            let secs = ticks / TIMER_HZ as u64;
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            let mut buf = FmtBuf::new();
            let _ = write!(buf, " {}h {:02}m {:02}s ", h, m, s);
            crate::drivers::vga::update_status_bar(" PolarOs v0.1.0", buf.as_str());
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

/// Preemptive scheduler entry point called from the timer interrupt handler.
/// Uses try_lock to avoid deadlocks — if the scheduler is already locked
/// (e.g., during yield_now or spawn), we simply skip this tick.
#[no_mangle]
pub extern "C" fn scheduler_schedule() {
    use crate::kernel::task::SCHEDULER;

    let switch = {
        if let Some(mut sched) = SCHEDULER.try_lock() {
            sched.schedule_preempt()
        } else {
            return; // scheduler busy, skip this tick
        }
    }; // lock dropped here — BEFORE context switch

    if let Some((old_ctx, new_ctx)) = switch {
        unsafe { switch_context(old_ctx, new_ctx); }
    }
}

```

src/lib.rs
```rust
#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

extern crate alloc;

pub mod kernel;
pub mod drivers;
pub mod fs;
pub mod shell;
pub mod gui;

/// Early init: GDT, IDT, PICs, timer, syscall. No heap required.
pub fn init() {
    serial_println!("[INIT] GDT...");
    kernel::gdt::init();
    serial_println!("[INIT] IDT...");
    kernel::idt::init_idt();
    serial_println!("[INIT] PICs...");
    unsafe { kernel::pic::PICS.lock().initialize() };
    serial_println!("[INIT] Timer...");
    kernel::timer::init_timer();
    serial_println!("[INIT] Syscall...");
    kernel::syscall::init();
    serial_println!("[INIT] Early init done");
}

/// Late init: requires heap. Enables interrupts and initializes drivers.
pub fn init_late() {
    serial_println!("[INIT] Enabling interrupts...");
    x86_64::instructions::interrupts::enable();
    serial_println!("[INIT] ATA...");
    drivers::ata::init();
    serial_println!("[INIT] All done");
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    serial_println!("[PANIC] {}", info);
    println!("{}", info);
    hlt_loop()
}

```

src/main.rs
```rust
#![no_std]
#![no_main]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use systemoperacyjny::kernel::memory::{paging, frame_allocator, heap};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // Initialize GDT, IDT, PICs
    systemoperacyjny::serial_println!("[BOOT] Starting init...");
    systemoperacyjny::init();
    systemoperacyjny::serial_println!("[BOOT] Init done");

    // Initialize memory management
    systemoperacyjny::serial_println!("[BOOT] Setting up memory...");
    let phys_mem_offset = x86_64::VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { paging::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { frame_allocator::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    // Initialize heap
    heap::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");
    systemoperacyjny::serial_println!("[BOOT] Heap ready");

    // Save to global memory manager
    {
        let mut mm = systemoperacyjny::kernel::memory::MEMORY_MANAGER.lock();
        mm.mapper = Some(mapper);
        mm.frame_allocator = Some(frame_allocator);
    }
    systemoperacyjny::serial_println!("[BOOT] Memory manager saved");

    // Late init: enable interrupts + ATA (requires heap)
    systemoperacyjny::init_late();

    // Initialize filesystem with sample files
    systemoperacyjny::fs::init();
    systemoperacyjny::serial_println!("[BOOT] Filesystem initialized");

    // Launch GUI
    systemoperacyjny::serial_println!("[BOOT] Launching GUI...");
    systemoperacyjny::gui::run()
}

```

src/shell/commands.rs
```rust
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use crate::fs::{FS, FileSystem, RemoveResult};

/// Execute a single command and return its output as a String.
/// `pipe_input` is the output of the previous command in a pipeline (if any).
pub fn run_command(cmd: &str, args: &str, cwd: &mut Vec<String>, pipe_input: Option<&str>) -> String {
    match cmd {
        "help" => cmd_help(),
        "echo" => cmd_echo(args),
        "clear" => { crate::drivers::vga::clear_screen(); String::new() }
        "ls" => cmd_ls(cwd),
        "cat" => cmd_cat(args, cwd),
        "touch" => cmd_touch(args, cwd),
        "write" => cmd_write(args, cwd),
        "rm" => cmd_rm(args, cwd),
        "mkdir" => cmd_mkdir(args, cwd),
        "cd" => { cmd_cd(args, cwd); String::new() }
        "pwd" => cmd_pwd(cwd),
        "uptime" => cmd_uptime(),
        "info" => cmd_info(),
        "grep" => cmd_grep(args, cwd, pipe_input),
        "wc" => cmd_wc(args, cwd, pipe_input),
        "cp" => cmd_cp(args, cwd),
        "mv" => cmd_mv(args, cwd),
        "hexdump" => cmd_hexdump(args, cwd),
        "save" => cmd_save(),
        "load" => cmd_load(cwd),
        "ps" => cmd_ps(),
        "spawn" => cmd_spawn(args),
        "kill" => cmd_kill(args),
        "exec" => cmd_exec(args, cwd),
        "fatls" => cmd_fatls(),
        "env" => cmd_env(),
        "export" => cmd_export(args),
        "head" => cmd_head(args, cwd, pipe_input),
        "tail" => cmd_tail(args, cwd, pipe_input),
        "sort" => cmd_sort(args, cwd, pipe_input),
        "uniq" => cmd_uniq(pipe_input),
        "keymap" => cmd_keymap(args),
        _ => format!("Nieznana komenda: '{}'. Wpisz 'help' aby zobaczyc liste komend.", cmd),
    }
}

fn cmd_fatls() -> String {
    let files = crate::fs::fat::list_root_files();
    if files.is_empty() {
        return String::from("(brak plikow lub blad odczytu)");
    }
    let mut s = String::from("Pliki na dysku FAT (root):\n");
    for f in files {
        s.push_str(&format!("  {}\n", f));
    }
    if s.ends_with('\n') { s.pop(); }
    s
}

fn cmd_help() -> String {
    let mut s = String::new();
    s.push_str("Dostepne komendy:\n");
    s.push_str("  help              - Wyswietl te pomoc\n");
    s.push_str("  echo <tekst>      - Wyswietl tekst\n");
    s.push_str("  clear             - Wyczysc ekran\n");
    s.push_str("  ls                - Lista plikow i katalogow\n");
    s.push_str("  cat <plik>        - Wyswietl zawartosc pliku\n");
    s.push_str("  touch <plik>      - Utworz pusty plik\n");
    s.push_str("  write <plik> <t>  - Zapisz tekst do pliku\n");
    s.push_str("  rm <nazwa>        - Usun plik lub pusty katalog\n");
    s.push_str("  mkdir <nazwa>     - Utworz katalog\n");
    s.push_str("  cd <katalog>      - Zmien katalog (cd .. / cd /)\n");
    s.push_str("  pwd               - Wyswietl biezacy katalog\n");
    s.push_str("  grep <wz> <plik>  - Szukaj wzorca w pliku\n");
    s.push_str("  wc [plik]         - Policz linie/slowa/bajty\n");
    s.push_str("  cp <src> <dst>    - Kopiuj plik\n");
    s.push_str("  mv <src> <dst>    - Przenies/zmien nazwe pliku\n");
    s.push_str("  hexdump <plik>    - Zrzut szesnastkowy pliku\n");
    s.push_str("  head [-n N] [plik]- Pokaz pierwszych N linii\n");
    s.push_str("  tail [-n N] [plik]- Pokaz ostatnich N linii\n");
    s.push_str("  sort [plik]       - Sortuj linie\n");
    s.push_str("  uniq              - Usun powtorzenia (pipe)\n");
    s.push_str("  save              - Zapisz FS na dysk ATA\n");
    s.push_str("  load              - Wczytaj FS z dysku ATA\n");
    s.push_str("  uptime            - Czas dzialania systemu\n");
    s.push_str("  info              - Informacje systemowe\n");
    s.push_str("  ps                - Lista procesow/taskow\n");
    s.push_str("  spawn <nazwa>     - Uruchom demo task\n");
    s.push_str("  kill <id>         - Zakoncz task o podanym ID\n");
    s.push_str("  exec <program>    - Uruchom program uzytkownika\n");
    s.push_str("  env               - Pokaz zmienne srodowiskowe\n");
    s.push_str("  export K=V        - Ustaw zmienna srodowiskowa\n");
    s.push_str("  fatls             - Lista plikow FAT32\n");
    s.push_str("  keymap [layout]   - Pokaz/zmien layout klawiatury\n");
    s.push_str("Pipe: cmd1 | cmd2   Redirect: cmd > plik, >> plik, < plik");
    s
}

fn cmd_echo(args: &str) -> String {
    String::from(args)
}

fn cmd_ls(cwd: &[String]) -> String {
    let fs = FS.lock();
    match fs.list(cwd) {
        Some(entries) => {
            if entries.is_empty() {
                return String::from("(pusty katalog)");
            }
            let mut s = String::new();
            for entry in &entries {
                if entry.is_dir {
                    s.push_str(&format!("  {}/\n", entry.name));
                } else {
                    s.push_str(&format!("  {} ({} bajtow)\n", entry.name, entry.size));
                }
            }
            if s.ends_with('\n') { s.pop(); }
            s
        }
        None => String::from("Katalog nie istnieje."),
    }
}

fn cmd_cat(args: &str, cwd: &[String]) -> String {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => return String::from("Uzycie: cat <nazwa_pliku>"),
    };
    let fs = FS.lock();
    match fs.read(cwd, name) {
        Some(data) => {
            String::from(core::str::from_utf8(data).unwrap_or("<dane binarne>"))
        }
        None => format!("Plik '{}' nie istnieje.", name),
    }
}

fn cmd_touch(args: &str, cwd: &[String]) -> String {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => return String::from("Uzycie: touch <nazwa_pliku>"),
    };
    let mut fs = FS.lock();
    if fs.create(cwd, name) {
        format!("Utworzono plik '{}'.", name)
    } else {
        format!("'{}' juz istnieje.", name)
    }
}

fn cmd_write(args: &str, cwd: &[String]) -> String {
    let (name, content) = match args.split_once(' ') {
        Some((n, c)) => (n, c),
        None => return String::from("Uzycie: write <nazwa_pliku> <tekst>"),
    };
    if name.is_empty() {
        return String::from("Uzycie: write <nazwa_pliku> <tekst>");
    }
    let mut fs = FS.lock();
    if fs.write(cwd, name, content.as_bytes()) {
        format!("Zapisano {} bajtow do '{}'.", content.len(), name)
    } else {
        format!("Nie mozna zapisac do '{}'.", name)
    }
}

fn cmd_rm(args: &str, cwd: &[String]) -> String {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => return String::from("Uzycie: rm <nazwa>"),
    };
    let mut fs = FS.lock();
    match fs.remove(cwd, name) {
        RemoveResult::Ok => format!("Usunieto '{}'.", name),
        RemoveResult::NotFound => format!("'{}' nie istnieje.", name),
        RemoveResult::DirNotEmpty => format!("Katalog '{}' nie jest pusty.", name),
    }
}

fn cmd_mkdir(args: &str, cwd: &[String]) -> String {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => return String::from("Uzycie: mkdir <nazwa>"),
    };
    let mut fs = FS.lock();
    if fs.mkdir(cwd, name) {
        format!("Utworzono katalog '{}'.", name)
    } else {
        format!("'{}' juz istnieje.", name)
    }
}

fn cmd_cd(args: &str, cwd: &mut Vec<String>) {
    let target = match args.split_whitespace().next() {
        Some(t) => t,
        None => {
            cwd.clear();
            return;
        }
    };

    match target {
        "/" => cwd.clear(),
        ".." => { cwd.pop(); }
        "." => {}
        name => {
            let is_dir = {
                let fs = FS.lock();
                if !fs.exists(cwd, name) {
                    return;
                }
                fs.is_dir(cwd, name)
            };
            if is_dir {
                cwd.push(String::from(name));
            }
        }
    }
}

fn cmd_pwd(cwd: &[String]) -> String {
    if cwd.is_empty() {
        String::from("/")
    } else {
        let mut s = String::new();
        for component in cwd {
            s.push('/');
            s.push_str(component);
        }
        s
    }
}

fn cmd_uptime() -> String {
    let t = crate::kernel::timer::ticks();
    let hz = crate::kernel::timer::TIMER_HZ as u64;
    let total_secs = t / hz;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    format!("Uptime: {}h {:02}m {:02}s ({} tickow @ {}Hz)", hours, minutes, secs, t, hz)
}

fn cmd_info() -> String {
    let (heap_used, heap_free) = crate::kernel::memory::heap::heap_stats();
    let total_kb = (heap_used + heap_free) / 1024;
    let used_kb = heap_used / 1024;

    let mut s = String::new();
    s.push_str("=== Informacje systemowe ===\n");
    s.push_str("  System:        PolarOs v0.1.0\n");
    s.push_str("  Architektura:  x86_64\n");
    s.push_str("  Jezyk:         Rust (nightly)\n");
    s.push_str("  Klawiatura:    PS/2 (IRQ1)\n");
    s.push_str(&format!("  Heap:          {}/{} KiB\n", used_kb, total_kb));
    s.push_str("  Filesystem:    RamFs (drzewo katalogow)\n");
    s.push_str(&format!("  Dysk ATA:      {}", if crate::drivers::ata::is_available() { "dostepny" } else { "niedostepny" }));
    s
}

fn cmd_grep(args: &str, cwd: &[String], pipe_input: Option<&str>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let pattern = match parts.first() {
        Some(p) => *p,
        None => return String::from("Uzycie: grep <wzorzec> [plik]"),
    };

    let text = if let Some(input) = pipe_input {
        String::from(input)
    } else {
        let filename = match parts.get(1) {
            Some(f) => *f,
            None => return String::from("Uzycie: grep <wzorzec> <plik>"),
        };
        let fs = FS.lock();
        match fs.read(cwd, filename) {
            Some(data) => String::from(core::str::from_utf8(data).unwrap_or("")),
            None => return format!("Plik '{}' nie istnieje.", filename),
        }
    };

    let mut result = String::new();
    for line in text.lines() {
        if line.contains(pattern) {
            result.push_str(line);
            result.push('\n');
        }
    }
    if result.ends_with('\n') { result.pop(); }
    if result.is_empty() {
        format!("Brak wynikow dla '{}'.", pattern)
    } else {
        result
    }
}

fn cmd_wc(args: &str, cwd: &[String], pipe_input: Option<&str>) -> String {
    let text = if let Some(input) = pipe_input {
        String::from(input)
    } else {
        let name = match args.split_whitespace().next() {
            Some(n) => n,
            None => return String::from("Uzycie: wc <plik>"),
        };
        let fs = FS.lock();
        match fs.read(cwd, name) {
            Some(data) => String::from(core::str::from_utf8(data).unwrap_or("")),
            None => return format!("Plik '{}' nie istnieje.", name),
        }
    };

    let bytes = text.len();
    let lines = text.lines().count();
    let words = text.split_whitespace().count();
    format!("  {} linii  {} slow  {} bajtow", lines, words, bytes)
}

fn cmd_cp(args: &str, cwd: &[String]) -> String {
    let (src, dst) = match args.split_once(' ') {
        Some((s, d)) => (s, d.trim()),
        None => return String::from("Uzycie: cp <zrodlo> <cel>"),
    };
    let data = {
        let fs = FS.lock();
        match fs.read(cwd, src) {
            Some(d) => Vec::from(d),
            None => return format!("Plik '{}' nie istnieje.", src),
        }
    };
    let mut fs = FS.lock();
    if fs.write(cwd, dst, &data) {
        format!("Skopiowano '{}' -> '{}'.", src, dst)
    } else {
        format!("Nie mozna zapisac do '{}'.", dst)
    }
}

fn cmd_mv(args: &str, cwd: &[String]) -> String {
    let (src, dst) = match args.split_once(' ') {
        Some((s, d)) => (s, d.trim()),
        None => return String::from("Uzycie: mv <zrodlo> <cel>"),
    };
    if src == dst {
        return String::new();
    }
    let data = {
        let fs = FS.lock();
        match fs.read(cwd, src) {
            Some(d) => Vec::from(d),
            None => return format!("Plik '{}' nie istnieje.", src),
        }
    };
    let mut fs = FS.lock();
    if fs.write(cwd, dst, &data) {
        fs.remove(cwd, src);
        format!("Przeniesiono '{}' -> '{}'.", src, dst)
    } else {
        format!("Nie mozna przeniesc do '{}'.", dst)
    }
}

fn cmd_hexdump(args: &str, cwd: &[String]) -> String {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => return String::from("Uzycie: hexdump <plik>"),
    };
    let fs = FS.lock();
    match fs.read(cwd, name) {
        Some(data) => {
            let mut s = String::new();
            for (i, chunk) in data.chunks(16).enumerate() {
                s.push_str(&format!("{:08x}  ", i * 16));
                for (j, byte) in chunk.iter().enumerate() {
                    s.push_str(&format!("{:02x} ", byte));
                    if j == 7 { s.push(' '); }
                }
                for j in chunk.len()..16 {
                    s.push_str("   ");
                    if j == 7 { s.push(' '); }
                }
                s.push_str(" |");
                for byte in chunk {
                    if *byte >= 0x20 && *byte <= 0x7e {
                        s.push(*byte as char);
                    } else {
                        s.push('.');
                    }
                }
                s.push_str("|\n");
            }
            if s.ends_with('\n') { s.pop(); }
            s
        }
        None => format!("Plik '{}' nie istnieje.", name),
    }
}

fn cmd_save() -> String {
    use crate::drivers::ata;

    if !ata::is_available() {
        return String::from("Dysk ATA niedostepny.");
    }

    let data = {
        let fs = FS.lock();
        fs.serialize()
    };

    let total_len = data.len() as u32;
    let mut header = [0u8; 512];
    header[0..4].copy_from_slice(b"PLRS");
    header[4..8].copy_from_slice(&total_len.to_le_bytes());

    let first_chunk = data.len().min(504);
    header[8..8 + first_chunk].copy_from_slice(&data[..first_chunk]);

    if !ata::write_sector(ata::DATA_START_SECTOR, &header) {
        return String::from("Blad zapisu naglowka.");
    }

    let mut offset = first_chunk;
    let mut sector = ata::DATA_START_SECTOR + 1;
    while offset < data.len() {
        let mut buf = [0u8; 512];
        let chunk = (data.len() - offset).min(512);
        buf[..chunk].copy_from_slice(&data[offset..offset + chunk]);
        if !ata::write_sector(sector, &buf) {
            return format!("Blad zapisu sektora {}.", sector);
        }
        offset += chunk;
        sector += 1;
    }

    let sectors_written = sector - ata::DATA_START_SECTOR;
    format!("Zapisano {} bajtow ({} sektorow).", data.len(), sectors_written)
}

fn cmd_load(cwd: &mut Vec<String>) -> String {
    use crate::drivers::ata;
    use crate::fs::ramfs::RamFs;

    if !ata::is_available() {
        return String::from("Dysk ATA niedostepny.");
    }

    let mut header = [0u8; 512];
    if !ata::read_sector(ata::DATA_START_SECTOR, &mut header) {
        return String::from("Blad odczytu naglowka.");
    }

    if &header[0..4] != b"PLRS" {
        return String::from("Brak zapisanego systemu plikow na dysku.");
    }

    let total_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;

    let mut data = Vec::with_capacity(total_len);

    let first_chunk = total_len.min(504);
    data.extend_from_slice(&header[8..8 + first_chunk]);

    let mut sector = ata::DATA_START_SECTOR + 1;
    while data.len() < total_len {
        let mut buf = [0u8; 512];
        if !ata::read_sector(sector, &mut buf) {
            return format!("Blad odczytu sektora {}.", sector);
        }
        let remaining = total_len - data.len();
        let chunk = remaining.min(512);
        data.extend_from_slice(&buf[..chunk]);
        sector += 1;
    }

    match RamFs::load_from(&data) {
        Some(new_fs) => {
            let mut fs = FS.lock();
            fs.replace(new_fs);
            cwd.clear();
            format!("Wczytano system plikow ({} bajtow).", total_len)
        }
        None => String::from("Uszkodzone dane na dysku."),
    }
}

fn cmd_ps() -> String {
    use crate::kernel::task::{SCHEDULER, TaskState};

    let mut s = String::new();
    s.push_str("  ID  STAN         NAZWA\n");

    x86_64::instructions::interrupts::without_interrupts(|| {
        let sched = SCHEDULER.lock();
        for task in sched.task_list() {
            let state_str = match task.state {
                TaskState::Ready => "Ready      ",
                TaskState::Running => "Running    ",
                TaskState::Terminated => "Terminated ",
            };
            s.push_str(&format!("  {:3} {}  {}\n", task.id.0, state_str, task.name));
        }
    });
    if s.ends_with('\n') { s.pop(); }
    s
}

fn demo_counter() {
    use crate::kernel::task::yield_now;
    use crate::kernel::timer;

    let start = timer::ticks();
    for i in 0..5 {
        let now = timer::ticks();
        let secs = (now - start) / timer::TIMER_HZ as u64;
        crate::serial_println!("[demo] Krok {}/5  ({}s od startu)", i + 1, secs);
        // Busy-wait ~1 second then yield
        let target = now + timer::TIMER_HZ as u64;
        while timer::ticks() < target {
            x86_64::instructions::hlt();
        }
        yield_now();
    }
    crate::serial_println!("[demo] Task zakonczony.");
}

fn demo_hello() {
    use crate::kernel::task::yield_now;
    use crate::kernel::timer;

    for i in 0..3 {
        crate::serial_println!("[hello] Pozdrowienia nr {} z taska!", i + 1);
        let target = timer::ticks() + timer::TIMER_HZ as u64;
        while timer::ticks() < target {
            x86_64::instructions::hlt();
        }
        yield_now();
    }
    crate::serial_println!("[hello] Koniec.");
}

fn cmd_spawn(args: &str) -> String {
    use crate::kernel::task;

    let name = args.split_whitespace().next().unwrap_or("counter");
    match name {
        "counter" => {
            let id = task::spawn("demo-counter", demo_counter);
            format!("Uruchomiono task 'counter' (ID={})", id.0)
        }
        "hello" => {
            let id = task::spawn("demo-hello", demo_hello);
            format!("Uruchomiono task 'hello' (ID={})", id.0)
        }
        _ => String::from("Dostepne demo taski: counter, hello"),
    }
}

fn cmd_kill(args: &str) -> String {
    use crate::kernel::task::{SCHEDULER, TaskId};

    let id_str = match args.split_whitespace().next() {
        Some(s) => s,
        None => return String::from("Uzycie: kill <id>"),
    };

    let id: u64 = match id_str.parse() {
        Ok(n) => n,
        Err(_) => return format!("Nieprawidlowy ID: '{}'", id_str),
    };

    if id == 0 {
        return String::from("Nie mozna zabic taska jadra (ID=0).");
    }

    let result = x86_64::instructions::interrupts::without_interrupts(|| {
        let mut sched = SCHEDULER.lock();
        if sched.kill_task(TaskId(id)) {
            sched.cleanup_terminated();
            true
        } else {
            false
        }
    });

    if result {
        format!("Zakonczono task ID={}.", id)
    } else {
        format!("Nie znaleziono aktywnego taska o ID={}.", id)
    }
}

fn cmd_exec(args: &str, cwd: &[String]) -> String {
    use crate::kernel::task;
    use crate::kernel::syscall::userprogs;

    let name = args.split_whitespace().next().unwrap_or("");
    match name {
        "hello" => {
            let id = task::spawn("user-hello", userprogs::run_user_hello);
            format!("Uruchomiono user program 'hello' (ID={})", id.0)
        }
        "counter" => {
            let id = task::spawn("user-counter", userprogs::run_user_counter);
            format!("Uruchomiono user program 'counter' (ID={})", id.0)
        }
        _ => {
            // Try to load ELF from RamFS
            let fs = FS.lock();
            let data_opt = fs.read(cwd, name).map(|d| Vec::from(d));
            drop(fs);

            let data_opt = data_opt.or_else(|| {
                let root_files = crate::fs::fat::list_root_files();
                if root_files.is_empty() || root_files[0].contains("Error") || root_files[0].contains("Not valid") {
                    None
                } else {
                    crate::fs::fat::read_file(name)
                }
            });

            if let Some(data) = data_opt {
                match crate::kernel::elf::load_and_map_elf(&data) {
                    Ok(entry_addr) => {
                        let entry_fn: fn() = unsafe { core::mem::transmute(entry_addr) };
                        let id = task::spawn("user-elf", entry_fn);
                        format!("Uruchomiono ELF '{}' (ID={})", name, id.0)
                    }
                    Err(e) => format!("Blad ladowania ELF: {}", e),
                }
            } else {
                format!("Nie znaleziono programu '{}'. Dostepne wbudowane: hello, counter", name)
            }
        }
    }
}

// --- Environment variables ---

use alloc::collections::BTreeMap;
use spin::Mutex;

lazy_static::lazy_static! {
    pub static ref ENV_VARS: Mutex<BTreeMap<String, String>> = {
        let mut map = BTreeMap::new();
        map.insert(String::from("PATH"), String::from("/"));
        map.insert(String::from("HOME"), String::from("/"));
        map.insert(String::from("SHELL"), String::from("polarsh"));
        map.insert(String::from("OS"), String::from("PolarOs"));
        Mutex::new(map)
    };
}

/// Expand $VAR and ${VAR} in a string
pub fn expand_env_vars(input: &str) -> String {
    let env = ENV_VARS.lock();
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            let mut var_name = String::new();
            let braced = chars.peek() == Some(&'{');
            if braced { chars.next(); }

            while let Some(&c) = chars.peek() {
                if braced {
                    if c == '}' { chars.next(); break; }
                    var_name.push(c);
                    chars.next();
                } else if c.is_alphanumeric() || c == '_' {
                    var_name.push(c);
                    chars.next();
                } else {
                    break;
                }
            }

            if let Some(value) = env.get(&var_name) {
                result.push_str(value);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn cmd_env() -> String {
    let env = ENV_VARS.lock();
    let mut s = String::new();
    for (key, value) in env.iter() {
        s.push_str(&format!("{}={}\n", key, value));
    }
    if s.ends_with('\n') { s.pop(); }
    s
}

fn cmd_export(args: &str) -> String {
    let args = args.trim();
    if let Some((key, value)) = args.split_once('=') {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() {
            return String::from("Uzycie: export KLUCZ=WARTOSC");
        }
        let mut env = ENV_VARS.lock();
        env.insert(String::from(key), String::from(value));
        format!("{}={}", key, value)
    } else {
        // Show single variable
        let env = ENV_VARS.lock();
        match env.get(args) {
            Some(val) => format!("{}={}", args, val),
            None => format!("Zmienna '{}' nie istnieje.", args),
        }
    }
}

// --- Extra pipe-friendly commands ---

fn cmd_head(args: &str, cwd: &[String], pipe_input: Option<&str>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let mut n: usize = 10;
    let mut file_arg: Option<&str> = None;

    let mut i = 0;
    while i < parts.len() {
        if parts[i] == "-n" {
            if let Some(num_str) = parts.get(i + 1) {
                n = num_str.parse().unwrap_or(10);
                i += 2;
                continue;
            }
        }
        file_arg = Some(parts[i]);
        i += 1;
    }

    let text = if let Some(input) = pipe_input {
        String::from(input)
    } else if let Some(filename) = file_arg {
        let fs = FS.lock();
        match fs.read(cwd, filename) {
            Some(data) => String::from(core::str::from_utf8(data).unwrap_or("")),
            None => return format!("Plik '{}' nie istnieje.", filename),
        }
    } else {
        return String::from("Uzycie: head [-n N] <plik>");
    };

    let mut result = String::new();
    for line in text.lines().take(n) {
        result.push_str(line);
        result.push('\n');
    }
    if result.ends_with('\n') { result.pop(); }
    result
}

fn cmd_tail(args: &str, cwd: &[String], pipe_input: Option<&str>) -> String {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let mut n: usize = 10;
    let mut file_arg: Option<&str> = None;

    let mut i = 0;
    while i < parts.len() {
        if parts[i] == "-n" {
            if let Some(num_str) = parts.get(i + 1) {
                n = num_str.parse().unwrap_or(10);
                i += 2;
                continue;
            }
        }
        file_arg = Some(parts[i]);
        i += 1;
    }

    let text = if let Some(input) = pipe_input {
        String::from(input)
    } else if let Some(filename) = file_arg {
        let fs = FS.lock();
        match fs.read(cwd, filename) {
            Some(data) => String::from(core::str::from_utf8(data).unwrap_or("")),
            None => return format!("Plik '{}' nie istnieje.", filename),
        }
    } else {
        return String::from("Uzycie: tail [-n N] <plik>");
    };

    let all_lines: Vec<&str> = text.lines().collect();
    let start = if all_lines.len() > n { all_lines.len() - n } else { 0 };
    let mut result = String::new();
    for line in &all_lines[start..] {
        result.push_str(line);
        result.push('\n');
    }
    if result.ends_with('\n') { result.pop(); }
    result
}

fn cmd_sort(args: &str, cwd: &[String], pipe_input: Option<&str>) -> String {
    let text = if let Some(input) = pipe_input {
        String::from(input)
    } else {
        let name = match args.split_whitespace().next() {
            Some(n) => n,
            None => return String::from("Uzycie: sort <plik>"),
        };
        let fs = FS.lock();
        match fs.read(cwd, name) {
            Some(data) => String::from(core::str::from_utf8(data).unwrap_or("")),
            None => return format!("Plik '{}' nie istnieje.", name),
        }
    };

    let mut lines: Vec<&str> = text.lines().collect();
    lines.sort();
    let mut result = String::new();
    for line in lines {
        result.push_str(line);
        result.push('\n');
    }
    if result.ends_with('\n') { result.pop(); }
    result
}

fn cmd_keymap(args: &str) -> String {
    use crate::drivers::keyboard;

    let name = args.trim();
    if name.is_empty() {
        let current = keyboard::current_layout();
        let mut s = format!("Aktualny layout: {}\n", keyboard::layout_name(current));
        s.push_str("Dostepne: us, uk, de, fr, dvorak, colemak");
        return s;
    }

    match keyboard::layout_from_name(name) {
        Some(layout) => {
            keyboard::set_layout(layout);
            format!("Layout zmieniony na: {}", keyboard::layout_name(layout))
        }
        None => {
            format!("Nieznany layout '{}'. Dostepne: us, uk, de, fr, dvorak, colemak", name)
        }
    }
}

fn cmd_uniq(pipe_input: Option<&str>) -> String {
    let text = match pipe_input {
        Some(input) => input,
        None => return String::from("uniq wymaga danych z pipe (np. sort plik | uniq)"),
    };

    let mut result = String::new();
    let mut prev: Option<&str> = None;
    for line in text.lines() {
        if prev != Some(line) {
            result.push_str(line);
            result.push('\n');
            prev = Some(line);
        }
    }
    if result.ends_with('\n') { result.pop(); }
    result
}

```

src/shell/completion.rs
```rust
use alloc::string::String;
use alloc::vec::Vec;
use crate::fs::{FS, FileSystem};

pub const COMMANDS: &[&str] = &[
    "help", "echo", "clear", "ls", "cat", "touch", "write", "rm",
    "mkdir", "cd", "pwd", "uptime", "info", "grep", "wc", "cp",
    "mv", "hexdump", "save", "load", "ps", "spawn", "kill", "exec",
    "fatls", "env", "export", "head", "tail", "sort", "uniq", "keymap",
];

pub fn tab_complete(input: &str, cwd: &[String]) -> (usize, Vec<String>) {
    if let Some(space_pos) = input.rfind(' ') {
        let word_start = space_pos + 1;
        let partial = &input[word_start..];
        let fs = FS.lock();
        let names = fs.names(cwd);
        let matches: Vec<String> = names.into_iter()
            .filter(|n| n.starts_with(partial))
            .collect();
        (word_start, matches)
    } else {
        let matches: Vec<String> = COMMANDS.iter()
            .filter(|c| c.starts_with(input))
            .map(|c| String::from(*c))
            .collect();
        (0, matches)
    }
}

pub fn common_prefix(strings: &[String]) -> String {
    if strings.is_empty() { return String::new(); }
    let first = &strings[0];
    let mut len = first.len();
    for s in &strings[1..] {
        len = len.min(s.len());
        for (i, (a, b)) in first.bytes().zip(s.bytes()).enumerate() {
            if a != b {
                len = len.min(i);
                break;
            }
        }
    }
    String::from(&first[..len])
}

```

src/shell/mod.rs
```rust
pub mod commands;
pub mod completion;

use alloc::string::String;
use alloc::vec::Vec;
use crate::{print, println};
use crate::drivers::keyboard;
use crate::drivers::vga;
use crate::fs::{FS, FileSystem};

const MAX_LINE: usize = 256;
const HISTORY_MAX: usize = 16;

#[macro_export]
macro_rules! shell_error {
    ($($arg:tt)*) => {{
        $crate::drivers::vga::set_color($crate::drivers::vga::Color::LightRed, $crate::drivers::vga::Color::Black);
        $crate::println!($($arg)*);
        $crate::drivers::vga::set_color($crate::drivers::vga::Color::LightGreen, $crate::drivers::vga::Color::Black);
    }};
}

pub struct CommandHistory {
    entries: Vec<String>,
    cursor: usize,
}

impl CommandHistory {
    fn new() -> Self {
        CommandHistory {
            entries: Vec::new(),
            cursor: 0,
        }
    }

    fn push(&mut self, cmd: &str) {
        if self.entries.last().map(|s| s.as_str()) == Some(cmd) {
            self.cursor = self.entries.len();
            return;
        }
        if self.entries.len() >= HISTORY_MAX {
            self.entries.remove(0);
        }
        self.entries.push(String::from(cmd));
        self.cursor = self.entries.len();
    }

    fn up(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        Some(self.entries[self.cursor].as_str())
    }

    fn down(&mut self) -> Option<&str> {
        if self.cursor >= self.entries.len() {
            return None;
        }
        self.cursor += 1;
        if self.cursor < self.entries.len() {
            Some(self.entries[self.cursor].as_str())
        } else {
            None
        }
    }
}

pub fn run() {
    let mut history = CommandHistory::new();
    let mut cwd: Vec<String> = Vec::new();

    println!();
    print_banner();
    println!();
    println!("Wpisz 'help' aby zobaczyc dostepne komendy.");
    println!();

    loop {
        print_prompt(&cwd);
        let line = read_line(&mut history, &cwd);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        history.push(trimmed);
        execute_line(trimmed, &mut cwd);
    }
}

/// Execute a full command line with pipe, redirect, and env var support.
pub fn execute_line(line: &str, cwd: &mut Vec<String>) {
    // 1. Expand environment variables ($VAR, ${VAR})
    let expanded = commands::expand_env_vars(line);
    let line = expanded.trim();
    if line.is_empty() {
        return;
    }

    // 2. Parse I/O redirections from the line
    //    Supported: > file, >> file, < file
    let (pipeline_str, redirect) = parse_redirections(line);

    // 3. Split on pipe '|' and chain commands
    let mut pipe_data: Option<String> = None;

    // If we have input redirection, read the file as initial pipe data
    if let Some(ref input_file) = redirect.input_file {
        let fs = FS.lock();
        match fs.read(cwd, input_file) {
            Some(data) => {
                pipe_data = Some(String::from(core::str::from_utf8(data).unwrap_or("")));
            }
            None => {
                vga::set_color(vga::Color::LightRed, vga::Color::Black);
                println!("Plik wejsciowy '{}' nie istnieje.", input_file);
                vga::set_color(vga::Color::LightGreen, vga::Color::Black);
                return;
            }
        }
    }

    for part in pipeline_str.split('|') {
        let part = part.trim();
        if part.is_empty() { continue; }

        let (cmd, args) = match part.split_once(' ') {
            Some((c, a)) => (c, a),
            None => (part, ""),
        };

        let output = commands::run_command(cmd, args, cwd, pipe_data.as_deref());
        pipe_data = Some(output);
    }

    // 4. Handle output
    if let Some(output) = pipe_data {
        match redirect.output_mode {
            OutputMode::Print => {
                if !output.is_empty() {
                    println!("{}", output);
                }
            }
            OutputMode::Write(ref filename) => {
                let mut fs = FS.lock();
                if fs.write(cwd, filename, output.as_bytes()) {
                    println!("Zapisano do '{}'.", filename);
                } else {
                    vga::set_color(vga::Color::LightRed, vga::Color::Black);
                    println!("Nie mozna zapisac do '{}'.", filename);
                    vga::set_color(vga::Color::LightGreen, vga::Color::Black);
                }
            }
            OutputMode::Append(ref filename) => {
                let mut fs = FS.lock();
                // Read existing content, append new output
                let mut existing = match fs.read(cwd, filename) {
                    Some(data) => Vec::from(data),
                    None => Vec::new(),
                };
                if !existing.is_empty() && existing.last() != Some(&b'\n') {
                    existing.push(b'\n');
                }
                existing.extend_from_slice(output.as_bytes());
                if fs.write(cwd, filename, &existing) {
                    println!("Dopisano do '{}'.", filename);
                } else {
                    vga::set_color(vga::Color::LightRed, vga::Color::Black);
                    println!("Nie mozna dopisac do '{}'.", filename);
                    vga::set_color(vga::Color::LightGreen, vga::Color::Black);
                }
            }
        }
    }
}

enum OutputMode {
    Print,
    Write(String),
    Append(String),
}

struct Redirect {
    input_file: Option<String>,
    output_mode: OutputMode,
}

fn parse_redirections(line: &str) -> (&str, Redirect) {
    let mut redirect = Redirect {
        input_file: None,
        output_mode: OutputMode::Print,
    };

    // Find the last occurrence of redirect operators (not inside pipes)
    // We search from the end of the line for >, >>, <
    // Simple approach: find the last pipe segment and check for redirects there

    // Find output redirect: >> or >
    if let Some(pos) = line.rfind(">>") {
        let filename = line[pos + 2..].trim();
        if !filename.is_empty() && !filename.contains('|') {
            let before = &line[..pos];
            redirect.output_mode = OutputMode::Append(String::from(filename));

            // Check for input redirect in the remaining part
            if let Some(ipos) = before.rfind('<') {
                let input_name = before[ipos + 1..].trim();
                if !input_name.is_empty() {
                    redirect.input_file = Some(String::from(input_name));
                    return (&before[..ipos], redirect);
                }
            }
            return (before, redirect);
        }
    }

    if let Some(pos) = line.rfind('>') {
        // Make sure it's not >>
        if pos == 0 || line.as_bytes()[pos - 1] != b'>' {
            let filename = line[pos + 1..].trim();
            if !filename.is_empty() && !filename.contains('|') {
                let before = &line[..pos];
                redirect.output_mode = OutputMode::Write(String::from(filename));

                if let Some(ipos) = before.rfind('<') {
                    let input_name = before[ipos + 1..].trim();
                    if !input_name.is_empty() {
                        redirect.input_file = Some(String::from(input_name));
                        return (&before[..ipos], redirect);
                    }
                }
                return (before, redirect);
            }
        }
    }

    // Check for input redirect only
    if let Some(pos) = line.rfind('<') {
        let input_name = line[pos + 1..].trim();
        if !input_name.is_empty() && !input_name.contains('|') && !input_name.contains('>') {
            redirect.input_file = Some(String::from(input_name));
            return (&line[..pos], redirect);
        }
    }

    (line, redirect)
}

fn print_banner() {
    vga::set_color(vga::Color::LightCyan, vga::Color::Black);
    println!("========================================");
    println!("         PolarOs v0.1.0");
    println!("   Dopierdolony System w ruscie");
    println!("========================================");
    vga::set_color(vga::Color::LightGreen, vga::Color::Black);
}

pub fn print_prompt(cwd: &[String]) {
    vga::set_color(vga::Color::LightCyan, vga::Color::Black);
    print!("myos");
    vga::set_color(vga::Color::White, vga::Color::Black);
    print!(":");
    if cwd.is_empty() {
        print!("/");
    } else {
        for component in cwd {
            print!("/{}", component);
        }
    }
    print!("> ");
    vga::set_color(vga::Color::LightGreen, vga::Color::Black);
}

fn clear_input(len: usize) {
    for _ in 0..len {
        vga::delete_last_char();
    }
}

fn read_line(history: &mut CommandHistory, cwd: &[String]) -> String {
    let mut buf = String::new();

    loop {
        match keyboard::read_key() {
            keyboard::KeyEvent::Char('\n') => {
                println!();
                return buf;
            }
            keyboard::KeyEvent::Char('\u{8}') => {
                if !buf.is_empty() {
                    buf.pop();
                    vga::delete_last_char();
                }
            }
            keyboard::KeyEvent::Char('\t') => {
                let (word_start, completions) = completion::tab_complete(&buf, cwd);
                let partial_len = buf.len() - word_start;
                if completions.len() == 1 {
                    let suffix = &completions[0][partial_len..];
                    buf.push_str(suffix);
                    print!("{}", suffix);
                } else if completions.len() > 1 {
                    let prefix = completion::common_prefix(&completions);
                    if prefix.len() > partial_len {
                        let suffix = &prefix[partial_len..];
                        buf.push_str(suffix);
                        print!("{}", suffix);
                    } else {
                        println!();
                        for c in &completions {
                            print!("  {}", c);
                        }
                        println!();
                        print_prompt(cwd);
                        print!("{}", buf);
                    }
                }
            }
            keyboard::KeyEvent::Char(c) if c >= ' ' && buf.len() < MAX_LINE => {
                buf.push(c);
                print!("{}", c);
            }
            keyboard::KeyEvent::ArrowUp => {
                if let Some(entry) = history.up() {
                    clear_input(buf.len());
                    buf.clear();
                    buf.push_str(entry);
                    print!("{}", buf);
                }
            }
            keyboard::KeyEvent::ArrowDown => {
                clear_input(buf.len());
                buf.clear();
                if let Some(entry) = history.down() {
                    buf.push_str(entry);
                    print!("{}", buf);
                }
            }
            _ => {}
        }
    }
}

```

