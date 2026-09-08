//! The abstract tests of the OGC GeoParquet 2.0 draft (ogc/abstract_tests), one function
//! per conformance class, run on a single file with nothing but the parquet crate.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use arrow_array::{Array, ArrayRef, BinaryArray, BinaryViewArray, LargeBinaryArray};
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::{ArrowReaderOptions, ParquetRecordBatchReaderBuilder};
use parquet::basic::{ConvertedType, LogicalType, Repetition, Type as PhysicalType};
use parquet::file::metadata::ColumnChunkMetaData;
use parquet::file::metadata::ParquetMetaData;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::file::statistics::Statistics;
use parquet::schema::types::Type as SchemaType;
use serde::Serialize;
use serde_json::Value;

use crate::crs::{self, Crs};
use crate::source::Source;
use crate::spatial;
use crate::wkb;

pub const GEO_METADATA: &str = "/conf/core/geo-metadata";
pub const FILE_METADATA: &str = "/conf/core/file-metadata";
pub const COLUMN_METADATA: &str = "/conf/core/column-metadata";
pub const GEOMETRY_COLUMN_TYPE: &str = "/conf/core/geometry-column-type";
pub const GEOMETRY_COLUMN_NESTING: &str = "/conf/core/geometry-column-nesting";
pub const GEOMETRY_COLUMN_REPETITION: &str = "/conf/core/geometry-column-repetition";
pub const WKB: &str = "/conf/core/wkb";
pub const GEOMETRY_TYPES: &str = "/conf/core/geometry-types";
pub const CRS_PROJJSON: &str = "/conf/core/crs-projjson";
pub const CRS_CONSISTENCY: &str = "/conf/core/crs-consistency";
pub const CRS_DEFAULT: &str = "/conf/core/crs-default";
pub const AXIS_ORDER: &str = "/conf/core/axis-order";
pub const EDGES_VALUE: &str = "/conf/core/edges-value";
pub const EPOCH: &str = "/conf/core/epoch";
pub const ORIENTATION_VALUE: &str = "/conf/core/orientation-value";
pub const ORIENTATION_RINGS: &str = "/conf/core/orientation-rings";
pub const BBOX_ARRAY: &str = "/conf/core/bbox-array";
pub const BBOX_CRS: &str = "/conf/core/bbox-crs";
pub const BBOX_EXTENT: &str = "/conf/core/bbox-extent";
pub const MEDIA_TYPE: &str = "/conf/core/media-type";
pub const COV_KEYS: &str = "/conf/covering/keys";
pub const COV_BBOX_PATHS: &str = "/conf/covering/bbox-paths";
pub const COV_STRUCTURE: &str = "/conf/covering/bbox-column-structure";
pub const COV_TYPE: &str = "/conf/covering/bbox-column-type";
pub const COV_REPETITION: &str = "/conf/covering/bbox-column-repetition";
pub const COV_NESTING: &str = "/conf/covering/bbox-column-nesting";
pub const DIST_STATS: &str = "/conf/distribution/geospatial-statistics";
pub const DIST_SPATIAL_ORDER: &str = "/conf/distribution/spatial-order";

pub const ALL_IDS: [&str; 28] = [
    GEO_METADATA,
    FILE_METADATA,
    COLUMN_METADATA,
    GEOMETRY_COLUMN_TYPE,
    GEOMETRY_COLUMN_NESTING,
    GEOMETRY_COLUMN_REPETITION,
    WKB,
    GEOMETRY_TYPES,
    CRS_PROJJSON,
    CRS_CONSISTENCY,
    CRS_DEFAULT,
    AXIS_ORDER,
    EDGES_VALUE,
    EPOCH,
    ORIENTATION_VALUE,
    ORIENTATION_RINGS,
    BBOX_ARRAY,
    BBOX_CRS,
    BBOX_EXTENT,
    MEDIA_TYPE,
    COV_KEYS,
    COV_BBOX_PATHS,
    COV_STRUCTURE,
    COV_TYPE,
    COV_REPETITION,
    COV_NESTING,
    DIST_STATS,
    DIST_SPATIAL_ORDER,
];
const COVERING_IDS: [&str; 6] = [
    COV_KEYS,
    COV_BBOX_PATHS,
    COV_STRUCTURE,
    COV_TYPE,
    COV_REPETITION,
    COV_NESTING,
];
const EDGES: [&str; 6] = [
    "planar",
    "spherical",
    "vincenty",
    "thomas",
    "andoyer",
    "karney",
];
const EDGES_1X: [&str; 2] = ["planar", "spherical"];
const GEOARROW: [&str; 6] = [
    "point",
    "linestring",
    "polygon",
    "multipoint",
    "multilinestring",
    "multipolygon",
];

/// Which GeoParquet specification the file claims; it decides which rules apply. The tests are
/// written for 2.0; 1.0 and 1.1 files are checked against their own version of the
/// specification, with the same test identifiers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spec {
    V1_0,
    V1_1,
    V2_0,
}

impl Spec {
    fn from_version(v: &str) -> Option<Spec> {
        if v.starts_with("2.0") {
            Some(Spec::V2_0)
        } else if v.starts_with("1.1") {
            Some(Spec::V1_1)
        } else if v.starts_with("1.0") {
            Some(Spec::V1_0)
        } else {
            None
        }
    }
    fn rules(self) -> &'static str {
        match self {
            Spec::V2_0 => "the GeoParquet 2.0.0 specification; the checks align with the OGC draft",
            Spec::V1_1 => "the GeoParquet 1.1.0 specification",
            Spec::V1_0 => "the GeoParquet 1.0.0 specification",
        }
    }
    fn versions(self) -> &'static [&'static str] {
        match self {
            Spec::V2_0 => &["2.0.0"],
            Spec::V1_1 => &["1.1.0"],
            Spec::V1_0 => &["1.0.0", "1.0.0-beta.1"],
        }
    }
    fn edges(self) -> &'static [&'static str] {
        match self {
            Spec::V2_0 => &EDGES,
            _ => &EDGES_1X,
        }
    }
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pass,
    Fail,
    Skip,
}

#[derive(Serialize, Debug)]
pub struct Outcome {
    pub id: &'static str,
    pub status: Status,
    pub message: String,
}

impl Outcome {
    pub fn class(&self) -> &str {
        self.id.split('/').nth(2).unwrap_or("")
    }
}

#[derive(Clone, Default)]
pub struct Options {
    /// Read only the first N rows (whole row groups); data tests are then sampled.
    pub max_rows: Option<usize>,
}

/// How a distribution practice looks on this file. Advice never decides conformance: the
/// Cloud-Optimized Distribution class states two requirements, and the rest are recommendations
/// of the GeoParquet distribution best practices.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Good,
    Consider,
    Poor,
}

#[derive(Serialize)]
pub struct Advice {
    pub topic: &'static str,
    pub level: Level,
    pub message: String,
}

#[derive(Serialize)]
pub struct Report {
    pub file: String,
    pub outcomes: Vec<Outcome>,
    /// (bytes fetched, range requests) for remote sources
    pub traffic: Option<(u64, u64)>,
    pub sampled: bool,
    /// the `version` the file declares ("unknown" when absent)
    pub version: String,
    /// which rules were applied, in words
    pub rules: String,
    /// how the distribution best practices look on this file (never a conformance verdict)
    pub advice: Vec<Advice>,
    /// the shape of this document; bumped when a field changes meaning or goes away
    pub report_version: u32,
    pub tool: Tool,
}

/// The program that wrote a report.
#[derive(Serialize, Clone, Copy)]
pub struct Tool {
    pub name: &'static str,
    pub version: &'static str,
}

