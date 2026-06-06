#!/bin/bash
set -e

echo "============================================================================"
echo "WSL2 Setup - Installing GRUB and ISO Tools"
echo "============================================================================"
echo ""

# Detect if running in WSL
if grep -qi microsoft /proc/version; then
    echo "[OK] Running in WSL2"
else
    echo "[WARNING] Not running in WSL2 - this script should be run in WSL2 Ubuntu terminal"
    exit 1
fi

echo "[1] Updating package lists..."
sudo apt update

echo ""
echo "[2] Installing GRUB tools..."
sudo apt install -y grub-pc-bin grub-common

echo ""
echo "[3] Installing xorriso and mtools (for ISO creation)..."
sudo apt install -y xorriso mtools

echo ""
echo "[4] Verifying installation..."
echo "    grub-mkrescue: $(which grub-mkrescue)"
echo "    xorriso: $(which xorriso)"
echo "    mtools: $(which mtools)"

echo ""
echo "============================================================================"
echo "[OK] WSL2 setup complete!"
echo "============================================================================"
echo ""
echo "Now you can run: .\run.bat"
echo ""
