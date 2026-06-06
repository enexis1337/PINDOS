/* Multiboot2 header - MUST be absolutely first in the binary */
.set MB2_MAGIC, 0xe85250d6
.set MB2_ARCH, 0          
.set MB2_HLEN, 16         
.set MB2_CHECKSUM, -(MB2_MAGIC + MB2_ARCH + MB2_HLEN)

.section .boot_header, "ax"
.align 8
.global multiboot2_header
multiboot2_header:
    .long MB2_MAGIC           
    .long MB2_ARCH            
    .long MB2_HLEN            
    .long MB2_CHECKSUM        
    /* End tag */
    .word 0                   
    .word 0                   
    .long 8                   

/* Multiboot2 entry point */
.section .text
.global _start
.align 4
_start:
    /* 
     * Multiboot2 bootloader (GRUB) passes:
     * EAX = magic (0x36d76289)
     * EBX = pointer to Multiboot2 boot info structure
     */
    
    /* Disable interrupts */
    cli
    
    /* Set up minimal stack */
    mov $stack_top, %esp
    
    /* Call Rust entry point: _start_multiboot2(magic, boot_info_addr) */
    push %ebx        /* arg2: boot_info_addr in EBX */
    push %eax        /* arg1: magic in EAX */
    call _start_multiboot2
    
    /* Should not return */
    hlt
    jmp .

.section .bss
.align 4096
stack_bottom:
    .space 65536  /* 64KB stack */
stack_top:

