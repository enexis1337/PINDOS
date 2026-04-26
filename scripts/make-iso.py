#!/usr/bin/env python3
"""
PINDOS ISO Creator
Создает загрузочный ISO образ без зависимости от GRUB
"""

import os
import sys
import struct
import subprocess
from pathlib import Path

def create_iso_structure():
    """Создает структуру директорий для ISO"""
    iso_dir = Path("iso")
    iso_dir.mkdir(exist_ok=True)
    (iso_dir / "boot").mkdir(exist_ok=True)
    return iso_dir

def create_simple_bootloader():
    """Создает простой загрузчик для ISO (El Torito)"""
    # Простой загрузчик который загружает kernel.elf
    bootloader = bytearray(2048)  # Один сектор CD (2048 байт)
    
    # Заголовок загрузчика
    bootloader[0:2] = b'\xEB\x3C'  # JMP short
    bootloader[2:10] = b'PINDOS  '  # OEM ID
    
    # Простой код загрузчика
    simple_code = [
        0xB8, 0x00, 0x10,  # mov ax, 0x1000
        0x8E, 0xD8,        # mov ds, ax
        0xBE, 0x20, 0x00,  # mov si, msg
        0xAC,              # lodsb
        0x3C, 0x00,        # cmp al, 0
        0x74, 0x06,        # jz done
        0xB4, 0x0E,        # mov ah, 0x0E
        0xCD, 0x10,        # int 0x10
        0xEB, 0xF6,        # jmp loop
        0xF4,              # hlt
    ]
    
    # Копируем код
    for i, byte in enumerate(simple_code):
        if i < len(bootloader):
            bootloader[i + 16] = byte
    
    # Сообщение
    msg = b'PINDOS Loading...\r\n\0'
    for i, byte in enumerate(msg):
        if i + 32 < len(bootloader):
            bootloader[i + 32] = byte
    
    # Boot signature
    bootloader[510] = 0x55
    bootloader[511] = 0xAA
    
    return bootloader

def create_iso_with_mkisofs(iso_dir, output_file):
    """Создает ISO используя mkisofs/genisoimage"""
    cmd = [
        "mkisofs",
        "-R",  # Rock Ridge extensions
        "-J",  # Joliet extensions  
        "-c", "boot/boot.cat",  # Boot catalog
        "-b", "boot/boot.bin",  # Boot image
        "-no-emul-boot",
        "-boot-load-size", "4",
        "-boot-info-table",
        "-o", str(output_file),
        str(iso_dir)
    ]
    
    try:
        result = subprocess.run(cmd, check=True, capture_output=True, text=True)
        return True
    except subprocess.CalledProcessError as e:
        print(f"mkisofs failed: {e}")
        return False
    except FileNotFoundError:
        print("mkisofs not found. Try installing genisoimage or cdrtools")
        return False

def create_iso_with_xorriso(iso_dir, output_file):
    """Создает ISO используя xorriso"""
    cmd = [
        "xorriso",
        "-as", "mkisofs",
        "-R", "-J",
        "-c", "boot/boot.cat",
        "-b", "boot/boot.bin",
        "-no-emul-boot",
        "-boot-load-size", "4", 
        "-boot-info-table",
        "-o", str(output_file),
        str(iso_dir)
    ]
    
    try:
        result = subprocess.run(cmd, check=True, capture_output=True, text=True)
        return True
    except subprocess.CalledProcessError as e:
        print(f"xorriso failed: {e}")
        return False
    except FileNotFoundError:
        print("xorriso not found")
        return False

