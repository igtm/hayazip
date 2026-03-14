pub mod error;
pub mod archive;
pub mod entry;
pub mod extract;

pub use error::{HayazipError, Result};
pub use archive::ZipArchive;
pub use entry::ZipEntry;
pub use extract::extract;

#[cfg(feature = "pyo3")]
use pyo3::prelude::*;

#[cfg(feature = "pyo3")]
#[pyfunction]
fn extract_zip(archive_path: String, dest_path: String) -> PyResult<()> {
    extract(&archive_path, &dest_path).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Extraction failed: {}", e))
    })
}

#[cfg(feature = "pyo3")]
#[pymodule]
fn hayazip(_py: Python, m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(extract_zip, m)?)?;
    Ok(())
}
