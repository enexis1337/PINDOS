/// Multiboot2 entry point для загрузки через GRUB
/// Эта функция вызывается GRUB вместо _start из main.rs

/// Multiboot2 boot info структура
#[repr(C)]
pub struct Multiboot2BootInfo {
    pub size: u32,
    pub reserved: u32,
    pub tags: [u8; 0],
}

/// Entry point который GRUB вызывает через Multiboot2
/// ebx указывает на адрес Multiboot2 boot info
#[no_mangle]
pub unsafe extern "C" fn _start_multiboot2(magic: u32, boot_info_addr: u32) -> ! {
    // Проверяем Multiboot2 magic число
    const MULTIBOOT2_MAGIC: u32 = 0x36d76289;
    
    if magic != MULTIBOOT2_MAGIC {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }

    // Инициализируем COM1 для вывода
    unsafe {
        crate::drivers::serial::SERIAL.lock().init();
    }

    crate::kprintln!("====================================================");
    crate::kprintln!("      PINDOS OS - Hammam Kernel (Multiboot2)       ");
    crate::kprintln!("====================================================");
    crate::kprintln!("Boot magic: {:#X} (expected {:#X})", magic, MULTIBOOT2_MAGIC);
    crate::kprintln!("Boot info at: {:#X}", boot_info_addr);

    // Parse Multiboot2 tags
    parse_multiboot2_tags(boot_info_addr);

    crate::kprintln!("\nKernel initialized via Multiboot2");
    crate::kprintln!("System is ready.");
    
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
    
    let mut tag_ptr = unsafe { boot_info.tags.as_ptr() };
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
