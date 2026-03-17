use crate::error::{HayazipError, Result};
use byteorder::{LittleEndian, ReadBytesExt};
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
    pub fn data_offset(&self, archive_data: &[u8]) -> Result<usize> {
        let offset = self.local_header_offset;
        let header = slice(archive_data, offset, 30)?;
        let mut cursor = Cursor::new(header);

        let signature = match cursor.read_u32::<LittleEndian>() {
            Ok(s) => s,
            Err(e) => {
                return Err(e.into());
            }
        };
        if signature != 0x04034b50 {
            return Err(HayazipError::InvalidFormat(
                "Invalid Local File Header signature",
            ));
        }

        cursor.set_position(26);
        let filename_len = cursor.read_u16::<LittleEndian>()? as usize;
        let extra_field_len = cursor.read_u16::<LittleEndian>()? as usize;

        let data_offset = offset
            .checked_add(30)
            .and_then(|value| value.checked_add(filename_len))
            .and_then(|value| value.checked_add(extra_field_len))
            .ok_or(HayazipError::InvalidFormat(
                "Local file header exceeds archive bounds",
            ))?;

        if data_offset > archive_data.len() {
            return Err(HayazipError::InvalidFormat(
                "Local file header exceeds archive bounds",
            ));
        }

        Ok(data_offset)
    }

    pub fn data<'a>(&self, archive_data: &'a [u8]) -> Result<&'a [u8]> {
        let offset = self.data_offset(archive_data)?;
        slice(archive_data, offset, self.compressed_size)
    }

    pub fn is_unix_symlink(&self) -> bool {
        // Unix mode is in upper 16 bits of external_attr
        // Mode 0120000 means symbolic link
        let mode = self.external_attr >> 16;
        (mode & 0o170000) == 0o120000
    }

    pub fn unix_mode(&self) -> Option<u32> {
        let mode = self.external_attr >> 16;
        if mode != 0 { Some(mode & 0o7777) } else { None }
    }
}

fn slice(data: &[u8], start: usize, len: usize) -> Result<&[u8]> {
    let end = start.checked_add(len).ok_or(HayazipError::InvalidFormat(
        "ZIP entry exceeds archive bounds",
    ))?;
    data.get(start..end).ok_or(HayazipError::InvalidFormat(
        "ZIP entry exceeds archive bounds",
    ))
}
