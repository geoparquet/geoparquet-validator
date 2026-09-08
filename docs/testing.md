# How the validator is tested

Every claim in the README traces back to one of the runs below; the findings the runs produced for the specification are recorded in the OGC document's issue list (docs/ogc/spec-issues.md in opengeospatial/geoparquet#304). Dates are when the run was made; the
numbers are reproduced by CI or by the scripts named.

## Test suite

Four layers, all but the last in CI (`.github/workflows/ci.yml`):

1. `cargo test`: unit tests of the WKB decoder and the spatial-order metric.
2. `geoparquet-validator corpus corpus`: the corpus submodule's `data/` must pass Core and `bad_data/` must fail
   the test mapped from its `expected_failure`.
3. `geoparquet-validator verify fixtures/out`: 198 generated fixtures in four sets (the author's, and
   the spec, hostile and code reviewers') against `fixtures/expected.json`, the manifest of which
   tests must fail for each file. See [`fixtures/README.md`](fixtures/README.md).
4. `fixtures/remote_smoke.sh`: the public files on S3 and HTTPS listed in the results below (needs the network).

## Results (2026-09-05)

this corpus at `main` 6f7ede1 (48 valid + 26 defective files), whole run 0.03 s (0.7 s before the PROJJSON schema was vendored, all of it a network fetch):

* data/: 48 of 48 pass Core; all 48 carry geospatial statistics; spatial order not measurable
  (single row group).
* bad_data/: 24 of 24 files with an OGC requirement are failed by the mapped test; the remaining
  two (`edges_mismatch`, `epoch_unsupported`) have no OGC requirement, as expected.

33 adversarial fixtures of the author's (`fixtures/make_fixtures.py`: 6/8-element and antimeridian bbox
violations, 11 covering positives/negatives incl. nullness mismatch and nested column, 7 Parquet
`crs` forms: `EPSG:3857`, inline PROJJSON, `srid:0`, `projjson:<key>`, mismatches): all behave as
intended.

Multi-row-group files (DuckDB, `GEOPARQUET_VERSION 'V2'`), full Core + Distribution run, Docker on an
M-series laptop:

| File | Rows | Row groups | Wall time | spatial-order (ours) | gpio `check spatial` (main) |
| --- | --- | --- | --- | --- | --- |
| points, random order | 1 000 000 | 10 | 0.41 s | FAIL ratio 0.00 | poor |
| points, Hilbert | 1 000 000 | 10 | 0.22 s | PASS ratio 0.93 | ordered |
| 20 clusters, Hilbert | 500 000 | 10 | 0.16 s | PASS ratio 1.00 | ordered |
| squares, DuckDB ST_Hilbert on polygons | 200 000 | 4 | 0.17 s | ratio 0.67, verdict withheld (< 5 row groups) | poor |
| same squares after `gpio sort hilbert` | 200 000 | 4 | 0.17 s | ratio 0.93, verdict withheld (< 5 row groups) | ordered |

The two tools agree on every file. The wall time includes decoding every WKB value; gpio's full
`check spec` on the same files takes several seconds because of the DuckDB start-up and sampling.

Remote files, from a laptop (about 5 MB/s to S3):

| Target | Rows read | Bytes / requests | Wall time | Result |
| --- | --- | --- | --- | --- |
| opengeospatial/geoparquet `examples/example.parquet` (GitHub raw) | all | 0.03 MB / 1 | 0.26 s | Core conformant |
| Overture 2026-08-19.0 buildings part-00000 (5.0 M rows, S3) | first 100 000 | 21 MB / 2 | 6 s | conformant under the 1.1 rules |
| Overture buildings partition (512 objects), `--max-files 2` | 100 000 each | 2 x 21 MB | 12 s | same, per file |
| Overture 2026-08-19.0 divisions/division_area part-00000 (721 MB, 138 481 polygons, S3), whole file | all | 804 MB / 341 | 240 s | every WKB polygon decoded; same verdicts |
| source.coop / geoarrow-data 1.0 files (HTTPS) | all | 1 to 8 MB / 1 | 1 to 4 s | 1.0 files fail version and logical type as expected |

## Hardening round (2026-09-06)

