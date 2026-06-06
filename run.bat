@echo off
setlocal enabledelayedexpansion

echo ============================================================================
echo PINDOS OS Phase 6 - Hammam Kernel Boot (GRUB + ISO)
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
echo [2] Creating bootable ISO via WSL2...
wsl bash tools/make_iso.sh
if !errorlevel! neq 0 (
  echo [ERROR] Failed to create ISO
  exit /b !errorlevel!
)

echo.
echo [3] Launching QEMU with ISO...
echo ============================================================================

qemu-system-x86_64 ^
  -M pc ^
  -cdrom hammam.iso ^
  -serial stdio ^
  -display gtk ^
  -m 256M ^
  -netdev user,id=net0,hostfwd=tcp::8080-:80 ^
  -device virtio-net-pci,netdev=net0 ^
  -boot d

echo.
echo [DONE] QEMU session ended
echo ============================================================================
pause
