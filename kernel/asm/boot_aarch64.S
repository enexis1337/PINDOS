# AArch64 boot code for PINDOS Hammam microkernel
# This assembly code handles early boot sequence for ARM64 (UEFI/GRUB)

.section .text.boot
.code 64

.global _start
.type _start, @function

# Entry point called by UEFI/GRUB
_start:
    # CurrentEL register tells us current exception level
    mrs x0, CurrentEL
    and x0, x0, #0xC        # Extract EL bits
    
    cmp x0, #0xC            # Are we in EL3 (secure monitor)?
    b.eq enter_el3
    
    cmp x0, #0x8            # Are we in EL2 (hypervisor)?
    b.eq enter_el2
    
    cmp x0, #0x4            # Are we in EL1 (kernel)?
    b.eq enter_el1
    
    # Should not reach here
    b .

enter_el3:
    # We're in EL3, need to drop to EL1
    # Configure EL3 -> EL1 transition
    mov x0, #0x5            # EL2 mode
    msr spsr_el3, x0
    mov x0, xzr
    msr elr_el3, x1         # Return address
    eret

enter_el2:
    # We're in EL2, drop to EL1
    # Disable hypervisor features
    mov x0, #0
    msr hcr_el2, x0
    
    # Configure EL2 -> EL1 transition
    mov x0, #0x5            # EL1 mode
    msr spsr_el2, x0
    mov x0, xzr
    msr elr_el2, x1
    eret

enter_el1:
    # We're in EL1 (kernel mode)
    
    # Disable interrupts
    msr daifset, #0xF
    
    # Set up stack pointer (SP_EL0 for now)
    ldr x0, =stack_top
    mov sp, x0
    
    # Clear registers
    mov x0, xzr
    mov x1, xzr
    mov x2, xzr
    mov x3, xzr
    mov x4, xzr
    mov x5, xzr
    mov x6, xzr
    mov x7, xzr
    mov x8, xzr
    mov x9, xzr
    mov x10, xzr
    mov x11, xzr
    mov x12, xzr
    mov x13, xzr
    mov x14, xzr
    mov x15, xzr
    mov x16, xzr
    mov x17, xzr
    mov x18, xzr
    mov x19, xzr
    mov x20, xzr
    mov x21, xzr
    mov x22, xzr
    mov x23, xzr
    mov x24, xzr
    mov x25, xzr
    mov x26, xzr
    mov x27, xzr
    mov x28, xzr
    mov x29, xzr
    mov x30, xzr
    
    # Enable MMU and caches
    # Set MAIR (Memory Attribute Indirection Register)
    ldr x0, =mair_el1
    msr mair_el1, x0
    
    # Set TCR (Translation Control Register)
    ldr x0, =tcr_el1
    msr tcr_el1, x0
    
    # Set TTBR0 (Translation Table Base Register 0)
    ldr x0, =ttbr0_el1
    msr ttbr0_el1, x0
    
    # Enable MMU
    mrs x0, sctlr_el1
    orr x0, x0, #1          # M bit (enable MMU)
    orr x0, x0, #2          # A bit (alignment check)
    orr x0, x0, #4          # C bit (data cache)
    orr x0, x0, #8          # I bit (instruction cache)
    msr sctlr_el1, x0
    
    # Invalidate TLB
    tlbi vmalle1is
    dsb ish
    isb
    
    # Clear BSS section
    ldr x0, =bss_start
    ldr x1, =bss_end
    sub x1, x1, x0
    lsr x1, x1, #3
    mov x2, xzr
bss_clear:
    cbz x1, bss_done
    str xzr, [x0], #8
    sub x1, x1, #1
    b bss_clear
bss_done:
    
    # Call Rust kernel entry point
    # x0 = DTB address (passed by bootloader)
    # x1 = Magic number (0 = UEFI, 1 = Multiboot2)
    mov %rdi, x0
    mov %esi, x1
    bl kmain

# Halt loop (should never be reached)
halt_loop:
    wfi
    b halt_loop

# Page table configuration
.align 12
mair_el1:
    .quad 0x000000000000FF44   # Device memory, Normal memory

tcr_el1:
    .quad (1 << 14) | (0x2 << 32) | (0x1 << 16) | (0x0 << 12) | (0x0 << 8) | (0x0 << 4) | 0x0
    # TCR_EL1 fields:
    # - T0SZ = 32 (4GB address space with 4KB granule)
    # - IPS = 0x2 (36-bit physical address)
    # - TG0 = 0 (4KB granule)
    # - SH0 = 0x1 (inner shareable)
    # - ORGN0 = 0x1 (write-back, read-allocate)
    # - IRGN0 = 0x1 (write-back, read-allocate)

# Translation table (covers first 1GB)
.align 12
ttbr0_el1:
    .quad page_table
    .quad 0

# Page table structure
.align 12
page_table:
    # Level 3 table (512 entries, each covers 2MB)
    .rept 512
    .quad (0x0000000000000003) | (0x0000000000000000 + (. - page_table) / 8 * 0x200000)
    .endr

# BSS section
.section .bss
.align 16
bss_start:
    .skip 4096
stack_top:

.size _start, . - _start
.type halt_loop, @function