# PINDOS ISO build script
# Создает загрузочный ISO образ с GRUB2

$NASM    = "C:\msys64\usr\bin\nasm.exe"
$QEMU    = "C:\msys64\mingw64\bin\qemu-system-i386.exe"
$SYSROOT = rustup run nightly-x86_64-pc-windows-gnu rustc --print sysroot
$LLD     = "$SYSROOT\lib\rustlib\x86_64-pc-windows-gnu\bin\rust-lld.exe"
$OBJCOPY = "$SYSROOT\lib\rustlib\x86_64-pc-windows-gnu\bin\llvm-objcopy.exe"
$TARGET  = "i686-unknown-none"

# Утилиты для создания ISO (нужно установить через msys2)
$GRUB_MKRESCUE = "C:\msys64\usr\bin\grub-mkrescue.exe"
$XORRISO = "C:\msys64\usr\bin\xorriso.exe"

$action = if ($args.Count -gt 0) { $args[0] } else { "build" }

if ($action -eq "clean") {
    Remove-Item -Recurse -Force target -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force iso -ErrorAction SilentlyContinue
    Set-Location kernel; rustup run nightly-x86_64-pc-windows-gnu cargo clean; Set-Location ..
    Write-Host "Cleaned."
    exit 0
}

# Проверяем наличие необходимых утилит
if (!(Test-Path $GRUB_MKRESCUE)) {
    Write-Host "ERROR: grub-mkrescue not found. Install with: pacman -S grub"
    Write-Host "       Or use msys2: pacman -S grub xorriso"
    exit 1
}

New-Item -ItemType Directory -Force -Path target | Out-Null
New-Item -ItemType Directory -Force -Path iso | Out-Null
New-Item -ItemType Directory -Force -Path iso/boot | Out-Null
New-Item -ItemType Directory -Force -Path iso/boot/grub | Out-Null

# 1. Font
Write-Host "[1/7] Generating font..."
python scripts/gen_font.py

# 2. Rust kernel
Write-Host "[2/7] Building Rust kernel..."
Set-Location kernel
rustup run nightly-x86_64-pc-windows-gnu cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: cargo build failed"; Set-Location ..; exit 1 }
Set-Location ..
Copy-Item "kernel/target/$TARGET/release/libpindos_kernel.a" target/

# 3. Kernel entry ASM
Write-Host "[3/7] Assembling kernel entry..."
& $NASM -f elf32 kernel/kernel.asm -o target/kernel_entry.o
if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: kernel.asm failed"; exit 1 }

# 4. Link
Write-Host "[4/7] Linking..."
& $LLD -flavor ld -m elf_i386 -T linker.ld --gc-sections `
    target/kernel_entry.o target/libpindos_kernel.a `
    -o target/kernel.elf
if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: linking failed"; exit 1 }

# 5. Copy kernel to ISO directory
Write-Host "[5/7] Preparing ISO structure..."
Copy-Item target/kernel.elf iso/boot/pindos.elf

# 6. Create GRUB config
Write-Host "[6/7] Creating GRUB configuration..."
@"
set timeout=5
set default=0

menuentry "PINDOS 0.2" {
    multiboot /boot/pindos.elf
    boot
}

menuentry "PINDOS 0.2 (VESA 1024x768)" {
    multiboot /boot/pindos.elf vesa=1024x768x32
    boot
}

menuentry "PINDOS 0.2 (Safe Mode)" {
    multiboot /boot/pindos.elf safe
    boot
}
"@ | Out-File -FilePath iso/boot/grub/grub.cfg -Encoding ASCII

# 7. Create ISO
Write-Host "[7/7] Creating ISO image..."
& $GRUB_MKRESCUE -o target/pindos.iso iso
if ($LASTEXITCODE -ne 0) { Write-Host "ERROR: ISO creation failed"; exit 1 }

$isoSize = (Get-Item target/pindos.iso).Length
Write-Host ""
Write-Host "ISO build complete: target/pindos.iso ($([math]::Round($isoSize/1MB, 2)) MB)"
Write-Host "  Kernel ELF: target/kernel.elf"

if ($action -eq "run") {
    Write-Host "Launching QEMU with ISO..."
    & $QEMU -cdrom target/pindos.iso -m 64M -display sdl -no-reboot -vga std
}
elseif ($action -eq "run-vesa") {
    Write-Host "Launching QEMU with ISO (VESA)..."
    & $QEMU -cdrom target/pindos.iso -m 64M -display sdl -no-reboot
}

Write-Host ""
Write-Host "To burn to CD/DVD: Use any burning software with target/pindos.iso"
Write-Host "To create USB: Use Rufus, balenaEtcher, or dd command"
Write-Host "To test in VirtualBox: Create new VM and mount target/pindos.iso"