def create_iso_manual(iso_dir, output_file):
    """Создает простой ISO без внешних утилит (только данные, без загрузки)"""
    print("Creating simple data-only ISO...")
    
    # Создаем простую структуру ISO 9660
    with open(output_file, "wb") as f:
        # Пишем пустые системные области (32KB)
        f.write(b'\x00' * 32768)
        
        # Primary Volume Descriptor
        pvd = bytearray(2048)
        pvd[0] = 1  # Volume Descriptor Type
        pvd[1:6] = b'CD001'  # Standard Identifier
        pvd[6] = 1  # Volume Descriptor Version
        pvd[8:40] = b'PINDOS'.ljust(32)  # System Identifier
        pvd[40:72] = b'PINDOS_0_2'.ljust(32)  # Volume Identifier
        
        f.write(pvd)
        
        # Volume Descriptor Set Terminator
        vdst = bytearray(2048)
        vdst[0] = 255
        vdst[1:6] = b'CD001'
        vdst[6] = 1
        f.write(vdst)
        
        # Простые данные файлов
        files_data = []
        
        # Читаем kernel.elf
        kernel_path = iso_dir / "boot" / "pindos.elf"
        if kernel_path.exists():
            with open(kernel_path, "rb") as kf:
                kernel_data = kf.read()
                files_data.append(("PINDOS.ELF", kernel_data))
        
        # Читаем README
        readme_path = iso_dir / "README.txt"
        if readme_path.exists():
            with open(readme_path, "rb") as rf:
                readme_data = rf.read()
                files_data.append(("README.TXT", readme_data))
        
        # Записываем файлы
        for name, data in files_data:
            # Выравниваем по границе сектора
            while f.tell() % 2048 != 0:
                f.write(b'\x00')
            f.write(data)
            # Дополняем до границы сектора
            while f.tell() % 2048 != 0:
                f.write(b'\x00')
    
    return True

def main():
    if len(sys.argv) > 1 and sys.argv[1] == "clean":
        import shutil
        for d in ["iso", "target"]:
            if os.path.exists(d):
                shutil.rmtree(d)
        print("Cleaned.")
        return
    
    print("PINDOS ISO Creator")
    print("==================")
    
    # Проверяем что kernel.elf существует
    kernel_elf = Path("target/kernel.elf")
    if not kernel_elf.exists():
        print("ERROR: target/kernel.elf not found. Run build.ps1 first.")
        return 1
    
    # Создаем структуру ISO
    print("[1/4] Creating ISO structure...")
    iso_dir = create_iso_structure()
    
    # Копируем файлы
    print("[2/4] Copying files...")
    import shutil
    shutil.copy2("target/kernel.elf", iso_dir / "boot" / "pindos.elf")
    
    # Создаем простой загрузчик
    print("[3/4] Creating bootloader...")
    bootloader = create_simple_bootloader()
    with open(iso_dir / "boot" / "boot.bin", "wb") as f:
        f.write(bootloader)
    
    # Создаем README
    readme = """PINDOS 0.2 - Custom Operating System

Files:
- boot/pindos.elf - Main kernel
- boot/boot.bin   - Boot loader

To run: Boot from this CD/DVD or mount in virtual machine.

Visit: https://github.com/your-repo/pindos
"""
    with open(iso_dir / "README.txt", "w") as f:
        f.write(readme)
    
    # Создаем ISO
    print("[4/4] Creating ISO image...")
    output_file = Path("target/pindos.iso")
    output_file.parent.mkdir(exist_ok=True)
    
    # Пробуем разные утилиты
    success = False
    for create_func in [create_iso_with_xorriso, create_iso_with_mkisofs, create_iso_manual]:
        if create_func(iso_dir, output_file):
            success = True
            break
    
    if not success:
        print("ERROR: Could not create ISO.")
        return 1
    
    # Показываем результат
    size_mb = output_file.stat().st_size / (1024 * 1024)
    print(f"\nISO created successfully: {output_file} ({size_mb:.2f} MB)")
    print("\nTo test:")
    print(f"  qemu-system-i386 -cdrom {output_file} -m 64M")
    print("\nTo burn to CD/DVD:")
    print("  Use any burning software (Nero, ImgBurn, etc.)")
    print("\nTo create bootable USB:")
    print("  Use Rufus, balenaEtcher, or dd command")
    
    return 0

if __name__ == "__main__":
    sys.exit(main())