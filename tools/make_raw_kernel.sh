#!/bin/bash
set -e

# Create a raw Multiboot2 kernel binary from ELF
# This puts the header at offset 0 which GRUB can definitely find

INPUT="$1"
OUTPUT="$2"

if [ ! -f "$INPUT" ]; then
    echo "Error: Input file not found: $INPUT"
    exit 1
fi

echo "[*] Creating raw Multiboot2 kernel..."

# Extract just the .text section which contains our header + entry code
echo "[1] Extracting .text section..."
objcopy -O binary -j .text "$INPUT" "$OUTPUT" || {
    echo "Error: Could not extract .text section"
    exit 1
}

SIZE=$(wc -c < "$OUTPUT")
echo "[OK] Raw kernel created: $OUTPUT ($SIZE bytes)"

# Verify the header is correct
echo "[2] Verifying Multiboot2 header at offset 0..."
hexdump -C "$OUTPUT" | head -2

# Check magic
MAGIC=$(xxd -p -l 4 "$OUTPUT")
if [ "$MAGIC" = "d65052e8" ]; then
    echo "[OK] Multiboot2 magic found: 0xE85250D6"
else
    echo "[WARNING] Magic might be wrong. Got: $MAGIC (little-endian: $MAGIC)"
fi
