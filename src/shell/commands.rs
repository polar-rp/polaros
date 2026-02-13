use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use crate::fs::{FS, FileSystem, RemoveResult};

pub struct CommandEntry {
    pub name: &'static str,
    help: &'static str,
    handler: Option<fn(&str, &mut Vec<String>, Option<&str>) -> String>,
}

/// Single source of truth for all commands: name, help text, and handler.
/// Commands with `handler: None` are handled specially in `run_command`.
static COMMAND_TABLE: &[CommandEntry] = &[
    CommandEntry { name: "help",    help: "Wyswietl te pomoc",            handler: Some(cmd_help_wrapper) },
    CommandEntry { name: "echo",    help: "Wyswietl tekst",              handler: Some(cmd_echo_wrapper) },
    CommandEntry { name: "clear",   help: "Wyczysc ekran",               handler: None },
    CommandEntry { name: "ls",      help: "Lista plikow i katalogow",    handler: Some(cmd_ls_wrapper) },
    CommandEntry { name: "cat",     help: "Wyswietl zawartosc pliku",    handler: Some(cmd_cat_wrapper) },
    CommandEntry { name: "touch",   help: "Utworz pusty plik",           handler: Some(cmd_touch_wrapper) },
    CommandEntry { name: "write",   help: "Zapisz tekst do pliku",       handler: Some(cmd_write_wrapper) },
    CommandEntry { name: "rm",      help: "Usun plik lub pusty katalog", handler: Some(cmd_rm_wrapper) },
    CommandEntry { name: "mkdir",   help: "Utworz katalog",              handler: Some(cmd_mkdir_wrapper) },
    CommandEntry { name: "cd",      help: "Zmien katalog (cd .. / cd /)", handler: None },
    CommandEntry { name: "pwd",     help: "Wyswietl biezacy katalog",    handler: Some(cmd_pwd_wrapper) },
    CommandEntry { name: "grep",    help: "Szukaj wzorca w pliku",       handler: Some(cmd_grep_wrapper) },
    CommandEntry { name: "wc",      help: "Policz linie/slowa/bajty",    handler: Some(cmd_wc_wrapper) },
    CommandEntry { name: "cp",      help: "Kopiuj plik",                 handler: Some(cmd_cp_wrapper) },
    CommandEntry { name: "mv",      help: "Przenies/zmien nazwe pliku",  handler: Some(cmd_mv_wrapper) },
    CommandEntry { name: "hexdump", help: "Zrzut szesnastkowy pliku",    handler: Some(cmd_hexdump_wrapper) },
    CommandEntry { name: "head",    help: "Pokaz pierwszych N linii",    handler: Some(cmd_head_wrapper) },
    CommandEntry { name: "tail",    help: "Pokaz ostatnich N linii",     handler: Some(cmd_tail_wrapper) },
    CommandEntry { name: "sort",    help: "Sortuj linie",                handler: Some(cmd_sort_wrapper) },
    CommandEntry { name: "uniq",    help: "Usun powtorzenia (pipe)",     handler: Some(cmd_uniq_wrapper) },
    CommandEntry { name: "save",    help: "Zapisz FS na dysk ATA",       handler: Some(cmd_save_wrapper) },
    CommandEntry { name: "load",    help: "Wczytaj FS z dysku ATA",      handler: None },
    CommandEntry { name: "uptime",  help: "Czas dzialania systemu",      handler: Some(cmd_uptime_wrapper) },
    CommandEntry { name: "info",    help: "Informacje systemowe",        handler: Some(cmd_info_wrapper) },
    CommandEntry { name: "ps",      help: "Lista procesow/taskow",       handler: Some(cmd_ps_wrapper) },
    CommandEntry { name: "spawn",   help: "Uruchom demo task",           handler: Some(cmd_spawn_wrapper) },
    CommandEntry { name: "kill",    help: "Zakoncz task o podanym ID",   handler: Some(cmd_kill_wrapper) },
    CommandEntry { name: "exec",    help: "Uruchom program uzytkownika", handler: Some(cmd_exec_wrapper) },
    CommandEntry { name: "env",     help: "Pokaz zmienne srodowiskowe",  handler: Some(cmd_env_wrapper) },
    CommandEntry { name: "export",  help: "Ustaw zmienna srodowiskowa",  handler: Some(cmd_export_wrapper) },
    CommandEntry { name: "fatls",   help: "Lista plikow FAT32",          handler: Some(cmd_fatls_wrapper) },
    CommandEntry { name: "keymap",  help: "Pokaz/zmien layout klawiatury", handler: Some(cmd_keymap_wrapper) },
];

