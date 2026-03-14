use std::process::Command;
use std::path::{Path, PathBuf};
use std::fs;
use tempfile::TempDir;

fn create_random_files(dir: &Path) {
    // Create some files
    fs::write(dir.join("test1.txt"), b"Hello, World!").unwrap();
    fs::write(dir.join("test2.bin"), vec![0u8; 10000]).unwrap();
    
    // Create nested directory
    let nested = dir.join("a/b/c");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("deep.txt"), b"Deep file").unwrap();
    
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
        
        assert!(target_path.exists(), "Path {:?} missing in extracted dir", rel_path);
        
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
                assert_eq!(meta1.permissions().mode() & 0o777, meta2.permissions().mode() & 0o777, "Permission mismatch for {:?}", rel_path);
            }
        } else if meta1.is_symlink() {
            let target1 = fs::read_link(entry.path()).unwrap();
            let target2 = fs::read_link(&target_path).unwrap();
            assert_eq!(target1, target2, "Symlink target mismatch for {:?}", rel_path);
        }
    }
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
