/*
 * PINDOS Hammam Microkernel Linker Script for AArch64
 * UEFI/GRUB-compatible ELF64 output
 */

OUTPUT_FORMAT("elf64-littleaarch64")
OUTPUT_ARCH("aarch64")
ENTRY(_start)

/* Base address for kernel */
KERNEL_VIRT_BASE = 0xFFFF000000000000;
KERNEL_LOAD_ADDR = 0x100000;

/* Sections */
SECTIONS
{
    /* Boot code - must be at beginning */
    .text.boot : {
        *(.text.boot)
    } =0xCC

    /* Text section - code */
    .text : {
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
    }
}