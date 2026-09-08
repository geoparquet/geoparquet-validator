#!/usr/bin/env sh
# Public GeoParquet example files from other projects (1.0, 1.1, 2.0 and pre-1.0), downloaded into
# fixtures/out/public/ and checked. Not part of CI (network). Usage, from the repository root:
#   sh fixtures/public_samples.sh [path/to/geoparquet-validator]
set -e
BIN=${1:-./target/release/geoparquet-validator}
OUT=fixtures/out/public
mkdir -p "$OUT/spec" "$OUT/gdal" "$OUT/sedona" "$OUT/geoarrow-data"
for t in v1.0.0 v1.1.0 main; do
  curl -sL -o "$OUT/spec/example-$t.parquet" "https://raw.githubusercontent.com/opengeospatial/geoparquet/$t/examples/example.parquet"
done
for f in all_geoms bbox_similar_to_overturemaps_2024-04-16-beta.0 example geoparquet_1_1_no_explicit_crs_but_geoarrow_wkb_declared \
         gh_14610 overture_map_extract poly poly_geoarrow_polygon_not_geoparquet poly_wkb_large_binary poly_wkt_large_string \
         test test_geoparquet_1_1 test_single_group test_with_fid_and_geometry_bbox wkt_with_dict; do
  curl -sL -o "$OUT/gdal/$f.parquet" "https://raw.githubusercontent.com/OSGeo/gdal/master/autotest/ogr/data/parquet/$f.parquet"
done
for f in geoparquet-1.0.0 geoparquet-1.1.0 overture-bbox plain; do
  curl -sL -o "$OUT/sedona/$f.parquet" "https://raw.githubusercontent.com/apache/sedona-testing/main/data/parquet/$f.parquet"
done
for f in ns-water_water-point_geo ns-water_water-point_native ns-water_water-junc_geo; do
  curl -sL -o "$OUT/geoarrow-data/$f.parquet" "https://github.com/geoarrow/geoarrow-data/releases/download/v0.2.0/$f.parquet"
done
$BIN check "$OUT/" --max-files 50
