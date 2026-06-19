/* Multiboot2 header - MUST be first in .text */
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
.global multiboot2_header_end
multiboot2_header_end:
