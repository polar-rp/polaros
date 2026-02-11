use alloc::string::String;
use alloc::vec::Vec;
use crate::{print, println, shell_error};
use crate::fs::{FS, FileSystem, RemoveResult};
use crate::drivers::vga;

pub fn execute(line: &str, cwd: &mut Vec<String>) {
    let (cmd, args) = match line.split_once(' ') {
        Some((c, a)) => (c, a),
        None => (line, ""),
    };

    match cmd {
        "help" => cmd_help(),
        "echo" => cmd_echo(args),
        "clear" => cmd_clear(),
        "ls" => cmd_ls(cwd),
        "cat" => cmd_cat(args, cwd),
        "touch" => cmd_touch(args, cwd),
        "write" => cmd_write(args, cwd),
        "rm" => cmd_rm(args, cwd),
        "mkdir" => cmd_mkdir(args, cwd),
        "cd" => cmd_cd(args, cwd),
        "pwd" => cmd_pwd(cwd),
        "uptime" => cmd_uptime(),
        "info" => cmd_info(),
        "grep" => cmd_grep(args, cwd),
        "wc" => cmd_wc(args, cwd),
        "cp" => cmd_cp(args, cwd),
        "mv" => cmd_mv(args, cwd),
        "hexdump" => cmd_hexdump(args, cwd),
        "save" => cmd_save(),
        "load" => cmd_load(cwd),
        "ps" => cmd_ps(),
        "spawn" => cmd_spawn(args),
        "kill" => cmd_kill(args),
        "exec" => cmd_exec(args),
        _ => {
            shell_error!("Nieznana komenda: '{}'. Wpisz 'help' aby zobaczyc liste komend.", cmd);
        }
    }
}

fn cmd_help() {
    vga::set_color(vga::Color::Yellow, vga::Color::Black);
    println!("Dostepne komendy:");
    vga::set_color(vga::Color::White, vga::Color::Black);
    println!("  help              - Wyswietl te pomoc");
    println!("  echo <tekst>      - Wyswietl tekst");
    println!("  clear             - Wyczysc ekran");
    println!("  ls                - Lista plikow i katalogow");
    println!("  cat <plik>        - Wyswietl zawartosc pliku");
    println!("  touch <plik>      - Utworz pusty plik");
    println!("  write <plik> <t>  - Zapisz tekst do pliku");
    println!("  rm <nazwa>        - Usun plik lub pusty katalog");
    println!("  mkdir <nazwa>     - Utworz katalog");
    println!("  cd <katalog>      - Zmien katalog (cd .. / cd /)");
    println!("  pwd               - Wyswietl biezacy katalog");
    println!("  grep <wz> <plik>  - Szukaj wzorca w pliku");
    println!("  wc <plik>         - Policz linie/slowa/bajty");
    println!("  cp <src> <dst>    - Kopiuj plik");
    println!("  mv <src> <dst>    - Przenies/zmien nazwe pliku");
    println!("  hexdump <plik>    - Zrzut szesnastkowy pliku");
    println!("  save              - Zapisz FS na dysk ATA");
    println!("  load              - Wczytaj FS z dysku ATA");
    println!("  uptime            - Czas dzialania systemu");
    println!("  info              - Informacje systemowe");
    println!("  ps                - Lista procesow/taskow");
    println!("  spawn <nazwa>     - Uruchom demo task");
    println!("  kill <id>         - Zakoncz task o podanym ID");
    println!("  exec <program>    - Uruchom program uzytkownika");
    vga::set_color(vga::Color::LightGreen, vga::Color::Black);
}

fn cmd_echo(args: &str) {
    println!("{}", args);
}

fn cmd_clear() {
    vga::clear_screen();
}

fn cmd_ls(cwd: &[String]) {
    let fs = FS.lock();
    match fs.list(cwd) {
        Some(entries) => {
            if entries.is_empty() {
                println!("(pusty katalog)");
            } else {
                for entry in &entries {
                    if entry.is_dir {
                        vga::set_color(vga::Color::LightBlue, vga::Color::Black);
                        println!("  {}/", entry.name);
                        vga::set_color(vga::Color::LightGreen, vga::Color::Black);
                    } else {
                        println!("  {} ({} bajtow)", entry.name, entry.size);
                    }
                }
            }
        }
        None => {
            shell_error!("Katalog nie istnieje.");
        }
    }
}

