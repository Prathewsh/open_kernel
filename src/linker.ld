ENTRY(_start)

SECTIONS {
    . = 0x200000; /* 2 MB - safe conventional memory above BIOS/UEFI reserved area */

    .text : {
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        *(.bss .bss.*)
    }

    /* Discard unnecessary sections */
    /DISCARD/ : {
        *(.eh_frame)
        *(.note .note.*)
    }
}
