@echo off
echo ========================================
echo PINDOS Build and ISO Creation Script
echo ========================================
echo.

echo [1/2] Building kernel...
powershell -ExecutionPolicy Bypass -File build.ps1
if %errorlevel% neq 0 (
    echo ERROR: Kernel build failed!
    pause
    exit /b 1
)

echo.
echo [2/2] Creating ISO image...
python make-iso.py
if %errorlevel% neq 0 (
    echo ERROR: ISO creation failed!
    pause
    exit /b 1
)

echo.
echo ========================================
echo SUCCESS! Files created:
echo   target\pindos.img  - Floppy disk image
echo   target\pindos.iso  - CD/DVD ISO image
echo ========================================
echo.
echo To test in QEMU:
echo   qemu-system-i386 -cdrom target\pindos.iso -m 64M
echo.
echo To burn to CD/DVD:
echo   Use ImgBurn, Nero, or Windows built-in burning
echo.
echo To create bootable USB:
echo   Use Rufus or balenaEtcher
echo.
pause