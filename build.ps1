# PINDOS build script for Windows 11
# Usage: .\build.ps1 [run|run-grub|clean]

$NASM    = "C:\msys64\usr\bin\nasm.exe"
$QEMU    = "C:\msys64\mingw64\bin\qemu-system-i386.exe"
$SYSROOT = rustup run nightly-x86_64-pc-windows-gnu rustc --print sysroot
$LLD     = "$SYSROOT\lib\rustlib\x86_64-pc-windows-gnu\bin\rust-lld.exe"
$OBJCOPY = "$SYSROOT\lib\rustlib\x86_64-pc-windows-gnu\bin\llvm-objcopy.exe"
$TARGET  = "i686-unknown-none"

$action = if ($args.Count -gt 0) { $args[0] } else { "build" }

if ($action -eq "clean") {
    Remove-Item -Recurse -Force target -ErrorAction SilentlyContinue
    Set-Location kernel; rustup run nightly-x86_64-pc-windows-gnu cargo clean; Set-Location ..
    Write-Host "Cleaned."
    exit 0
}

New-Item -ItemType Directory -Force -Path target | Out-Null

# 1. Font
Write-Host "[1/6] Generating font..."
python scripts/gen_font.py

# 2. Bootloader
Write-Host "[2/6] Assembling bootloader..."
& $NASM -f bin bootloader/boot.asm -o target/boot.bin
if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: bootloader failed"; exit 1 }

# 3. Rust kernel
Write-Host "[3/6] Building Rust kernel..."
Set-Location kernel
rustup run nightly-x86_64-pc-windows-gnu cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: cargo build failed"; Set-Location ..; exit 1 }
Set-Location ..
Copy-Item "kernel/target/$TARGET/release/libpindos_kernel.a" target/

# 4. Kernel entry ASM
Write-Host "[4/6] Assembling kernel entry..."
& $NASM -f elf32 kernel/kernel.asm -o target/kernel_entry.o
if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: kernel.asm failed"; exit 1 }

# 5. Link
Write-Host "[5/6] Linking..."
& $LLD -flavor ld -m elf_i386 -T linker.ld --gc-sections `
    target/kernel_entry.o target/libpindos_kernel.a `
    -o target/kernel.elf
if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: linking failed"; exit 1 }

# 6. Binary + image
Write-Host "[6/6] Creating disk image..."
& $OBJCOPY -O binary target/kernel.elf target/kernel.bin
python -c "
import os
boot = open('target/boot.bin','rb').read()
kern = open('target/kernel.bin','rb').read()
img  = boot + kern
pad  = 1474560 - len(img)
if pad > 0: img += b'\x00' * pad
open('target/pindos.img','wb').write(img)
print(f'  boot: {len(boot)}b  kernel: {len(kern)}b  image: {len(img)}b')
"

Write-Host ""
Write-Host "Build complete: target/pindos.img"
Write-Host "  ELF: target/kernel.elf"

if ($action -eq "run") {
    Write-Host "Launching QEMU (floppy)..."
    & $QEMU -drive format=raw,file=target/pindos.img,if=floppy -m 32M -display sdl -no-reboot `
        -vga std
}
elseif ($action -eq "run-grub") {
    Write-Host "Launching QEMU (multiboot/kernel direct + VESA)..."
    & $QEMU -kernel target/kernel.elf -m 64M -display sdl -no-reboot
}
elseif ($action -eq "run-vesa") {
    Write-Host "Launching QEMU (kernel + VESA 1024x768)..."
    & $QEMU -kernel target/kernel.elf -m 64M -display sdl -no-reboot
}
