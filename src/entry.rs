use crate::error::{Result, HayazipError};
use byteorder::{LittleEndian, ReadBytesExt};
use memmap2::Mmap;
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct ZipEntry {
    pub filename: String,
    pub local_header_offset: usize,
    pub compressed_size: usize,
    pub uncompressed_size: usize,
    pub crc32: u32,
    pub method: u16,
    pub flags: u16,
    pub is_dir: bool,
    pub external_attr: u32,
}

impl ZipEntry {
    /// Read the local file header and return the start offset of the actual compressed data
    pub fn data_offset(&self, mmap: &Mmap) -> Result<usize> {
        let offset = self.local_header_offset;
        let max_len = std::cmp::min(offset + 30, mmap.len());
        let mut cursor = Cursor::new(&mmap[offset..max_len]); // Safely slice

        let signature = match cursor.read_u32::<LittleEndian>() {
            Ok(s) => s,
            Err(e) => {
                return Err(e.into());
            }
        };
        if signature != 0x04034b50 {
            return Err(HayazipError::InvalidFormat("Invalid Local File Header signature"));
        }
        
        cursor.set_position(26);
        let filename_len = cursor.read_u16::<LittleEndian>()? as usize;
        let extra_field_len = cursor.read_u16::<LittleEndian>()? as usize;
        
        Ok(offset + 30 + filename_len + extra_field_len)
    }

    pub fn data<'a>(&self, mmap: &'a Mmap) -> Result<&'a [u8]> {
        let offset = self.data_offset(mmap)?;
        Ok(&mmap[offset..offset + self.compressed_size])
    }

    pub fn is_unix_symlink(&self) -> bool {
        // Unix mode is in upper 16 bits of external_attr
        // Mode 0120000 means symbolic link
        let mode = self.external_attr >> 16;
        (mode & 0o170000) == 0o120000
    }

    pub fn unix_mode(&self) -> Option<u32> {
        let mode = self.external_attr >> 16;
        if mode != 0 {
            Some(mode & 0o7777)
        } else {
            None
        }
    }
}
