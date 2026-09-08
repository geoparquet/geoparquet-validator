# geoparquet-validator

Validate GeoParquet files. One command, also a library, that checks GeoParquet 1.0, 1.1 and 2.0
files against the [specification](https://github.com/opengeospatial/geoparquet) on a file, a
directory or an object-store URL, and reports the [distribution best practices](https://github.com/opengeospatial/geoparquet/blob/main/format-specs/distributing-geoparquet.md)
as advice. The 2.0 checks align with the abstract tests of the
[OGC GeoParquet 2.0 draft](https://github.com/opengeospatial/geoparquet/pull/304). Written in Rust;
no DuckDB, GDAL or PROJ.

**Try it in your browser: https://validator.geoparquet.org/.** Files stay on your machine; a URL is
read with range requests, so a 500 MB file on S3 takes a few seconds.

```
$ geoparquet-validator check buildings.parquet
buildings.parquet
  version 1.1.0 · rules: the GeoParquet 1.1.0 specification
  PASS  /conf/core/geo-metadata
  ...
  => core: 13 pass, 0 fail, 7 skipped: conformant
  => covering: 6 pass, 0 fail, 0 skipped: conformant
  => distribution: 2 pass, 0 fail, 0 skipped: conformant
  distribution best practices (advice, not conformance):
  ok    spatial ordering: the rows are well ordered: a query window can skip 99 % of the row groups ...
  ok    row group size: 256 row group(s), at most 28734 rows each (5007414 rows, 538 MB)
  ok    compression: every column chunk uses ZSTD, the codec the best practices recommend ...
  ok    bbox covering: a bounding box covering column is declared, so readers can prune pages as well as row groups
```

## Installation

| | |
| --- | --- |
| macOS, Linux | download the archive for your platform from the [releases](https://github.com/geoparquet/geoparquet-validator/releases) and put `geoparquet-validator` on your PATH (Homebrew tap coming: [#5](https://github.com/geoparquet/geoparquet-validator/issues/5)) |
| Windows | the `.zip` from the [releases](https://github.com/geoparquet/geoparquet-validator/releases); `winget` coming ([#6](https://github.com/geoparquet/geoparquet-validator/issues/6)) |
| Python | `pip install geoparquet-validator` gives the command and `import geoparquet_validator` |
| Rust | `cargo install geoparquet-validator` |
| conda | coming ([#3](https://github.com/geoparquet/geoparquet-validator/issues/3)) |
| Node, bundlers | `npm install geoparquet-validator` (WebAssembly) |
| C, and anything with a foreign-function interface | `libgeoparquet_validator` and `include/geoparquet_validator.h`, in every release archive |
| From source | `git clone --recurse-submodules https://github.com/geoparquet/geoparquet-validator && cd geoparquet-validator && cargo build --release` |

## Usage

```
geoparquet-validator check file.parquet                      # text report, exit 0 / 1 / 2
geoparquet-validator check file.parquet --json               # the report as JSON (schemas/report.schema.json)
geoparquet-validator check file.parquet --class core         # only Core decides the exit code
geoparquet-validator check s3://bucket/prefix/ --max-files 5 --max-rows 100000 --s3-region us-west-2
geoparquet-validator check https://host/path/file.parquet --max-rows 100000
geoparquet-validator check ./directory/                      # every .parquet below it
```

`--max-rows N` reads only the first row groups holding N rows; the report then says the data tests
were sampled, which is not a conformance pass. Exit codes: 0 conformant, 1 a test failed, 2 the tool
could not run (unreadable path, bad URL, network).

A report has three conformance classes, each `conformant`, `NOT CONFORMANT` or `not claimed`:

| Class | Applies to | Tests |
| --- | --- | --- |
| Core | every file | 20 |
| Bounding Box Covering | files that declare a `covering` | 6 |
| Cloud-Optimized Distribution | an optional profile for direct cloud access | 2, plus the advice |

Each file is checked against the rules of the version it declares, under the same test identifiers;
the report says which rules were applied. Details of every check, the 1.x rules, the
spatial-ordering metric and the browser build are in [docs/design.md](docs/design.md).

Every surface returns the same report, described by [`schemas/report.schema.json`](schemas/report.schema.json):
the 28 outcomes (`pass`, `fail`, `skip`, each with a message), the version the file declares, the rules
applied, whether the data tests were sampled, and the distribution advice. `report_version` is bumped
when a field changes meaning; fields may be added without a bump.

**Command line, from any language**

```sh
geoparquet-validator check file.parquet --json            # the report on stdout
geoparquet-validator check s3://bucket/prefix/ --json --max-rows 100000
# exit codes: 0 conformant, 1 a test failed, 2 the tool could not run (unreadable file, network)
```

**Python** (`pip install geoparquet-validator`)

```python
import geoparquet_validator as gpv

report = gpv.check("file.parquet")                 # or an s3://, gs://, az://, https:// URL
report = gpv.check_bytes("upload.parquet", data)   # bytes already in memory
gpv.conformant(report, "core")                     # True when no Core test failed
gpv.failed(report)                                 # ["/conf/core/bbox-extent", ...]
report["advice"]                                   # the distribution best practices, measured
```

**Rust** (`cargo add geoparquet-validator`, docs on [docs.rs](https://docs.rs/geoparquet-validator))

```rust
let report = geoparquet_validator::validate("file.parquet", None)?;
let failed: Vec<_> = report.outcomes.iter().filter(|o| o.status == Status::Fail).collect();
// checks::run(&source, &schemas, &options) takes any `Source`: Local, InMemory, or a range reader
```

**Node and browsers** (`npm install geoparquet-validator`)

```js
import { check, conformant, failed } from "geoparquet-validator";
const report = await check("file.parquet");        // Node; an https:// URL is downloaded whole
```

In a browser, import `geoparquet-validator/bundler` and call `check_bytes(name, uint8array)`; the
page at https://validator.geoparquet.org/ is exactly that, plus a worker that gives the
range reader synchronous fetch callbacks so URLs are read in pieces.

**C, and everything with a foreign-function interface**

```c
#include "geoparquet_validator.h"
char *report = gpv_check("file.parquet", 0);        /* JSON, or {"error": "..."} */
gpv_free(report);
```

`examples/check.c` is a complete program; build the library with `cargo build --release --features capi`.
Go through cgo, Java, R, .NET and Julia can call the same four functions.

## How it is tested

Against the official [test corpus](https://github.com/geoparquet/geoparquet-testing) (48 valid files
pass, 24 defective files caught), 198 generated fixtures with a verdict manifest, files from seven
writers (DuckDB, SedonaDB, GDAL, GeoPandas, geoarrow, pyarrow), the public example files of the
specification, GDAL, Apache Sedona and geoarrow-data, and real files on S3 including Overture Maps.
All of it is in [docs/testing.md](docs/testing.md); what it surfaced for the specification is in
[docs/findings.md](docs/findings.md).

## Support

Questions and bug reports: [issues](https://github.com/geoparquet/geoparquet-validator/issues). A
report that looks wrong is worth an issue with the file, or its URL, attached; the validator is meant
to be right about the specification, and where it is not, the specification or the validator gets
fixed.

## Contributing

Pull requests are welcome. Clone with `--recurse-submodules`, build with `cargo build --release`
(the toolchain is pinned by `rust-toolchain.toml`), and run what CI runs:

```
cargo fmt --check && cargo clippy --release --all-targets -- -D warnings && cargo test --release
./target/release/geoparquet-validator corpus corpus
cd corpus/scripts && uv sync && uv run python ../../fixtures/generate.py && cd ../..
./target/release/geoparquet-validator verify fixtures/out
```

A change that alters a verdict must update `fixtures/expected.json` (`verify --update`, then review
the diff). The abstract tests this tool implements live in
[opengeospatial/geoparquet#304](https://github.com/opengeospatial/geoparquet/pull/304); a
disagreement with them is a bug in one or the other and belongs in an issue there or here.

## Layout

`src/` the crate (features `cli`, `remote`, `wasm`, `python`, `capi`) · `web/` the browser app ·
`python/` the Python package · `npm/` the npm package · `include/`, `examples/` the C interface ·
`fixtures/` the generators and the verdict manifest · `corpus/` the official test corpus, as a
submodule · `packaging/` Homebrew, winget and conda files · `docs/` the long version of this README.

## Acknowledgements

Built for the [OGC GeoParquet Standards Working Group](https://www.ogc.org/) as the executable
counterpart of the 2.0 abstract tests, alongside the community validator
[geoparquet-io](https://github.com/geoparquet/geoparquet-io), whose spatial-ordering metric this
tool reuses. The test corpus is maintained in
[geoparquet/geoparquet-testing](https://github.com/geoparquet/geoparquet-testing).

## License

[Apache-2.0](LICENSE).
