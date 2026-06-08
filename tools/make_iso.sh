#!/bin/bash
set -e

KERNEL_ELF="${KERNEL_PATH:-hammam/target/x86_64-unknown-none/debug/hammam-kernel}"
ISO_DIR="iso_root"
OUTPUT="hammam.iso"

if [ ! -d "hammam" ]; then
    echo "[ERROR] Not in project root"
    exit 1
fi

if [ ! -f "$KERNEL_ELF" ]; then
    echo "[ERROR] Kernel not found at $KERNEL_ELF"
    exit 1
fi

echo "[1] Preparing ISO directory..."
mkdir -p "$ISO_DIR/boot/grub"

echo "[2] Copying kernel..."
echo "  Using kernel: $KERNEL_ELF"
cp "$KERNEL_ELF" "$ISO_DIR/boot/hammam.elf"
chmod 644 "$ISO_DIR/boot/hammam.elf"

echo "[3] Creating GRUB config..."
cat > "$ISO_DIR/boot/grub/grub.cfg" << 'EOF'
set timeout=3
set default=0

menuentry "PINDOS Hammam Kernel" {
    multiboot2 /boot/hammam.elf
    boot
}
EOF

echo "[4] Creating ISO with grub-mkrescue..."
# Build into a temp file first so a locked hammam.iso (e.g. QEMU still running)
# does not block grub-mkrescue.
OUTPUT_TMP="${OUTPUT}.new"

if command -v grub-mkrescue >/dev/null 2>&1; then
    MKRESCUE=grub-mkrescue
elif command -v grub2-mkrescue >/dev/null 2>&1; then
    MKRESCUE=grub2-mkrescue
else
    echo "[ERROR] grub-mkrescue not found!"
    echo "Install: sudo apt install grub-pc-bin xorriso"
    exit 1
fi

rm -f "$OUTPUT_TMP"
"$MKRESCUE" -o "$OUTPUT_TMP" "$ISO_DIR" 2>&1 | grep -v "NOTE:" || true

if [ ! -f "$OUTPUT_TMP" ]; then
    echo "[ERROR] ISO creation failed"
    exit 1
fi

if rm -f "$OUTPUT" 2>/dev/null; then
    mv -f "$OUTPUT_TMP" "$OUTPUT"
elif mv -f "$OUTPUT_TMP" "$OUTPUT" 2>/dev/null; then
    :
else
    echo "[ERROR] Cannot replace $OUTPUT (file is locked)."
    echo "Close QEMU or any program using the ISO, then run again."
    echo "New image is available at: $OUTPUT_TMP"
    exit 1
fi

if [ -f "$OUTPUT" ]; then
    SIZE=$(ls -lh "$OUTPUT" | awk '{print $5}')
    echo ""
    echo "[OK] ISO created: $OUTPUT ($SIZE)"
    echo ""
else
    echo "[ERROR] ISO creation failed"
    exit 1
fi
