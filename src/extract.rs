use crate::archive::ZipArchive;
use crate::entry::ZipEntry;
use crate::error::{HayazipError, Result};
use crc32fast::Hasher;
use libdeflater::Decompressor;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

const STORED_METHOD: u16 = 0;
const DEFLATE_METHOD: u16 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightEntry {
    pub archive_name: String,
    pub normalized_name: String,
    pub output_path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub compressed_size: usize,
    pub uncompressed_size: usize,
    pub compression_method: u16,
    pub crc32: u32,
    pub external_attr: u32,
}

#[derive(Debug, Clone)]
struct ExtractionPlanEntry {
    entry: ZipEntry,
    preflight: PreflightEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OccupiedKind {
    Directory,
    FileLike,
}

impl ZipArchive {
    pub fn preflight(&self) -> Result<Vec<PreflightEntry>> {
        Ok(build_extraction_plan(self.entries(), self.bytes())?
            .into_iter()
            .map(|plan| plan.preflight)
            .collect())
    }

    pub fn extract_all<P: AsRef<Path>>(&self, dest: P) -> Result<()> {
        extract_entries(self.entries(), self.bytes(), dest)
    }
}

pub fn preflight<P: AsRef<Path>>(archive_path: P) -> Result<Vec<PreflightEntry>> {
    let archive = ZipArchive::open(archive_path)?;
    archive.preflight()
}

pub fn preflight_bytes(archive_bytes: &[u8]) -> Result<Vec<PreflightEntry>> {
    let entries = ZipArchive::parse_entries(archive_bytes)?;
    Ok(build_extraction_plan(&entries, archive_bytes)?
        .into_iter()
        .map(|plan| plan.preflight)
        .collect())
}

pub fn extract<P: AsRef<Path>, Q: AsRef<Path>>(archive_path: P, dest_path: Q) -> Result<()> {
    let archive = ZipArchive::open(archive_path)?;
    archive.extract_all(dest_path)
}

pub fn extract_from_bytes<Q: AsRef<Path>>(archive_bytes: &[u8], dest_path: Q) -> Result<()> {
    let entries = ZipArchive::parse_entries(archive_bytes)?;
    extract_entries(&entries, archive_bytes, dest_path)
}

fn extract_entries<Q: AsRef<Path>>(
    entries: &[ZipEntry],
    archive_data: &[u8],
    dest_path: Q,
) -> Result<()> {
    let plan = build_extraction_plan(entries, archive_data)?;
    let dest = dest_path.as_ref();

    plan.par_iter().try_for_each_init(
        Decompressor::new,
        |decompressor, plan_entry| -> Result<()> {
            let out_path = dest.join(&plan_entry.preflight.output_path);

            if plan_entry.preflight.is_dir {
                std::fs::create_dir_all(&out_path)?;
                return Ok(());
            }

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            #[cfg(unix)]
            if plan_entry.preflight.is_symlink {
                let out_data = decode_entry_data(&plan_entry.entry, archive_data, decompressor)?;
                validate_crc32(plan_entry.entry.crc32, &out_data)?;
                let target = std::str::from_utf8(&out_data)
                    .map_err(|_| HayazipError::InvalidFormat("Invalid symlink target UTF-8"))?;
                std::os::unix::fs::symlink(target, &out_path)?;
                return Ok(());
            }

            let out_data = decode_entry_data(&plan_entry.entry, archive_data, decompressor)?;
            validate_crc32(plan_entry.entry.crc32, &out_data)?;

            let mut file = std::fs::File::create(&out_path)?;
            file.write_all(&out_data)?;

            #[cfg(unix)]
            if let Some(mode) = plan_entry.entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(mode);
                std::fs::set_permissions(&out_path, perms)?;
            }

            Ok(())
        },
    )
}

fn build_extraction_plan(
    entries: &[ZipEntry],
    archive_data: &[u8],
) -> Result<Vec<ExtractionPlanEntry>> {
    let mut plan = Vec::with_capacity(entries.len());
    let mut occupied = HashMap::with_capacity(entries.len());
    let mut file_like_paths = HashSet::with_capacity(entries.len());
    let mut path_prefixes = HashSet::with_capacity(entries.len());

    for entry in entries {
        validate_supported_entry(entry)?;

        let (output_path, normalized_name) = normalize_entry_path(&entry.filename)?;
        let is_symlink = entry.is_unix_symlink();

        ensure_no_conflicts(
            &output_path,
            entry_kind(entry),
            &occupied,
            &file_like_paths,
            &path_prefixes,
            &entry.filename,
        )?;

        let _ = entry.data(archive_data)?;

        #[cfg(unix)]
        if is_symlink && !entry.is_dir {
            let mut decompressor = Decompressor::new();
            let target_bytes = decode_entry_data(entry, archive_data, &mut decompressor)?;
            validate_crc32(entry.crc32, &target_bytes)?;
            std::str::from_utf8(&target_bytes)
                .map_err(|_| HayazipError::InvalidFormat("Invalid symlink target UTF-8"))?;
        }

        occupied.insert(output_path.clone(), entry_kind(entry));
        if !entry.is_dir {
            file_like_paths.insert(output_path.clone());
        }
        for ancestor in output_path.ancestors().skip(1) {
            if ancestor.as_os_str().is_empty() {
                break;
            }
            path_prefixes.insert(ancestor.to_path_buf());
        }

        plan.push(ExtractionPlanEntry {
            entry: entry.clone(),
            preflight: PreflightEntry {
                archive_name: entry.filename.clone(),
                normalized_name,
                output_path,
                is_dir: entry.is_dir,
                is_symlink,
                compressed_size: entry.compressed_size,
                uncompressed_size: entry.uncompressed_size,
                compression_method: entry.method,
                crc32: entry.crc32,
                external_attr: entry.external_attr,
            },
        });
    }

    Ok(plan)
}

