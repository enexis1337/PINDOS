/*
 * PINDOS Hammam Microkernel Linker Script for x86_64
 * GRUB-compatible ELF64 output
 */

OUTPUT_FORMAT("elf64-x86-64")
OUTPUT_ARCH("i386:x86-64")
ENTRY(_start)

/* Base address for higher half kernel */
KERNEL_VIRT_BASE = 0xFFFF800000000000;
KERNEL_LOAD_ADDR = 0x100000;

/* Segment sizes */
PHYSICAL_LOAD_SIZE = 2M;

/* Memory layout */
PHYSICAL_MEMORY_START = 0x0;
PHYSICAL_MEMORY_END = 0x80000000; /* 2GB - typical kernel space */

/* Sections */
SECTIONS
{
    /* Multiboot header must be in first 32KB */
    .multiboot_header : {
        KEEP(*(.multiboot_header))
    } = 0x90

    /* Text section - code */
    .text : AT(LOADADDR(.multiboot_header) + SIZEOF(.multiboot_header)) {
        *(.text*)
        *(.text..*)
        *(.gnu.linkonce.t*)
    } =0xCC

    /* Read-only data */
    .rodata : {
        *(.rodata*)
        *(.gnu.linkonce.r*)
        *(.eh_frame)
    } =0xCC

    /* Exception tables */
    .eh_frame_hdr : {
        *(.eh_frame_hdr*)
    }

    .eh_frame : {
        *(.eh_frame*)
    } =0xCC

    /* Data section - initialized data */
    .data : {
        *(.data*)
        *(.gnu.linkonce.d*)
        *(.BSS*)
        *(.bss*)
        *(.COMMON*)
        *(.got*)
        *(.got.plt*)
    } =0xCC

    /* Thread-local storage */
    .tdata : {
        *(.tdata*)
        *(.tbss*)
    }

    /* Dynamic linking sections */
    .dynamic : {
        *(.dynamic*)
    }

    .dynsym : {
        *(.dynsym*)
    }

    .dynstr : {
        *(.dynstr*)
    }

    .rel.dyn : {
        *(.rel*)
    }

    .rela.dyn : {
        *(.rela*)
    }

    /* Symbol table for debugging */
    .symtab : {
        *(.symtab*)
    }

    .strtab : {
        *(.strtab*)
    }

    /* Debug info */
    .debug : {
        *(.debug*)
    }

    .debug_info : {
        *(.debug_info*)
    }

    .debug_abbrev : {
        *(.debug_abbrev*)
    }

    .debug_line : {
        *(.debug_line*)
    }

    .debug_str : {
        *(.debug_str*)
    }

    .comment : {
        *(.comment*)
    }

    /* Discard non-allocated sections */
    /DISCARD/ : {
        *(.note*)
        *(.MIPS.abiflags)
        *(.MIPS.options)
        *(.MIPS.reginfo)
        *(.MIPS.dynsym)
    }
}

/*
 * GRUB memory map requirements:
 * - Kernel must be loaded at a physical address
 * - Kernel must be relocatable for KASLR (optional)
 * - Kernel must have a valid ELF header
 */