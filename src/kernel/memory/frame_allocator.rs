use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
    PhysAddr,
};

const PAGE_SIZE: u64 = 4096;

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next_addr: u64,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next_addr: 0,
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        for region in self.memory_map.iter() {
            if region.region_type != MemoryRegionType::Usable {
                continue;
            }
            let region_start = region.range.start_addr();
            let region_end = region.range.end_addr();
            let addr = if self.next_addr >= region_start {
                self.next_addr
            } else {
                region_start
            };
            if addr < region_end {
                self.next_addr = addr + PAGE_SIZE;
                return Some(PhysFrame::containing_address(PhysAddr::new(addr)));
            }
        }
        None
    }
}
