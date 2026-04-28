# PINDOS

Простая операционная система для архитектуры x86 (32-bit protected mode).  
Написана на ассемблере (NASM) и Rust (`no_std`).

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

## Версии

| Компонент | Версия |
|-----------|--------|
| OS        | PINDOS 0.2 |
| Kernel    | Hammam 0.2.0 |
| Desktop   | Mell 0.2 |

---

## Что умеет

- Загрузка с диска через собственный MBR bootloader
- Переход из real mode в 32-bit protected mode
- VGA текстовый режим 80×25 с цветным выводом
- VESA framebuffer (1024×768, 32bpp) для графического рабочего стола
- Ввод с клавиатуры и мыши через PS/2
- In-memory файловая система с поддержкой директорий и путей
- FAT файловая система (чтение с ATA/IDE дисков)
- Система пользователей с паролями (root + обычные пользователи, SU привилегии)
- UNIX-подобный шелл `uglyshell (ush)`
- Страничная адресация (x86 paging, 4KB страницы)
- Запуск DOS `.COM` программ через Virtual 8086 Mode
- Запуск статически слинкованных Linux ELF32 бинарников (ring 3 + int 0x80)
- Планировщик задач `dealduckd`
- Графический рабочий стол **Mell** в стиле Windows 95

---

## Рабочий стол Mell

Mell — графическая оболочка поверх VESA framebuffer.

**Возможности:**
- Иконки на рабочем столе с привязкой к сетке
- Перетаскивание иконок, мультиселект (rubber band), Ctrl+A
- Контекстные меню (ПКМ на иконке и рабочем столе)
- Окна с заголовком, кнопками закрыть/свернуть/развернуть
- Перемещение и изменение размера окон мышью
- Таскбар с кнопкой **Mell** и часами
- Меню **Mell** (аналог Start)

**Приложения Mell:**

| Приложение | Описание |
|------------|----------|
| Mocha      | Файловый менеджер |
| Qinn       | Текстовый редактор |
| Burmalda   | Встроенный терминал |
| Settings   | Настройки системы (вкладки: System, Users, Display, About) |
| Viewer     | Просмотр изображений и медиа |

Запуск: команда `mell` в шелле.

---

## Шелл (uglyshell)

### Команды

```
Навигация:    pwd, cd, ls, ll
Файлы:        cat, touch, mkdir, rm, rmdir, cp, mv, echo
Текст:        head, tail, wc, grep, find
Система:      uname, whoami, uptime, free, df, ps, env
Пользователи: users, useradd, userdel, passwd, su
Приложения:   mocha, qinn, fastfetch, mell, dealduckd
Запуск:       run <file.com>, exec <elf>
Редирект:     cmd > file, cmd >> file
```

### Встроенные утилиты

| Утилита      | Описание |
|--------------|----------|
| `mocha`      | Файловый менеджер (TUI) |
| `qinn`       | Текстовый редактор |
| `fastfetch`  | Системная информация с ASCII артом |
| `mell`       | Запуск графического рабочего стола |
| `dealduckd`  | Управление планировщиком задач |

---

## Первый запуск (Drun)

При первом старте запускается **Drun** — мастер начальной настройки:

1. Установка пароля root
2. Создание нового пользователя (имя, пароль, SU привилегии)

При каждом последующем запуске — запрос логина и пароля.  
3 неверных попытки — возврат к экрану логина.

---

## Структура проекта

