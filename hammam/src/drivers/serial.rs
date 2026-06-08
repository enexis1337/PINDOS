use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};
use core::cell::UnsafeCell;

/// Простой Spinlock Mutex для синхронизации доступа к аппаратуре в no_std окружении.
pub struct SpinMutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinMutex<T> {}
unsafe impl<T: Send> Send for SpinMutex<T> {}

impl<T> SpinMutex<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        while self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            // Ждем освобождения блокировки (spin loop)
            core::hint::spin_loop();
        }
        SpinMutexGuard { mutex: self }
    }
}

/// Guard для автоматического освобождения блокировки при выходе из области видимости.
pub struct SpinMutexGuard<'a, T> {
    mutex: &'a SpinMutex<T>,
}

impl<'a, T> core::ops::Deref for SpinMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY: Мы захватили блокировку locked = true, поэтому доступ к данным эксклюзивен.
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> core::ops::DerefMut for SpinMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Мы захватили блокировку locked = true, поэтому доступ к данным эксклюзивен.
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for SpinMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, Ordering::Release);
    }
}

/// Базовый адрес первого последовательного порта COM1
pub const COM1_BASE: u16 = 0x3F8;

/// Драйвер UART 16550 последовательного порта для вывода логов ядра.
pub struct Serial(u16);

impl Serial {
    /// Создает новый экземпляр драйвера последовательного порта с указанной базой портов.
    pub const fn new(port: u16) -> Self {
        Self(port)
    }

    /// Инициализирует последовательный порт для работы на реальном ПК.
    ///
    /// # Safety
    /// Функция осуществляет запись в порты ввода-вывода (Port I/O). Должна вызываться один раз при старте ядра.
    pub unsafe fn init(&self) {
        // SAFETY: Настройка регистров UART в соответствии со спецификацией 16550.
        unsafe {
            self.outb(1, 0x00); // Отключение всех прерываний порта
            self.outb(3, 0x80); // Включение DLAB (Divisor Latch Access Bit) для установки скорости
            self.outb(0, 0x03); // Делитель частоты: 3 (115200 / 3 = 38400 бод) - младший байт
            self.outb(1, 0x00); // Старший байт делителя
            self.outb(3, 0x03); // Режим работы: 8 бит данных, без четности, 1 стоп-бит (DLAB отключается)
            self.outb(2, 0xC7); // Включение FIFO, очистка буферов передачи/приема, триггер на 14 байт
            self.outb(4, 0x0B); // Активация DTR, RTS и Out2 (необходимо для работы аппаратного управления потоком)
        }
    }

    /// Проверяет, пуст ли передающий буфер UART.
    fn is_transmit_empty(&self) -> bool {
        // Line Status Register (LSR) находится на смещении +5. Bit 5 = Transmit Holding Register Empty.
        // SAFETY: Чтение статуса порта.
        unsafe { (self.inb(5) & 0x20) != 0 }
    }

    /// Записывает один байт данных в последовательный порт.
    pub fn write_byte(&self, byte: u8) {
        // Ожидаем готовности передатчика
        while !self.is_transmit_empty() {
            core::hint::spin_loop();
        }
        // Записываем байт в Transmitter Holding Register на смещении +0
        // SAFETY: Запись байта в порт вывода.
        unsafe {
            self.outb(0, byte);
        }
    }

    /// Вспомогательная функция для записи байта в порт (outb)
    #[inline]
    unsafe fn outb(&self, offset: u16, value: u8) {
        // SAFETY: Прямая запись в порт ввода-вывода.
        unsafe {
            core::arch::asm!(
                "out dx, al",
                in("dx") self.0 + offset,
                in("al") value,
                options(nomem, nostack, preserves_flags)
            );
        }
    }

    /// Вспомогательная функция для чтения байта из порта (inb)
    #[inline]
    unsafe fn inb(&self, offset: u16) -> u8 {
        let value: u8;
        // SAFETY: Прямое чтение из порта ввода-вывода.
        unsafe {
            core::arch::asm!(
                "in al, dx",
                out("al") value,
                in("dx") self.0 + offset,
                options(nomem, nostack, preserves_flags)
            );
        }
        value
    }
}

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            // На реальных ПК часто требуется переводить \n в \r\n для корректного отображения в терминале
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}

/// Wrapper вокруг UnsafeCell для реализации Sync (ядро однопоточное).
pub struct SerialPort(UnsafeCell<Serial>);

unsafe impl Sync for SerialPort {}

impl SerialPort {
    pub unsafe fn get(&self) -> &mut Serial {
        // SAFETY: Вызывающий должен гарантировать отсутствие гонок.
        unsafe { &mut *self.0.get() }
    }
}

/// Глобальный экземпляр последовательного порта (без блокировки — ядро однопоточное).
pub static SERIAL: SerialPort = SerialPort(UnsafeCell::new(Serial::new(COM1_BASE)));

/// Макрос для вывода форматированной строки в COM-порт ядра Hammam.
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {
        // SAFETY: Ядро однопоточное на этапе загрузки — без блокировки безопасен.
        let serial = unsafe { $crate::drivers::serial::SERIAL.get() };
        <$crate::drivers::serial::Serial as core::fmt::Write>::write_fmt(serial, format_args!($($arg)*)).ok();
    };
}

/// Макрос для вывода строки с переносом строки в COM-порт ядра Hammam.
#[macro_export]
macro_rules! kprintln {
    () => ($crate::kprint!("\n"));
    ($($arg:tt)*) => ($crate::kprint!("{}\n", format_args!($($arg)*)));
}