impl Tool {
    pub const fn current() -> Tool {
        Tool {
            name: "geoparquet-validator",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

/// The shape of the JSON report; see `schemas/report.schema.json`.
pub const REPORT_VERSION: u32 = 1;

impl Report {
    pub fn failed(&self, class: &str) -> Vec<&'static str> {
        self.outcomes
            .iter()
            .filter(|o| o.class() == class && o.status == Status::Fail)
            .map(|o| o.id)
            .collect()
    }
    pub fn get(&self, id: &str) -> Option<&Outcome> {
        self.outcomes.iter().find(|o| o.id == id)
    }
}

pub struct Schemas {
    geo: [jsonschema::Validator; 3],
    crs: [jsonschema::Validator; 2],
}

impl Schemas {
    pub fn load() -> Result<Schemas> {
        let projjson7: Value =
            serde_json::from_str(include_str!("../schemas/projjson.schema.json"))?;
        let projjson5: Value =
            serde_json::from_str(include_str!("../schemas/projjson-v0.5.schema.json"))?;
        // The GeoParquet schemas refer to the PROJJSON schemas by URL; serve the vendored copies,
        // unmodified, so /conf/core/geo-metadata validates against the published schema offline.
        let registry = jsonschema::Registry::new()
            .add(
                "https://proj.org/schemas/v0.7/projjson.schema.json",
                jsonschema::Resource::from_contents(projjson7.clone()),
            )
            .and_then(|b| {
                b.add(
                    "https://proj.org/schemas/v0.5/projjson.schema.json",
                    jsonschema::Resource::from_contents(projjson5.clone()),
                )
            })
            .and_then(|b| b.prepare())
            .map_err(|e| anyhow!("schema registry: {e}"))?;
        let geo_validator = |text: &str| -> Result<jsonschema::Validator> {
            let schema: Value = serde_json::from_str(text)?;
            jsonschema::options()
                .with_registry(&registry)
                .build(&schema)
                .map_err(|e| anyhow!("geoparquet schema: {e}"))
        };
        // The PROJJSON schema also describes datums, ellipsoids and operations; a GeoParquet
        // `crs` must be a CRS, so /conf/core/crs-projjson validates against its `crs` definition.
        let crs_validator = |mut schema: Value| -> Result<jsonschema::Validator> {
            schema["oneOf"] = serde_json::json!([{ "$ref": "#/definitions/crs" }]);
            jsonschema::validator_for(&schema).map_err(|e| anyhow!("projjson schema: {e}"))
        };
        Ok(Schemas {
            geo: [
                geo_validator(include_str!("../schemas/geoparquet-1.0.0.json"))?,
                geo_validator(include_str!("../schemas/geoparquet-1.1.0.json"))?,
                geo_validator(include_str!("../schemas/geoparquet-2.0.0.json"))?,
            ],
            crs: [crs_validator(projjson5)?, crs_validator(projjson7)?],
        })
    }
    fn geo(&self, spec: Spec) -> &jsonschema::Validator {
        &self.geo[match spec {
            Spec::V1_0 => 0,
            Spec::V1_1 => 1,
            Spec::V2_0 => 2,
        }]
    }
    fn crs(&self, spec: Spec) -> &jsonschema::Validator {
        &self.crs[usize::from(spec != Spec::V1_0)]
    }
}

fn schema_errors(v: &jsonschema::Validator, inst: &Value, max: usize) -> Vec<String> {
    v.iter_errors(inst)
        .take(max)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect()
}

/// Accumulates one abstract test's verdict across columns.
struct T {
    id: &'static str,
    fails: Vec<String>,
    notes: Vec<String>,
    applicable: bool,
}

impl T {
    fn new(id: &'static str) -> T {
        T {
            id,
            fails: Vec::new(),
            notes: Vec::new(),
            applicable: false,
        }
    }
    fn ok(&mut self) {
        self.applicable = true;
    }
    fn fail(&mut self, m: impl Into<String>) {
        self.applicable = true;
        self.fails.push(m.into());
    }
    fn note(&mut self, m: impl Into<String>) {
        self.notes.push(m.into());
    }
    fn finish(self) -> Outcome {
        let (status, message) = if !self.fails.is_empty() {
            (Status::Fail, self.fails.join("; "))
        } else if self.applicable {
            (Status::Pass, self.notes.join("; "))
        } else if self.notes.is_empty() {
            (Status::Skip, "not applicable".to_string())
        } else {
            (Status::Skip, self.notes.join("; "))
        };
        Outcome {
            id: self.id,
            status,
            message,
        }
    }
}

fn skip(id: &'static str, m: impl Into<String>) -> Outcome {
    Outcome {
        id,
        status: Status::Skip,
        message: m.into(),
    }
}

fn logical(t: &SchemaType) -> Option<LogicalType> {
    t.get_basic_info().logical_type_ref().cloned()
}

fn is_geo_logical(lt: &Option<LogicalType>) -> bool {
    matches!(
        lt,
        Some(LogicalType::Geometry(_)) | Some(LogicalType::Geography(_))
    )
}

fn parquet_crs(lt: &Option<LogicalType>) -> Option<String> {
    match lt {
        Some(LogicalType::Geometry(g)) => g.crs.clone(),
        Some(LogicalType::Geography(g)) => g.crs.clone(),
        _ => None,
    }
}

/// Whether a field named `name` exists anywhere below the root (in a group), i.e. is nested.
fn schema_has_nested(fields: &[Arc<SchemaType>], name: &str) -> bool {
    fields.iter().any(|f| {
        f.is_group()
            && f.get_fields().iter().any(|c| {
                c.name() == name
                    || (c.is_group() && schema_has_nested(std::slice::from_ref(c), name))
            })
    })
}

fn valid_type_name(s: &str, spec: Spec) -> bool {
    let base = if spec == Spec::V2_0 {
        s.strip_suffix(" ZM")
            .or_else(|| s.strip_suffix(" Z"))
            .or_else(|| s.strip_suffix(" M"))
            .unwrap_or(s)
    } else {
        s.strip_suffix(" Z").unwrap_or(s)
    };
    wkb::BASE_NAMES.contains(&base)
}

/// GeoArrow encodings of GeoParquet 1.1: a struct of DOUBLE x, y (and z), wrapped in as many
/// Parquet lists as the geometry type has levels (linestring 1, polygon 2, multipolygon 3).
fn geoarrow_shape(f: &SchemaType, encoding: &str) -> Result<(), String> {
    let want = match encoding {
        "point" => 0,
        "linestring" | "multipoint" => 1,
        "polygon" | "multilinestring" => 2,
        "multipolygon" => 3,
        _ => return Err(format!("unknown GeoArrow encoding {encoding:?}")),
    };
    let mut cur = f;
    let mut depth = 0;
    loop {
        if !cur.is_group() {
            return Err(format!(
                "GeoArrow encoding {encoding:?} needs a group column, found a primitive"
            ));
        }
        let is_list = matches!(logical(cur), Some(LogicalType::List))
            || cur.get_basic_info().converted_type() == ConvertedType::LIST;
        if !is_list {
            break;
        }
        let inner = cur.get_fields().first().ok_or("empty list group")?;
        let elem = if inner.is_group()
            && !matches!(logical(inner), Some(LogicalType::List))
            && inner.get_fields().len() == 1
        {
            &inner.get_fields()[0]
        } else {
            inner
        };
        cur = elem;
        depth += 1;
        if depth > 4 {
            return Err("more than four list levels".into());
        }
    }
    if depth != want {
        return Err(format!(
            "GeoArrow encoding {encoding:?} needs {want} list level(s), found {depth}"
        ));
    }
    let names: Vec<&str> = cur.get_fields().iter().map(|c| c.name()).collect();
    if !(names == ["x", "y"] || names == ["x", "y", "z"])
        || cur
            .get_fields()
            .iter()
            .any(|c| c.is_group() || c.get_physical_type() != PhysicalType::DOUBLE)
    {
        return Err(format!(
            "GeoArrow coordinates must be a struct of DOUBLE x, y[, z]; found {names:?}"
        ));
    }
    Ok(())
}

/// Bytes as something a person reads.
fn human(bytes: i64) -> String {
    let b = bytes.max(0) as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.0} kB", b / 1e3)
    }
}

