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
