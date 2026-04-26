# PINDOS

Простая операционная система на ассемблере (x86) и Rust.


```
    ....                   ...            
   .+@%=.               .=**%
   .+@@@@+. ..:---:. .-*=:.++
   .=@@@@@@@@+#.%--#:#*.   #:
    -%@@@@@@:-#.#=-+   -%+:#
    .=@@@*:   :..:.     .=+.
    :+..                  ==
   .=-                    .#...
  .#+*:::...   .::..  ..::=%%-.
   .=:...-+*: .@@*. .+-....*:
.-=**=---==.  .=.  .=----=*--=..
    .**%#+-:.       .::-+#%:
 .+:..:#..             .-*. .:.
      .-+=:        .-++.
         .:========-..                                                                        
_____ _____ _   _ _____   ____   _____
|  __ \_   _| \ | |  __ \ / __ \ / ____|
| |__) || | |  \| | |  | | |  | | (___
|  ___/ | | | . ` | |  | | |  | |\___ \
| |    _| |_| |\  | |__| | |__| |____) |
|_|   |_____|_| \_|_____/ \____/|_____/
```

---

## Описание

PINDOS — учебная операционная система для архитектуры x86 (32-bit protected mode).
Написана на ассемблере (NASM) и Rust (`no_std`).

### Что умеет

- Загрузка с диска через собственный bootloader (MBR)
- Переход из real mode в 32-bit protected mode
- VGA текстовый режим 80x25 с цветным выводом
- Ввод с клавиатуры через порт 0x60
- In-memory файловая система с поддержкой директорий и путей
- Система пользователей с паролями (root + обычные пользователи, SU привилегии)
- UNIX-подобный шелл с промптом `user@pindos:/path$`
- Страничная адресация (x86 paging, 4KB страницы)
- Запуск DOS `.COM` программ через Virtual 8086 Mode
- Запуск статически слинкованных Linux ELF32 бинарников через ring 3 + int 0x80

### Встроенные утилиты

| Утилита | Описание |
|---------|----------|
| `mocha` | Файловый менеджер (ls, copy, cut, paste, delete, rename, edit, view) |
| `qinn`  | Текстовый редактор (qinn is not notepad) |
| `fastfetch` | Системная информация с ASCII артом |

### Команды шелла

```
Навигация:    pwd, cd, ls, ll
Файлы:        cat, touch, mkdir, rm, rmdir, cp, mv, echo
Текст:        head, tail, wc, grep, find
Система:      uname, whoami, uptime, free, df, ps, env
Пользователи: users, useradd, userdel, passwd, su
Приложения:   mocha, qinn, fastfetch, run <file.com>, exec <elf>
Редирект:     cmd > file, cmd >> file
```

---

## Структура проекта

```
zaebOS/
├── bootloader/
│   └── boot.asm          # MBR загрузчик, real mode → protected mode
├── kernel/
│   ├── kernel.asm        # ASM точка входа, вызывает kernel_main()
│   ├── Cargo.toml
│   ├── .cargo/config.toml
│   └── src/
│       ├── main.rs       # kernel_main, panic handler
│       ├── vga.rs        # VGA драйвер, ввод с клавиатуры
│       ├── fs.rs         # файловая система (директории, пути)
│       ├── shell.rs      # UNIX-подобный шелл
│       ├── auth.rs       # пользователи, пароли, логин
│       ├── dos/
│       │   ├── loader.rs # загрузчик .COM файлов
│       │   ├── int21.rs  # эмуляция DOS INT 21h
│       │   └── v86.rs    # Virtual 8086 Mode
│       ├── linux/
│       │   ├── paging.rs # x86 страничная адресация
│       │   ├── elf.rs    # загрузчик ELF32
│       │   ├── syscall.rs# эмуляция Linux syscalls (int 0x80)
│       │   └── process.rs# запуск процессов в ring 3
│       └── utils/
│           ├── mocha.rs  # файловый менеджер
│           ├── qinn.rs   # текстовый редактор
│           └── fastfetch.rs # системная информация
├── i686-unknown-none.json # Rust target spec
├── linker.ld              # скрипт линковщика
├── build.ps1              # сборка для Windows
├── make-iso.py            # создание ISO образа
├── build-all.bat          # автоматическая сборка + ISO
└── Makefile
```

---

## Сборка на Windows

### Зависимости

**Rust:**
```powershell
# Установить Rust с https://rustup.rs/
rustup install nightly
rustup component add rust-src llvm-tools-preview --toolchain nightly
```

**NASM:**
```powershell
# Скачать с https://www.nasm.us/pub/nasm/releasebuilds/
# Добавить в PATH
```

**QEMU (опционально):**
```powershell
# Скачать с https://www.qemu.org/download/#windows
```

### Сборка

```powershell
# Сборка ядра
.\build.ps1

