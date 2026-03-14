use byteorder::{LittleEndian, WriteBytesExt};
use crc32fast::Hasher;
use libdeflater::{CompressionLvl, Compressor};
use memmap2::Mmap;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{HayazipError, Result};

const LOCAL_FILE_HEADER_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0201_4b50;
const END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0605_4b50;
const VERSION_NEEDED: u16 = 20;
const VERSION_MADE_BY_UNIX: u16 = (3 << 8) | VERSION_NEEDED;
const UTF8_FLAG: u16 = 1 << 11;
const STORED_METHOD: u16 = 0;
const DEFLATE_METHOD: u16 = 8;
const DOS_DATE_MIN: u16 = 33;
const DOS_TIME_MIN: u16 = 0;
const MIN_DEFLATE_SIZE: usize = 64;
const MAX_COMPRESSION_THREADS: usize = 8;

#[derive(Clone, Debug)]
enum SourceKind {
    Directory,
    File,
    Symlink,
}

#[derive(Clone, Debug)]
struct SourceEntry {
    source_path: PathBuf,
    archive_name: String,
    kind: SourceKind,
    external_attr: u32,
}

#[derive(Debug)]
enum Payload {
    None,
    Inline(Vec<u8>),
    SourceFile(PathBuf),
    ScratchFile(PathBuf),
}

#[derive(Debug)]
struct PreparedEntry {
    archive_name: String,
    method: u16,
    flags: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    external_attr: u32,
    payload: Payload,
}

#[derive(Debug)]
struct CentralDirectoryRecord {
    archive_name: String,
    method: u16,
    flags: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    external_attr: u32,
    local_header_offset: u32,
}

struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    fn new() -> Result<Self> {
        let pid = std::process::id();

        for attempt in 0..1024 {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| HayazipError::InvalidInput(format!("System clock error: {e}")))?
                .as_nanos();
            let path = std::env::temp_dir().join(format!("hayazip-{pid}-{stamp}-{attempt}"));

            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }

        Err(HayazipError::InvalidInput(
            "Failed to allocate a temporary scratch directory".to_string(),
        ))
    }

    fn entry_path(&self, index: usize) -> PathBuf {
        self.path.join(format!("{index:08}.deflate"))
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn create_zip<P: AsRef<Path>, Q: AsRef<Path>>(source_path: P, archive_path: Q) -> Result<()> {
    let source_path = source_path.as_ref();
    let archive_path = archive_path.as_ref();

    let source_abs = absolute_path(source_path)?;
    let archive_abs = absolute_path(archive_path)?;

    if source_abs == archive_abs {
        return Err(HayazipError::InvalidInput(
            "Source path and archive path must be different".to_string(),
        ));
    }

    let entries = collect_source_entries(&source_abs, &archive_abs)?;
    let scratch = ScratchDir::new()?;
    let prepared = prepare_entries(&entries, &scratch)?;
    write_archive(&prepared, archive_path)
}

fn collect_source_entries(source_path: &Path, archive_path: &Path) -> Result<Vec<SourceEntry>> {
    let metadata = fs::symlink_metadata(source_path)?;
    let mut entries = Vec::new();

    if metadata.file_type().is_dir() {
        collect_directory_entries(source_path, source_path, archive_path, &mut entries)?;
    } else {
        let archive_name = source_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| {
                HayazipError::InvalidInput(format!(
                    "Could not derive an archive name from {}",
                    source_path.display()
                ))
            })?;
        entries.push(build_source_entry(
            source_path.to_path_buf(),
            &metadata,
            archive_name,
        )?);
    }

    Ok(entries)
}

fn collect_directory_entries(
    root: &Path,
    directory: &Path,
    archive_path: &Path,
    entries: &mut Vec<SourceEntry>,
) -> Result<()> {
    let mut children = Vec::new();
    for child in fs::read_dir(directory)? {
        children.push(child?.path());
    }
    children.sort_by(|left, right| left.file_name().cmp(&right.file_name()));

    for child in children {
        if child == archive_path {
            continue;
        }

        let metadata = fs::symlink_metadata(&child)?;
        let archive_name = archive_name_from_relative(
            child.strip_prefix(root).map_err(|_| {
                HayazipError::InvalidInput("Failed to strip source prefix".to_string())
            })?,
            metadata.file_type().is_dir(),
        )?;
        entries.push(build_source_entry(child.clone(), &metadata, archive_name)?);

        if metadata.file_type().is_dir() {
            collect_directory_entries(root, &child, archive_path, entries)?;
        }
    }

    Ok(())
}

