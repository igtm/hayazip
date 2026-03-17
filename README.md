# Hayazip
[**日本語**](README_JA.md) | **English**

🚀 **Blazing Fast, Multi-Threaded SIMD ZIP Library for Rust & Python**

`hayazip` is an ultra-fast ZIP archive library designed from the ground up to leverage modern hardware capabilities. It combines memory-mapped I/O, SIMD-accelerated compression and decompression (via `libdeflater`), and thread-pool-based parallelism (via `rayon`) to accelerate both ZIP extraction and ZIP creation.

## Features
- **Zero-Copy Parsers:** Uses `memmap2` to map the ZIP file directly into memory, skipping expensive kernel-to-user-space copies.
- **SIMD Optimized Compression and Decompression:** Powered by `libdeflater` to leverage AVX2, AVX-512, or NEON depending on the architecture.
- **Multi-threaded ZIP Creation and Extraction:** Uses `rayon` to process independent files in parallel.
- **Hardware-accelerated CRC32:** Validates integrity using hardware instructions through `crc32fast`.
- **Low-footprint Archive Writing:** Spools compressed members to temporary files instead of holding the full archive in memory.
- **Path-safe Extraction:** Normalizes entry separators and rejects traversal, absolute, and drive-prefixed output paths before writing starts.
- **Archive Preflight:** Validates every central-directory entry up front so callers can inspect safe output paths before extraction.
- **Cross-platform Python Bindings:** Built with PyO3 for easy, out-of-the-box integration in any Python environment.

## Python Quick Start

### Installation
You can install `hayazip` directly from PyPI with `uv` or `pip`. Prebuilt `abi3` wheels are published for CPython 3.8+ on Linux, macOS, and Windows, and a source distribution is published as a fallback:
```bash
uv add hayazip
# or
pip install hayazip
```

### Usage
Creating and extracting archives in Python is straightforward:
```python
import hayazip

source_dir = "project_files"
archive_path = "project_files.zip"
output_dir = "extracted_files"

hayazip.create_zip(source_dir, archive_path)
hayazip.extract_zip(archive_path, output_dir)
print("Done!")
```

If you already have ZIP bytes in memory, you can preflight and extract them directly without a temporary file:
```python
import hayazip

entries = hayazip.preflight_zip_bytes(pptx_bytes)
for entry in entries:
    print(entry["path"], entry["compress_type"])

hayazip.extract_zip_bytes(pptx_bytes, "workdir/unpacked")
```

## Rust Quick Start

Add `hayazip` to your `Cargo.toml`:
```toml
[dependencies]
hayazip = "0.2.0"
```

### Usage
```rust
use hayazip::{create_zip, extract, extract_from_bytes, preflight};

fn main() {
    let source_dir = "project_files";
    let archive_path = "project_files.zip";
    let output_dir = "extracted_files";

    create_zip(source_dir, archive_path).expect("Archive creation failed");

    if let Err(e) = extract(archive_path, output_dir) {
        eprintln!("Extraction failed: {}", e);
    } else {
        println!("Extraction successful!");
    }

    let safe_entries = preflight(archive_path).expect("Preflight failed");
    println!("{} entries validated", safe_entries.len());

    let archive_bytes = std::fs::read(archive_path).expect("read failed");
    extract_from_bytes(&archive_bytes, "extracted_from_bytes").expect("bytes extraction failed");
}
```

## Extraction Safety
`hayazip` performs a metadata-only preflight before it creates files or directories. During that pass it:

- normalizes separator variants to forward-slash archive paths,
- rejects `..`, absolute paths, and Windows drive prefixes,
- detects duplicate or conflicting output paths such as `dir` and `dir/file.txt`,
- validates that each entry's local header and compressed payload are structurally readable.

Use `preflight` / `preflight_bytes` in Rust or `preflight_zip` / `preflight_zip_bytes` in Python if you want the validated path list without extracting yet.

## Compression Method Support
Current extraction support:

- `0` (`Stored` / no compression)
- `8` (`Deflate`)

Current archive creation support:

- `Stored` for directories, symlinks, empty files, and files where compression is not beneficial
- `Deflate` for regular files when it reduces size

Currently unsupported for extraction and creation:

- any other ZIP compression method, including `Deflate64` (`9`), `BZIP2` (`12`), `LZMA` (`14`), `PPMd` (`98`), and `Zstandard` (`93`)
- encrypted ZIP entries

## Benchmarks
On modern CPUs, `hayazip` uses `libdeflater` for SIMD-accelerated DEFLATE and `rayon` for parallel file processing. Archive creation writes members with bounded worker parallelism and a temporary spool to keep memory usage predictable while still saturating multiple cores.

## Current Scope
`create_zip` is the only public write API today. A lower-level metadata-preserving writer for explicit entry order, timestamps, compression method, and external attributes is not exposed yet.

## Build from Source (Python)
To compile from source and install into your local Python environment:
```bash
pip install maturin
maturin develop --release
```

## License
MIT