fn stat_min_max(cc: &ColumnChunkMetaData) -> (Option<f64>, Option<f64>) {
    match cc.statistics() {
        Some(Statistics::Double(s)) => (s.min_opt().copied(), s.max_opt().copied()),
        Some(Statistics::Float(s)) => (
            s.min_opt().map(|v| f64::from(*v)),
            s.max_opt().map(|v| f64::from(*v)),
        ),
        _ => (None, None),
    }
}

/// Row-group bounding boxes from the Parquet column statistics of a bbox covering column, the
/// statistics source of 1.x files (and of 2.0 files written without native statistics).
fn covering_boxes(meta: &ParquetMetaData, bcol: &str) -> Vec<Option<[f64; 4]>> {
    (0..meta.num_row_groups())
        .map(|rg| {
            let mut v: [Option<f64>; 4] = [None; 4];
            for cc in meta.row_group(rg).columns() {
                let parts = cc.column_path().parts();
                if parts.len() == 2 && parts[0] == bcol {
                    let (min, max) = stat_min_max(cc);
                    match parts[1].as_str() {
                        "xmin" => v[0] = min,
                        "ymin" => v[1] = min,
                        "xmax" => v[2] = max,
                        "ymax" => v[3] = max,
                        _ => {}
                    }
                }
            }
            match v {
                [Some(a), Some(b), Some(c), Some(d)] => Some([a, b, c, d]),
                _ => None,
            }
        })
        .collect()
}

fn bbox_nums(col: &Value) -> Option<Vec<f64>> {
    let a = col.get("bbox")?.as_array()?;
    if ![4, 6, 8].contains(&a.len()) {
        return None;
    }
    a.iter().map(Value::as_f64).collect()
}

fn geographic(col: &Value) -> bool {
    crs::is_geographic(&Crs::from_geo(col), col.get("crs"))
}

/// `covering.bbox` -> the bounding box column it names, or why it cannot be resolved.
fn resolve_bbox_paths(cov: &Value) -> Result<String, String> {
    let bbox = cov.get("bbox").ok_or("covering has no `bbox`")?;
    let obj = bbox.as_object().ok_or("covering.bbox is not an object")?;
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    if keys != ["xmax", "xmin", "ymax", "ymin"] {
        return Err(format!(
            "covering.bbox members are {keys:?}, expected exactly xmin, ymin, xmax, ymax"
        ));
    }
    let mut column: Option<&str> = None;
    for (k, v) in obj {
        let arr = v
            .as_array()
            .filter(|a| a.len() == 2 && a.iter().all(Value::is_string))
            .ok_or_else(|| format!("covering.bbox.{k} is not an array of two strings"))?;
        let (first, second) = (arr[0].as_str().unwrap(), arr[1].as_str().unwrap());
        if second != k {
            return Err(format!(
                "covering.bbox.{k} second element is \"{second}\", expected \"{k}\""
            ));
        }
        match column {
            None => column = Some(first),
            Some(c) if c != first => {
                return Err(format!(
                    "covering.bbox paths name different columns (\"{c}\" and \"{first}\")"
                ));
            }
            _ => {}
        }
    }
    Ok(column.unwrap().to_string())
}

#[derive(Default)]
struct Scan {
    rows: usize,
    nulls: usize,
    wkb_errors: Vec<String>,
    types: BTreeSet<String>,
    lonlat_bad: usize,
    outside_bbox: usize,
    rings_checked: usize,
    rings_bad: usize,
    bbox_rep_checked: bool,
    bbox_rep_mismatch: usize,
    total_rows: i64,
    sampled: bool,
    /// vertices with x >= bbox xmin / x <= bbox xmax, counted when the declared bbox wraps
    wrap_east: usize,
    wrap_west: usize,
}

fn binary_at(arr: &ArrayRef, i: usize) -> Result<Option<&[u8]>> {
    if arr.is_null(i) {
        return Ok(None);
    }
    if let Some(a) = arr.as_any().downcast_ref::<BinaryArray>() {
        return Ok(Some(a.value(i)));
    }
    if let Some(a) = arr.as_any().downcast_ref::<LargeBinaryArray>() {
        return Ok(Some(a.value(i)));
    }
    if let Some(a) = arr.as_any().downcast_ref::<BinaryViewArray>() {
        return Ok(Some(a.value(i)));
    }
    Err(anyhow!(
        "column is not a binary array ({:?})",
        arr.data_type()
    ))
}

fn inside_bbox(bb: &[f64], g: &wkb::Geom) -> bool {
    let half = bb.len() / 2;
    let (xmin, ymin, xmax, ymax) = (bb[0], bb[1], bb[half], bb[half + 1]);
    let Some((y0, y1)) = g.range[1] else {
        return true;
    };
    if y0 < ymin || y1 > ymax {
        return false;
    }
    let (x0, x1) = g.range[0].unwrap();
    if xmin <= xmax {
        if x0 < xmin || x1 > xmax {
            return false;
        }
    } else if !g.xy.iter().all(|(x, _)| *x >= xmin || *x <= xmax) {
        return false; // antimeridian: RFC 7946 section 5
    }
    let (z, m) = match bb.len() {
        6 => (Some((bb[2], bb[5])), None),
        8 => (Some((bb[2], bb[6])), Some((bb[3], bb[7]))),
        _ => (None, None),
    };
    for (bounds, range) in [(z, g.range[2]), (m, g.range[3])] {
        if let (Some((lo, hi)), Some((g0, g1))) = (bounds, range)
            && (g0 < lo || g1 > hi)
        {
            return false;
        }
    }
    true
}