fn cmd_cat(args: &str, cwd: &[String]) {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => {
            println!("Uzycie: cat <nazwa_pliku>");
            return;
        }
    };
    let fs = FS.lock();
    match fs.read(cwd, name) {
        Some(data) => {
            let text = core::str::from_utf8(data).unwrap_or("<dane binarne>");
            println!("{}", text);
        }
        None => {
            shell_error!("Plik '{}' nie istnieje.", name);
        }
    }
}

fn cmd_touch(args: &str, cwd: &[String]) {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => {
            println!("Uzycie: touch <nazwa_pliku>");
            return;
        }
    };
    let mut fs = FS.lock();
    if fs.create(cwd, name) {
        println!("Utworzono plik '{}'.", name);
    } else {
        println!("'{}' juz istnieje.", name);
    }
}

fn cmd_write(args: &str, cwd: &[String]) {
    let (name, content) = match args.split_once(' ') {
        Some((n, c)) => (n, c),
        None => {
            println!("Uzycie: write <nazwa_pliku> <tekst>");
            return;
        }
    };
    if name.is_empty() {
        println!("Uzycie: write <nazwa_pliku> <tekst>");
        return;
    }
    let mut fs = FS.lock();
    if fs.write(cwd, name, content.as_bytes()) {
        println!("Zapisano {} bajtow do '{}'.", content.len(), name);
    } else {
        shell_error!("Nie mozna zapisac do '{}'.", name);
    }
}

fn cmd_rm(args: &str, cwd: &[String]) {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => {
            println!("Uzycie: rm <nazwa>");
            return;
        }
    };
    let mut fs = FS.lock();
    match fs.remove(cwd, name) {
        RemoveResult::Ok => println!("Usunieto '{}'.", name),
        RemoveResult::NotFound => shell_error!("'{}' nie istnieje.", name),
        RemoveResult::DirNotEmpty => shell_error!("Katalog '{}' nie jest pusty.", name),
    }
}

fn cmd_mkdir(args: &str, cwd: &[String]) {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => {
            println!("Uzycie: mkdir <nazwa>");
            return;
        }
    };
    let mut fs = FS.lock();
    if fs.mkdir(cwd, name) {
        println!("Utworzono katalog '{}'.", name);
    } else {
        shell_error!("'{}' juz istnieje.", name);
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
                    shell_error!("'{}' nie istnieje.", name);
                    return;
                }
                fs.is_dir(cwd, name)
            };
            if is_dir {
                cwd.push(String::from(name));
            } else {
                shell_error!("'{}' nie jest katalogiem.", name);
            }
        }
    }
}

fn cmd_pwd(cwd: &[String]) {
    if cwd.is_empty() {
        println!("/");
    } else {
        for component in cwd {
            print!("/{}", component);
        }
        println!();
    }
}

fn cmd_uptime() {
    let t = crate::kernel::timer::ticks();
    let hz = crate::kernel::timer::TIMER_HZ as u64;
    let total_secs = t / hz;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    println!("Uptime: {}h {:02}m {:02}s ({} tickow @ {}Hz)", hours, minutes, secs, t, hz);
}

fn cmd_info() {
    let (heap_used, heap_free) = crate::kernel::memory::heap::heap_stats();
    let total_kb = (heap_used + heap_free) / 1024;
    let used_kb = heap_used / 1024;

    vga::set_color(vga::Color::Yellow, vga::Color::Black);
    println!("=== Informacje systemowe ===");
    vga::set_color(vga::Color::White, vga::Color::Black);
    println!("  System:        PolarOs v0.1.0");
    println!("  Architektura:  x86_64");
    println!("  Jezyk:         Rust (nightly)");
    println!("  Tryb wideo:    VGA tekst 80x25");
    println!("  Klawiatura:    PS/2 (IRQ1)");
    println!("  Heap:          {}/{} KiB", used_kb, total_kb);
    println!("  Filesystem:    RamFs (drzewo katalogow)");
    println!("  Dysk ATA:      {}", if crate::drivers::ata::is_available() { "dostepny" } else { "niedostepny" });
    vga::set_color(vga::Color::LightGreen, vga::Color::Black);
}

fn cmd_grep(args: &str, cwd: &[String]) {
    let (pattern, filename) = match args.split_once(' ') {
        Some((p, f)) => (p, f.trim()),
        None => {
            println!("Uzycie: grep <wzorzec> <plik>");
            return;
        }
    };
    let fs = FS.lock();
    match fs.read(cwd, filename) {
        Some(data) => {
            let text = core::str::from_utf8(data).unwrap_or("");
            let mut found = false;
            for line in text.lines() {
                if line.contains(pattern) {
                    println!("{}", line);
                    found = true;
                }
            }
            if !found {
                println!("Brak wynikow dla '{}'.", pattern);
            }
        }
        None => shell_error!("Plik '{}' nie istnieje.", filename),
    }
}

