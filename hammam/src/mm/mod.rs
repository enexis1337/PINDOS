pub mod heap;
pub mod paging;
pub mod physical;

pub use heap::HEAP_ALLOCATOR;
pub use paging::{active_pml4, map_page, unmap_page, translate, PageFlags, PageTable, PageTableEntry};
pub use physical::{AllocError, BuddyAllocator, PhysFrame, PHYSICAL_ALLOCATOR};
