#![no_std]
#![no_main]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use systemoperacyjny::kernel::memory::{paging, frame_allocator, heap};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // Initialize GDT, IDT, PICs
    systemoperacyjny::init();

    // Initialize memory management
    let phys_mem_offset = x86_64::VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { paging::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { frame_allocator::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    // Initialize heap
    heap::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");

    // Initialize filesystem with sample files
    systemoperacyjny::fs::init();

    // Launch GUI
    systemoperacyjny::gui::run()
}