fn cmd_wc(args: &str, cwd: &[String]) {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => {
            println!("Uzycie: wc <plik>");
            return;
        }
    };
    let fs = FS.lock();
    match fs.read(cwd, name) {
        Some(data) => {
            let bytes = data.len();
            let text = core::str::from_utf8(data).unwrap_or("");
            let lines = text.lines().count();
            let words = text.split_whitespace().count();
            println!("  {} linii  {} slow  {} bajtow  {}", lines, words, bytes, name);
        }
        None => shell_error!("Plik '{}' nie istnieje.", name),
    }
}

fn cmd_cp(args: &str, cwd: &[String]) {
    let (src, dst) = match args.split_once(' ') {
        Some((s, d)) => (s, d.trim()),
        None => {
            println!("Uzycie: cp <zrodlo> <cel>");
            return;
        }
    };
    let data = {
        let fs = FS.lock();
        match fs.read(cwd, src) {
            Some(d) => Vec::from(d),
            None => {
                shell_error!("Plik '{}' nie istnieje.", src);
                return;
            }
        }
    };
    let mut fs = FS.lock();
    if fs.write(cwd, dst, &data) {
        println!("Skopiowano '{}' -> '{}'.", src, dst);
    } else {
        shell_error!("Nie mozna zapisac do '{}'.", dst);
    }
}

fn cmd_mv(args: &str, cwd: &[String]) {
    let (src, dst) = match args.split_once(' ') {
        Some((s, d)) => (s, d.trim()),
        None => {
            println!("Uzycie: mv <zrodlo> <cel>");
            return;
        }
    };
    if src == dst {
        return;
    }
    let data = {
        let fs = FS.lock();
        match fs.read(cwd, src) {
            Some(d) => Vec::from(d),
            None => {
                shell_error!("Plik '{}' nie istnieje.", src);
                return;
            }
        }
    };
    let mut fs = FS.lock();
    if fs.write(cwd, dst, &data) {
        fs.remove(cwd, src);
        println!("Przeniesiono '{}' -> '{}'.", src, dst);
    } else {
        shell_error!("Nie mozna przeniesc do '{}'.", dst);
    }
}

fn cmd_hexdump(args: &str, cwd: &[String]) {
    let name = match args.split_whitespace().next() {
        Some(n) => n,
        None => {
            println!("Uzycie: hexdump <plik>");
            return;
        }
    };
    let fs = FS.lock();
    match fs.read(cwd, name) {
        Some(data) => {
            for (i, chunk) in data.chunks(16).enumerate() {
                print!("{:08x}  ", i * 16);
                for (j, byte) in chunk.iter().enumerate() {
                    print!("{:02x} ", byte);
                    if j == 7 { print!(" "); }
                }
                for j in chunk.len()..16 {
                    print!("   ");
                    if j == 7 { print!(" "); }
                }
                print!(" |");
                for byte in chunk {
                    if *byte >= 0x20 && *byte <= 0x7e {
                        print!("{}", *byte as char);
                    } else {
                        print!(".");
                    }
                }
                println!("|");
            }
        }
        None => shell_error!("Plik '{}' nie istnieje.", name),
    }
}

fn cmd_save() {
    use crate::drivers::ata;

    if !ata::is_available() {
        shell_error!("Dysk ATA niedostepny.");
        return;
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
        shell_error!("Blad zapisu naglowka.");
        return;
    }

    let mut offset = first_chunk;
    let mut sector = ata::DATA_START_SECTOR + 1;
    while offset < data.len() {
        let mut buf = [0u8; 512];
        let chunk = (data.len() - offset).min(512);
        buf[..chunk].copy_from_slice(&data[offset..offset + chunk]);
        if !ata::write_sector(sector, &buf) {
            shell_error!("Blad zapisu sektora {}.", sector);
            return;
        }
        offset += chunk;
        sector += 1;
    }

    let sectors_written = sector - ata::DATA_START_SECTOR;
    println!("Zapisano {} bajtow ({} sektorow).", data.len(), sectors_written);
}

