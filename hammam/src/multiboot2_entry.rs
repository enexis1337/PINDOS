/// Multiboot2 entry point для загрузки через GRUB
/// Эта функция вызывается GRUB вместо _start из main.rs

/// Минимальная IDT структура для обработки исключений
#[repr(C, align(16))]
#[derive(Copy, Clone)]
struct IDTEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

#[repr(C, align(16))]
struct IDTR {
    limit: u16,
    base: u64,
}

static mut IDT: [IDTEntry; 256] = [IDTEntry {
    offset_low: 0,
    selector: 0,
    ist: 0,
    flags: 0,
    offset_mid: 0,
    offset_high: 0,
    reserved: 0,
}; 256];

static mut IDTR: IDTR = IDTR {
    limit: 0,
    base: 0,
};

/// Dummy exception handler
extern "C" fn dummy_exception_handler() {
    unsafe {
        core::arch::asm!(
            "cli",
            "2:",
            "hlt",
            "jmp 2b",
            options(noreturn)
        );
    }
}

/// Setup minimal IDT
unsafe fn setup_idt() {
    let handler_addr = dummy_exception_handler as u64;
    
    // Setup all 256 entries to point to dummy handler
    for i in 0..256 {
        unsafe {
            IDT[i] = IDTEntry {
                offset_low: (handler_addr & 0xFFFF) as u16,
                selector: 0x08, // Code segment from GDT
                ist: 0,
                flags: 0x8E, // Present, DPL=0, Type=Interrupt Gate
                offset_mid: ((handler_addr >> 16) & 0xFFFF) as u16,
                offset_high: ((handler_addr >> 32) & 0xFFFFFFFF) as u32,
                reserved: 0,
            };
        }
    }
    
    // Setup IDTR
    unsafe {
        IDTR.limit = (core::mem::size_of::<IDTEntry>() * 256 - 1) as u16;
        IDTR.base = IDT.as_ptr() as u64;
        
        // Load IDT
        core::arch::asm!(
            "lidt [{}]",
            in(reg) &IDTR as *const IDTR,
            options(nostack, preserves_flags)
        );
    }
}

/// Multiboot2 boot info структура
#[repr(C)]
pub struct Multiboot2BootInfo {
    pub size: u32,
    pub reserved: u32,
    pub tags: [u8; 0],
}

