use hayazip::HayazipError;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn create_random_files(dir: &Path) {
    // Create some files
    fs::write(dir.join("test1.txt"), b"Hello, World!").unwrap();
    fs::write(dir.join("test2.bin"), vec![0u8; 10000]).unwrap();

    // Create nested directory
    let nested = dir.join("a/b/c");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("deep.txt"), b"Deep file").unwrap();
    fs::create_dir_all(dir.join("empty_dir/subdir")).unwrap();

    // Create a large file
    let mut large_data = Vec::with_capacity(1024 * 1024);
    for i in 0..1024 * 1024 {
        large_data.push((i % 256) as u8);
    }
    fs::write(dir.join("large.bin"), large_data).unwrap();

    #[cfg(unix)]
    {
        // Try creating a symlink
        std::os::unix::fs::symlink("test1.txt", dir.join("symlink.txt")).unwrap();

        let path = dir.join("executable.sh");
        fs::write(&path, b"#!/bin/sh\necho hi\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
}

fn compare_dirs(dir1: &Path, dir2: &Path) {
    let entries1: Vec<_> = walkdir::WalkDir::new(dir1)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    for entry in entries1 {
        let rel_path = entry.path().strip_prefix(dir1).unwrap();
        if rel_path.as_os_str().is_empty() {
            continue;
        }
        let target_path = dir2.join(rel_path);

        assert!(
            target_path.exists(),
            "Path {:?} missing in extracted dir",
            rel_path
        );

        let meta1 = fs::symlink_metadata(entry.path()).unwrap();
        let meta2 = fs::symlink_metadata(&target_path).unwrap();

        assert_eq!(meta1.is_dir(), meta2.is_dir());
        assert_eq!(meta1.is_symlink(), meta2.is_symlink());

        if meta1.is_file() {
            let data1 = fs::read(entry.path()).unwrap();
            let data2 = fs::read(&target_path).unwrap();
            assert_eq!(data1, data2, "File content mismatch for {:?}", rel_path);

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    meta1.permissions().mode() & 0o777,
                    meta2.permissions().mode() & 0o777,
                    "Permission mismatch for {:?}",
                    rel_path
                );
            }
        } else if meta1.is_symlink() {
            let target1 = fs::read_link(entry.path()).unwrap();
            let target2 = fs::read_link(&target_path).unwrap();
            assert_eq!(
                target1, target2,
                "Symlink target mismatch for {:?}",
                rel_path
            );
        }
    }
}

struct ManualZipEntry<'a> {
    name: &'a str,
    data: &'a [u8],
    external_attr: u32,
}