fn validate_supported_entry(entry: &ZipEntry) -> Result<()> {
    if entry.is_dir {
        return Ok(());
    }

    if entry.method == STORED_METHOD || entry.method == DEFLATE_METHOD {
        Ok(())
    } else {
        Err(HayazipError::UnsupportedCompression(entry.method))
    }
}

fn normalize_entry_path(raw_name: &str) -> Result<(PathBuf, String)> {
    if raw_name.is_empty() {
        return Err(HayazipError::UnsafePath(
            "Archive entry name cannot be empty".to_string(),
        ));
    }

    if raw_name.contains('\0') {
        return Err(HayazipError::UnsafePath(format!(
            "{raw_name:?} contains a NUL byte"
        )));
    }

    let normalized_separators = raw_name.replace('\\', "/");
    if normalized_separators.starts_with('/') {
        return Err(HayazipError::UnsafePath(format!(
            "{raw_name:?} resolves to an absolute path"
        )));
    }

    let mut parts = Vec::new();
    for component in normalized_separators.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }

        if parts.is_empty() && looks_like_drive_letter(component) {
            return Err(HayazipError::UnsafePath(format!(
                "{raw_name:?} starts with a Windows drive prefix"
            )));
        }

        if component == ".." {
            return Err(HayazipError::UnsafePath(format!(
                "{raw_name:?} contains a parent-directory segment"
            )));
        }

        parts.push(component.to_string());
    }

    if parts.is_empty() {
        return Err(HayazipError::UnsafePath(format!(
            "{raw_name:?} does not contain a writable relative path"
        )));
    }

    let mut path = PathBuf::new();
    for part in &parts {
        path.push(part);
    }

    Ok((path, parts.join("/")))
}

fn ensure_no_conflicts(
    output_path: &Path,
    kind: OccupiedKind,
    occupied: &HashMap<PathBuf, OccupiedKind>,
    file_like_paths: &HashSet<PathBuf>,
    path_prefixes: &HashSet<PathBuf>,
    archive_name: &str,
) -> Result<()> {
    if let Some(existing_kind) = occupied.get(output_path) {
        return Err(HayazipError::EntryConflict(format!(
            "{archive_name:?} maps to {:?}, which is already reserved as {}",
            output_path,
            kind_name(*existing_kind),
        )));
    }

    for ancestor in output_path.ancestors().skip(1) {
        if ancestor.as_os_str().is_empty() {
            break;
        }

        if file_like_paths.contains(ancestor) {
            return Err(HayazipError::EntryConflict(format!(
                "{archive_name:?} would place data under {:?}, which is already a file or symlink",
                ancestor,
            )));
        }
    }

    if kind == OccupiedKind::FileLike && path_prefixes.contains(output_path) {
        return Err(HayazipError::EntryConflict(format!(
            "{archive_name:?} maps to {:?}, which is already needed as a parent directory",
            output_path,
        )));
    }

    Ok(())
}

fn entry_kind(entry: &ZipEntry) -> OccupiedKind {
    if entry.is_dir {
        OccupiedKind::Directory
    } else {
        OccupiedKind::FileLike
    }
}

fn kind_name(kind: OccupiedKind) -> &'static str {
    match kind {
        OccupiedKind::Directory => "directory",
        OccupiedKind::FileLike => "file",
    }
}

fn decode_entry_data(
    entry: &ZipEntry,
    archive_data: &[u8],
    decompressor: &mut Decompressor,
) -> Result<Vec<u8>> {
    if entry.is_dir {
        return Ok(Vec::new());
    }

    let data = entry.data(archive_data)?;

    match entry.method {
        STORED_METHOD => {
            if data.len() != entry.uncompressed_size {
                return Err(HayazipError::Decompression(
                    "Stored data length mismatch".to_string(),
                ));
            }
            Ok(data.to_vec())
        }
        DEFLATE_METHOD => {
            let mut out_data = vec![0u8; entry.uncompressed_size];
            decompressor
                .deflate_decompress(data, &mut out_data)
                .map_err(|e| HayazipError::Decompression(format!("{e:?}")))?;
            Ok(out_data)
        }
        _ => Err(HayazipError::UnsupportedCompression(entry.method)),
    }
}

fn validate_crc32(expected: u32, data: &[u8]) -> Result<()> {
    let mut hasher = Hasher::new();
    hasher.update(data);
    let actual = hasher.finalize();

    if actual == expected {
        Ok(())
    } else {
        Err(HayazipError::CrcMismatch { expected, actual })
    }
}

fn looks_like_drive_letter(component: &str) -> bool {
    let bytes = component.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}