/// Entry point который GRUB вызывает через Multiboot2
/// Вызывается из 64-bit режима после переключения в boot.rs
#[no_mangle]
pub unsafe extern "C" fn _start_multiboot2(magic: u64, boot_info_addr: u64) -> ! {
    // КРИТИЧЕСКИ ВАЖНО: Установить IDT перед любыми операциями
    unsafe { setup_idt(); }
    
    // Простая функция для вывода строки без SpinMutex
    fn print_str(s: &str) {
        const COM1: u16 = 0x3F8;
        for byte in s.bytes() {
            // Ждем пока передатчик готов
            loop {
                let lsr: u8;
                unsafe {
                    core::arch::asm!(
                        "in al, dx",
                        out("al") lsr,
                        in("dx") COM1 + 5,
                        options(nostack, preserves_flags)
                    );
                }
                if (lsr & 0x20) != 0 {
                    break;
                }
            }
            
            // Отправляем байт
            let out_byte = if byte == b'\n' { b'\r' } else { byte };
            unsafe {
                core::arch::asm!(
                    "out dx, al",
                    in("dx") COM1,
                    in("al") out_byte,
                    options(nostack, preserves_flags)
                );
            }
            if byte == b'\n' {
                // Отправляем \n после \r
                loop {
                    let lsr: u8;
                    unsafe {
                        core::arch::asm!(
                            "in al, dx",
                            out("al") lsr,
                            in("dx") COM1 + 5,
                            options(nostack, preserves_flags)
                        );
                    }
                    if (lsr & 0x20) != 0 {
                        break;
                    }
                }
                unsafe {
                    core::arch::asm!(
                        "out dx, al",
                        in("dx") COM1,
                        in("al") b'\n',
                        options(nostack, preserves_flags)
                    );
                }
            }
        }
    }
    
    // DEBUG: Простейший вывод в COM1 БЕЗ инициализации
    unsafe {
        let port: u16 = 0x3F8;
        let byte: u8 = b'P';
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") byte,
            options(nostack, preserves_flags)
        );
    }
    
    // Инициализируем COM1 напрямую (обходим SpinMutex)
    unsafe {
        const COM1_BASE: u16 = 0x3F8;
        core::arch::asm!("out dx, al", in("dx") COM1_BASE + 1, in("al") 0x00u8);
        core::arch::asm!("out dx, al", in("dx") COM1_BASE + 3, in("al") 0x80u8);
        core::arch::asm!("out dx, al", in("dx") COM1_BASE + 0, in("al") 0x03u8);
        core::arch::asm!("out dx, al", in("dx") COM1_BASE + 1, in("al") 0x00u8);
        core::arch::asm!("out dx, al", in("dx") COM1_BASE + 3, in("al") 0x03u8);
        core::arch::asm!("out dx, al", in("dx") COM1_BASE + 2, in("al") 0xC7u8);
        core::arch::asm!("out dx, al", in("dx") COM1_BASE + 4, in("al") 0x0Bu8);
    }
    
    // DEBUG: После инициализации
    unsafe {
        let port: u16 = 0x3F8;
        let byte: u8 = b'Q';
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") byte,
            options(nostack, preserves_flags)
        );
    }
    
    // Используем нашу простую функцию вместо kprintln
    print_str("====================================================\n");
    print_str("      PINDOS OS - Hammam Kernel (Multiboot2)       \n");
    print_str("====================================================\n");
    print_str("[OK] IDT initialized with 256 entries\n");
    print_str("[OK] Exception handlers installed\n");
    print_str("\n");
    
    // Проверяем Multiboot2 magic число
    const MULTIBOOT2_MAGIC: u32 = 0x36d76289;
    let magic_u32 = magic as u32;
    
    print_str("Boot magic: 0x");
    // Простой вывод hex (magic_u32)
    for i in (0..8).rev() {
        let nibble = ((magic_u32 >> (i * 4)) & 0xF) as u8;
        let ch = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
        unsafe {
            core::arch::asm!(
                "out dx, al",
                in("dx") 0x3F8u16,
                in("al") ch,
                options(nostack, preserves_flags)
            );
        }
    }
    print_str("\n");
    
    print_str("Boot info at: 0x");
    for i in (0..16).rev() {
        let nibble = ((boot_info_addr >> (i * 4)) & 0xF) as u8;
        let ch = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
        unsafe {
            core::arch::asm!(
                "out dx, al",
                in("dx") 0x3F8u16,
                in("al") ch,
                options(nostack, preserves_flags)
            );
        }
    }
    print_str("\n");
    
    if magic_u32 != MULTIBOOT2_MAGIC {
        print_str("[ERROR] Invalid Multiboot2 magic! Halting...\n");
        loop {
            unsafe {
                core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
            }
        }
    }

    print_str("\nMultiboot2 magic is valid!\n");
    print_str("Parsing Multiboot2 tags...\n");
    
    // Parse Multiboot2 tags (упрощенно, без вызова функции)
    print_str("Multiboot2 tags parsing skipped for now\n");
    
    print_str("\nKernel initialized via Multiboot2\n");
    print_str("System is ready.\n");
    print_str("\n*** Hammam kernel halted ***\n");
    
    // Остановка процессора
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

fn parse_multiboot2_tags(boot_info_addr: u32) {
    crate::kprintln!("\nParsing Multiboot2 tags:");
    
    let boot_info = unsafe { &*(boot_info_addr as *const Multiboot2BootInfo) };
    crate::kprintln!("Boot info size: {} bytes", boot_info.size);
    
    let mut tag_ptr = boot_info.tags.as_ptr();
    let end_ptr = (boot_info_addr as u32 + boot_info.size) as *const u8;
    
    let mut tag_count = 0u32;
    
    while (tag_ptr as u32) < (end_ptr as u32) {
        let tag = unsafe { &*(tag_ptr as *const Multiboot2Tag) };
        
        if tag.tag_type == 0 && tag.tag_size == 8 {
            // Terminator tag
            crate::kprintln!("  [Tag {}] Terminator", tag_count);
            break;
        }
        
        let tag_name = match tag.tag_type {
            1 => "Boot command line",
            2 => "Bootloader name",
            3 => "Module info",
            4 => "Basic memory info",
            5 => "BIOS boot device",
            6 => "Memory map",
            8 => "Frame buffer",
            9 => "ELF symbols",
            10 => "APM table",
            12 => "ACPI (old RSDP)",
            14 => "ACPI (new RSDP)",
            15 => "Network info",
            16 => "SMBIOS",
            17 => "ACPI (another variant)",
            18 => "Load base address",
            _ => "Unknown tag type",
        };
        
        crate::kprintln!("  [Tag {}] Type: {} ({}), Size: {}", tag_count, tag.tag_type, tag_name, tag.tag_size);
        
        // Align to 8-byte boundary
        let next_tag_offset = (tag.tag_size + 7) & !7;
        tag_ptr = unsafe { tag_ptr.add(next_tag_offset as usize) };
        tag_count += 1;
    }
    
    crate::kprintln!("Total tags parsed: {}", tag_count);
}

#[repr(C)]
struct Multiboot2Tag {
    tag_type: u32,
    tag_size: u32,
}