```
pindos/
├── bootloader/
│   └── boot.asm              # MBR загрузчик, real mode → protected mode
├── kernel/                   # Ядро Hammam
│   ├── kernel.asm            # ASM точка входа → kernel_main()
│   ├── Cargo.toml            # name = "hammam", version = "0.2.0"
│   ├── .cargo/config.toml
│   └── src/
│       ├── main.rs           # kernel_main, panic handler
│       ├── version.rs        # версии OS / Kernel / Desktop
│       ├── vga.rs            # VGA драйвер, ввод с клавиатуры
│       ├── fs.rs             # in-memory файловая система
│       ├── auth.rs           # пользователи, пароли, логин, Drun
│       ├── uglyshell.rs      # UNIX-подобный шелл
│       ├── dealduckd.rs      # планировщик задач
│       ├── drivers/
│       │   ├── vesa.rs       # VESA framebuffer
│       │   ├── ps2.rs        # клавиатура и мышь
│       │   ├── ata.rs        # ATA/IDE диски
│       │   ├── rtc.rs        # часы реального времени
│       │   ├── speaker.rs    # PC speaker
│       │   └── ...
│       ├── dos/
│       │   ├── loader.rs     # загрузчик .COM файлов
│       │   ├── int21.rs      # эмуляция DOS INT 21h
│       │   └── v86.rs        # Virtual 8086 Mode
│       ├── linux/
│       │   ├── paging.rs     # x86 страничная адресация
│       │   ├── elf.rs        # загрузчик ELF32
│       │   ├── syscall.rs    # эмуляция Linux syscalls (int 0x80)
│       │   └── process.rs    # запуск процессов в ring 3
│       ├── fs_fat/           # FAT файловая система
│       ├── mell/             # Рабочий стол Mell
│       │   ├── mod.rs        # runtime, WM, иконки, меню
│       │   ├── vga_gui.rs    # GUI примитивы поверх VESA
│       │   ├── wm.rs         # оконный менеджер
│       │   └── apps/         # приложения Mell
│       │       ├── mocha.rs
│       │       ├── qinn.rs
│       │       ├── burmalda.rs
│       │       ├── settings.rs
│       │       └── viewer.rs
│       └── utils/
│           ├── mocha.rs      # файловый менеджер (TUI)
│           ├── qinn.rs       # текстовый редактор (TUI)
│           └── fastfetch.rs  # системная информация
├── scripts/
│   ├── gen_font.py           # генерация шрифта
│   ├── make-iso.py           # создание ISO образа
│   └── make_usb.sh           # запись на USB
├── i686-unknown-none.json    # Rust target spec
├── linker.ld                 # скрипт линковщика
├── build.ps1                 # сборка (Windows)
├── build-iso.ps1             # сборка ISO (Windows)
├── Makefile                  # сборка (Linux)
└── build-all.bat             # полная сборка + ISO (Windows)
```

---

## Сборка

### Windows

**Зависимости:**
```powershell
# Rust nightly
rustup install nightly
rustup component add rust-src --toolchain nightly

# NASM: https://www.nasm.us/
# QEMU (опционально): https://www.qemu.org/download/#windows
```

**Сборка:**
```powershell
.\build.ps1          # собрать ядро + слинковать
.\build-iso.ps1      # собрать ISO образ
.\build-all.bat      # всё сразу
```

**Запуск:**
```powershell
qemu-system-i386 -cdrom target\pindos.iso -m 64M
```

### Linux

**Зависимости (Ubuntu/Debian):**
```bash
sudo apt install nasm binutils-multiarch qemu-system-x86 make python3 \
                 grub-pc-bin xorriso mtools
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup install nightly && rustup component add rust-src --toolchain nightly
```

**Сборка и запуск:**
```bash
make          # сборка
make run      # BIOS режим
make iso      # создать ISO
qemu-system-i386 -cdrom target/pindos.iso -m 64M
```

### Запись на железо

```bash
# BIOS/CSM
sudo dd if=target/pindos.img of=/dev/sdX bs=512 status=progress

# UEFI (через ISO + GRUB)
sudo dd if=target/pindos.iso of=/dev/sdX bs=4M status=progress
```

Через **Ventoy**: скопировать `pindos.iso` на флешку с Ventoy.

**Настройки BIOS/UEFI:** Secure Boot → Disable, Boot Order → USB первым.

---

## Запуск DOS и Linux программ

**DOS `.COM`:**
```
run program.com
```

**Linux ELF32 (статически слинкованный):**
```bash
# На хост-машине
gcc -m32 -static -o hello hello.c
# Внутри PINDOS
exec hello
```

Поддерживаемые syscalls: `read`, `write`, `open`, `close`, `exit`, `brk`, `mmap`,
`getpid`, `uname`, `getcwd`, `unlink`, `mkdir`, `rename`, `ioctl`, `lseek`, `dup`, `dup2` и др.

---

## Требования

| | |
|---|---|
| Архитектура | x86 (i686), 32-bit |
| RAM | минимум 32 MB |
| Видео | VGA текст или VESA 1024×768 |
| Рекомендуется | QEMU `qemu-system-i386` |