fn build_stored_zip(entries: &[ManualZipEntry<'_>]) -> Vec<u8> {
    const LOCAL_FILE_HEADER_SIGNATURE: u32 = 0x0403_4b50;
    const CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0201_4b50;
    const END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0605_4b50;
    const VERSION_NEEDED: u16 = 20;
    const UTF8_FLAG: u16 = 1 << 11;

    let mut archive = Vec::new();
    let mut central_directory = Vec::new();

    for entry in entries {
        let name = entry.name.as_bytes();
        let crc32 = crc32fast::hash(entry.data);
        let local_header_offset = archive.len() as u32;

        archive.extend_from_slice(&LOCAL_FILE_HEADER_SIGNATURE.to_le_bytes());
        archive.extend_from_slice(&VERSION_NEEDED.to_le_bytes());
        archive.extend_from_slice(&UTF8_FLAG.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(&crc32.to_le_bytes());
        archive.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
        archive.extend_from_slice(&0u16.to_le_bytes());
        archive.extend_from_slice(name);
        archive.extend_from_slice(entry.data);

        central_directory.extend_from_slice(&CENTRAL_DIRECTORY_SIGNATURE.to_le_bytes());
        central_directory.extend_from_slice(&((3 << 8) | VERSION_NEEDED).to_le_bytes());
        central_directory.extend_from_slice(&VERSION_NEEDED.to_le_bytes());
        central_directory.extend_from_slice(&UTF8_FLAG.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&crc32.to_le_bytes());
        central_directory.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        central_directory.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        central_directory.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&entry.external_attr.to_le_bytes());
        central_directory.extend_from_slice(&local_header_offset.to_le_bytes());
        central_directory.extend_from_slice(name);
    }

    let central_directory_offset = archive.len() as u32;
    archive.extend_from_slice(&central_directory);
    let central_directory_size = archive.len() as u32 - central_directory_offset;

    archive.extend_from_slice(&END_OF_CENTRAL_DIRECTORY_SIGNATURE.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&central_directory_size.to_le_bytes());
    archive.extend_from_slice(&central_directory_offset.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());

    archive
}

#[test]
fn test_unzip_compatibility() {
    let work_dir = TempDir::new().unwrap();
    let src_dir = work_dir.path().join("src");
    let extract_dir = work_dir.path().join("extracted");
    let zip_file = work_dir.path().join("archive.zip");

    fs::create_dir(&src_dir).unwrap();
    create_random_files(&src_dir);

    // Command `zip -r -y archive.zip .`
    let status = Command::new("zip")
        .arg("-r")
        .arg("-y") // store symlinks
        .arg(&zip_file)
        .arg(".")
        .current_dir(&src_dir)
        .status()
        .expect("Failed to run zip command");

    assert!(status.success(), "ZIP command failed");

    // Using Hayazip to extract
    fs::create_dir(&extract_dir).unwrap();
    hayazip::extract(&zip_file, &extract_dir).expect("Hayazip extraction failed");

    // Compare
    compare_dirs(&src_dir, &extract_dir);
}

#[test]
fn test_zip_roundtrip_with_hayazip() {
    let work_dir = TempDir::new().unwrap();
    let src_dir = work_dir.path().join("src");
    let extract_dir = work_dir.path().join("extracted");
    let zip_file = work_dir.path().join("archive.zip");

    fs::create_dir(&src_dir).unwrap();
    create_random_files(&src_dir);

    hayazip::create_zip(&src_dir, &zip_file).expect("Hayazip archive creation failed");

    fs::create_dir(&extract_dir).unwrap();
    hayazip::extract(&zip_file, &extract_dir).expect("Hayazip extraction failed");

    compare_dirs(&src_dir, &extract_dir);
}

#[test]
fn test_zip_is_compatible_with_unzip() {
    let work_dir = TempDir::new().unwrap();
    let src_dir = work_dir.path().join("src");
    let extract_dir = work_dir.path().join("unzip-extracted");
    let zip_file = work_dir.path().join("archive.zip");

    fs::create_dir(&src_dir).unwrap();
    create_random_files(&src_dir);
    fs::create_dir(&extract_dir).unwrap();

    hayazip::create_zip(&src_dir, &zip_file).expect("Hayazip archive creation failed");

    let status = Command::new("unzip")
        .arg("-q")
        .arg(&zip_file)
        .arg("-d")
        .arg(&extract_dir)
        .status()
        .expect("Failed to run unzip command");

    assert!(status.success(), "unzip command failed");
    compare_dirs(&src_dir, &extract_dir);
}

#[test]
fn test_extract_from_bytes_roundtrip() {
    let work_dir = TempDir::new().unwrap();
    let src_dir = work_dir.path().join("src");
    let extract_dir = work_dir.path().join("from-bytes");
    let zip_file = work_dir.path().join("archive.zip");

    fs::create_dir(&src_dir).unwrap();
    create_random_files(&src_dir);
    fs::create_dir(&extract_dir).unwrap();

    hayazip::create_zip(&src_dir, &zip_file).expect("Hayazip archive creation failed");
    let archive_bytes = fs::read(&zip_file).unwrap();

    hayazip::extract_from_bytes(&archive_bytes, &extract_dir)
        .expect("Hayazip bytes extraction failed");

    compare_dirs(&src_dir, &extract_dir);
}

#[test]
fn test_preflight_normalizes_separator_variants() {
    let archive_bytes = build_stored_zip(&[ManualZipEntry {
        name: ".\\nested//file.txt",
        data: b"payload",
        external_attr: 0,
    }]);

    let entries = hayazip::preflight_bytes(&archive_bytes).expect("Preflight should succeed");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].archive_name, ".\\nested//file.txt");
    assert_eq!(entries[0].normalized_name, "nested/file.txt");
    assert_eq!(entries[0].output_path, Path::new("nested").join("file.txt"));
}

#[test]
fn test_preflight_rejects_drive_letter_paths() {
    let archive_bytes = build_stored_zip(&[ManualZipEntry {
        name: "C:\\escape.txt",
        data: b"payload",
        external_attr: 0,
    }]);

    let err = hayazip::preflight_bytes(&archive_bytes).expect_err("Drive-prefixed path must fail");
    assert!(matches!(err, HayazipError::UnsafePath(_)));
}

#[test]
fn test_extract_preflight_rejects_before_writing() {
    let work_dir = TempDir::new().unwrap();
    let extract_dir = work_dir.path().join("extracted");
    fs::create_dir(&extract_dir).unwrap();

    let archive_bytes = build_stored_zip(&[
        ManualZipEntry {
            name: "safe.txt",
            data: b"safe",
            external_attr: 0,
        },
        ManualZipEntry {
            name: "../escape.txt",
            data: b"unsafe",
            external_attr: 0,
        },
    ]);

    let err = hayazip::extract_from_bytes(&archive_bytes, &extract_dir)
        .expect_err("Traversal path must fail during preflight");
    assert!(matches!(err, HayazipError::UnsafePath(_)));
    assert!(
        fs::read_dir(&extract_dir).unwrap().next().is_none(),
        "Preflight should fail before any files are written",
    );
}
