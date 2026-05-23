#![no_std]

use spin::Mutex;
use bit_field::BitField;

/// Physical frame size (4KB)
pub const PAGE_SIZE: usize = 4096;

/// Maximum physical memory supported (256GB)
pub const MAX_PHYSICAL_MEMORY: usize = 256 * 1024 * 1024 * 1024;

/// Number of frames
pub const NUM_FRAMES: usize = MAX_PHYSICAL_MEMORY / PAGE_SIZE;

/// Bitmap allocator for physical frames
pub struct PhysicalFrameAllocator {
    bitmap: Mutex<[u64; NUM_FRAMES / 64 / 8]>,
    next_free_frame: Mutex<usize>,
    total_frames: usize,
    reserved_frames: usize,
}

impl PhysicalFrameAllocator {
    /// Create a new allocator
    pub const fn new() -> Self {
        PhysicalFrameAllocator {
            bitmap: Mutex::new([0; NUM_FRAMES / 64 / 8]),
            next_free_frame: Mutex::new(0),
            total_frames: NUM_FRAMES,
            reserved_frames: 0,
        }
    }

    /// Initialize the allocator with known memory regions
    pub fn initialize(&mut self) {
        // Mark kernel and reserved regions as used
        self.reserve_region(0x0, 0x100000); // First 1MB (BIOS, VGA, etc.)
        self.reserve_region(0x100000, 0x1000000); // Low memory
        self.reserve_region(0x100000000, 0x100000000); // High memory region
    }

    /// Reserve a memory region (mark as used)
    pub fn reserve_region(&mut self, start: usize, size: usize) {
        let start_frame = start / PAGE_SIZE;
        let num_frames = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        
        let mut bitmap = self.bitmap.lock();
        for i in start_frame..start_frame + num_frames {
            if i < NUM_FRAMES {
                bitmap[i / 64].set_bit(i % 64, true);
                self.reserved_frames += 1;
            }
        }
    }

    /// Allocate a single frame
    pub fn allocate_frame(&self) -> Option<usize> {
        let mut bitmap = self.bitmap.lock();
        let mut start = self.next_free_frame.lock();
        
        // Search from current position
        for i in *start..NUM_FRAMES {
            if !bitmap[i / 64].get_bit(i % 64) {
                bitmap[i / 64].set_bit(i % 64, true);
                *start = i + 1;
                return Some(i * PAGE_SIZE);
            }
        }
        
        // Wrap around and search from beginning
        for i in 0..*start {
            if !bitmap[i / 64].get_bit(i % 64) {
                bitmap[i / 64].set_bit(i % 64, true);
                *start = i + 1;
                return Some(i * PAGE_SIZE);
            }
        }
        
        None // No free frames
    }

    /// Allocate multiple contiguous frames
    pub fn allocate_frames(&self, count: usize) -> Option<usize> {
        if count == 0 {
            return Some(0);
        }
        
        let mut bitmap = self.bitmap.lock();
        let mut start = self.next_free_frame.lock();
        
        for i in *start..NUM_FRAMES - count {
            let mut found = true;
            for j in 0..count {
                if bitmap[(i + j) / 64].get_bit((i + j) % 64) {
                    found = false;
                    break;
                }
            }
            
            if found {
                for j in 0..count {
                    bitmap[(i + j) / 64].set_bit((i + j) % 64, true);
                }
                *start = i + count;
                return Some(i * PAGE_SIZE);
            }
        }
        
        None
    }

    /// Free a frame
    pub fn free_frame(&self, frame_addr: usize) {
        let frame = frame_addr / PAGE_SIZE;
        if frame < NUM_FRAMES {
            let mut bitmap = self.bitmap.lock();
            bitmap[frame / 64].set_bit(frame % 64, false);
        }
    }

    /// Get total free frames
    pub fn free_frames(&self) -> usize {
        let bitmap = self.bitmap.lock();
        let mut free = 0;
        for i in 0..bitmap.len() {
            free += bitmap[i].count_ones() as usize;
        }
        NUM_FRAMES - free - self.reserved_frames
    }

    /// Get total frames
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }
}

/// Global frame allocator instance
pub static FRAME_ALLOCATOR: PhysicalFrameAllocator = PhysicalFrameAllocator::new();

/// Virtual memory area
#[derive(Debug, Clone)]
pub struct VmArea {
    pub start: usize,
    pub end: usize,
    pub flags: VmFlags,
    pub name: &'static str,
}

bitflags! {
    pub struct VmFlags: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXEC = 1 << 2;
        const PRIVATE = 1 << 3;
        const SHARED = 1 << 4;
        const GROWABLE = 1 << 5;
    }
}

/// Address space for a process
pub struct AddressSpace {
    pub areas: Vec<VmArea>,
    pub page_table: usize, // Physical address of page table
}

impl AddressSpace {
    /// Create a new empty address space
    pub const fn new() -> Self {
        AddressSpace {
            areas: Vec::new(),
            page_table: 0,
        }
    }

    /// Map a region of virtual memory
    pub fn mmap(&mut self, start: usize, size: usize, flags: VmFlags, name: &'static str) -> Option<usize> {
        let page_start = start & !(PAGE_SIZE - 1);
        let page_end = ((start + size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1));
        
        let area = VmArea {
            start: page_start,
            end: page_end,
            flags,
            name,
        };
        
        self.areas.push(area);
        Some(page_start)
    }

    /// Unmap a region of virtual memory
    pub fn munmap(&mut self, start: usize, size: usize) -> bool {
        let page_start = start & !(PAGE_SIZE - 1);
        let page_end = ((start + size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1));
        
        let initial_len = self.areas.len();
        self.areas.retain(|area| {
            !(area.start >= page_start && area.end <= page_end)
        });
        
        initial_len != self.areas.len()
    }

    /// Find a free region large enough
    pub fn find_free_region(&self, size: usize, align: usize) -> Option<usize> {
        let mut current = 0x10000; // Start from low memory
        
        for area in &self.areas {
            if current + size <= area.start {
                let aligned = (current + align - 1) & !(align - 1);
                if aligned + size <= area.start {
                    return Some(aligned);
                }
            }
            current = area.end.max(current);
        }
        
        None
    }
}