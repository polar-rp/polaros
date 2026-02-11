pub mod paging;
pub mod frame_allocator;
pub mod heap;

use spin::Mutex;
use x86_64::structures::paging::OffsetPageTable;
use self::frame_allocator::BootInfoFrameAllocator;

pub struct MemoryManager {
    pub mapper: Option<OffsetPageTable<'static>>,
    pub frame_allocator: Option<BootInfoFrameAllocator>,
}

pub static MEMORY_MANAGER: Mutex<MemoryManager> = Mutex::new(MemoryManager {
    mapper: None,
    frame_allocator: None,
});
