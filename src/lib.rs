//! Validate GeoParquet files: the abstract tests of the OGC GeoParquet 2.0 draft, and the 1.0
//! and 1.1 community rules, on top of the Apache Arrow Rust `parquet` crate. No DuckDB, GDAL or
//! PROJ.
//!
//! The simplest entry points are [`validate`] for a path or URL and [`validate_bytes`] for a file
//! already in memory; both return a [`checks::Report`], which serialises to the JSON described by
//! `schemas/report.schema.json`. [`checks::run`] takes any [`source::Source`] for finer control.
//!
//! ```no_run
//! let report = geoparquet_validator::validate("example.parquet", None)?;
//! for o in &report.outcomes {
//!     println!("{:?} {} {}", o.status, o.id, o.message);
//! }
//! println!("{}", serde_json::to_string_pretty(&report)?);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! The same code is the `geoparquet-validator` command (`cli`, the default feature), the Python
//! package (`python`), a C library (`capi`) and the browser build (`wasm`).

pub mod checks;
pub mod corpus;
pub mod crs;
pub mod source;
pub mod spatial;
pub mod verify;
pub mod wkb;

#[cfg(feature = "capi")]
pub mod capi;
#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "python")]
mod python;
#[cfg(feature = "wasm")]
pub mod wasm;

use checks::{Options, Report, Schemas};

/// Check a local path or, with the `remote` feature, an `s3://`, `gs://`, `az://` or `https://`
/// URL. `max_rows` reads only the first row groups holding that many rows; the report then says
/// the data tests were sampled.
pub fn validate(target: &str, max_rows: Option<usize>) -> anyhow::Result<Report> {
    let schemas = Schemas::load()?;
    let options = Options { max_rows };
    #[cfg(feature = "remote")]
    if source::is_remote(target) {
        let url = url::Url::parse(target)?;
        let opts = source::RemoteOptions {
            s3_region: None,
            extra: Vec::new(),
        };
        let src = source::open_remote(&url, &opts)?;
        return checks::run(&src, &schemas, &options);
    }
    let src = source::Local(std::path::PathBuf::from(target));
    checks::run(&src, &schemas, &options)
}

/// Check a file already in memory; `name` only labels the report.
pub fn validate_bytes(
    name: &str,
    bytes: Vec<u8>,
    max_rows: Option<usize>,
) -> anyhow::Result<Report> {
    let schemas = Schemas::load()?;
    let src = source::InMemory {
        name: name.to_string(),
        bytes: bytes::Bytes::from(bytes),
    };
    checks::run(&src, &schemas, &Options { max_rows })
}

#[cfg(test)]
mod tests {
    #[test]
    fn report_matches_its_schema() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../schemas/report.schema.json")).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        // the corpus submodule is present in a checkout, not in the published crate
        let path = "corpus/data/bbox/bbox-present.parquet";
        if !std::path::Path::new(path).exists() {
            eprintln!("skipped: {path} not present");
            return;
        }
        let report = super::validate(path, None).unwrap();
        let value = serde_json::to_value(&report).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(value["report_version"], 1);
    }
}