fn archive_name_from_relative(relative: &Path, is_dir: bool) -> Result<String> {
    let mut archive_name = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    if archive_name.is_empty() {
        return Err(HayazipError::InvalidInput(
            "Archive entry name cannot be empty".to_string(),
        ));
    }

    if is_dir && !archive_name.ends_with('/') {
        archive_name.push('/');
    }

    Ok(archive_name)
}

fn build_source_entry(
    path: PathBuf,
    metadata: &fs::Metadata,
    archive_name: String,
) -> Result<SourceEntry> {
    let file_type = metadata.file_type();
    let (kind, external_attr) = if file_type.is_dir() {
        (SourceKind::Directory, external_attr_for_directory(metadata))
    } else if file_type.is_file() {
        (SourceKind::File, external_attr_for_file(metadata))
    } else if file_type.is_symlink() {
        (SourceKind::Symlink, external_attr_for_symlink(metadata))
    } else {
        return Err(HayazipError::InvalidInput(format!(
            "Unsupported filesystem entry: {}",
            path.display()
        )));
    };

    Ok(SourceEntry {
        source_path: path,
        archive_name,
        kind,
        external_attr,
    })
}

fn prepare_entries(entries: &[SourceEntry], scratch: &ScratchDir) -> Result<Vec<PreparedEntry>> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let threads = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1)
        .min(MAX_COMPRESSION_THREADS)
        .max(1)
        .min(entries.len());

    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| HayazipError::Compression(e.to_string()))?;

    pool.install(|| {
        entries
            .par_iter()
            .enumerate()
            .map(|(index, entry)| prepare_entry(index, entry, scratch))
            .collect()
    })
}

fn prepare_entry(index: usize, entry: &SourceEntry, scratch: &ScratchDir) -> Result<PreparedEntry> {
    match entry.kind {
        SourceKind::Directory => Ok(PreparedEntry {
            archive_name: entry.archive_name.clone(),
            method: STORED_METHOD,
            flags: UTF8_FLAG,
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            external_attr: entry.external_attr,
            payload: Payload::None,
        }),
        SourceKind::Symlink => prepare_symlink_entry(entry),
        SourceKind::File => prepare_file_entry(index, entry, scratch),
    }
}

fn prepare_symlink_entry(entry: &SourceEntry) -> Result<PreparedEntry> {
    let target = fs::read_link(&entry.source_path)?;
    #[cfg(unix)]
    let raw = target.as_os_str().as_bytes().to_vec();
    #[cfg(not(unix))]
    let raw = target.to_string_lossy().into_owned().into_bytes();

    let crc32 = checksum(&raw);
    let size = to_u32(raw.len(), "Symlink target exceeds ZIP limits")?;

    Ok(PreparedEntry {
        archive_name: entry.archive_name.clone(),
        method: STORED_METHOD,
        flags: UTF8_FLAG,
        crc32,
        compressed_size: size,
        uncompressed_size: size,
        external_attr: entry.external_attr,
        payload: Payload::Inline(raw),
    })
}

