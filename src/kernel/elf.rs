use xmas_elf::{ElfFile, program::Type};
use x86_64::{
    structures::paging::{Page, PageTableFlags, Mapper, Size4KiB, FrameAllocator},
    VirtAddr,
};
use crate::kernel::memory::MEMORY_MANAGER;

pub fn load_and_map_elf(data: &[u8]) -> Result<u64, &'static str> {
    let elf = ElfFile::new(data).map_err(|_| "Failed to parse ELF")?;

    if elf.header.pt1.class() != xmas_elf::header::Class::SixtyFour {
        return Err("Not 64-bit ELF");
    }
    if elf.header.pt2.machine().as_machine() != xmas_elf::header::Machine::X86_64 {
        return Err("Not x86_64 ELF");
    }

    let mut mm = MEMORY_MANAGER.lock();
    // Destructure to split borrows
    let mm_ref = &mut *mm;
    let mapper = mm_ref.mapper.as_mut().ok_or("Memory manager not initialized")?;
    let allocator = mm_ref.frame_allocator.as_mut().ok_or("Frame allocator not initialized")?;

    for ph in elf.program_iter() {
        if ph.get_type() == Ok(Type::Load) {
            let virt_start_addr = ph.virtual_addr();
            let mem_size = ph.mem_size();
            let file_size = ph.file_size();
            let file_offset = ph.offset();

            let start_page = Page::containing_address(VirtAddr::new(virt_start_addr));
            let end_page = Page::containing_address(VirtAddr::new(virt_start_addr + mem_size - 1));

            let mut flags = PageTableFlags::PRESENT;
            if !ph.flags().is_execute() {
                flags |= PageTableFlags::NO_EXECUTE;
            }
            if ph.flags().is_write() {
                flags |= PageTableFlags::WRITABLE;
            }

            for page in Page::<Size4KiB>::range_inclusive(start_page, end_page) {
                // Check if already mapped
                if mapper.translate_page(page).is_err() {
                    let frame = allocator.allocate_frame().ok_or("Out of memory")?;
                    unsafe {
                        mapper.map_to(page, frame, flags, allocator)
                            .map_err(|_| "Map failed")?
                            .flush();
                    }
                }
            }

            // Copy data
            let dest_ptr = virt_start_addr as *mut u8;
            unsafe {
                // Copy segment data
                if file_size > 0 {
                    let data_start = file_offset as usize;
                    let data_end = data_start + file_size as usize;
                    let src = &data[data_start..data_end];
                    core::ptr::copy_nonoverlapping(src.as_ptr(), dest_ptr, src.len());
                }
                // Zero out BSS
                if mem_size > file_size {
                    let bss_start = dest_ptr.add(file_size as usize);
                    core::ptr::write_bytes(bss_start, 0, (mem_size - file_size) as usize);
                }
            }
        }
    }

    Ok(elf.header.pt2.entry_point())
}
