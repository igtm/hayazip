pub mod archive;
pub mod compress;
pub mod entry;
pub mod error;
pub mod extract;

pub use archive::ZipArchive;
pub use compress::create_zip;
pub use entry::ZipEntry;
pub use error::{HayazipError, Result};
pub use extract::{PreflightEntry, extract, extract_from_bytes, preflight, preflight_bytes};

use pyo3::prelude::*;
use pyo3::types::PyDict;

#[pyfunction]
fn extract_zip(archive_path: String, dest_path: String) -> PyResult<()> {
    extract(&archive_path, &dest_path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Extraction failed: {}", e)))
}

#[pyfunction]
fn extract_zip_bytes(archive_bytes: &[u8], dest_path: String) -> PyResult<()> {
    extract_from_bytes(archive_bytes, &dest_path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Extraction failed: {}", e)))
}

#[pyfunction(name = "create_zip")]
fn create_zip_py(source_path: String, archive_path: String) -> PyResult<()> {
    create_zip(&source_path, &archive_path).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Archive creation failed: {}", e))
    })
}

#[pyfunction]
fn preflight_zip(py: Python<'_>, archive_path: String) -> PyResult<Vec<Py<PyDict>>> {
    let entries = preflight(&archive_path).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Preflight failed: {}", e))
    })?;
    preflight_entries_to_py(py, entries)
}

#[pyfunction]
fn preflight_zip_bytes(py: Python<'_>, archive_bytes: &[u8]) -> PyResult<Vec<Py<PyDict>>> {
    let entries = preflight_bytes(archive_bytes).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Preflight failed: {}", e))
    })?;
    preflight_entries_to_py(py, entries)
}

#[pymodule]
fn hayazip(_py: Python, m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(extract_zip, m)?)?;
    m.add_function(wrap_pyfunction!(extract_zip_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(create_zip_py, m)?)?;
    m.add_function(wrap_pyfunction!(preflight_zip, m)?)?;
    m.add_function(wrap_pyfunction!(preflight_zip_bytes, m)?)?;
    Ok(())
}

fn preflight_entries_to_py(
    py: Python<'_>,
    entries: Vec<PreflightEntry>,
) -> PyResult<Vec<Py<PyDict>>> {
    entries
        .into_iter()
        .map(|entry| {
            let dict = PyDict::new(py);
            dict.set_item("archive_name", entry.archive_name)?;
            dict.set_item("path", entry.normalized_name)?;
            dict.set_item("is_dir", entry.is_dir)?;
            dict.set_item("is_symlink", entry.is_symlink)?;
            dict.set_item("file_size", entry.uncompressed_size)?;
            dict.set_item("compress_size", entry.compressed_size)?;
            dict.set_item("compress_type", entry.compression_method)?;
            dict.set_item("crc32", entry.crc32)?;
            dict.set_item("external_attr", entry.external_attr)?;
            Ok(dict.unbind())
        })
        .collect()
}
