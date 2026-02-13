<div align="center">

<img src="PolarOS.png" alt="PolarOS" width="600"/>

# PolarOS

🧊 A custom x86_64 operating system written in Rust

[![Rust](https://img.shields.io/badge/language-Rust-orange?logo=rust)](https://www.rust-lang.org/)
[![OS](https://img.shields.io/badge/type-Operating%20System-blue)]()

</div>

---

## Features

- **Custom kernel** — GDT, IDT, PIC, timer, memory management (paging, heap, frame allocator)
- **GUI** — framebuffer-based compositor, windowing system, cursor, wallpaper, theming
- **Drivers** — VGA, ATA, PS/2 keyboard & mouse, serial
- **File systems** — FAT and RAM filesystem support
- **Shell** — built-in shell with tab completion
- **Multitasking** — scheduler, context switching, ELF loading
- **Syscalls** — user-space program support

## Building

```bash
cargo bootimage
```

## Running

```bash
qemu-system-x86_64 -drive format=raw,file=target/x86_64-myos/debug/bootimage-systemoperacyjny.bin
```
