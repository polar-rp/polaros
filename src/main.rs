#![no_std]
#![no_main]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use systemoperacyjny::kernel::memory::{paging, frame_allocator, heap};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // Initialize GDT, IDT, PICs
    systemoperacyjny::serial_println!("[BOOT] Starting init...");
    systemoperacyjny::init();
    systemoperacyjny::serial_println!("[BOOT] Init done");

    // Initialize memory management
    systemoperacyjny::serial_println!("[BOOT] Setting up memory...");
    let phys_mem_offset = x86_64::VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { paging::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { frame_allocator::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    // Initialize heap
    heap::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");
    systemoperacyjny::serial_println!("[BOOT] Heap ready");

    // Save to global memory manager
    {
        let mut mm = systemoperacyjny::kernel::memory::MEMORY_MANAGER.lock();
        mm.mapper = Some(mapper);
        mm.frame_allocator = Some(frame_allocator);
    }
    systemoperacyjny::serial_println!("[BOOT] Memory manager saved");

    // Late init: enable interrupts + ATA (requires heap)
    systemoperacyjny::init_late();

    // Initialize filesystem with sample files
    systemoperacyjny::fs::init();
    systemoperacyjny::serial_println!("[BOOT] Filesystem initialized");

    // Launch GUI
    systemoperacyjny::serial_println!("[BOOT] Launching GUI...");
    systemoperacyjny::gui::run()
}
