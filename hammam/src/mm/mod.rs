pub mod heap;
pub mod paging;
pub mod physical;
pub mod page_cache;
pub mod userptr;

pub use heap::HEAP_ALLOCATOR;
pub use paging::{active_pml4, map_page, unmap_page, translate, translate_flags, PageFlags, PageTable, PageTableEntry};
pub use physical::{AllocError, BuddyAllocator, PhysFrame, PHYSICAL_ALLOCATOR};
pub use page_cache::{PageCache, CachedPage, PAGE_CACHE, page_cache_get_or_load, page_cache_evict};
pub use userptr::{validate_user_slice, validate_user_slice_mut, UserPtrError};
