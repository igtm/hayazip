pub mod archive;
pub mod compress;
pub mod entry;
pub mod error;
pub mod extract;

pub use archive::ZipArchive;
pub use compress::create_zip;
pub use entry::ZipEntry;
pub use error::{HayazipError, Result};
pub use extract::extract;

use pyo3::prelude::*;

#[pyfunction]
fn extract_zip(archive_path: String, dest_path: String) -> PyResult<()> {
    extract(&archive_path, &dest_path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Extraction failed: {}", e)))
}

#[pyfunction(name = "create_zip")]
fn create_zip_py(source_path: String, archive_path: String) -> PyResult<()> {
    create_zip(&source_path, &archive_path).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Archive creation failed: {}", e))
    })
}

#[pymodule]
fn hayazip(_py: Python, m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(extract_zip, m)?)?;
    m.add_function(wrap_pyfunction!(create_zip_py, m)?)?;
    Ok(())
}
