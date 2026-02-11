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