Three independent reviews (spec-conformance, hostile input, Rust code/perf) and 150+ crafted files.
No crash, hang or memory blow-up was found; the verdict and text problems they found are fixed here:
bounded WKB allocations and count checks, Multi* member type and dimension checks, NaN only allowed
for empty points, shoelace computed relative to the first vertex, the ideal tiling with exactly n
tiles, non-finite statistics rejected, one-dimensional extents measured, wrapping row-group boxes
split, strict `<authority>:<code>` parsing and PROJJSON `ids`, CRS comparison never by name, inconclusive
CRS comparisons reported as notes instead of failures, `geo-metadata` validated against the published
schema (and `crs-projjson` against the PROJJSON `crs` definition), the antimeridian form of `bbox`
required to be justified by the data, a missing `columns` member no longer failing nesting, a missing
bounding-box column failing only `bbox-paths`, `encoding` missing or non-string failing
`geometry-column-type`, dictionary-hinted binary columns read as WKB, unread columns reported on every
data test, tool errors distinguished from conformance failures (exit 2), `--class` validated. Unit
tests cover the decoder and the metric (`cargo test`).

## Writer zoo (2026-09-06)

The same 250 features (200 points, 50 CCW squares, lon/lat) written by every writer at hand
(`fixtures/writer_zoo.py`), then checked. Only two writers produce GeoParquet 2.0 today:

| Writer | Asked for | Result |
| --- | --- | --- |
| DuckDB 1.5.5 spatial, `GEOPARQUET_VERSION 'V2'` | CRS84 | **Core conformant**. Parquet `crs` is inline PROJJSON (schema v0.5) EPSG:4326, `geo.crs` PROJJSON EPSG:4326. |
| DuckDB 1.5.5, same, after `ST_Transform` to EPSG:3857 | EPSG:3857 | **Mislabelled**: DuckDB geometries carry no CRS, so the file declares the default OGC:CRS84 while coordinates are metres. Caught by `crs-default` (250 geometries outside lon/lat range) and `bbox-crs`. A DuckDB user cannot fix this from `COPY` today. |
| SedonaDB 0.4.1, `geoparquet_version="2.0"` | CRS84 and EPSG:3857 | **Core conformant** both; PROJJSON (v0.7) in both the Parquet `crs` and `geo.crs`. |
| GDAL 3.12.2 `ogr2ogr -lco USE_PARQUET_GEO_TYPES=YES` | CRS84 and EPSG:3857 | Native GEOMETRY types with consistent CRS, covering bbox in the right order, but `geo.version` is **1.1.0**: GDAL has no 2.0.0 writer yet. Under the 1.1 rules the file is conformant, with a note that it carries a 2.0 logical type. `USE_PARQUET_GEO_TYPES=ONLY` writes no `geo` block at all. |
| GeoPandas 1.1.4 | `schema_version="2.0.0"` | Rejected: `must be one of 0.1.0 ... 1.1.0`. Its 1.1.0 output (with `write_covering_bbox`) is well formed, bbox fields in the right order. |
| geoarrow-pyarrow 0.3 `write_geoparquet_table` | default | Writes 1.0.0, no logical type, `crs: null` for lon/lat data. |
| pyarrow 25 alone | native type only | GEOMETRY logical type, no `geo` block: not GeoParquet, as the spec says. |

## Public sample sweep (2026-09-07)

`fixtures/public_samples.sh` downloads the example files other projects publish (the specification's
own examples at 1.0.0, 1.1.0 and main; GDAL's autotest Parquet data; Apache Sedona's test data;
geoarrow-data) and checks them: 38 files, no crashes, every verdict explainable. Its main find: the
**1.1.0 specification's own example file** orders its bbox struct `xmax, xmin, ymax, ymin`, so the
"MUST be ordered in this same way" sentence was never followed even by the reference example. That
evidence retired the rule (SI-26, dropped when PR #302 merged). GDAL's 1.1 test files, including one
with a covering, are fully conformant under the 1.1 rules.
Pre-1.0 files (GeoParquet 0.1.0, 0.4.0) are checked as 2.0 with a note, since their rules are not
implemented.
