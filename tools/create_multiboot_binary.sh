#!/bin/bash
set -e

# Create a raw Multiboot2 kernel binary from ELF

KERNEL_ELF="$1"
OUTPUT_BIN="$2"

if [ ! -f "$KERNEL_ELF" ]; then
    echo "Error: Kernel ELF not found: $KERNEL_ELF"
    exit 1
fi

echo "[*] Creating Multiboot2 kernel binary..."

# Extract the .text section (contains code and data)
echo "[1] Extracting .text section..."
objcopy -O binary -j .text "$KERNEL_ELF" /tmp/kernel_text.bin 2>/dev/null || {
    echo "Error: .text section not found in ELF"
    exit 1
}

# Get text section LMA (load address)
TEXT_LMA=$(readelf -S "$KERNEL_ELF" 2>/dev/null | grep "\.text" | awk '{print $4}')
echo "    .text LMA: $TEXT_LMA"

# Create Multiboot2 header (24 bytes total):
# - Magic: 0xE85250D6 (4 bytes)
# - ISA: 0x00000000 (4 bytes)  
# - Length: 0x00000010 (4 bytes)
# - Checksum: 0x1AACAF2A (4 bytes)
# - End tag type: 0x0000 (2 bytes)
# - End tag flags: 0x0000 (2 bytes)
# - End tag size: 0x00000008 (4 bytes)

echo "[2] Creating Multiboot2 header..."
printf '\xd6\x50\x52\xe8' > /tmp/mb2_header.bin  # Magic: 0xE85250D6
printf '\x00\x00\x00\x00' >> /tmp/mb2_header.bin # ISA: 0
printf '\x10\x00\x00\x00' >> /tmp/mb2_header.bin # Length: 16
printf '\x1a\xaf\xad\x17' >> /tmp/mb2_header.bin # Checksum
printf '\x00\x00\x00\x00' >> /tmp/mb2_header.bin # End tag
printf '\x08\x00\x00\x00' >> /tmp/mb2_header.bin # End tag size

# Combine: header + text section
echo "[3] Combining header and kernel code..."
cat /tmp/mb2_header.bin /tmp/kernel_text.bin > "$OUTPUT_BIN"

SIZE=$(wc -c < "$OUTPUT_BIN")
echo "[OK] Multiboot2 kernel created: $OUTPUT_BIN ($SIZE bytes)"

# Verify header
echo "[4] Verifying Multiboot2 header..."
hexdump -C "$OUTPUT_BIN" | head -3

rm -f /tmp/mb2_header.bin /tmp/kernel_text.bin
