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
