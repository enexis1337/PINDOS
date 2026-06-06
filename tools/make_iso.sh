#!/bin/bash
set -e

# Convert Windows path if running from WSL
KERNEL="hammam/target/x86_64-unknown-none/debug/hammam-kernel"
ISO_DIR="iso_root"
OUTPUT="hammam.iso"

# If running from WSL, ensure we're in the right directory
if [ ! -d "hammam" ]; then
    echo "[ERROR] Not in project root. Current dir: $(pwd)"
    exit 1
fi

# Проверяем что ядро собрано
if [ ! -f "$KERNEL" ]; then
    echo "[ERROR] Kernel not found at $KERNEL"
    ls -la hammam/target/x86_64-unknown-none/debug/ 2>/dev/null || echo "Debug dir not found"
    exit 1
fi

echo "[1] Preparing ISO directory structure..."
rm -rf "$ISO_DIR"
mkdir -p "$ISO_DIR/boot/grub"

echo "[2] Copying kernel..."
cp "$KERNEL" "$ISO_DIR/boot/hammam.elf"

echo "[3] Creating GRUB configuration..."
cat > "$ISO_DIR/boot/grub/grub.cfg" << 'EOF'
set timeout=0
set default=0

menuentry "PINDOS / Hammam" {
    multiboot2 /boot/hammam.elf
    boot
}
EOF

echo "[4] Building ISO with GRUB..."
grub-mkrescue -o "$OUTPUT" "$ISO_DIR" 2>&1 || {
    echo "[ERROR] grub-mkrescue failed"
    exit 1
}

if [ -f "$OUTPUT" ]; then
    SIZE=$(ls -lh "$OUTPUT" | awk '{print $5}')
    echo "[OK] ISO created successfully: $OUTPUT ($SIZE)"
    exit 0
else
    echo "[ERROR] Failed to create ISO"
    exit 1
fi
