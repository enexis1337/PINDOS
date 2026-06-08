#!/usr/bin/env python3
"""
Create a proper Multiboot2 kernel from ELF.
Extracts all LOAD segments and creates a raw binary with Multiboot2 header.
"""

import sys
import struct
import subprocess

def get_elf_sections(elf_file):
    """Use objdump to extract section information."""
    result = subprocess.run(
        ['objdump', '-h', elf_file],
        capture_output=True,
        text=True
    )
    if result.returncode != 0:
        print(f"Error running objdump: {result.stderr}", file=sys.stderr)
        return None
    return result.stdout

def extract_text_section(elf_file):
    """Extract .text section as raw binary."""
    result = subprocess.run(
        ['objcopy', '-O', 'binary', '-j', '.text', elf_file, '/tmp/text.bin'],
        capture_output=True,
        text=True
    )
    if result.returncode != 0:
        print(f"Error extracting .text: {result.stderr}", file=sys.stderr)
        return None
    
    try:
        with open('/tmp/text.bin', 'rb') as f:
            return f.read()
    except:
        return None

def create_multiboot_kernel(elf_file, output_file):
    """Create Multiboot2 kernel binary."""
    
    print("[*] Creating Multiboot2 kernel...")
    
    # Extract .text section
    print("[1] Extracting .text section from ELF...")
    text_data = extract_text_section(elf_file)
    
    if text_data is None:
        print("[ERROR] Could not extract .text section", file=sys.stderr)
        return False
    
    print(f"    .text size: {len(text_data)} bytes")
    
    # Create Multiboot2 header
    print("[2] Creating Multiboot2 header...")
    
    # Header structure:
    # Offset  Size  Field
    # 0       4     magic (0xE85250D6)
    # 4       4     architecture (0 = i386)
    # 8       4     header_length (16 bytes)
    # 12      4     checksum
    # 16      4     end tag (type=0, flags=0)
    # 20      4     end tag size (8)
    
    magic = 0xE85250D6
    arch = 0
    header_len = 16
    checksum = (0x100000000 - (magic + arch + header_len)) & 0xFFFFFFFF
    
    # Create header
    header = struct.pack('<IIII', magic, arch, header_len, checksum)
    header += struct.pack('<II', 0x00000000, 0x00000008)  # End tag
    
    print(f"    Header: {header.hex()}")
    
    # Combine header + text section
    print("[3] Combining header and kernel code...")
    kernel_data = header + text_data
    
    # Write output
    print(f"[4] Writing to {output_file}...")
    try:
        with open(output_file, 'wb') as f:
            f.write(kernel_data)
        print(f"[OK] Multiboot2 kernel created: {len(kernel_data)} bytes")
        return True
    except Exception as e:
        print(f"[ERROR] Failed to write output: {e}", file=sys.stderr)
        return False

if __name__ == '__main__':
    if len(sys.argv) != 3:
        print("Usage: create_multiboot_kernel.py <input.elf> <output.bin>")
        sys.exit(1)
    
    elf_file = sys.argv[1]
    output_file = sys.argv[2]
    
    if not create_multiboot_kernel(elf_file, output_file):
        sys.exit(1)
