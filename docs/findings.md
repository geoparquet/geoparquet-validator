# Findings for the specification and the corpus

Things the validator surfaced while being built, and what happened to them.

0. **The bbox field-order rule is gone, and this checker is why.** Every Overture Maps file orders
   its bbox struct `xmin, xmax, ymin, ymax`; the 1.1.0 specification's own `examples/example.parquet`
   orders it `xmax, xmin, ymax, ymin`; Apache Sedona's 1.1 test file does the same. GeoParquet 1.1 and
   the draft of PR #302 required the order, and no validator had ever checked it. #302 merged on
   2026-09-07 **without** the rule for the four-field form (the six-field form keeps its order), and
   the checker follows the merged text: those three files are now Covering conformant.
1. `bad_data/crs-invalid-projjson.parquet` (a `crs` without `type`) passes the PROJJSON JSON Schema:
   the schema's top-level `oneOf` also accepts ellipsoids, datums and operations, and `{id, name}` is
   a valid ellipsoid. The OGC test `/conf/core/crs-projjson` should say "PROJJSON **CRS** object"
   (the schema's `definitions/crs`), and so should `geoparquet.md`. Done here.
2. `bad_data/epoch-on-unsupported-crs.parquet` carries a stub `crs` (`{"type": "GeographicCRS",
   "id": ...}`) that is not valid PROJJSON (no name, datum or coordinate system). The fixture wants to
   test only the epoch; it should carry a full PROJJSON. gpio does not notice because its
   `crs_valid` check only looks at `type`.
3. pyarrow 25 writes an unset Parquet `crs` when asked for `EPSG:4326` or `OGC:CRS84`; fine for
   the spec (both mean OGC:CRS84) but a test suite must treat "unset" and those two as equal.
4. `schema.json` reaches the PROJJSON schema through a remote `$ref`; validators must vendor it or
   they need the network at validation time.
5. DuckDB `ORDER BY ST_Hilbert(...)` on polygons produced a file whose first row group spans the
   whole extent (rows 0-51 200 came from everywhere); `gpio sort hilbert` sorts the same data
   correctly. Worth a look by whoever owns the DuckDB spatial example in the guide.

## Known limits

* Semantic CRS comparison (PROJJSON without `id`, `srid:<n>` with a registry): needs PROJ or a
  registry table; the prototype reports "cannot compare" instead. This is the one place where
  "pure Rust" costs something real.
* GeoArrow-encoded geometry columns are not read (GeoParquet 2.0 Core requires WKB, so the
  abstract tests do not need them either).
* A report format agreed with OGC CITE, packaging (cargo install, static binaries, a Python wheel
  via maturin if wanted), and one arrow pass for files with several geometry columns (today each
  geometry column is scanned separately).
* More unit tests; the fixture sets and their manifest are the regression suite today.