fn scan_column<S: Source>(
    src: &S,
    name: &str,
    bbox_col: Option<&str>,
    declared_bbox: Option<&[f64]>,
    geodesic: bool,
    max_rows: Option<usize>,
) -> Result<Scan> {
    // Ignore the writer's Arrow schema hint so a dictionary- or string-hinted BYTE_ARRAY still
    // arrives as a plain BinaryArray.
    let mut builder = ParquetRecordBatchReaderBuilder::try_new_with_options(
        src.open()?,
        ArrowReaderOptions::new().with_skip_arrow_metadata(true),
    )?;
    let schema = builder.parquet_schema();
    let root = schema.root_schema().get_fields();
    let mut idx = vec![
        root.iter()
            .position(|f| f.name() == name)
            .ok_or_else(|| anyhow!("no root column {name}"))?,
    ];
    let bbox_col = bbox_col.filter(|b| {
        if let Some(i) = root.iter().position(|f| f.name() == *b) {
            idx.push(i);
            true
        } else {
            false
        }
    });
    let mask = ProjectionMask::roots(schema, idx.clone());
    let total_rows = builder.metadata().file_metadata().num_rows();
    let mut groups: Vec<usize> = (0..builder.metadata().num_row_groups()).collect();
    if let Some(max) = max_rows {
        let mut acc = 0i64;
        groups.clear();
        for (i, rg) in builder.metadata().row_groups().iter().enumerate() {
            if acc >= max as i64 {
                break;
            }
            groups.push(i);
            acc += rg.num_rows();
        }
    }
    // Tell the source which column chunks the scan will read (all leaves under the projected roots).
    let roots: Vec<&str> = idx.iter().map(|i| root[*i].name()).collect();
    let mut ranges = Vec::new();
    for rg in &groups {
        for cc in builder.metadata().row_group(*rg).columns() {
            if cc
                .column_path()
                .parts()
                .first()
                .is_some_and(|p| roots.contains(&p.as_str()))
            {
                ranges.push(cc.byte_range());
            }
        }
    }
    src.hint_ranges(ranges);
    if max_rows.is_some() {
        builder = builder.with_row_groups(groups);
    }
    let reader = builder
        .with_projection(mask)
        .with_batch_size(16 * 1024)
        .build()?;
    let mut sc = Scan {
        total_rows,
        ..Scan::default()
    };
    'rows: for batch in reader {
        let batch = batch?;
        let geom = batch
            .column_by_name(name)
            .ok_or_else(|| anyhow!("column {name} missing from batch"))?;
        let bbox_arr = bbox_col.and_then(|b| batch.column_by_name(b));
        for i in 0..batch.num_rows() {
            if max_rows.is_some_and(|m| sc.rows >= m) {
                break 'rows;
            }
            sc.rows += 1;
            let value = binary_at(geom, i)?;
            if let Some(b) = bbox_arr {
                sc.bbox_rep_checked = true;
                if b.is_null(i) != value.is_none() {
                    sc.bbox_rep_mismatch += 1;
                }
            }
            let Some(bytes) = value else {
                sc.nulls += 1;
                continue;
            };
            let g = match wkb::parse(bytes) {
                Ok(g) => g,
                Err(e) => {
                    if sc.wkb_errors.len() < 5 {
                        sc.wkb_errors.push(format!("row {}: {e}", sc.rows - 1));
                    }
                    continue;
                }
            };
            sc.types.insert(g.type_name());
            if g.xy
                .iter()
                .any(|(x, y)| !(-180.0..=180.0).contains(x) || !(-90.0..=90.0).contains(y))
            {
                sc.lonlat_bad += 1;
            }
            if let Some(bb) = declared_bbox {
                if !inside_bbox(bb, &g) {
                    sc.outside_bbox += 1;
                }
                let half = bb.len() / 2;
                if bb[0] > bb[half] {
                    sc.wrap_east += g.xy.iter().filter(|(x, _)| *x >= bb[0]).count();
                    sc.wrap_west += g.xy.iter().filter(|(x, _)| *x <= bb[half]).count();
                }
            }
            for rings in &g.polygons {
                for (k, (s, n)) in rings.iter().enumerate() {
                    if *n < 4 {
                        continue;
                    }
                    let pts = &g.xy[*s..*s + *n];
                    let a = wkb::signed_area2(pts);
                    if a == 0.0 {
                        continue;
                    }
                    if geodesic {
                        let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                            (lo.min(p.0), hi.max(p.0))
                        });
                        if hi - lo > 180.0 {
                            continue;
                        }
                    }
                    sc.rings_checked += 1;
                    if (k == 0) != (a > 0.0) {
                        sc.rings_bad += 1;
                    }
                }
            }
        }
    }
    sc.sampled = (sc.rows as i64) < total_rows;
    Ok(sc)
}

fn chunk<'a>(
    meta: &'a ParquetMetaData,
    rg: usize,
    name: &str,
) -> Option<&'a parquet::file::metadata::ColumnChunkMetaData> {
    meta.row_groups()[rg]
        .columns()
        .iter()
        .find(|c| c.column_path().parts().first().map(String::as_str) == Some(name))
}

