#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::all)]

extern crate alloc;

pub mod boot_info;
pub mod drivers;
pub mod mm;
pub mod acpi;
pub mod arch;
pub mod sched;
pub mod smp;
pub mod loader;
pub mod cap;
pub mod process;
pub mod userspace_blob;
pub mod vfs;
pub mod io;
pub mod block;
pub mod security;

use boot_info::BootInfo;
use alloc::vec::Vec;
use alloc::boxed::Box;

/// Точка входа в ядро Hammam операционной системы PINDOS.
///
/// # Safety
/// Данная функция вызывается загрузчиком (UEFI) напрямую при передаче управления ядру.
/// Указатель `boot_info` должен быть валидной ссылкой на структуру `BootInfo` в статической памяти.
#[no_mangle]
pub unsafe extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    // SAFETY: Вызов инициализации последовательного порта гарантированно безопасен,
    // так как это первая операция с аппаратурой на COM1 до переключения в мультипроцессорный режим.
    unsafe {
        drivers::serial::SERIAL.lock().init();
    }

    // SAFETY: Инициализация GDT с дескрипторами Ring 0 и Ring 3, необходимая для работы ядра.
    arch::gdt::init();

    // SAFETY: Инициализация защитных механизмов ядра (SMEP, SMAP, NX, etc.)
    security::enable_smep_smap();
    security::enable_nx();
    security::init_canary();

    // SAFETY: Инициализация SYSCALL/SYSRET механизма для перехвата системных вызовов из Ring 3.
    arch::syscall::init();

    kprintln!("====================================================");
    kprintln!("      PINDOS OS - Hammam Kernel Bootstrapping       ");
    kprintln!("====================================================");
    kprintln!("Kernel Physical Base Address: {:#X}", boot_info.kernel_phys_base);
    
    if let Some(rsdp) = boot_info.rsdp_addr {
        kprintln!("ACPI RSDP Table Found at: {:#X}", rsdp);
    } else {
        kprintln!("WARNING: ACPI RSDP Table NOT Found!");
    }

    if let Some(fb) = boot_info.framebuffer {
        kprintln!("Graphics Mode Initialized: {}x{} (format: {:?})", fb.width, fb.height, fb.pixel_format);
    } else {
        kprintln!("Graphics Mode: Text/Serial Console Only");
    }

    kprintln!("\nParsing memory map regions...");
    let mut total_usable_mem: u64 = 0;
    for (i, region) in boot_info.memory_map.iter().enumerate() {
        if region.kind == boot_info::MemoryKind::Usable {
            total_usable_mem += region.end - region.start;
        }
        kprintln!("  Region {:02}: [{:#010X} - {:#010X}] -> {:?}", i, region.start, region.end, region.kind);
    }
    
    kprintln!("\nTotal Usable RAM: {} MiB", total_usable_mem / (1024 * 1024));

    kprintln!("\nInitializing Buddy Allocator for Physical Frames...");
    {
        let mut allocator = mm::PHYSICAL_ALLOCATOR.lock();
        // SAFETY: Передача валидной карты памяти UEFI, гарантирующей безопасность инициализации.
        unsafe {
            allocator.init(boot_info.memory_map);
        }
        kprintln!("Buddy Allocator Active:");
        kprintln!("  Total frames managed: {} ({} MiB)", allocator.total_frames(), (allocator.total_frames() * 4) / 256);
        kprintln!("  Free frames: {} ({} MiB)", allocator.free_frames(), (allocator.free_frames() * 4) / 256);

        kprintln!("\nRunning Buddy Allocator Self-Tests...");
        // Тест 1: Выделение страницы порядка 0 (4 KiB)
        let frame0 = allocator.allocate(0).expect("Failed to allocate order-0 frame");
        kprintln!("  [SUCCESS] Allocated Order 0 frame at physical address: {:#X}", frame0.start_address);

        // Тест 2: Выделение страницы порядка 3 (32 KiB)
        let frame3 = allocator.allocate(3).expect("Failed to allocate order-3 frame");
        kprintln!("  [SUCCESS] Allocated Order 3 frame at physical address: {:#X}", frame3.start_address);

        // Тест 3: Освобождение фреймов и проверка автоматического слияния близнецов (buddy merge)
        let free_before = allocator.free_frames();
        allocator.deallocate(frame0, 0);
        allocator.deallocate(frame3, 3);
        let free_after = allocator.free_frames();
        kprintln!("  [SUCCESS] Deallocated test frames. Free frames recovered: {}", free_after - free_before);
        kprintln!("\nRunning Paging (Virtual Memory) Self-Tests...");
        let test_virt_addr = 0x0000_DEAE_BEEF_0000;

        // 1. Проверяем текущее состояние: адрес не должен быть отображен
        if let Some(phys) = mm::translate(test_virt_addr) {
            kprintln!("  [WARNING] Address {:#X} is already mapped to {:#X}", test_virt_addr, phys);
        } else {
            kprintln!("  [SUCCESS] Verified {:#X} is currently NOT mapped", test_virt_addr);
        }

        // 2. Выделяем физический фрейм для отображения
        let phys_frame = allocator.allocate(0).expect("Failed to allocate frame for paging test");
        kprintln!("  Allocated physical frame for mapping: {:#X}", phys_frame.start_address);

        // 3. Отображаем виртуальную страницу на физический фрейм
        // SAFETY: Выделенный фрейм валиден, а адрес 0xDEAEBEEF0000 свободен в пространстве ядра.
        unsafe {
            mm::map_page(
                test_virt_addr,
                phys_frame,
                mm::PageFlags::PRESENT | mm::PageFlags::WRITABLE,
                &mut allocator,
            ).expect("Failed to map page");
        }
        kprintln!("  [SUCCESS] Mapped virtual {:#X} to physical {:#X}", test_virt_addr, phys_frame.start_address);

        // 4. Проверяем трансляцию адреса
        if let Some(phys) = mm::translate(test_virt_addr) {
            if phys == phys_frame.start_address {
                kprintln!("  [SUCCESS] Verified translation: virtual {:#X} translates to physical {:#X}", test_virt_addr, phys);
            } else {
                kprintln!("  [FAILURE] Translation mismatch! Expected {:#X}, got {:#X}", phys_frame.start_address, phys);
            }
        } else {
            kprintln!("  [FAILURE] Failed to translate mapped virtual address!");
        }

        // 5. Размонтируем (unmap) виртуальную страницу
        // SAFETY: unmap_page безопасен для отлаженного тестового адреса в Ring 0.
        unsafe {
            let unmapped_frame = mm::unmap_page(test_virt_addr).expect("Failed to unmap page");
            kprintln!("  [SUCCESS] Unmapped virtual {:#X}. Recovered frame address: {:#X}", test_virt_addr, unmapped_frame.start_address);
            allocator.deallocate(unmapped_frame, 0);
        }

        // 6. Проверяем, что адрес больше не транслируется
        if mm::translate(test_virt_addr).is_none() {
            kprintln!("  [SUCCESS] Verified translation is cleared after unmap");
        } else {
            kprintln!("  [FAILURE] Virtual address still translates after unmap!");
        }
    }

    kprintln!("\nInitializing Kernel Heap (LockedHeap)...");
    // SAFETY: Инициализация статического пула кучи ядра гарантированно безопасна при старте ядра.
    unsafe {
        mm::HEAP_ALLOCATOR.init();
    }
    kprintln!("  [SUCCESS] Global heap initialized");

    kprintln!("\nRunning Global Allocator (extern crate alloc) Self-Tests...");
    // Тест 1: Аллокация единственного значения через Box
    let boxed_val = Box::new(42u32);
    kprintln!("  [SUCCESS] Allocated value inside Box: {}", boxed_val);

    // Тест 2: Аллокация динамического вектора Vec
    let mut vec_test = Vec::new();
    for i in 0..10 {
        vec_test.push(i * 10);
    }
    kprintln!("  [SUCCESS] Dynamic Vec allocated and filled: {:?}", vec_test);

    kprintln!("====================================================");
    kprintln!("System is ready. Entering idle loop...");

    loop {
        // SAFETY: Безопасный asm-интринсик останова процессора до следующего прерывания,
        // чтобы не перегревать реальный ПК в бесконечном цикле.
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

/// Обработчик паник ядра Hammam. При возникновении ошибки выводит лог в COM-порт и останавливает процессор.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    kprintln!("\n!!! KERNEL PANIC !!!");
    if let Some(location) = info.location() {
        kprintln!("Location: {}:{}:{}", location.file(), location.line(), location.column());
    }
    kprintln!("Message: {}", info);
    kprintln!("====================================================");

    loop {
        // SAFETY: Остановка процессора при панике ядра.
        unsafe {
            core::arch::asm!("cli; hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