# Создание ISO образа
python make-iso.py

# Все в одной команде
.\build-all.bat
```

### Запуск

```powershell
# Floppy образ в QEMU
.\build.ps1 run-grub

# ISO образ в QEMU  
qemu-system-i386 -cdrom target\pindos.iso -m 64M

# Запись на USB (используйте Rufus или balenaEtcher)
```

---

## Создание ISO образа

PINDOS поддерживает создание загрузочных ISO образов для записи на CD/DVD или USB.

### Автоматическое создание

```bash
# Linux/macOS
make iso

# Windows
python make-iso.py
# или
.\build-all.bat
```

### Ручное создание (с GRUB)

```bash
# Требует: grub-mkrescue, xorriso
.\build-iso.ps1        # Windows (MSYS2)
```

### Использование ISO

**Виртуальные машины:**
- QEMU: `qemu-system-i386 -cdrom pindos.iso -m 64M`
- VirtualBox: подключить ISO как CD-ROM
- VMware: подключить ISO как CD-ROM

**Запись на физический носитель:**
- **CD/DVD**: ImgBurn, Nero, встроенная запись Windows
- **USB**: Rufus, balenaEtcher, или `dd if=pindos.iso of=/dev/sdX`

**Размер ISO:** ~0.4 MB (очень компактно!)

---

## Сборка на Linux

### Зависимости

**Ubuntu / Debian:**
```bash
sudo apt update
sudo apt install nasm binutils-multiarch qemu-system-x86 make python3 \
                 grub-pc-bin grub-efi-amd64-bin xorriso mtools
```

**Arch Linux:**
```bash
sudo pacman -S nasm qemu-system-x86 make python grub xorriso mtools
```

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup install nightly
rustup component add rust-src llvm-tools-preview --toolchain nightly
```

### Сборка

```bash
cd pindos-0.2
make
```

### Запуск в QEMU

```bash
make run        # BIOS режим (floppy образ)
make run-grub   # Multiboot2/GRUB режим (как на реальном UEFI железе)
```

### Запуск на реальном железе

**BIOS / CSM системы (Core 2 Duo и старше):**
```bash
sudo dd if=target/pindos.img of=/dev/sdX bs=512 status=progress
```

**UEFI системы (современные ПК, ноутбуки):**
```bash
# Создать ISO с GRUB (поддерживает BIOS + UEFI)
make iso
# Записать на USB
sudo dd if=target/pindos.iso of=/dev/sdX bs=4M status=progress
```

**Через Ventoy (самый простой способ для UEFI):**
1. Установить [Ventoy](https://ventoy.net) на USB флешку
2. Скопировать `target/pindos.iso` на флешку
3. Загрузиться с USB — выбрать PINDOS в меню Ventoy

**Настройки BIOS/UEFI для запуска:**
- Secure Boot → **Disable**
- Boot Mode → **UEFI** (для ISO с GRUB) или **Legacy/CSM** (для прямой записи)
- Boot Order → USB первым

---

## Запуск Linux программ

PINDOS поддерживает запуск статически слинкованных 32-bit ELF бинарников.

Компиляция программы под PINDOS (на хост-машине):
```bash
gcc -m32 -static -o hello hello.c
```

Загрузка в FS и запуск (внутри PINDOS пока не реализована загрузка с диска в рантайме — файлы добавляются через FS при старте ядра):
```
exec hello
```

Поддерживаемые syscalls: `read`, `write`, `open`, `close`, `exit`, `brk`, `mmap`,
`getpid`, `uname`, `getcwd`, `unlink`, `mkdir`, `rename`, `ioctl`, `lseek`, `dup`, `dup2` и др.

---

## Запуск DOS программ

PINDOS поддерживает `.COM` файлы через Virtual 8086 Mode.

```
run program.com
```

Поддерживаемые INT 21h функции: вывод символа/строки, ввод, работа с файлами, завершение.

---

## Первый запуск

При первом старте система запросит:
1. Пароль для root
2. Создание нового пользователя (имя, пароль, SU привилегии)

При каждом последующем запуске — запрос логина и пароля.
3 неверных попытки — возврат к экрану логина.

---

## Требования к железу / эмулятору

- Архитектура: x86 (i686), 32-bit
- RAM: минимум 32MB
- Диск: образ 1.44MB (floppy)
- Рекомендуется: QEMU `qemu-system-i386`
