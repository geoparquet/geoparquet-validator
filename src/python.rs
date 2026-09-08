//! The Python extension module, `geoparquet_validator._native`. It returns reports as JSON text;
//! the Python package parses them, so the two sides share nothing but the report schema.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

fn to_json(r: anyhow::Result<crate::checks::Report>) -> PyResult<String> {
    r.and_then(|r| Ok(serde_json::to_string(&r)?))
        .map_err(|e| PyRuntimeError::new_err(format!("{e:#}")))
}

/// Check a path or URL; returns the report as a JSON string.
#[pyfunction]
#[pyo3(signature = (target, max_rows=None))]
fn check(py: Python<'_>, target: &str, max_rows: Option<usize>) -> PyResult<String> {
    py.detach(|| to_json(crate::validate(target, max_rows)))
}

/// Check a file held in memory; returns the report as a JSON string.
#[pyfunction]
#[pyo3(signature = (name, data, max_rows=None))]
fn check_bytes(
    py: Python<'_>,
    name: &str,
    data: &[u8],
    max_rows: Option<usize>,
) -> PyResult<String> {
    let bytes = data.to_vec();
    py.detach(move || to_json(crate::validate_bytes(name, bytes, max_rows)))
}

/// The command line, for the `geoparquet-validator` console script; returns the exit code.
#[pyfunction]
fn main(py: Python<'_>, argv: Vec<String>) -> i32 {
    py.detach(|| crate::cli::run(argv))
}

#[pymodule(name = "_native")]
fn native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(check, m)?)?;
    m.add_function(wrap_pyfunction!(check_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(main, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
