@echo off
setlocal enabledelayedexpansion

echo ============================================================================
echo PINDOS OS Phase 6 - Hammam Kernel Boot (QEMU with Multiboot2)
echo ============================================================================
echo.

echo [1] Building kernel...
cd hammam
cargo build --target x86_64-unknown-none
if !errorlevel! neq 0 (
  echo [ERROR] Kernel build failed
  exit /b !errorlevel!
)
echo [OK] Kernel built successfully
cd ..

echo.
echo [2] Verifying ELF sections...
objdump -h hammam\target\x86_64-unknown-none\debug\hammam-kernel | findstr multiboot
if !errorlevel! equ 0 (
  echo [OK] Multiboot2 section verified
) else (
  echo [WARNING] Multiboot2 section not found
)

objdump -h hammam\target\x86_64-unknown-none\debug\hammam-kernel | findstr "note.pvh"
if !errorlevel! equ 0 (
  echo [OK] PVH note section verified
) else (
  echo [WARNING] PVH note section not found
)

echo.
echo [3] Displaying kernel entry points and sections...
objdump -h hammam\target\x86_64-unknown-none\debug\hammam-kernel

echo.
echo [4] Launching QEMU with Multiboot2 loader...
echo ============================================================================
REM Using QEMU's built-in multiboot2 support to load the kernel

qemu-system-x86_64 ^
  -M pc ^
  -kernel hammam\target\x86_64-unknown-none\debug\hammam-kernel ^
  -append "console=ttyS0" ^
  -serial stdio ^
  -display none ^
  -m 256M ^
  -netdev user,id=net0,hostfwd=tcp::8080-:80 ^
  -device virtio-net-pci,netdev=net0

echo.
echo [DONE] QEMU session ended
echo ============================================================================
pause
