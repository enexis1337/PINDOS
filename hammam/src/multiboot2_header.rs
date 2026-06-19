/// PVH (Paravirtualized x86) ELF Note для QEMU 10.x+
/// Это требуется для QEMU новых версий для загрузки ненжатого ядра

#[repr(C, align(4))]
pub struct PvhNote {
    namesz: u32,
    descsz: u32,
    note_type: u32,
    name: [u8; 8],  // "PVH\0" + 4 bytes padding for alignment
}

#[used]
#[link_section = ".note.pvh"]
static PVH_NOTE: PvhNote = PvhNote {
    namesz: 4,          // "PVH\0" = 4 bytes
    descsz: 0,          // No descriptor
    note_type: 0x13,    // PVH type
    name: [b'P', b'V', b'H', 0, 0, 0, 0, 0],
};

