use crate::archive::ZipArchive;
use crate::error::{Result, HayazipError};
use libdeflater::Decompressor;
use rayon::prelude::*;
use std::path::Path;
use std::io::Write;
use crc32fast::Hasher;

impl ZipArchive {
    pub fn extract_all<P: AsRef<Path>>(&self, dest: P) -> Result<()> {
        let dest = dest.as_ref();

        // Use try_for_each_init to initialize a Decompressor per worker thread
        self.entries().par_iter().try_for_each_init(
            || Decompressor::new(),
            |decompressor, entry| -> Result<()> {
                let out_path = dest.join(&entry.filename);
                
                if entry.is_dir {
                    std::fs::create_dir_all(&out_path)?;
                    return Ok(());
                }
                
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                
                #[cfg(unix)]
                if entry.is_unix_symlink() {
                    let data = entry.data(self.get_mmap())?;
                    let target = std::str::from_utf8(data)
                        .map_err(|_| HayazipError::InvalidFormat("Invalid symlink target UTF-8"))?;
                    std::os::unix::fs::symlink(target, &out_path)?;
                    return Ok(());
                }

                let mut out_data = vec![0u8; entry.uncompressed_size];
                
                if entry.method == 0 {
                    // Stored (no compression)
                    let data = entry.data(self.get_mmap())?;
                    if data.len() != entry.uncompressed_size {
                        return Err(HayazipError::Decompression("Stored data length mismatch".to_string()));
                    }
                    out_data.copy_from_slice(data);
                } else if entry.method == 8 {
                    // Deflate
                    let data = entry.data(self.get_mmap())?;
                    // ZIP uses raw DEFLATE
                    decompressor.deflate_decompress(data, &mut out_data)
                        .map_err(|e| HayazipError::Decompression(format!("{:?}", e)))?;
                } else {
                    return Err(HayazipError::UnsupportedCompression(entry.method));
                }
                
                // CRC32 check
                let mut hasher = Hasher::new();
                hasher.update(&out_data);
                let actual_crc = hasher.finalize();
                
                if actual_crc != entry.crc32 {
                    return Err(HayazipError::CrcMismatch {
                        expected: entry.crc32,
                        actual: actual_crc,
                    });
                }
                
                let mut file = std::fs::File::create(&out_path)?;
                file.write_all(&out_data)?;
                
                #[cfg(unix)]
                if let Some(mode) = entry.unix_mode() {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(mode);
                    std::fs::set_permissions(&out_path, perms)?;
                }
                
                Ok(())
            }
        )
    }
}

pub fn extract<P: AsRef<Path>, Q: AsRef<Path>>(archive_path: P, dest_path: Q) -> Result<()> {
    let archive = ZipArchive::open(archive_path)?;
    archive.extract_all(dest_path)
}