/// Get command names from the table (used by completion).
pub fn command_names() -> &'static [CommandEntry] {
    COMMAND_TABLE
}

// Wrapper functions that adapt existing handlers to the unified signature.
fn cmd_help_wrapper(_: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_help() }
fn cmd_echo_wrapper(a: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_echo(a) }
fn cmd_ls_wrapper(_: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_ls(c) }
fn cmd_cat_wrapper(a: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_cat(a, c) }
fn cmd_touch_wrapper(a: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_touch(a, c) }
fn cmd_write_wrapper(a: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_write(a, c) }
fn cmd_rm_wrapper(a: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_rm(a, c) }
fn cmd_mkdir_wrapper(a: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_mkdir(a, c) }
fn cmd_pwd_wrapper(_: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_pwd(c) }
fn cmd_grep_wrapper(a: &str, c: &mut Vec<String>, p: Option<&str>) -> String { cmd_grep(a, c, p) }
fn cmd_wc_wrapper(a: &str, c: &mut Vec<String>, p: Option<&str>) -> String { cmd_wc(a, c, p) }
fn cmd_cp_wrapper(a: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_cp(a, c) }
fn cmd_mv_wrapper(a: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_mv(a, c) }
fn cmd_hexdump_wrapper(a: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_hexdump(a, c) }
fn cmd_head_wrapper(a: &str, c: &mut Vec<String>, p: Option<&str>) -> String { cmd_head(a, c, p) }
fn cmd_tail_wrapper(a: &str, c: &mut Vec<String>, p: Option<&str>) -> String { cmd_tail(a, c, p) }
fn cmd_sort_wrapper(a: &str, c: &mut Vec<String>, p: Option<&str>) -> String { cmd_sort(a, c, p) }
fn cmd_uniq_wrapper(_: &str, _: &mut Vec<String>, p: Option<&str>) -> String { cmd_uniq(p) }
fn cmd_save_wrapper(_: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_save() }
fn cmd_uptime_wrapper(_: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_uptime() }
fn cmd_info_wrapper(_: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_info() }
fn cmd_ps_wrapper(_: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_ps() }
fn cmd_spawn_wrapper(a: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_spawn(a) }
fn cmd_kill_wrapper(a: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_kill(a) }
fn cmd_exec_wrapper(a: &str, c: &mut Vec<String>, _: Option<&str>) -> String { cmd_exec(a, c) }
fn cmd_env_wrapper(_: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_env() }
fn cmd_export_wrapper(a: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_export(a) }
fn cmd_fatls_wrapper(_: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_fatls() }
fn cmd_keymap_wrapper(a: &str, _: &mut Vec<String>, _: Option<&str>) -> String { cmd_keymap(a) }

/// Read text either from pipe input, or from a file in the filesystem.
fn read_text_input(pipe_input: Option<&str>, filename: Option<&str>, cwd: &[String], usage: &str) -> Result<String, String> {
    if let Some(input) = pipe_input {
        Ok(String::from(input))
    } else if let Some(name) = filename {
        let fs = FS.lock();
        match fs.read(cwd, name) {
            Some(data) => Ok(String::from(core::str::from_utf8(data).unwrap_or(""))),
            None => Err(format!("Plik '{}' nie istnieje.", name)),
        }
    } else {
        Err(String::from(usage))
    }
}

/// Remove trailing newline from result string.
fn trim_trailing_newline(s: &mut String) {
    if s.ends_with('\n') { s.pop(); }
}

