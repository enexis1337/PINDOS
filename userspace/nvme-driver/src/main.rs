/// NVMe драйвер для PINDOS
/// Работает в userspace (Ring 3) — это обычный процесс
/// Получает доступ к PCI BAR через capability от ядра
/// Регистрирует себя как блочное устройство в ядре

use std::io::{self, Write};

fn main() -> io::Result<()> {
    writeln!(io::stdout(), "nvme-driver: starting...")?;

    // 1. Запросить capability на PCI BAR NVMe контроллера от ядра
    //    через syscall: cap_request(CapKind::IoPort, nvme_bar_addr)
    writeln!(io::stdout(), "nvme-driver: requesting capability for NVMe PCI BAR...")?;
    
    // TODO: выполнить syscall для получения capability
    // let cap = sys_cap_request(CapKind::IoPort, NVME_BAR_ADDR);

    // 2. Инициализировать NVMe контроллер через MMIO
    //    - Создать Admin Queue
    //    - Создать I/O Queue (при необходимости)
    //    - Отправить команды инициализации
    writeln!(
        io::stdout(),
        "nvme-driver: initializing NVMe controller through MMIO..."
    )?;
    
    // TODO: инициализировать NVMe
    // - Прочитать CAP (Capabilities)
    // - Настроить AQA (Admin Queue Attributes)
    // - Написать ASQ/ACQ (Admin Queue addresses)
    // - Включить контроллер (CC.EN = 1)
    // - Ждать готовности (CSTS.RDY = 1)
    // - Отправить Identify команду
    // - Создать I/O Queue Pair

    // 3. Зарегистрировать себя как блочное устройство в ядре
    //    через syscall block_register(dev_name, block_size, block_count)
    writeln!(
        io::stdout(),
        "nvme-driver: registering with kernel block subsystem..."
    )?;
    
    // TODO: syscall для регистрации в BlockRegistry ядра
    // let dev_id = sys_block_register("nvme0n1", 4096, BLOCK_COUNT);

    // 4. Войти в main event loop
    //    - Читать из io_uring Submission Queue
    //    - Выполнять I/O операции через NVMe контроллер
    //    - Писать результаты в Completion Queue
    writeln!(io::stdout(), "nvme-driver: entering main event loop...")?;
    
    // TODO: основной цикл обработки io_uring запросов
    // loop {
    //     let sqe = read_sq_entry();  // блокирует до наличия работы
    //     match sqe.opcode {
    //         OP_READ => handle_read(sqe),
    //         OP_WRITE => handle_write(sqe),
    //         _ => {}
    //     }
    //     write_cq_entry(result);
    // }

    writeln!(io::stdout(), "nvme-driver: TODO - not yet implemented")?;
    Ok(())
}

// ============================================================================
// Sketch функций которые нужно реализовать
// ============================================================================

#[allow(dead_code)]
fn init_nvme_controller() {
    // TODO: инициализация NVMe контроллера
    // 1. Читаем CAP регистр для определения возможностей
    // 2. Выделяем память для Admin Queue
    // 3. Записываем адреса в ASQ/ACQ
    // 4. Включаем контроллер через CC.EN = 1
    // 5. Ждем CSTS.RDY = 1
    // 6. Отправляем Identify команду для получения информации о диске
    // 7. Создаем I/O Queue пару если нужно
}

#[allow(dead_code)]
fn handle_read_request(_lba: u64, _block_count: u32) {
    // TODO: подготовить Read команду для NVMe
    // Отправить команду в I/O Queue
    // Дождаться завершения (через interrupt или polling)
}

#[allow(dead_code)]
fn handle_write_request(_lba: u64, _block_count: u32) {
    // TODO: подготовить Write команду для NVMe
    // Отправить команду в I/O Queue
    // Дождаться завершения (через interrupt или polling)
}

#[allow(dead_code)]
fn process_completion_queue() {
    // TODO: читать результаты из Completion Queue
    // Обновлять статус операций в io_uring
}
