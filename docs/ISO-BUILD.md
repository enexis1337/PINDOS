# Создание ISO образа PINDOS

Этот документ описывает как создать загрузочный ISO образ PINDOS.

## Требования

### Windows (рекомендуется MSYS2)
```bash
# Установка MSYS2 (если еще не установлен)
# Скачать с https://www.msys2.org/

# В MSYS2 терминале:
pacman -S grub xorriso mtools
```

### Ubuntu/Debian
```bash
sudo apt update
sudo apt install xorriso grub-pc-bin mtools
```

### macOS
```bash
brew install xorriso grub mtools
```

## Методы создания ISO

### Метод 1: PowerShell скрипт (рекомендуется)
```powershell
# Сборка и создание ISO
.\build-iso.ps1

# Запуск в QEMU
.\build-iso.ps1 run

# Очистка
.\build-iso.ps1 clean
```

### Метод 2: Python скрипт (универсальный)
```bash
# Сначала собираем ядро
.\build.ps1

# Создаем ISO
python make-iso.py

# Тестируем
qemu-system-i386 -cdrom target/pindos.iso -m 64M
```

### Метод 3: Makefile
```bash
# Все в одной команде
make -f Makefile.iso iso

# Сборка и запуск
make -f Makefile.iso run-iso

# Справка
make -f Makefile.iso help
```

## Структура ISO

Созданный ISO содержит:
```
/
├── boot/
│   ├── grub/
│   │   └── grub.cfg          # Конфигурация GRUB
│   ├── pindos.elf           # Ядро системы
│   └── boot.bin             # Загрузчик (если нужен)
└── README.txt               # Информация о системе
```

## Варианты загрузки

GRUB предоставляет несколько вариантов:

1. **PINDOS 0.2** - Обычная загрузка
2. **PINDOS 0.2 (VESA 1024x768)** - С графическим режимом
3. **PINDOS 0.2 (Safe Mode)** - Безопасный режим

## Использование ISO

### Запуск в виртуальной машине
- **QEMU**: `qemu-system-i386 -cdrom pindos.iso -m 64M`
- **VirtualBox**: Создать новую ВМ, подключить ISO как CD
- **VMware**: Создать новую ВМ, подключить ISO

### Запись на физический носитель

#### CD/DVD
Используйте любую программу записи:
- Windows: ImgBurn, Nero, встроенная запись Windows
- Linux: Brasero, K3b, или `wodim`
- macOS: Дисковая утилита

#### USB флешка
- **Rufus** (Windows) - рекомендуется
- **balenaEtcher** (кроссплатформенный)
- **dd** (Linux/macOS): `sudo dd if=pindos.iso of=/dev/sdX bs=4M`

### Загрузка на реальном железе

1. Запишите ISO на CD/DVD или USB
2. Настройте BIOS/UEFI для загрузки с CD/USB
3. Перезагрузите компьютер
4. Выберите нужный вариант в меню GRUB

## Устранение проблем

### "grub-mkrescue not found"
```bash
# Windows (MSYS2)
pacman -S grub

# Ubuntu/Debian  
sudo apt install grub-pc-bin

# macOS
brew install grub
```

### "xorriso not found"
```bash
# Windows (MSYS2)
pacman -S xorriso

# Ubuntu/Debian
sudo apt install xorriso

# macOS
brew install xorriso
```

### ISO не загружается
- Проверьте что BIOS настроен на загрузку с CD/USB
- Убедитесь что ISO записан правильно
- Попробуйте другую программу записи
- Проверьте совместимость с Legacy BIOS (не UEFI)

### Ошибки сборки
- Убедитесь что `build.ps1` работает корректно
- Проверьте что `target/kernel.elf` создается
- Проверьте права доступа к директориям

## Размер ISO

Типичный размер ISO образа PINDOS: **2-5 MB**

Это очень компактно по сравнению с другими ОС:
- Linux дистрибутивы: 700MB - 4GB
- Windows: 4-8GB
- FreeBSD: 700MB - 2GB

## Дополнительные возможности

### Добавление файлов в ISO
Поместите файлы в директорию `iso/` перед созданием образа:
```bash
mkdir -p iso/files
cp myfile.txt iso/files/
python make-iso.py
```

### Кастомизация GRUB
Отредактируйте `iso/boot/grub/grub.cfg` для изменения меню загрузки.

### Автоматическая сборка
Создайте batch файл для автоматизации:
```batch
@echo off
echo Building PINDOS ISO...
powershell -ExecutionPolicy Bypass -File build-iso.ps1
if %errorlevel% equ 0 (
    echo Success! ISO created: target\pindos.iso
) else (
    echo Build failed!
)
pause
```