/// Parse `-n N [filename]` arguments common to head/tail.
fn parse_n_args(args: &str, default_n: usize) -> (usize, Option<&str>) {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let mut n = default_n;
    let mut file_arg: Option<&str> = None;
    let mut i = 0;
    while i < parts.len() {
        if parts[i] == "-n" {
            if let Some(num_str) = parts.get(i + 1) {
                n = num_str.parse().unwrap_or(default_n);
                i += 2;
                continue;
            }
        }
        file_arg = Some(parts[i]);
        i += 1;
    }
    (n, file_arg)
}

/// Execute a single command and return its output as a String.
/// `pipe_input` is the output of the previous command in a pipeline (if any).
pub fn run_command(cmd: &str, args: &str, cwd: &mut Vec<String>, pipe_input: Option<&str>) -> String {
    // Handle special commands that can't use the table (need &mut cwd or side effects)
    match cmd {
        "clear" => { crate::drivers::vga::clear_screen(); return String::new() }
        "cd" => { cmd_cd(args, cwd); return String::new() }
        "load" => { return cmd_load(cwd) }
        _ => {}
    }

    // Look up in command table
    for entry in COMMAND_TABLE {
        if entry.name == cmd {
            if let Some(handler) = entry.handler {
                return handler(args, cwd, pipe_input);
            }
        }
    }

    format!("Nieznana komenda: '{}'. Wpisz 'help' aby zobaczyc liste komend.", cmd)
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
    trim_trailing_newline(&mut s);
    s
}

fn cmd_help() -> String {
    let mut s = String::from("Dostepne komendy:\n");
    for entry in COMMAND_TABLE {
        s.push_str(&format!("  {:16}- {}\n", entry.name, entry.help));
    }
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
            trim_trailing_newline(&mut s);
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

    let text = match read_text_input(pipe_input, parts.get(1).copied(), cwd, "Uzycie: grep <wzorzec> <plik>") {
        Ok(t) => t,
        Err(e) => return e,
    };

    let mut result = String::new();
    for line in text.lines() {
        if line.contains(pattern) {
            result.push_str(line);
            result.push('\n');
        }
    }
    trim_trailing_newline(&mut result);
    if result.is_empty() {
        format!("Brak wynikow dla '{}'.", pattern)
    } else {
        result
    }
}

fn cmd_wc(args: &str, cwd: &[String], pipe_input: Option<&str>) -> String {
    let text = match read_text_input(pipe_input, args.split_whitespace().next(), cwd, "Uzycie: wc <plik>") {
        Ok(t) => t,
        Err(e) => return e,
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
            trim_trailing_newline(&mut s);
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
    trim_trailing_newline(&mut s);
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
    trim_trailing_newline(&mut s);
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
    let (n, file_arg) = parse_n_args(args, 10);
    let text = match read_text_input(pipe_input, file_arg, cwd, "Uzycie: head [-n N] <plik>") {
        Ok(t) => t,
        Err(e) => return e,
    };

    let mut result = String::new();
    for line in text.lines().take(n) {
        result.push_str(line);
        result.push('\n');
    }
    trim_trailing_newline(&mut result);
    result
}

fn cmd_tail(args: &str, cwd: &[String], pipe_input: Option<&str>) -> String {
    let (n, file_arg) = parse_n_args(args, 10);
    let text = match read_text_input(pipe_input, file_arg, cwd, "Uzycie: tail [-n N] <plik>") {
        Ok(t) => t,
        Err(e) => return e,
    };

    let all_lines: Vec<&str> = text.lines().collect();
    let start = if all_lines.len() > n { all_lines.len() - n } else { 0 };
    let mut result = String::new();
    for line in &all_lines[start..] {
        result.push_str(line);
        result.push('\n');
    }
    trim_trailing_newline(&mut result);
    result
}

fn cmd_sort(args: &str, cwd: &[String], pipe_input: Option<&str>) -> String {
    let text = match read_text_input(pipe_input, args.split_whitespace().next(), cwd, "Uzycie: sort <plik>") {
        Ok(t) => t,
        Err(e) => return e,
    };

    let mut lines: Vec<&str> = text.lines().collect();
    lines.sort();
    let mut result = String::new();
    for line in lines {
        result.push_str(line);
        result.push('\n');
    }
    trim_trailing_newline(&mut result);
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
    trim_trailing_newline(&mut result);
    result
}
