OUTPUT_FORMAT(elf64-x86-64)
ENTRY(kmain)
SECTIONS
{
    . = 0xffffffff80000000;

    .text : { *(.text) }
    .rodata : { *(.rodata) }
    .data : {
        . = ALIGN(4096);
        _start = .;
        *(.data)
        . = ALIGN(4096);
        _end = .;
        /* Keep the Limine requests */
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
    }
    .bss : {
        *(.bss)
        *(COMMON)
    }
    /DISCARD/ : { *(.note.GNU-stack) *(.comment) }
}