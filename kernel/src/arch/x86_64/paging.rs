// x86_64 Paging implementation
use x86_64::structures::paging::{PageTable, Page, PhysFrame, Size4KiB, Mapper};
use x86_64::VirtAddr;
use x86_64::PhysAddr;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Kernel virtual address where physical memory is identity-mapped
pub const KERNEL_VIRT_BASE: u64 = 0xFFFF_8000_0000_0000;

/// Root page table (PML4)
static mut PML4: PageTable = PageTable::new();

/// Current page table pointer (CR3)
static CURRENT_CR3: AtomicUsize = AtomicUsize::new(0);

/// Initialize paging
pub fn init() {
    unsafe {
        // Create a new page table
        let mut pml4 = &mut PML4;
        
        // Identity-map the first 1GB for early boot
        let start_frame = PhysFrame::containing_address(PhysAddr::new(0));
        let end_frame = PhysFrame::containing_address(PhysAddr::new(0x4000_0000));
        
        for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
            let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64()));
            let flags = x86_64::structures::paging::PageTableFlags::PRESENT
                | x86_64::structures::paging::PageTableFlags::WRITABLE
                | x86_64::structures::paging::PageTableFlags::GLOBAL;
            
            let result = pml4.map_to(page, frame, flags, &mut DummyFrameAllocator);
            if result.is_err() {
                // Handle mapping error
            }
        }
        
        // Map kernel space (higher half)
        let kernel_start = VirtAddr::new(KERNEL_VIRT_BASE);
        let kernel_end = VirtAddr::new(KERNEL_VIRT_BASE + 0x1000_0000); // 256GB kernel space
        
        for page in Page::range_inclusive(
            Page::containing_address(kernel_start),
            Page::containing_address(kernel_end)
        ) {
            let phys_addr = page.start_address() - KERNEL_VIRT_BASE + 0x1000_0000;
            let frame = PhysFrame::containing_address(PhysAddr::new(phys_addr.as_u64()));
            let flags = x86_64::structures::paging::PageTableFlags::PRESENT
                | x86_64::structures::paging::PageTableFlags::WRITABLE
                | x86_64::structures::paging::PageTableFlags::GLOBAL;
            
            let _ = pml4.map_to(page, frame, flags, &mut DummyFrameAllocator);
        }
        
        // Load the page table into CR3
        let cr3_value = VirtAddr::new(&PML4 as *const _ as u64);
        asm!("mov cr3, {}", in(reg) cr3_value.as_u64());
        CURRENT_CR3.store(cr3_value.as_u64() as usize, Ordering::SeqCst);
    }
}

/// Switch to a new page table
pub fn switch_page_table(new_pml4: &PageTable) {
    unsafe {
        let cr3_value = VirtAddr::new(new_pml4 as *const _ as u64);
        asm!("mov cr3, {}", in(reg) cr3_value.as_u64());
        CURRENT_CR3.store(cr3_value.as_u64() as usize, Ordering::SeqCst);
    }
}

/// Get the current page table root
pub fn get_current_cr3() -> usize {
    CURRENT_CR3.load(Ordering::SeqCst)
}

/// Dummy frame allocator for initial mapping
struct DummyFrameAllocator;

unsafe impl x86_64::structures::paging::FrameAllocator<Size4KiB> for DummyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        None
    }
}

/// Translate a virtual address to physical
pub fn translate(virt_addr: VirtAddr) -> Option<PhysAddr> {
    unsafe {
        let pml4 = &PML4;
        let offset = virt_addr.as_u64() & 0xFFF;
        
        // Walk the page table
        let pml4_entry = pml4[virt_addr.p4_index()];
        if !pml4_entry.flags().contains(x86_64::structures::paging::PageTableFlags::PRESENT) {
            return None;
        }
        
        let pdpt = &*(pml4_entry.addr().as_u64() as *const PageTable);
        let pdpt_entry = pdpt[virt_addr.p3_index()];
        if !pdpt_entry.flags().contains(x86_64::structures::paging::PageTableFlags::PRESENT) {
            return None;
        }
        
        let pd = &*(pdpt_entry.addr().as_u64() as *const PageTable);
        let pd_entry = pd[virt_addr.p2_index()];
        if !pd_entry.flags().contains(x86_64::structures::paging::PageTableFlags::PRESENT) {
            return None;
        }
        
        let pt = &*(pd_entry.addr().as_u64() as *const PageTable);
        let pt_entry = pt[virt_addr.p1_index()];
        if !pt_entry.flags().contains(x86_64::structures::paging::PageTableFlags::PRESENT) {
            return None;
        }
        
        Some(PhysAddr::new(pt_entry.addr().as_u64() | offset as u64))
    }
}