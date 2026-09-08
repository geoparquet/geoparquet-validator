//! GeoParquet validation: the abstract tests of the OGC GeoParquet 2.0 draft, and the 1.0 and
//! 1.1 community rules, on top of the Apache Arrow Rust `parquet` crate. The command line lives
//! in `main.rs`; `wasm.rs` exposes the same checks to a browser.

pub mod checks;
pub mod corpus;
pub mod crs;
pub mod source;
pub mod spatial;
pub mod verify;
#[cfg(feature = "wasm")]
pub mod wasm;
pub mod wkb;
