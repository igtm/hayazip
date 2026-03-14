use std::fs::File;
use std::path::Path;
use memmap2::Mmap;
use crate::error::{Result, HayazipError};
use crate::entry::ZipEntry;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

pub struct ZipArchive {
    mmap: Mmap,
    entries: Vec<ZipEntry>,
}

impl ZipArchive {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        
        let (cd_offset, num_entries) = Self::find_eocd(&mmap)?;
        let entries = Self::parse_central_directory(&mmap, cd_offset, num_entries)?;
        
        Ok(ZipArchive { mmap, entries })
    }

    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    pub fn get_mmap(&self) -> &Mmap {
        &self.mmap
    }

    fn find_eocd(mmap: &[u8]) -> Result<(usize, usize)> {
        let len = mmap.len();
        if len < 22 {
            return Err(HayazipError::InvalidFormat("File too short to be a ZIP file"));
        }

        let max_comment_len = 65535;
        let search_start = len.saturating_sub(22 + max_comment_len);
        
        for i in (search_start..=(len - 22)).rev() {
            if mmap[i..i+4] == [0x50, 0x4B, 0x05, 0x06] {
                // Found EOCD
                let mut cursor = Cursor::new(&mmap[i+8..i+20]);
                let _num_entries_this_disk = cursor.read_u16::<LittleEndian>()?;
                let num_entries = cursor.read_u16::<LittleEndian>()? as usize;
                let _cd_size = cursor.read_u32::<LittleEndian>()?;
                let cd_offset = cursor.read_u32::<LittleEndian>()? as usize;
                
                return Ok((cd_offset, num_entries));
            }
        }
        
        Err(HayazipError::InvalidFormat("EOCD signature not found"))
    }

    fn parse_central_directory(mmap: &[u8], offset: usize, num_entries: usize) -> Result<Vec<ZipEntry>> {
        eprintln!("parse_central_directory: offset={}, num_entries={}, mmap.len()={}", offset, num_entries, mmap.len());
        let mut entries = Vec::with_capacity(num_entries);
        let mut cursor = Cursor::new(&mmap[offset..]);
        
        for i in 0..num_entries {
            eprintln!("Reading entry {}, cursor pos: {}", i, cursor.position());
            let signature = cursor.read_u32::<LittleEndian>()?;
            if signature != 0x02014B50 {
                return Err(HayazipError::InvalidFormat("Invalid Central Directory signature"));
            }
            
            let _version_made_by = cursor.read_u16::<LittleEndian>()?;
            let _version_needed = cursor.read_u16::<LittleEndian>()?;
            let flags = cursor.read_u16::<LittleEndian>()?;
            let method = cursor.read_u16::<LittleEndian>()?;
            let last_mod_time = cursor.read_u16::<LittleEndian>()?;
            let last_mod_date = cursor.read_u16::<LittleEndian>()?;
            let crc32 = cursor.read_u32::<LittleEndian>()?;
            let compressed_size = cursor.read_u32::<LittleEndian>()?;
            let uncompressed_size = cursor.read_u32::<LittleEndian>()?;
            let filename_len = cursor.read_u16::<LittleEndian>()? as usize;
            let extra_field_len = cursor.read_u16::<LittleEndian>()? as usize;
            let comment_len = cursor.read_u16::<LittleEndian>()? as usize;
            let _disk_num_start = cursor.read_u16::<LittleEndian>()?;
            let _internal_attr = cursor.read_u16::<LittleEndian>()?;
            let external_attr = cursor.read_u32::<LittleEndian>()?;
            let local_header_offset = cursor.read_u32::<LittleEndian>()? as usize;
            
            // Read filename
            let pos = cursor.position() as usize;
            let filename_bytes = &mmap[offset + pos .. offset + pos + filename_len];
            let filename = String::from_utf8_lossy(filename_bytes).into_owned();
            
            cursor.set_position((pos + filename_len + extra_field_len + comment_len) as u64);
            
            let is_dir = filename.ends_with('/');
            
            entries.push(ZipEntry {
                filename,
                local_header_offset,
                compressed_size: compressed_size as usize,
                uncompressed_size: uncompressed_size as usize,
                crc32,
                method,
                flags,
                is_dir,
                external_attr,
            });
        }
        
        Ok(entries)
    }
}
