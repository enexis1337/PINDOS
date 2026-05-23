# Multiboot2 boot header for PINDOS Hammam microkernel
# This assembly code handles early boot sequence for x86_64

#include <asm/multiboot2.h>

.section .multiboot_header, "a"
.align 8

# Multiboot2 header
multiboot_header:
    .long MULTIBOOT2_HEADER_MAGIC
    .long ARCHITECTURE_I386_MULTIBOOT2_HEADER_OPTIONAL
    .long MULTIBOOT2_HEADER_LENGTH
    .long -(MULTIBOOT2_HEADER_MAGIC + ARCHITECTURE_I386_MULTIBOOT2_HEADER_OPTIONAL + MULTIBOOT2_HEADER_LENGTH)

    # End tag
    .short MULTIBOOT2_TAG_TYPE_END
    .short 0
    .long 8

.section .text
.code32

.global _start
.type _start, @function

# Entry point called by GRUB
_start:
    # Disable interrupts
    cli
    
    # Save Multiboot2 information pointer
    mov %eax, %edi        # Multiboot2 magic in eax
    mov %ebx, %esi        # Multiboot2 info pointer in ebx
    
    # Enable A20 line
    inb $0x92, %al
    or $2, %al
    outb %al, $0x92
    
    # Set up temporary page tables for identity mapping
    # We'll map the first 2MB of physical memory
    mov $pml4, %eax
    mov %eax, %cr3
    
    # Enable PAE (Physical Address Extension)
    mov %cr4, %eax
    or $(1 << 5), %eax    # PAE bit
    mov %eax, %cr4
    
    # Enable long mode
    mov $0xC0000080, %ecx
    rdmsr
    or $(1 << 8), %eax    # LME bit
    wrmsr
    
    # Enable paging
    mov %cr0, %eax
    or $(1 << 31), %eax   # PG bit
    mov %eax, %cr0
    
    # Now in long mode, switch to 64-bit code
    lgdt gdt64_desc
    ljmp $0x08, $start64

.code64
start64:
    # Set up segment registers
    mov $0x10, %eax
    mov %eax, %ds
    mov %eax, %es
    mov %eax, %fs
    mov %eax, %gs
    mov %eax, %ss
    
    # Set up stack
    mov $stack_top, %rsp
    mov %rsp, %rbp
    
    # Clear BSS section
    mov $bss_start, %rdi
    mov $bss_end, %rcx
    sub %rdi, %rcx
    shr $3, %rcx
    xor %rax, %rax
    rep stosq
    
    # Call Rust kernel entry point
    call kmain

# Halt loop (should never be reached)
halt_loop:
    hlt
    jmp halt_loop

# GDT for 64-bit mode
.align 16
gdt64:
    .quad 0x0000000000000000    # Null descriptor
    .quad 0x0020980000000000    # Kernel code (64-bit)
    .quad 0x0000920000000000    # Kernel data
    .quad 0x0020F80000000000    # User code (64-bit)
    .quad 0x0000F20000000000    # User data

gdt64_desc:
    .word gdt64_desc - gdt64 - 1
    .quad gdt64

# Page table structures (PML4, PDPT, PD, PT)
.align 4096
pml4:
    .quad pdpt + 0x7            # Identity map first entry
    .fill 4095, 8, 0

pdpt:
    .quad pd + 0x7              # Map first 1GB
    .fill 4095, 8, 0

pd:
    .quad pt + 0x7              # Map first 2MB
    .fill 511, 8, 0

pt:
    .fill 512, 8, 0

# BSS section for uninitialized data
.section .bss
.align 16
bss_start:
    .skip 4096                  # Kernel stack
stack_top:

# Symbol sizes
.size _start, . - _start
.type halt_loop, @function