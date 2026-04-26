// FAT12/FAT16/FAT32 файловая система
// Поддерживает: чтение/запись файлов, директории, длинные имена (LFN)

pub mod bpb;
pub mod fat;
pub mod dir;
pub mod file;

pub use fat::FatFs;
pub use file::{FatFile, OpenMode};
