pub mod commands;
pub mod completion;

use alloc::string::String;
use alloc::vec::Vec;
use crate::{print, println};
use crate::drivers::keyboard;
use crate::drivers::vga;

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
        commands::execute(trimmed, &mut cwd);
    }
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
