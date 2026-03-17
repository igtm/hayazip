use thiserror::Error;

#[derive(Error, Debug)]
pub enum HayazipError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Invalid ZIP format: {0}")]
    InvalidFormat(&'static str),
    #[error("Archive is too large: {0}")]
    ArchiveTooLarge(&'static str),
    #[error("Unsupported compression method: {0}")]
    UnsupportedCompression(u16),
    #[error("Compression error: {0}")]
    Compression(String),
    #[error("Decompression error: {0}")]
    Decompression(String),
    #[error("CRC32 mismatch (expected {expected:08x}, got {actual:08x})")]
    CrcMismatch { expected: u32, actual: u32 },
    #[error("Unsafe archive entry path: {0}")]
    UnsafePath(String),
    #[error("Archive entry conflict: {0}")]
    EntryConflict(String),
}

pub type Result<T> = std::result::Result<T, HayazipError>;