fn prepare_file_entry(
    index: usize,
    entry: &SourceEntry,
    scratch: &ScratchDir,
) -> Result<PreparedEntry> {
    let metadata = fs::metadata(&entry.source_path)?;
    let file_len = usize::try_from(metadata.len())
        .map_err(|_| HayazipError::ArchiveTooLarge("File is too large for this ZIP writer"))?;

    if file_len == 0 {
        return Ok(PreparedEntry {
            archive_name: entry.archive_name.clone(),
            method: STORED_METHOD,
            flags: UTF8_FLAG,
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            external_attr: entry.external_attr,
            payload: Payload::None,
        });
    }

    let file = File::open(&entry.source_path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let data = &mmap[..];
    let crc32 = checksum(data);
    let uncompressed_size = to_u32(data.len(), "File exceeds ZIP32 size limits")?;

    if data.len() < MIN_DEFLATE_SIZE {
        return Ok(PreparedEntry {
            archive_name: entry.archive_name.clone(),
            method: STORED_METHOD,
            flags: UTF8_FLAG,
            crc32,
            compressed_size: uncompressed_size,
            uncompressed_size,
            external_attr: entry.external_attr,
            payload: Payload::SourceFile(entry.source_path.clone()),
        });
    }

    let mut compressor = Compressor::new(CompressionLvl::default());
    let mut compressed = vec![0u8; compressor.deflate_compress_bound(data.len())];
    let compressed_size = compressor
        .deflate_compress(data, &mut compressed)
        .map_err(|e| HayazipError::Compression(e.to_string()))?;

    if compressed_size >= data.len() {
        return Ok(PreparedEntry {
            archive_name: entry.archive_name.clone(),
            method: STORED_METHOD,
            flags: UTF8_FLAG,
            crc32,
            compressed_size: uncompressed_size,
            uncompressed_size,
            external_attr: entry.external_attr,
            payload: Payload::SourceFile(entry.source_path.clone()),
        });
    }

    let scratch_path = scratch.entry_path(index);
    let mut scratch_file = File::create(&scratch_path)?;
    scratch_file.write_all(&compressed[..compressed_size])?;

    Ok(PreparedEntry {
        archive_name: entry.archive_name.clone(),
        method: DEFLATE_METHOD,
        flags: UTF8_FLAG,
        crc32,
        compressed_size: to_u32(compressed_size, "Compressed data exceeds ZIP32 size limits")?,
        uncompressed_size,
        external_attr: entry.external_attr,
        payload: Payload::ScratchFile(scratch_path),
    })
}

fn write_archive(entries: &[PreparedEntry], archive_path: &Path) -> Result<()> {
    if let Some(parent) = archive_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let file = File::create(archive_path)?;
    let mut writer = BufWriter::new(file);
    let mut records = Vec::with_capacity(entries.len());
    let mut offset = 0_u64;

    for entry in entries {
        let archive_name = entry.archive_name.as_bytes();
        let local_header_offset = to_u32(offset, "Archive exceeds ZIP32 offset limits")?;

        write_local_file_header(&mut writer, entry, archive_name)?;
        offset += local_file_header_len(archive_name.len())?;

        copy_payload(&mut writer, entry)?;
        offset += u64::from(entry.compressed_size);

        records.push(CentralDirectoryRecord {
            archive_name: entry.archive_name.clone(),
            method: entry.method,
            flags: entry.flags,
            crc32: entry.crc32,
            compressed_size: entry.compressed_size,
            uncompressed_size: entry.uncompressed_size,
            external_attr: entry.external_attr,
            local_header_offset,
        });
    }

    let central_directory_offset = to_u32(offset, "Archive exceeds ZIP32 offset limits")?;
    for record in &records {
        let archive_name = record.archive_name.as_bytes();
        write_central_directory_record(&mut writer, record, archive_name)?;
        offset += central_directory_len(archive_name.len())?;
    }

    let central_directory_size = to_u32(
        offset - u64::from(central_directory_offset),
        "Central directory exceeds ZIP32 size limits",
    )?;
    write_end_of_central_directory(
        &mut writer,
        to_u16(records.len(), "ZIP entry count exceeds ZIP32 limits")?,
        central_directory_size,
        central_directory_offset,
    )?;

    writer.flush()?;
    Ok(())
}

fn write_local_file_header(
    writer: &mut BufWriter<File>,
    entry: &PreparedEntry,
    archive_name: &[u8],
) -> Result<()> {
    writer.write_u32::<LittleEndian>(LOCAL_FILE_HEADER_SIGNATURE)?;
    writer.write_u16::<LittleEndian>(VERSION_NEEDED)?;
    writer.write_u16::<LittleEndian>(entry.flags)?;
    writer.write_u16::<LittleEndian>(entry.method)?;
    writer.write_u16::<LittleEndian>(DOS_TIME_MIN)?;
    writer.write_u16::<LittleEndian>(DOS_DATE_MIN)?;
    writer.write_u32::<LittleEndian>(entry.crc32)?;
    writer.write_u32::<LittleEndian>(entry.compressed_size)?;
    writer.write_u32::<LittleEndian>(entry.uncompressed_size)?;
    writer.write_u16::<LittleEndian>(to_u16(
        archive_name.len(),
        "Archive entry name exceeds ZIP limits",
    )?)?;
    writer.write_u16::<LittleEndian>(0)?;
    writer.write_all(archive_name)?;
    Ok(())
}

fn write_central_directory_record(
    writer: &mut BufWriter<File>,
    record: &CentralDirectoryRecord,
    archive_name: &[u8],
) -> Result<()> {
    writer.write_u32::<LittleEndian>(CENTRAL_DIRECTORY_SIGNATURE)?;
    writer.write_u16::<LittleEndian>(VERSION_MADE_BY_UNIX)?;
    writer.write_u16::<LittleEndian>(VERSION_NEEDED)?;
    writer.write_u16::<LittleEndian>(record.flags)?;
    writer.write_u16::<LittleEndian>(record.method)?;
    writer.write_u16::<LittleEndian>(DOS_TIME_MIN)?;
    writer.write_u16::<LittleEndian>(DOS_DATE_MIN)?;
    writer.write_u32::<LittleEndian>(record.crc32)?;
    writer.write_u32::<LittleEndian>(record.compressed_size)?;
    writer.write_u32::<LittleEndian>(record.uncompressed_size)?;
    writer.write_u16::<LittleEndian>(to_u16(
        archive_name.len(),
        "Archive entry name exceeds ZIP limits",
    )?)?;
    writer.write_u16::<LittleEndian>(0)?;
    writer.write_u16::<LittleEndian>(0)?;
    writer.write_u16::<LittleEndian>(0)?;
    writer.write_u16::<LittleEndian>(0)?;
    writer.write_u32::<LittleEndian>(record.external_attr)?;
    writer.write_u32::<LittleEndian>(record.local_header_offset)?;
    writer.write_all(archive_name)?;
    Ok(())
}

fn write_end_of_central_directory(
    writer: &mut BufWriter<File>,
    entries: u16,
    central_directory_size: u32,
    central_directory_offset: u32,
) -> Result<()> {
    writer.write_u32::<LittleEndian>(END_OF_CENTRAL_DIRECTORY_SIGNATURE)?;
    writer.write_u16::<LittleEndian>(0)?;
    writer.write_u16::<LittleEndian>(0)?;
    writer.write_u16::<LittleEndian>(entries)?;
    writer.write_u16::<LittleEndian>(entries)?;
    writer.write_u32::<LittleEndian>(central_directory_size)?;
    writer.write_u32::<LittleEndian>(central_directory_offset)?;
    writer.write_u16::<LittleEndian>(0)?;
    Ok(())
}

fn copy_payload(writer: &mut BufWriter<File>, entry: &PreparedEntry) -> Result<()> {
    match &entry.payload {
        Payload::None => Ok(()),
        Payload::Inline(bytes) => {
            writer.write_all(bytes)?;
            Ok(())
        }
        Payload::SourceFile(path) => copy_file(path, writer, u64::from(entry.uncompressed_size)),
        Payload::ScratchFile(path) => copy_file(path, writer, u64::from(entry.compressed_size)),
    }
}

fn copy_file(path: &Path, writer: &mut BufWriter<File>, expected_len: u64) -> Result<()> {
    let mut file = File::open(path)?;
    let copied = io::copy(&mut file, writer)?;
    if copied != expected_len {
        return Err(HayazipError::InvalidInput(format!(
            "File changed while building the archive: {}",
            path.display()
        )));
    }
    Ok(())
}

fn checksum(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn local_file_header_len(name_len: usize) -> Result<u64> {
    Ok(30 + u64::from(to_u16(name_len, "Archive entry name exceeds ZIP limits")?))
}

fn central_directory_len(name_len: usize) -> Result<u64> {
    Ok(46 + u64::from(to_u16(name_len, "Archive entry name exceeds ZIP limits")?))
}

fn to_u16<T>(value: T, message: &'static str) -> Result<u16>
where
    u16: TryFrom<T>,
{
    u16::try_from(value).map_err(|_| HayazipError::ArchiveTooLarge(message))
}

fn to_u32<T>(value: T, message: &'static str) -> Result<u32>
where
    u32: TryFrom<T>,
{
    u32::try_from(value).map_err(|_| HayazipError::ArchiveTooLarge(message))
}

#[cfg(unix)]
fn external_attr_for_file(metadata: &fs::Metadata) -> u32 {
    let mode = normalize_unix_mode(metadata.permissions().mode(), 0o100644);
    mode << 16
}

#[cfg(not(unix))]
fn external_attr_for_file(_metadata: &fs::Metadata) -> u32 {
    0o100644 << 16
}

#[cfg(unix)]
fn external_attr_for_directory(metadata: &fs::Metadata) -> u32 {
    let mode = normalize_unix_mode(metadata.permissions().mode(), 0o040755);
    (mode << 16) | 0x10
}

#[cfg(not(unix))]
fn external_attr_for_directory(_metadata: &fs::Metadata) -> u32 {
    (0o040755 << 16) | 0x10
}

#[cfg(unix)]
fn external_attr_for_symlink(metadata: &fs::Metadata) -> u32 {
    let mode = normalize_unix_mode(metadata.permissions().mode(), 0o120777);
    mode << 16
}

#[cfg(not(unix))]
fn external_attr_for_symlink(_metadata: &fs::Metadata) -> u32 {
    0o120777 << 16
}

#[cfg(unix)]
fn normalize_unix_mode(mode: u32, fallback: u32) -> u32 {
    if (mode & 0o170000) == 0 {
        mode | fallback
    } else {
        mode
    }
}
