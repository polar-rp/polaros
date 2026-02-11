use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
use spin::Mutex;

const BUF_SIZE: usize = 128;

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
    static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
        Mutex::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        ));
}

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
