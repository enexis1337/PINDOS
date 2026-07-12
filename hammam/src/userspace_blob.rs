use alloc::vec::Vec;
use alloc::boxed::Box;

/// Compiled net-server userspace ELF binary (virtio-net driver + smoltcp stack).
/// Built with: cargo build --release --target x86_64-unknown-none in userspace/net-server
pub const NET_SERVER_ELF: &[u8] = include_bytes!(
    "../../userspace/net-server/target/x86_64-unknown-none/release/net-server"
);

/// Build a CPIO newc archive containing /hello with the net-server ELF binary.
/// Returns a leaked `&'static [u8]` suitable for `InitramfsFs::parse`.
pub fn build_initramfs() -> &'static [u8] {
    let mut cpio = Vec::with_capacity(NET_SERVER_ELF.len() + 512);
    append_cpio_entry(&mut cpio, b"/hello", NET_SERVER_ELF);
    append_cpio_entry(&mut cpio, b"TRAILER!!!", &[]);
    Box::leak(cpio.into_boxed_slice())
}

fn append_cpio_entry(cpio: &mut Vec<u8>, name: &[u8], data: &[u8]) {
    let name_with_nul = &[name, &[0]].concat();
    // magic "070701" (6) + ino(8) + mode(8) + uid(8) + gid(8)
    cpio.extend_from_slice(b"07070100000000");
    // mode: regular file 0100644 = 0x81A4
    cpio.extend_from_slice(b"000081a4");
    // uid, gid, nlink, mtime
    cpio.extend_from_slice(b"00000000000000000000000100000000");
    // filesize (8 hex chars)
    cpio.extend_from_slice(&hex8(data.len()));
    // devmajor, devminor, rdevmajor, rdevminor
    cpio.extend_from_slice(b"00000000000000000000000000000000");
    // namesize (8 hex chars)
    cpio.extend_from_slice(&hex8(name_with_nul.len()));
    // check
    cpio.extend_from_slice(b"00000000");
    // filename
    cpio.extend_from_slice(name_with_nul);
    align4(cpio);
    // file data
    cpio.extend_from_slice(data);
    align4(cpio);
}

fn hex8(v: usize) -> [u8; 8] {
    let mut buf = [0u8; 8];
    for i in 0..8 {
        let nibble = (v >> (4 * (7 - i))) & 0xF;
        buf[i] = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + nibble as u8 - 10 };
    }
    buf
}

fn align4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}