pub fn run<S: Source>(src: &S, schemas: &Schemas, opts: &Options) -> Result<Report> {
    let mut out: Vec<Outcome> = Vec::new();
    let file = src.describe();
    let sampled = std::cell::Cell::new(false);
    let version = std::cell::RefCell::new(String::from("unknown"));
    let rules = std::cell::RefCell::new(String::from(Spec::V2_0.rules()));
    let advice: std::cell::RefCell<Vec<Advice>> = std::cell::RefCell::new(Vec::new());
    let finish = |mut out: Vec<Outcome>, reason: &str| {
        for id in ALL_IDS {
            if !out.iter().any(|o| o.id == id) {
                out.push(skip(id, reason));
            }
        }
        out.sort_by_key(|o| ALL_IDS.iter().position(|i| *i == o.id).unwrap_or(99));
        Report {
            file: file.clone(),
            outcomes: out,
            traffic: src.traffic(),
            sampled: sampled.get(),
            version: version.borrow().clone(),
            rules: rules.borrow().clone(),
            advice: advice.borrow_mut().drain(..).collect(),
            report_version: REPORT_VERSION,
            tool: Tool::current(),
        }
    };

    let handle = src.open()?; // I/O errors are the tool's problem, not the file's
    let reader = match SerializedFileReader::new(handle) {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            let message = if msg.to_ascii_lowercase().contains("utf-8") {
                format!("a key/value metadata string (`geo`?) is not valid UTF-8: {msg}")
            } else {
                format!("Parquet footer cannot be read: {msg}")
            };
            out.push(Outcome {
                id: GEO_METADATA,
                status: Status::Fail,
                message,
            });
            return Ok(finish(out, "file not readable"));
        }
    };
    let meta = reader.metadata();
    let mut kv: HashMap<String, String> = HashMap::new();
    if let Some(list) = meta.file_metadata().key_value_metadata() {
        for e in list {
            kv.insert(e.key.clone(), e.value.clone().unwrap_or_default());
        }
    }

    // /conf/core/geo-metadata
    let mut t = T::new(GEO_METADATA);
    let mut spec = Spec::V2_0;
    let geo = match kv.get("geo") {
        None => {
            t.fail("no `geo` key in FileMetaData.key_value_metadata");
            None
        }
        Some(s) => match serde_json::from_str::<Value>(s) {
            Ok(v @ Value::Object(_)) => {
                match v.get("version").and_then(Value::as_str) {
                    Some(ver) => {
                        *version.borrow_mut() = ver.to_string();
                        match Spec::from_version(ver) {
                            Some(sp) => spec = sp,
                            None => t.note(format!(
                                "version \"{ver}\" is not a known GeoParquet version; validated as 2.0.0"
                            )),
                        }
                    }
                    None => t.note("no version member; validated as 2.0.0"),
                }
                *rules.borrow_mut() = spec.rules().to_string();
                let errs = schema_errors(schemas.geo(spec), &v, 5);
                if errs.is_empty() {
                    t.ok();
                }
                for e in errs {
                    t.fail(format!("JSON Schema: {e}"));
                }
                Some(v)
            }
            Ok(_) => {
                t.fail("`geo` parses as JSON but is not an object");
                None
            }
            Err(e) => {
                t.fail(format!("`geo` does not parse as JSON: {e}"));
                None
            }
        },
    };
    out.push(t.finish());
    let Some(geo) = geo else {
        return Ok(finish(out, "no usable geo metadata"));
    };

    // /conf/core/file-metadata
    let mut t = T::new(FILE_METADATA);
    match geo.get("version").and_then(Value::as_str) {
        Some(v) if spec.versions().contains(&v) => {
            t.ok();
            if v != spec.versions()[0] {
                t.note(format!("version \"{v}\" is a pre-release string"));
            }
        }
        Some(v) => t.fail(format!(
            "version is \"{v}\", expected {}",
            spec.versions()
                .iter()
                .map(|x| format!("\"{x}\""))
                .collect::<Vec<_>>()
                .join(" or ")
        )),
        None => t.fail("version missing or not a string"),
    }
    let primary = geo
        .get("primary_column")
        .and_then(Value::as_str)
        .unwrap_or("");
    if primary.is_empty() {
        t.fail("primary_column missing or empty");
    }
    let columns: BTreeMap<String, Value> = match geo.get("columns").and_then(Value::as_object) {
        Some(m) if !m.is_empty() => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => {
            t.fail("columns missing, not an object, or empty");
            BTreeMap::new()
        }
    };
    out.push(t.finish());

    let root: Vec<Arc<SchemaType>> = meta
        .file_metadata()
        .schema_descr()
        .root_schema()
        .get_fields()
        .to_vec();
    let mut root_by_name: HashMap<&str, &Arc<SchemaType>> = HashMap::new();
    for f in &root {
        root_by_name.entry(f.name()).or_insert(f);
    }

    // /conf/core/column-metadata
    let mut t = T::new(COLUMN_METADATA);
    t.ok();
    for f in &root {
        if f.is_primitive() && is_geo_logical(&logical(f)) && !columns.contains_key(f.name()) {
            t.fail(format!("root column `{}` has a GEOMETRY/GEOGRAPHY logical type but is not listed in `columns`", f.name()));
        }
    }
    if !primary.is_empty() && !columns.contains_key(primary) {
        t.fail(format!(
            "primary_column `{primary}` is not a member of `columns`"
        ));
    }
    for (name, col) in &columns {
        if !col.is_object() {
            t.fail(format!("columns.{name} is not an object"));
            continue;
        }
        if col.get("encoding").is_none() {
            t.fail(format!("columns.{name} has no `encoding`"));
        }
        if col.get("geometry_types").is_none() {
            t.fail(format!("columns.{name} has no `geometry_types`"));
        }
    }
    out.push(t.finish());
    let columns: BTreeMap<String, Value> =
        columns.into_iter().filter(|(_, c)| c.is_object()).collect();

    // schema-level tests per geometry column
    let mut t_type = T::new(GEOMETRY_COLUMN_TYPE);
    let mut t_nest = T::new(GEOMETRY_COLUMN_NESTING);
    let mut t_rep = T::new(GEOMETRY_COLUMN_REPETITION);
    let mut geom_fields: BTreeMap<&str, &Arc<SchemaType>> = BTreeMap::new();
    for (name, col) in &columns {
        let Some(f) = root_by_name.get(name.as_str()) else {
            if schema_has_nested(&root, name) {
                t_nest.fail(format!(
                    "`{name}` is nested below the root of the Parquet schema"
                ));
            } else {
                t_nest.note(format!(
                    "`{name}`: no schema element of that name (the specification does not require one)"
                ));
            }
            continue;
        };
        let encoding = col.get("encoding").and_then(Value::as_str);
        let geoarrow = spec != Spec::V2_0 && encoding.is_some_and(|e| GEOARROW.contains(&e));
        if f.is_group() && !geoarrow {
            t_nest.fail(format!("`{name}` is a group field"));
            continue;
        }
        t_nest.ok();
        geom_fields.insert(name, f);
        t_type.ok();
        if geoarrow {
            let enc = encoding.unwrap_or_default();
            if spec == Spec::V1_0 {
                t_type.fail(format!(
                    "`{name}`: encoding {enc:?} is a GeoArrow encoding, which 1.0.0 does not define"
                ));
            }
            if let Err(e) = geoarrow_shape(f, enc) {
                t_type.fail(format!("`{name}`: {e}"));
            }
        } else {
            if f.get_physical_type() != PhysicalType::BYTE_ARRAY {
                t_type.fail(format!(
                    "`{name}`: primitive type is {:?}, expected BYTE_ARRAY",
                    f.get_physical_type()
                ));
            }
            let lt = logical(f);
            if spec == Spec::V2_0 {
                if !is_geo_logical(&lt) {
                    t_type.fail(format!(
                        "`{name}`: logical type is {}, expected GEOMETRY or GEOGRAPHY",
                        lt.map(|l| format!("{l:?}")).unwrap_or("none".into())
                    ));
                }
            } else if is_geo_logical(&lt) {
                t_type.note(format!(
                    "`{name}`: carries a Parquet GEOMETRY/GEOGRAPHY type although the file declares {}",
                    spec.versions()[0]
                ));
            }
            let expected = if spec == Spec::V1_1 {
                "\"WKB\" or a GeoArrow encoding"
            } else {
                "\"WKB\""
            };
            match col.get("encoding") {
                Some(Value::String(e)) if e == "WKB" => {}
                Some(v) => t_type.fail(format!("`{name}`: encoding is {v}, expected {expected}")),
                None => t_type.fail(format!(
                    "`{name}`: encoding is missing, expected {expected}"
                )),
            }
        }
        let bi = f.get_basic_info();
        if !bi.has_repetition() {
            t_rep.fail(format!("`{name}` has no repetition"));
        } else if bi.repetition() == Repetition::REPEATED {
            t_rep.fail(format!("`{name}` is REPEATED"));
        } else {
            t_rep.ok();
        }
    }
    out.push(t_type.finish());
    out.push(t_nest.finish());
    out.push(t_rep.finish());

    // one pass over the data of every geometry column that is a root BYTE_ARRAY
    let mut scans: BTreeMap<&str, Scan> = BTreeMap::new();
    let mut scan_errors: BTreeMap<&str, String> = BTreeMap::new();
    let mut not_wkb: BTreeMap<&str, String> = BTreeMap::new();
    for (name, f) in &geom_fields {
        if f.is_group() {
            not_wkb.insert(
                name,
                "GeoArrow-encoded column; this checker decodes WKB only, so the data tests are not run".into(),
            );
            continue;
        }
        if f.get_physical_type() != PhysicalType::BYTE_ARRAY {
            continue;
        }
        let col = &columns[*name];
        let bbox = bbox_nums(col);
        let bbox_col = col.get("covering").and_then(|c| resolve_bbox_paths(c).ok());
        let geodesic = col
            .get("edges")
            .and_then(Value::as_str)
            .is_some_and(|e| e != "planar");
        match scan_column(
            src,
            name,
            bbox_col.as_deref(),
            bbox.as_deref(),
            geodesic,
            opts.max_rows,
        ) {
            Ok(sc) => {
                if sc.sampled {
                    sampled.set(true);
                }
                scans.insert(name, sc);
            }
            Err(e) => {
                scan_errors.insert(name, format!("{e:#}"));
            }
        }
    }

    let unread = |name: &str| -> Option<String> {
        scan_errors
            .get(name)
            .cloned()
            .or_else(|| not_wkb.get(name).cloned())
    };

    // /conf/core/wkb
    let mut t = T::new(WKB);
    for (name, e) in &scan_errors {
        t.fail(format!("`{name}`: cannot read column: {e}"));
    }
    for (name, e) in &not_wkb {
        t.note(format!("`{name}`: {e}"));
    }
    for (name, sc) in &scans {
        t.ok();
        for e in &sc.wkb_errors {
            t.fail(format!("`{name}`: {e}"));
        }
        let sample = if sc.sampled {
            format!(" (first {} of {} rows)", sc.rows, sc.total_rows)
        } else {
            String::new()
        };
        t.note(format!(
            "`{name}`: {} values decoded, {} null{sample}",
            sc.rows - sc.nulls,
            sc.nulls
        ));
    }
    out.push(t.finish());

    // /conf/core/geometry-types
    let mut t = T::new(GEOMETRY_TYPES);
    for (name, col) in &columns {
        let Some(list) = col.get("geometry_types").and_then(Value::as_array) else {
            continue;
        };
        t.ok();
        let mut declared: BTreeSet<String> = BTreeSet::new();
        for v in list {
            match v.as_str() {
                Some(s) if valid_type_name(s, spec) => {
                    if !declared.insert(s.to_string()) {
                        t.fail(format!("`{name}`: geometry_types lists \"{s}\" twice"));
                    }
                }
                Some(s) => t.fail(format!(
                    "`{name}`: \"{s}\" is not a valid geometry type string"
                )),
                None => t.fail(format!("`{name}`: geometry_types contains a non-string")),
            }
        }
        if declared.is_empty() {
            t.note(format!(
                "`{name}`: geometry_types is empty, data not constrained"
            ));
            continue;
        }
        if let Some(sc) = scans.get(name.as_str()) {
            for ty in &sc.types {
                if !declared.contains(ty) {
                    t.fail(format!(
                        "`{name}`: data contains {ty}, which is not in geometry_types {:?}",
                        declared
                    ));
                }
            }
        } else if let Some(e) = unread(name.as_str()) {
            t.note(format!(
                "`{name}`: data not read ({e}); types in the data not verified"
            ));
        }
        for rg in 0..meta.num_row_groups() {
            let Some(codes) = chunk(meta, rg, name)
                .and_then(|c| c.geo_statistics())
                .and_then(|s| s.geospatial_types())
            else {
                continue;
            };
            for c in codes {
                match wkb::type_name(*c as u32) {
                    Some(n) if declared.contains(&n) => {}
                    Some(n) => t.fail(format!("`{name}`: row group {rg} statistics list {n} (code {c}), not in geometry_types")),
                    None => t.fail(format!("`{name}`: row group {rg} statistics list unknown code {c}")),
                }
            }
        }
    }
    out.push(t.finish());

    // /conf/core/crs-projjson
    let mut t = T::new(CRS_PROJJSON);
    for (name, col) in &columns {
        match col.get("crs") {
            None | Some(Value::Null) => {}
            Some(v @ Value::Object(_)) => {
                t.ok();
                for e in schema_errors(schemas.crs(spec), v, 3) {
                    t.fail(format!("`{name}`: PROJJSON schema: {e}"));
                }
            }
            Some(v) => t.fail(format!(
                "`{name}`: crs is {} , expected a PROJJSON object or null",
                v
            )),
        }
    }
    out.push(t.finish());

    // /conf/core/crs-consistency and /conf/core/crs-default
    let mut t = T::new(CRS_CONSISTENCY);
    let mut td = T::new(CRS_DEFAULT);
    for (name, f) in &geom_fields {
        let col = &columns[*name];
        let lt = logical(f);
        // 1.x files have no Parquet crs parameter unless written with native types
        let parquet =
            is_geo_logical(&lt).then(|| Crs::from_parquet(parquet_crs(&lt).as_deref(), &kv));
        if let Some(b) = &parquet {
            let a = Crs::from_geo(col);
            match (&a, b) {
                (Crs::Undefined, Crs::Undefined) => t.ok(),
                (Crs::Authority(..), Crs::Authority(..)) if a == *b => t.ok(),
                (Crs::Authority(..), Crs::Authority(..))
                | (Crs::Undefined, _)
                | (_, Crs::Undefined) => t.fail(format!(
                    "`{name}`: geo crs is {} but Parquet crs is {}",
                    a.describe(),
                    b.describe()
                )),
                _ => t.note(format!(
                    "`{name}`: cannot compare {} with {} without a CRS library",
                    a.describe(),
                    b.describe()
                )),
            }
        }
        if col.get("crs").is_none() {
            match &parquet {
                Some(b) if *b == Crs::crs84() => td.ok(),
                Some(b @ (Crs::Authority(..) | Crs::Undefined)) => td.fail(format!(
                    "`{name}`: no geo crs (default OGC:CRS84) but Parquet crs is {}",
                    b.describe()
                )),
                Some(b) => td.note(format!(
                    "`{name}`: Parquet crs is {}; cannot compare with OGC:CRS84 without a CRS library",
                    b.describe()
                )),
                None => {}
            }
            if let Some(sc) = scans.get(name) {
                td.ok();
                if sc.lonlat_bad > 0 {
                    td.fail(format!(
                        "`{name}`: {} geometries have coordinates outside [-180,180]x[-90,90]",
                        sc.lonlat_bad
                    ));
                }
            } else if let Some(e) = unread(name) {
                td.note(format!(
                    "`{name}`: data not read ({e}); coordinate ranges not verified"
                ));
            }
        }
    }
    if spec != Spec::V2_0 && !t.applicable {
        t.note("GeoParquet 1.x has no Parquet crs parameter to compare with the geo crs");
    }
    out.push(t.finish());
    out.push(td.finish());

    // /conf/core/axis-order (heuristic; applies to CRSs whose first axis is latitude)
    let mut t = T::new(AXIS_ORDER);
    for (name, col) in &columns {
        match crs::lat_lon_order(col) {
            Some(true) => {
                if let Some(sc) = scans.get(name.as_str()) {
                    t.ok();
                    if sc.lonlat_bad > 0 {
                        t.fail(format!("`{name}`: {} geometries have first/second coordinates outside longitude/latitude range", sc.lonlat_bad));
                    }
                } else if let Some(e) = unread(name.as_str()) {
                    t.note(format!("`{name}`: data not read ({e})"));
                }
            }
            Some(false) => {
                t.ok();
                t.note(format!(
                    "`{name}`: easting-northing (or undefined) axis order, trivially satisfied"
                ));
            }
            None => t.note(format!(
                "`{name}`: axis order of the CRS cannot be determined; not checked"
            )),
        }
    }
    out.push(t.finish());

    // simple member tests
    let mut t = T::new(EDGES_VALUE);
    let mut te = T::new(EPOCH);
    let mut to = T::new(ORIENTATION_VALUE);
    for (name, col) in &columns {
        if let Some(v) = col.get("edges") {
            t.ok();
            if !v.as_str().is_some_and(|s| spec.edges().contains(&s)) {
                t.fail(format!(
                    "`{name}`: edges is {v}, expected one of {:?}",
                    spec.edges()
                ));
            }
        }
        if let Some(v) = col.get("epoch") {
            te.ok();
            if !v.is_number() {
                te.fail(format!("`{name}`: epoch is {v}, expected a number"));
            }
        }
        if let Some(v) = col.get("orientation") {
            to.ok();
            if v.as_str() != Some("counterclockwise") {
                to.fail(format!(
                    "`{name}`: orientation is {v}, expected \"counterclockwise\""
                ));
            }
        }
    }
    out.push(t.finish());
    out.push(te.finish());
    out.push(to.finish());

    // /conf/core/orientation-rings
    let mut t = T::new(ORIENTATION_RINGS);
    for (name, col) in &columns {
        if col.get("orientation").is_none() {
            continue;
        }
        let Some(sc) = scans.get(name.as_str()) else {
            if let Some(e) = unread(name.as_str()) {
                t.note(format!("`{name}`: data not read ({e})"));
            }
            continue;
        };
        t.ok();
        if sc.rings_bad > 0 {
            t.fail(format!(
                "`{name}`: {} of {} rings violate counterclockwise exterior / clockwise interior",
                sc.rings_bad, sc.rings_checked
            ));
        } else {
            t.note(format!("`{name}`: {} rings checked", sc.rings_checked));
        }
    }
    out.push(t.finish());

    // /conf/core/bbox-array, bbox-crs, bbox-extent
    let mut ta = T::new(BBOX_ARRAY);
    let mut tc = T::new(BBOX_CRS);
    let mut tx = T::new(BBOX_EXTENT);
    for (name, col) in &columns {
        let Some(b) = col.get("bbox") else { continue };
        ta.ok();
        let Some(arr) = b.as_array() else {
            ta.fail(format!("`{name}`: bbox is not an array"));
            continue;
        };
        if ![4, 6, 8].contains(&arr.len()) {
            ta.fail(format!(
                "`{name}`: bbox has {} elements, expected 4, 6 or 8",
                arr.len()
            ));
            continue;
        }
        let Some(nums) = bbox_nums(col) else {
            ta.fail(format!("`{name}`: bbox contains non-numbers"));
            continue;
        };
        let half = nums.len() / 2;
        let (xmin, ymin, xmax, ymax) = (nums[0], nums[1], nums[half], nums[half + 1]);
        if geographic(col) {
            if ymin > ymax {
                ta.fail(format!("`{name}`: bbox ymin {ymin} > ymax {ymax}"));
            }
            if xmin > xmax {
                match scans.get(name.as_str()) {
                    Some(sc) if sc.wrap_east == 0 || sc.wrap_west == 0 => ta.fail(format!(
                        "`{name}`: bbox xmin {xmin} > xmax {xmax} but the data does not cross the antimeridian ({} vertices at x >= xmin, {} at x <= xmax)",
                        sc.wrap_east, sc.wrap_west
                    )),
                    _ => ta.note(format!(
                        "`{name}`: bbox crosses the antimeridian (xmin {xmin} > xmax {xmax})"
                    )),
                }
            }
            tc.ok();
            if !(-180.0..=180.0).contains(&xmin)
                || !(-180.0..=180.0).contains(&xmax)
                || !(-90.0..=90.0).contains(&ymin)
                || !(-90.0..=90.0).contains(&ymax)
            {
                tc.fail(format!(
                    "`{name}`: bbox {nums:?} is outside longitude/latitude range"
                ));
            }
        } else {
            tc.note(format!(
                "`{name}`: projected or undefined CRS, area of use not checked"
            ));
        }
        if let Some(sc) = scans.get(name.as_str()) {
            tx.ok();
            if sc.outside_bbox > 0 {
                tx.fail(format!(
                    "`{name}`: {} geometries fall outside bbox {nums:?}",
                    sc.outside_bbox
                ));
            }
        } else if let Some(e) = unread(name.as_str()) {
            tx.note(format!("`{name}`: data not read ({e})"));
        }
    }
    out.push(ta.finish());
    out.push(tc.finish());
    out.push(tx.finish());
    out.push(skip(MEDIA_TYPE, "not testable on the file alone"));

    // Bounding Box Covering class
    let covering: Vec<(&String, &Value)> = columns
        .iter()
        .filter(|(_, c)| c.get("covering").is_some())
        .collect();
    if covering.is_empty() {
        for id in COVERING_IDS {
            out.push(skip(id, "no `covering` declared; class not claimed"));
        }
    } else {
        let mut tk = T::new(COV_KEYS);
        if spec == Spec::V1_0 {
            tk.note("`covering` is not defined in GeoParquet 1.0.0; checked with the 1.1 rules");
        }
        let mut tp = T::new(COV_BBOX_PATHS);
        let mut ts = T::new(COV_STRUCTURE);
        let mut tt = T::new(COV_TYPE);
        let mut tr = T::new(COV_REPETITION);
        let mut tn = T::new(COV_NESTING);
        for (name, col) in covering {
            let cov = &col["covering"];
            tk.ok();
            match cov.as_object() {
                None => {
                    tk.fail(format!("`{name}`: covering is not an object"));
                    continue;
                }
                Some(o) => {
                    for k in o.keys().filter(|k| *k != "bbox") {
                        tk.fail(format!("`{name}`: covering has unknown member `{k}`"));
                    }
                    if !o.contains_key("bbox") {
                        tk.fail(format!("`{name}`: covering has no `bbox` member"));
                    }
                }
            }
            tp.ok();
            let bcol = match resolve_bbox_paths(cov) {
                Ok(c) => c,
                Err(e) => {
                    tp.fail(format!("`{name}`: {e}"));
                    continue;
                }
            };
            let Some(bf) = root_by_name.get(bcol.as_str()) else {
                tp.fail(format!("`{name}`: covering names column `{bcol}`, which is not at the root of the Parquet schema"));
                if schema_has_nested(&root, &bcol) {
                    tn.fail(format!(
                        "`{name}`: bounding box column `{bcol}` is nested below the root"
                    ));
                } else {
                    tn.note(format!("`{name}`: no column `{bcol}` to check"));
                }
                continue;
            };
            tn.ok();
            ts.ok();
            if !bf.is_group() {
                ts.fail(format!("`{bcol}` is not a group field"));
                continue;
            }
            let children = bf.get_fields();
            let names: Vec<&str> = children.iter().map(|c| c.name()).collect();
            // The four child fields must be named but may come in any order (the order rule was
            // dropped when PR #302 merged); the six-child form keeps its order.
            match children.len() {
                4 => {
                    let mut sorted = names.clone();
                    sorted.sort_unstable();
                    if sorted != ["xmax", "xmin", "ymax", "ymin"] {
                        ts.fail(format!(
                            "`{bcol}` child fields are {names:?}, expected `xmin`, `ymin`, `xmax` and `ymax` in any order"
                        ));
                    }
                }
                6 => {
                    let expected = ["xmin", "ymin", "zmin", "xmax", "ymax", "zmax"];
                    if names != expected {
                        ts.fail(format!(
                            "`{bcol}` child fields are {names:?}, expected {expected:?} in that order"
                        ));
                    }
                }
                n => ts.fail(format!("`{bcol}` has {n} child fields, expected 4 or 6")),
            }
            tt.ok();
            let mut kinds: BTreeSet<String> = BTreeSet::new();
            for c in children {
                if c.is_group() {
                    tt.fail(format!(
                        "`{bcol}.{}` is a group, expected FLOAT or DOUBLE",
                        c.name()
                    ));
                    continue;
                }
                let pt = c.get_physical_type();
                if pt != PhysicalType::FLOAT && pt != PhysicalType::DOUBLE {
                    tt.fail(format!(
                        "`{bcol}.{}` is {pt:?}, expected FLOAT or DOUBLE",
                        c.name()
                    ));
                }
                kinds.insert(format!("{pt:?}"));
            }
            if kinds.len() > 1 {
                tt.fail(format!("`{bcol}` mixes child types {kinds:?}"));
            }
            tr.ok();
            if let Some(gf) = geom_fields.get(name.as_str()) {
                let (gb, bb) = (gf.get_basic_info(), bf.get_basic_info());
                if gb.has_repetition() && bb.has_repetition() && gb.repetition() != bb.repetition()
                {
                    tr.fail(format!(
                        "`{bcol}` is {:?} but `{name}` is {:?}",
                        bb.repetition(),
                        gb.repetition()
                    ));
                }
            }
            if let Some(sc) = scans.get(name.as_str())
                && sc.bbox_rep_checked
                && sc.bbox_rep_mismatch > 0
            {
                tr.fail(format!(
                    "`{bcol}`: {} rows where the bbox is present xor the geometry is present",
                    sc.bbox_rep_mismatch
                ));
            }
        }
        out.extend([
            tk.finish(),
            tp.finish(),
            ts.finish(),
            tt.finish(),
            tr.finish(),
            tn.finish(),
        ]);
    }

    // Cloud-Optimized Distribution class. 2.0 files carry native GeospatialStatistics; 1.x files
    // can only offer the column statistics of a bbox covering column.
    let native_boxes = |name: &str| -> Vec<Option<[f64; 4]>> {
        (0..meta.num_row_groups())
            .map(|rg| {
                chunk(meta, rg, name)
                    .and_then(|c| c.geo_statistics())
                    .and_then(|s| s.bounding_box())
                    .map(|b| [b.get_xmin(), b.get_ymin(), b.get_xmax(), b.get_ymax()])
            })
            .collect()
    };
    let mut primary_boxes: Option<Vec<Option<[f64; 4]>>> = None;
    let mut t = T::new(DIST_STATS);
    for (name, f) in &geom_fields {
        let col = &columns[*name];
        let native = native_boxes(name);
        let has_native = is_geo_logical(&logical(f));
        let covering = col.get("covering").and_then(|c| resolve_bbox_paths(c).ok());
        let (boxes, source): (Vec<Option<[f64; 4]>>, String) = if has_native
            && (spec == Spec::V2_0 || native.iter().all(Option::is_some))
        {
            (native, "native geospatial statistics".into())
        } else if let Some(bcol) = covering {
            (
                covering_boxes(meta, &bcol),
                format!("column statistics of the bbox covering `{bcol}`"),
            )
        } else if spec == Spec::V2_0 {
            continue;
        } else {
            t.fail(format!(
                    "`{name}`: no row-group statistics source; a {} file needs a bbox covering column (2.0 files carry native statistics)",
                    spec.versions()[0]
                ));
            continue;
        };
        t.ok();
        let missing: Vec<usize> = boxes
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.is_none().then_some(i))
            .collect();
        if missing.is_empty() {
            t.note(format!("`{name}`: {source}"));
        } else {
            t.fail(format!("`{name}`: no {source} in row groups {missing:?}"));
        }
        if *name == primary {
            primary_boxes = Some(boxes);
        }
    }
    out.push(t.finish());

    let metric = match &primary_boxes {
        Some(b) if b.iter().all(Option::is_some) => {
            let boxes: Vec<[f64; 4]> = b.iter().flatten().copied().collect();
            Some(spatial::measure(&boxes))
        }
        _ => None,
    };
    out.push(match &metric {
        None => skip(
            DIST_SPATIAL_ORDER,
            "primary column has no row-group bounding boxes (see geospatial-statistics)",
        ),
        Some(Err(reason)) => skip(DIST_SPATIAL_ORDER, reason.clone()),
        Some(Ok(m)) => Outcome {
            id: DIST_SPATIAL_ORDER,
            status: if m.ratio >= spatial::PASS_RATIO {
                Status::Pass
            } else {
                Status::Fail
            },
            message: format!(
                "{} row groups: skip rate {:.3} vs ideal tiling {:.3} (ratio {:.2}, pass at {:.2}); area factor {:.2}",
                m.row_groups, m.file_skip, m.ideal_skip, m.ratio, spatial::PASS_RATIO, m.area_factor
            ),
        },
    });

    // Advice on the distribution best practices. None of it decides conformance; it is the
    // guidance of format-specs/distributing-geoparquet.md, measured on this file.
    {
        let add = |topic: &'static str, level: Level, message: String| {
            advice.borrow_mut().push(Advice {
                topic,
                level,
                message,
            })
        };

        // spatial ordering
        match &metric {
            Some(Ok(m)) if m.ratio >= 0.9 => add(
                "spatial ordering",
                Level::Good,
                format!(
                    "the rows are well ordered: a query window can skip {:.0} % of the row groups, {:.0} % of what an ideal tiling of {} row groups would allow",
                    m.file_skip * 100.0,
                    (m.ratio * 100.0).min(100.0),
                    m.row_groups
                ),
            ),
            Some(Ok(m)) if m.ratio >= spatial::PASS_RATIO => add(
                "spatial ordering",
                Level::Consider,
                format!(
                    "the rows are ordered, but not tightly: a query window skips {:.0} % of the row groups against {:.0} % for an ideal tiling. Sorting by a space-filling curve (`ORDER BY ST_Hilbert(...)` in DuckDB, `gpio sort hilbert`) usually closes the gap",
                    m.file_skip * 100.0,
                    m.ideal_skip * 100.0
                ),
            ),
            Some(Ok(m)) => add(
                "spatial ordering",
                Level::Poor,
                format!(
                    "the rows are not spatially ordered: a query window can skip only {:.0} % of the row groups, against {:.0} % for an ideal tiling, so readers download most of the file for a small area. Sort by a space-filling curve before publishing (`ORDER BY ST_Hilbert(...)` in DuckDB, `gpio sort hilbert`)",
                    m.file_skip * 100.0,
                    m.ideal_skip * 100.0
                ),
            ),
            Some(Err(_)) if meta.num_row_groups() <= 1 => add(
                "spatial ordering",
                Level::Consider,
                "the file is a single row group, so there is nothing for a reader to skip. That is fine for a small file; larger ones should be written in row groups and sorted spatially".to_string(),
            ),
            Some(Err(reason)) => add(
                "spatial ordering",
                Level::Consider,
                format!("not measured: {reason}"),
            ),
            None => {}
        }

        // row group size
        let rows: Vec<i64> = meta.row_groups().iter().map(|rg| rg.num_rows()).collect();
        if !rows.is_empty() {
            let max = *rows.iter().max().unwrap();
            let total: i64 = rows.iter().sum();
            let bytes: i64 = meta
                .row_groups()
                .iter()
                .map(|rg| rg.compressed_size())
                .sum();
            let size = human(bytes);
            if max > 150_000 {
                add(
                    "row group size",
                    Level::Poor,
                    format!(
                        "the largest row group holds {max} rows ({} row groups, {total} rows, {size}). A reader can only skip whole row groups, so keep them at 150 000 rows or fewer",
                        rows.len()
                    ),
                );
            } else if rows.len() > 1 && max < 5_000 {
                add(
                    "row group size",
                    Level::Consider,
                    format!(
                        "row groups are small ({max} rows at most, {} of them). Very small row groups add footer and request overhead; a few tens of thousands of rows each is a good target",
                        rows.len()
                    ),
                );
            } else if rows.len() == 1 && total > 150_000 {
                add(
                    "row group size",
                    Level::Poor,
                    format!(
                        "the file is a single row group of {total} rows ({size}), so a reader cannot skip any of it. Write row groups of at most 150 000 rows"
                    ),
                );
            } else {
                add(
                    "row group size",
                    Level::Good,
                    format!(
                        "{} row group(s), at most {max} rows each ({total} rows, {size})",
                        rows.len()
                    ),
                );
            }
        }

        // compression
        let mut codecs: BTreeMap<String, u64> = BTreeMap::new();
        for rg in meta.row_groups() {
            for cc in rg.columns() {
                // Parquet records the codec, not its level, so drop the level the crate fills in
                let codec = format!("{:?}", cc.compression());
                let codec = codec
                    .split('(')
                    .next()
                    .unwrap_or(&codec)
                    .to_ascii_uppercase();
                *codecs.entry(codec).or_default() += cc.compressed_size().max(0) as u64;
            }
        }
        let names: Vec<&str> = codecs.keys().map(String::as_str).collect();
        if !names.is_empty() {
            let list = names.join(", ");
            if names.iter().all(|c| c.starts_with("ZSTD")) {
                add(
                    "compression",
                    Level::Good,
                    format!(
                        "every column chunk uses {list}, the codec the best practices recommend (the level is not recorded in the file)"
                    ),
                );
            } else if names.iter().any(|c| c.starts_with("UNCOMPRESSED")) {
                add(
                    "compression",
                    Level::Poor,
                    format!(
                        "some column chunks are stored uncompressed ({list}). ZSTD typically halves a GeoParquet file at no cost to readers"
                    ),
                );
            } else {
                add(
                    "compression",
                    Level::Consider,
                    format!(
                        "column chunks use {list}. ZSTD usually gives a better ratio than Snappy at a similar decompression speed, and the best practices recommend it"
                    ),
                );
            }
        }

        // a bbox covering column adds page-level pruning on top of the row-group statistics
        let has_covering = columns.values().any(|c| c.get("covering").is_some());
        if !has_covering {
            let (level, message) = if spec == Spec::V2_0 {
                (Level::Consider, "no bounding box covering column. The native statistics already let readers skip row groups; a covering column adds the Parquet page index, so readers can skip pages within a row group, and serves readers that predate the native statistics".to_string())
            } else {
                (
                    Level::Poor,
                    format!(
                        "no bounding box covering column, so a {} file offers readers no spatial statistics at all. Add one, or publish 2.0 with native geometry types",
                        spec.versions()[0]
                    ),
                )
            };
            add("bbox covering", level, message);
        } else {
            add("bbox covering", Level::Good, "a bounding box covering column is declared, so readers can prune pages as well as row groups".to_string());
        }

        // dataset-level: partitioning
        let bytes: i64 = meta
            .row_groups()
            .iter()
            .map(|rg| rg.compressed_size())
            .sum();
        if bytes > 2_000_000_000 {
            add(
                "file size",
                Level::Consider,
                format!(
                    "the file is about {:.1} GB. The best practices suggest partitioning a dataset above roughly 2 GB into files of 200 MB to 1 GB, so readers can skip whole files",
                    bytes as f64 / 1e9
                ),
            );
        }
    }

    Ok(finish(out, "not run"))
}
