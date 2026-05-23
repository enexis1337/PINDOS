// x86_64 Interrupt Descriptor Table implementation
use core::arch::asm;

const IDT_ENTRIES: usize = 256;

#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

#[repr(C, packed)]
struct IdtDescriptor {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; IDT_ENTRIES] = [IdtEntry {
    offset_low: 0,
    selector: 0,
    ist: 0,
    type_attr: 0,
    offset_mid: 0,
    offset_high: 0,
    reserved: 0,
}; IDT_ENTRIES];

static mut IDTR: IdtDescriptor = IdtDescriptor {
    limit: (core::mem::size_of::<IdtEntry>() * IDT_ENTRIES - 1) as u16,
    base: 0,
};

// Interrupt handler function type
type InterruptHandler = extern "C" fn(&mut InterruptFrame);

extern "C" {
    fn divide_error();
    fn debug_exception();
    fn nmi();
    fn breakpoint();
    fn overflow();
    fn bound_range_exceeded();
    fn invalid_opcode();
    fn device_not_available();
    fn double_fault();
    fn coprocessor_segment_overrun();
    fn invalid_tss();
    fn segment_not_present();
    fn stack_fault();
    fn general_protection_fault();
    fn page_fault();
    fn x87_floating_point();
    fn alignment_check();
    fn machine_check();
    fn simd_floating_point();
    fn virtualization_exception();
    fn syscall_handler();
}

/// Interrupt frame pushed by the CPU on interrupt
#[repr(C, packed)]
pub struct InterruptFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub interrupt_number: u64,
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

/// Set an IDT entry
unsafe fn set_idt_entry(index: usize, handler: usize, selector: u16, ist: u8, dpl: u8, present: bool) {
    let entry = &mut IDT[index];
    entry.offset_low = (handler & 0xFFFF) as u16;
    entry.selector = selector;
    entry.ist = ist;
    entry.type_attr = 0x8E | ((dpl as u8 & 0x3) << 5) | if present { 0x80 } else { 0 };
    entry.offset_mid = ((handler >> 16) & 0xFFFF) as u16;
    entry.offset_high = (handler >> 32) as u32;
    entry.reserved = 0;
}

/// Initialize the Interrupt Descriptor Table
pub fn init() {
    unsafe {
        // Get the address of the IDT
        let idt_addr = IDT.as_ptr() as u64;
        IDTR.base = idt_addr;
        IDTR.limit = (core::mem::size_of::<IdtEntry>() * IDT_ENTRIES - 1) as u16;
        
        // Set up exception handlers
        set_idt_entry(0, divide_error as usize, 0x08, 0, 0, true);
        set_idt_entry(1, debug_exception as usize, 0x08, 0, 0, true);
        set_idt_entry(2, nmi as usize, 0x08, 0, 0, true);
        set_idt_entry(3, breakpoint as usize, 0x08, 0, 3, true);
        set_idt_entry(4, overflow as usize, 0x08, 0, 3, true);
        set_idt_entry(5, bound_range_exceeded as usize, 0x08, 0, 0, true);
        set_idt_entry(6, invalid_opcode as usize, 0x08, 0, 0, true);
        set_idt_entry(7, device_not_available as usize, 0x08, 0, 0, true);
        set_idt_entry(8, double_fault as usize, 0x08, 0, 0, true);
        set_idt_entry(10, invalid_tss as usize, 0x08, 0, 0, true);
        set_idt_entry(11, segment_not_present as usize, 0x08, 0, 0, true);
        set_idt_entry(12, stack_fault as usize, 0x08, 0, 0, true);
        set_idt_entry(13, general_protection_fault as usize, 0x08, 0, 0, true);
        set_idt_entry(14, page_fault as usize, 0x08, 0, 0, true);
        set_idt_entry(16, x87_floating_point as usize, 0x08, 0, 0, true);
        set_idt_entry(17, alignment_check as usize, 0x08, 0, 0, true);
        set_idt_entry(18, machine_check as usize, 0x08, 0, 0, true);
        set_idt_entry(19, simd_floating_point as usize, 0x08, 0, 0, true);
        set_idt_entry(20, virtualization_exception as usize, 0x08, 0, 0, true);
        
        // Set up syscall handler (for fast system calls)
        set_idt_entry(0x80, syscall_handler as usize, 0x08, 0, 3, true);
        
        // Load IDT using lidt instruction
        asm!("lidt [{}]", in(reg) &IDTR);
    }
}

/// Enable interrupts
#[inline]
pub fn enable_interrupts() {
    unsafe {
        asm!("sti");
    }
}

/// Disable interrupts
#[inline]
pub fn disable_interrupts() {
    unsafe {
        asm!("cli");
    }
}

/// Check if interrupts are enabled
#[inline]
pub fn interrupts_enabled() -> bool {
    let rflags: u64;
    unsafe {
        asm!("pushfq; pop {}", out(reg) rflags);
    }
    (rflags & 0x200) != 0
}