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
