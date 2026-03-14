# Hayazip
[**日本語**](README_JA.md) | **English**

🚀 **Blazing Fast, Multi-Threaded SIMD ZIP Extraction Library for Rust & Python**

`hayazip` is an ultra-fast ZIP archive extraction library designed from the ground up to leverage modern hardware capabilities. It combines memory-mapped I/O, SIMD-accelerated decompression (via `libdeflater`), and thread-pool-based parallel extraction (via `rayon`) to achieve up to **10x faster extraction latency** compared to the standard Unix `unzip` utility.

## Features
- **Zero-Copy Parsers:** Uses `memmap2` to map the ZIP file directly into memory, skipping expensive kernel-to-user-space copies.
- **SIMD Optimized Decompression:** Powered by `libdeflater` to leverage AVX2, AVX-512, or NEON depending on the architecture.
- **Multi-threaded Extraction:** Uses `rayon` in a Fork-Join model to decompress and extract independent files in parallel securely.
- **Hardware-accelerated CRC32:** Validates integrity using hardware instructions through `crc32fast`.
- **Cross-platform Python Bindings:** Built with PyO3 for easy, out-of-the-box integration in any Python environment.

## Python Quick Start

### Installation
You can install `hayazip` directly from PyPI (binary wheels available for Linux, macOS, and Windows):
```bash
pip install hayazip
```

### Usage
Extracting archives in Python is easy and significantly faster than the standard `zipfile` module:
```python
import hayazip

archive_path = "huge_archive.zip"
output_dir = "extracted_files"

# Extracts the entire archive fully utilizing all CPU cores
hayazip.extract_zip(archive_path, output_dir)
print("Done!")
```

## Rust Quick Start

Add `hayazip` to your `Cargo.toml`:
```toml
[dependencies]
hayazip = "0.1.0"
```

### Usage
```rust
use hayazip::extract;

fn main() {
    let archive_path = "huge_archive.zip";
    let output_dir = "extracted_files";

    if let Err(e) = extract(archive_path, output_dir) {
        eprintln!("Extraction failed: {}", e);
    } else {
        println!("Extraction successful!");
    }
}
```

## Benchmarks
Extracting a 50MB realistically compressed test archive with 10 files (5MB each):
- `unzip` (Unix standard): ~162ms
- `hayazip`: **~16ms (10x faster)**

## Build from Source (Python)
To compile from source and install into your local Python environment:
```bash
pip install maturin
maturin develop --release
```

## License
MIT