fn cmd_load(cwd: &mut Vec<String>) {
    use crate::drivers::ata;
    use crate::fs::ramfs::RamFs;

    if !ata::is_available() {
        shell_error!("Dysk ATA niedostepny.");
        return;
    }

    let mut header = [0u8; 512];
    if !ata::read_sector(ata::DATA_START_SECTOR, &mut header) {
        shell_error!("Blad odczytu naglowka.");
        return;
    }

    if &header[0..4] != b"PLRS" {
        shell_error!("Brak zapisanego systemu plikow na dysku.");
        return;
    }

    let total_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;

    let mut data = Vec::with_capacity(total_len);

    let first_chunk = total_len.min(504);
    data.extend_from_slice(&header[8..8 + first_chunk]);

    let mut sector = ata::DATA_START_SECTOR + 1;
    while data.len() < total_len {
        let mut buf = [0u8; 512];
        if !ata::read_sector(sector, &mut buf) {
            shell_error!("Blad odczytu sektora {}.", sector);
            return;
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
            println!("Wczytano system plikow ({} bajtow).", total_len);
        }
        None => {
            shell_error!("Uszkodzone dane na dysku.");
        }
    }
}

fn cmd_ps() {
    use crate::kernel::task::{SCHEDULER, TaskState};

    vga::set_color(vga::Color::Yellow, vga::Color::Black);
    println!("  ID  STAN         NAZWA");
    vga::set_color(vga::Color::White, vga::Color::Black);

    let sched = SCHEDULER.lock();
    for task in sched.task_list() {
        let state_str = match task.state {
            TaskState::Ready => "Ready      ",
            TaskState::Running => "Running    ",
            TaskState::Terminated => "Terminated ",
        };
        println!("  {:3} {}  {}", task.id.0, state_str, task.name);
    }
    vga::set_color(vga::Color::LightGreen, vga::Color::Black);
}

fn demo_counter() {
    use crate::kernel::task::yield_now;
    use crate::kernel::timer;

    let start = timer::ticks();
    for i in 0..5 {
        let now = timer::ticks();
        let secs = (now - start) / timer::TIMER_HZ as u64;
        println!("[demo] Krok {}/5  ({}s od startu)", i + 1, secs);
        // Busy-wait ~1 second then yield
        let target = now + timer::TIMER_HZ as u64;
        while timer::ticks() < target {
            x86_64::instructions::hlt();
        }
        yield_now();
    }
    println!("[demo] Task zakonczony.");
}

fn demo_hello() {
    use crate::kernel::task::yield_now;
    use crate::kernel::timer;

    for i in 0..3 {
        println!("[hello] Pozdrowienia nr {} z taska!", i + 1);
        let target = timer::ticks() + timer::TIMER_HZ as u64;
        while timer::ticks() < target {
            x86_64::instructions::hlt();
        }
        yield_now();
    }
    println!("[hello] Koniec.");
}

fn cmd_spawn(args: &str) {
    use crate::kernel::task;

    let name = args.split_whitespace().next().unwrap_or("counter");
    match name {
        "counter" => {
            let id = task::spawn("demo-counter", demo_counter);
            println!("Uruchomiono task 'counter' (ID={})", id.0);
        }
        "hello" => {
            let id = task::spawn("demo-hello", demo_hello);
            println!("Uruchomiono task 'hello' (ID={})", id.0);
        }
        _ => {
            println!("Dostepne demo taski: counter, hello");
        }
    }
}

fn cmd_kill(args: &str) {
    use crate::kernel::task::{SCHEDULER, TaskId};

    let id_str = match args.split_whitespace().next() {
        Some(s) => s,
        None => {
            println!("Uzycie: kill <id>");
            return;
        }
    };

    let id: u64 = match id_str.parse() {
        Ok(n) => n,
        Err(_) => {
            shell_error!("Nieprawidlowy ID: '{}'", id_str);
            return;
        }
    };

    if id == 0 {
        shell_error!("Nie mozna zabic taska jądra (ID=0).");
        return;
    }

    let mut sched = SCHEDULER.lock();
    if sched.kill_task(TaskId(id)) {
        println!("Zakonczono task ID={}.", id);
        sched.cleanup_terminated();
    } else {
        shell_error!("Nie znaleziono aktywnego taska o ID={}.", id);
    }
}

fn cmd_exec(args: &str) {
    use crate::kernel::task;
    use crate::kernel::syscall::userprogs;

    let name = args.split_whitespace().next().unwrap_or("");
    match name {
        "hello" => {
            let id = task::spawn("user-hello", userprogs::run_user_hello);
            println!("Uruchomiono user program 'hello' (ID={})", id.0);
        }
        "counter" => {
            let id = task::spawn("user-counter", userprogs::run_user_counter);
            println!("Uruchomiono user program 'counter' (ID={})", id.0);
        }
        _ => {
            println!("Dostepne programy: hello, counter");
            println!("Uzycie: exec <program>");
        }
    }
}
