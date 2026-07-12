OUTPUT_FORMAT("elf64-x86-64")
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

SECTIONS {
    /* Start at 1 MiB (standard for multiboot) */
    . = 0x00100000;
    kernel_phys_start = .;
    
    /* Multiboot2 header must be in first 32KB and in loadable segment */
    .text : ALIGN(4096) {
        KEEP(*(.text.boot_header))
        *(.text.boot)
        KEEP(*(.text._hammam_entry))
        *(.text)
        *(.text.*)
    }
    
    .rodata : ALIGN(4096) {
        *(.rodata)
        *(.rodata.*)
    }
    
    .data : ALIGN(4096) {
        KEEP(*(.data.boot))
        *(.data)
        *(.data.*)
        *(.got)
    }
    
    .bss : ALIGN(4096) {
        *(.bss)
        *(.bss.*)
        *(COMMON)
    }
    
    . = ALIGN(4096);
    kernel_phys_end = .;

    /DISCARD/ : {
        *(.note.GNU-stack)
        *(.note.pvh)
        *(.eh_frame*)
    }
}
