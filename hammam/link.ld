ENTRY(_start)

SECTIONS {
    /* Start at 1MB physical address */
    . = 0x00100000;
    
    /* Boot header MUST be absolutely first - before any code */
    .boot_header : {
        KEEP(*(.boot_header))
    }
    
    /* Code and read-only data */
    .text : {
        *(.text .text.*)
        *(.rodata .rodata.*)
    }
    
    /* PVH note */
    .note.pvh : {
        KEEP(*(.note.pvh))
    }
    
    /* Initialized data */
    .data : {
        *(.data .data.*)
    }
    
    /* Uninitialized data */
    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    }
}
