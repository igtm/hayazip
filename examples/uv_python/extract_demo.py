# /// script
# requires-python = ">=3.8"
# dependencies = [
#     "hayazip",
# ]
# [tool.uv.sources]
# hayazip = { path = "../../" }
# ///

import os
import time
import zipfile
import hayazip
import shutil

ARCHIVE_NAME = "demo_archive.zip"
EXTRACT_DIR = "demo_extracted_files"

def create_dummy_archive():
    """Create a dummy zip file for demonstration purposes."""
    print(f"Creating a dummy archive: {ARCHIVE_NAME}...")
    with zipfile.ZipFile(ARCHIVE_NAME, "w", zipfile.ZIP_DEFLATED) as zf:
        # Create 5 files with 1MB random data
        for i in range(5):
            filename = f"file_{i}.bin"
            data = os.urandom(1024 * 1024)
            zf.writestr(filename, data)
    print("Archive created.\n")

def cleanup():
    """Clean up generated files and directories."""
    print("Cleaning up...")
    if os.path.exists(ARCHIVE_NAME):
        os.remove(ARCHIVE_NAME)
    if os.path.exists(EXTRACT_DIR):
        shutil.rmtree(EXTRACT_DIR)

def main():
    try:
        cleanup()
        create_dummy_archive()
        
        # Ensure extraction target directory exists
        os.makedirs(EXTRACT_DIR, exist_ok=True)
        
        print(f"Starting extraction with hayazip to '{EXTRACT_DIR}'...")
        start_time = time.time()
        
        # This is where the magic happens - utilizing Rust underneath!
        hayazip.extract_zip(ARCHIVE_NAME, EXTRACT_DIR)
        
        elapsed = time.time() - start_time
        print(f"Extraction completed successfully in {elapsed:.4f} seconds!")
        
        # Verify
        files_extracted: int = len(os.listdir(EXTRACT_DIR))
        print(f"Verified: {files_extracted} files extracted.")
        
    finally:
        # cleanup()
        print("\nNote: Generated files were kept for inspection. Rerun the script to clean up automatically.")

if __name__ == "__main__":
    main()
