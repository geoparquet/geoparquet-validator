# What it checks, and how

## The checks

```
geoparquet-validator check file.parquet [--json] [--class core|covering|distribution]
geoparquet-validator check s3://bucket/prefix/ --max-files 5 --max-rows 100000 --s3-region us-west-2
geoparquet-validator check https://host/path/file.parquet     # also gs://, az://, a local directory
geoparquet-validator corpus corpus  # the corpus submodule: data/ must pass, bad_data/ must fail the mapped test
```

Remote objects are read with range requests through the `object_store` crate (anonymous when no
credentials are in the environment; `--opt key=value` passes any object_store option). The footer
comes down in one request; the scan fetches whole column chunks, merged per row group and split into
16 MB parts fetched concurrently. `--max-rows N` reads only the first row groups that hold N rows and
marks the data tests as sampled, which is how a 700 MB Overture file is checked in a few seconds.
Exit codes: 0 conformant, 1 a test failed, 2 the tool could not run (unreadable path, bad URL).

Every abstract test of the three conformance classes is implemented and reports pass / fail / skip
with a message naming the column and the offending value:

| Class | Tests | Notes |
| --- | --- | --- |
| Core | 20 | `media-type` always skipped (not testable on a file). |
| Bounding Box Covering | 6 | Skipped as "not claimed" when no column declares `covering`. |
| Cloud-Optimized Distribution | 2 | `spatial-order` uses the pruning metric with gpio's parameters (geoparquet-io #774: 20 windows of 10 % side, seed 42, pass at 0.70 of the ideal tiling's skip rate, verdict withheld below five row groups) and also prints the area factor Σ row-group bbox area / extent. The window sequence differs from gpio's, so near-threshold verdicts can differ. |

Design: one pass over the data per geometry column (arrow record batches, WKB decoded by a
150-line ISO WKB reader that rejects EWKB), everything else from the footer. Schema validation
uses the vendored GeoParquet 2.0.0 `schema.json` and the PROJJSON 0.7 schema (registered under its
URL, so the tool works offline; validated against the PROJJSON `crs` definition only, because the
full PROJJSON schema also accepts datums and ellipsoids). CRS equality is by authority:code after
normalising EPSG:4326 / OGC:CRS84; a PROJJSON without an `id` is reported as "cannot compare
without a CRS library" rather than passed or failed.

## GeoParquet 1.0 and 1.1 files

The abstract tests are written for 2.0, but most files in the wild are still 1.0 or 1.1, so the
checker reads `version` and applies that version's own rules with the same test identifiers: the
1.0.0 or 1.1.0 JSON Schema (with PROJJSON 0.5 / 0.7), `WKB` plus the 1.1 GeoArrow encodings (checked
structurally: a struct of x, y[, z] under the right number of list levels; their data is not decoded),
no `M` types, `edges` limited to planar and spherical, no Parquet `crs` comparison unless the file
carries native types, and the bbox covering column's Parquet statistics as the row-group statistics
source for the Distribution class. The report says which rules were applied. Overture 2026-08-19.0
buildings under the 1.1 rules: Core, Covering and Distribution conformant, with spatial order measured
from the covering statistics over 256 row groups.

## Distribution best practices, as advice

Alongside the conformance verdicts, every report ends with advice on the practices in
`format-specs/distributing-geoparquet.md`, measured on the file and never counted as conformance:
how well the rows are spatially ordered (the skip rate a query window achieves, against an ideal
tiling of the same number of row groups, with the sort command to fix it), the row group sizes
against the 150 000-row ceiling, the compression codec, whether a bounding box covering column is
present, and whether the file is large enough to be worth partitioning. Each item is `good`,
`consider` or `poor`, in the text output and in the JSON under `advice`.

## In the browser

`web/` is the same checker compiled to WebAssembly behind a one-page UI: drop a file (it never leaves
the machine) or paste a URL (read with range requests from a Web Worker, so the host must allow
cross-origin reads; public S3 buckets with CORS, GitHub raw and source.coop do; Overture's bucket
does). URL checks default to the first 100 000 rows, which is a sample, not a conformance pass.
Published at https://validator.geoparquet.org/. Build with `sh web/build.sh` (needs the `wasm32-unknown-unknown`
target, `wasm-bindgen-cli` matching `Cargo.lock`, clang for zstd, optionally `wasm-opt`); CI deploys it
to GitHub Pages on every push to `main`. 

## Building

`cargo build --release` (the toolchain is pinned to 1.98.0 by `rust-toolchain.toml`; CI uses the same version), or with Docker:

```
docker run --rm -v "$PWD":/work -w /work rust:1-slim-bookworm cargo build --release
```

Dependencies: parquet 59.3, arrow-array, jsonschema (no network features), serde_json, anyhow;
with the default `cli` feature also clap, object_store (aws, gcp, azure, http), tokio, futures, url;
with the `wasm` feature wasm-bindgen and js-sys instead. MSRV 1.88.
