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
