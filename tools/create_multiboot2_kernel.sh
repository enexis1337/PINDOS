#!/bin/bash
set -e

# Script to create a proper Multiboot2 kernel image from ELF

INPUT_ELF="$1"
OUTPUT_BIN="$2"

if [ -z "$INPUT_ELF" ] || [ -z "$OUTPUT_BIN" ]; then
    echo "Usage: $0 <input.elf> <output.bin>"
    exit 1
fi

if [ ! -f "$INPUT_ELF" ]; then
    echo "Error: Input file not found: $INPUT_ELF"
    exit 1
fi

# Create a temporary directory
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

# Extract .text section from ELF
objcopy -O binary -j .text "$INPUT_ELF" "$TMPDIR/text.bin"

# Extract other sections needed
objcopy -O binary -j .data "$INPUT_ELF" "$TMPDIR/data.bin"
objcopy -O binary -j .bss "$INPUT_ELF" "$TMPDIR/bss.bin"

# Get size of BSS section
ELF_INFO=$(readelf -S "$INPUT_ELF" | grep "\.bss")
BSS_SIZE=$(echo "$ELF_INFO" | awk '{print $6}')
BSS_ADDR=$(echo "$ELF_INFO" | awk '{print $4}')

# Create Multiboot2 header (24 bytes)
# Magic, Arch, Length, Checksum, end tag (8 bytes)
MULTIBOOT2_HEADER=$(printf '\xd6\x50\x52\xe8\x00\x00\x00\x00\x10\x00\x00\x00\x1a\xaf\xad\x17\x00\x00\x00\x00\x08\x00\x00\x00')

# Create output binary: header + padding + text
(
    printf "%s" "$MULTIBOOT2_HEADER"
    # Pad to align text to page boundary (4096 bytes)
    dd if=/dev/zero bs=1 count=$((4096 - 24)) 2>/dev/null || true
    cat "$TMPDIR/text.bin"
) > "$OUTPUT_BIN"

echo "Created Multiboot2 kernel: $OUTPUT_BIN"
