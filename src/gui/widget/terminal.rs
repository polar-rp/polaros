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

        // Split on pipe and chain commands
        let mut pipe_data: Option<String> = None;
        for part in trimmed.split('|') {
            let part = part.trim();
            if part.is_empty() { continue; }
            let output = self.run_command(part, pipe_data.as_deref());
            pipe_data = Some(output);
        }

        if let Some(output) = pipe_data {
            if !output.is_empty() {
                for line in output.lines() {
                    self.push_line(line);
                }
            }
        }
    }

    fn run_command(&mut self, line: &str, pipe_input: Option<&str>) -> String {
        let (cmd, args) = match line.split_once(' ') {
            Some((c, a)) => (c, a),
            None => (line, ""),
        };

        match cmd {
            "help" => self.cmd_help(),
            "echo" => String::from(args),
            "clear" => {
                self.lines.clear();
                String::new()
            }
            "ls" => self.cmd_ls(),
            "cat" => self.cmd_cat(args),
            "touch" => self.cmd_touch(args),
            "write" => self.cmd_write(args),
            "rm" => self.cmd_rm(args),
            "mkdir" => self.cmd_mkdir(args),
            "cd" => { self.cmd_cd(args); String::new() }
            "pwd" => self.cmd_pwd(),
            "uptime" => self.cmd_uptime(),
            "info" => self.cmd_info(),
            "ps" => self.cmd_ps(),
            "grep" => self.cmd_grep(args, pipe_input),
            "wc" => self.cmd_wc(args, pipe_input),
            _ => format!("Nieznana komenda: '{}'. Wpisz 'help'.", cmd),
        }
    }

    fn cmd_help(&self) -> String {
        let mut s = String::new();
        s.push_str("Dostepne komendy:\n");
        s.push_str("  help      - Pomoc\n");
        s.push_str("  echo <t>  - Wyswietl tekst\n");
        s.push_str("  clear     - Wyczysc terminal\n");
        s.push_str("  ls        - Lista plikow\n");
        s.push_str("  cat <f>   - Pokaz plik\n");
        s.push_str("  touch <f> - Utworz plik\n");
        s.push_str("  write <f> <t> - Zapisz\n");
        s.push_str("  rm <n>    - Usun\n");
        s.push_str("  mkdir <n> - Nowy katalog\n");
        s.push_str("  cd <d>    - Zmien katalog\n");
        s.push_str("  pwd       - Biezacy katalog\n");
        s.push_str("  uptime    - Czas dzialania\n");
        s.push_str("  info      - Info systemowe\n");
        s.push_str("  ps        - Lista taskow\n");
        s.push_str("  grep <w> [f] - Szukaj wzorca\n");
        s.push_str("  wc [plik] - Policz linie/slowa\n");
        s.push_str("Pipe: cmd1 | cmd2");
        s
    }

    fn cmd_ls(&self) -> String {
        use crate::fs::{FS, FileSystem};
        let fs = FS.lock();
        match fs.list(&self.cwd) {
            Some(entries) => {
                if entries.is_empty() {
                    return String::from("(pusty katalog)");
                }
                let mut s = String::new();
                for entry in &entries {
                    if entry.is_dir {
                        s.push_str("  ");
                        s.push_str(&entry.name);
                        s.push_str("/\n");
                    } else {
                        s.push_str(&format!("  {} ({} B)\n", entry.name, entry.size));
                    }
                }
                if s.ends_with('\n') { s.pop(); }
                s
            }
            None => String::from("Katalog nie istnieje."),
        }
    }

    fn cmd_cat(&self, args: &str) -> String {
        use crate::fs::{FS, FileSystem};
        let name = match args.split_whitespace().next() {
            Some(n) => n,
            None => return String::from("Uzycie: cat <plik>"),
        };
        let fs = FS.lock();
        match fs.read(&self.cwd, name) {
            Some(data) => {
                String::from(core::str::from_utf8(data).unwrap_or("<dane binarne>"))
            }
            None => format!("Plik '{}' nie istnieje.", name),
        }
    }

    fn cmd_touch(&self, args: &str) -> String {
        use crate::fs::{FS, FileSystem};
        let name = match args.split_whitespace().next() {
            Some(n) => n,
            None => return String::from("Uzycie: touch <plik>"),
        };
        let mut fs = FS.lock();
        if fs.create(&self.cwd, name) {
            format!("Utworzono '{}'.", name)
        } else {
            format!("'{}' juz istnieje.", name)
        }
    }

    fn cmd_write(&self, args: &str) -> String {
        use crate::fs::{FS, FileSystem};
        let (name, content) = match args.split_once(' ') {
            Some((n, c)) => (n, c),
            None => return String::from("Uzycie: write <plik> <tekst>"),
        };
        let mut fs = FS.lock();
        if fs.write(&self.cwd, name, content.as_bytes()) {
            format!("Zapisano {} B do '{}'.", content.len(), name)
        } else {
            format!("Nie mozna zapisac do '{}'.", name)
        }
    }

    fn cmd_rm(&self, args: &str) -> String {
        use crate::fs::{FS, FileSystem, RemoveResult};
        let name = match args.split_whitespace().next() {
            Some(n) => n,
            None => return String::from("Uzycie: rm <nazwa>"),
        };
        let mut fs = FS.lock();
        match fs.remove(&self.cwd, name) {
            RemoveResult::Ok => format!("Usunieto '{}'.", name),
            RemoveResult::NotFound => format!("'{}' nie istnieje.", name),
            RemoveResult::DirNotEmpty => format!("Katalog '{}' nie jest pusty.", name),
        }
    }

    fn cmd_mkdir(&self, args: &str) -> String {
        use crate::fs::{FS, FileSystem};
        let name = match args.split_whitespace().next() {
            Some(n) => n,
            None => return String::from("Uzycie: mkdir <nazwa>"),
        };
        let mut fs = FS.lock();
        if fs.mkdir(&self.cwd, name) {
            format!("Utworzono katalog '{}'.", name)
        } else {
            format!("'{}' juz istnieje.", name)
        }
    }

    fn cmd_cd(&mut self, args: &str) {
        use crate::fs::{FS, FileSystem};
        let target = match args.split_whitespace().next() {
            Some(t) => t,
            None => { self.cwd.clear(); return; }
        };
        match target {
            "/" => self.cwd.clear(),
            ".." => { self.cwd.pop(); }
            "." => {}
            name => {
                let (exists, is_dir) = {
                    let fs = FS.lock();
                    (fs.exists(&self.cwd, name), fs.is_dir(&self.cwd, name))
                };
                if !exists {
                    self.push_line(&format!("'{}' nie istnieje.", name));
                } else if is_dir {
                    self.cwd.push(String::from(name));
                } else {
                    self.push_line(&format!("'{}' nie jest katalogiem.", name));
                }
            }
        }
    }

    fn cmd_pwd(&self) -> String {
        if self.cwd.is_empty() {
            String::from("/")
        } else {
            let mut s = String::new();
            for c in &self.cwd {
                s.push('/');
                s.push_str(c);
            }
            s
        }
    }

    fn cmd_uptime(&self) -> String {
        let t = crate::kernel::timer::ticks();
        let hz = crate::kernel::timer::TIMER_HZ as u64;
        let total_secs = t / hz;
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        let s = total_secs % 60;
        format!("Uptime: {}h {:02}m {:02}s ({} tickow)", h, m, s, t)
    }

    fn cmd_info(&self) -> String {
        let (heap_used, heap_free) = crate::kernel::memory::heap::heap_stats();
        let total_kb = (heap_used + heap_free) / 1024;
        let used_kb = heap_used / 1024;
        let mut s = String::new();
        s.push_str("=== Info systemowe ===\n");
        s.push_str("  System:    PolarOs v0.1.0\n");
        s.push_str("  Arch:      x86_64\n");
        s.push_str("  Tryb:      VGA 320x200 256c\n");
        s.push_str(&format!("  Heap:      {}/{} KiB", used_kb, total_kb));
        s
    }

    fn cmd_grep(&self, args: &str, pipe_input: Option<&str>) -> String {
        let parts: Vec<&str> = args.split_whitespace().collect();
        let pattern = match parts.first() {
            Some(p) => *p,
            None => return String::from("Uzycie: grep <wzorzec> [plik]"),
        };

        let text = if let Some(input) = pipe_input {
            String::from(input)
        } else {
            use crate::fs::{FS, FileSystem};
            let filename = match parts.get(1) {
                Some(f) => *f,
                None => return String::from("Uzycie: grep <wzorzec> <plik>"),
            };
            let fs = FS.lock();
            match fs.read(&self.cwd, filename) {
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

    fn cmd_wc(&self, args: &str, pipe_input: Option<&str>) -> String {
        let text = if let Some(input) = pipe_input {
            String::from(input)
        } else {
            use crate::fs::{FS, FileSystem};
            let name = match args.split_whitespace().next() {
                Some(n) => n,
                None => return String::from("Uzycie: wc <plik>"),
            };
            let fs = FS.lock();
            match fs.read(&self.cwd, name) {
                Some(data) => String::from(core::str::from_utf8(data).unwrap_or("")),
                None => return format!("Plik '{}' nie istnieje.", name),
            }
        };

        let bytes = text.len();
        let lines = text.lines().count();
        let words = text.split_whitespace().count();
        format!("  {} linii  {} slow  {} bajtow", lines, words, bytes)
    }

    fn cmd_ps(&self) -> String {
        use crate::kernel::task::{SCHEDULER, TaskState};
        let mut s = String::new();
        s.push_str("  ID  STAN        NAZWA\n");
        let sched = SCHEDULER.lock();
        for task in sched.task_list() {
            let state_str = match task.state {
                TaskState::Ready => "Ready     ",
                TaskState::Running => "Running   ",
                TaskState::Terminated => "Terminated",
            };
            s.push_str(&format!("  {:3} {} {}\n", task.id.0, state_str, task.name));
        }
        if s.ends_with('\n') { s.pop(); }
        s
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
