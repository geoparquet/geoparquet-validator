//! Spatial-order metric of /conf/distribution/spatial-order: the pruning efficiency of the
//! Portolan spatial-order method (portolan-spec `specs/incubating/spatial-order.md`), which
//! `gpio check spatial` implements from geoparquet-io 1.8.0 (PR #774).
//!
//! A query window is centred in row group `i` with probability `rows_i / N`, uniformly within its
//! box, and spans 10 % of each dimension of that box. The skip rate is the expected share of the
//! file's bytes (`total_compressed_size`) in row groups the window misses, computed in closed form.
//! Efficiency is that skip rate over the one of a near-square reference grid of the same number of
//! cells, clipped at 1. A file passes at 0.65. Below eight row groups the boxes are not judged:
//! the method sends such files to its row check, which, as in gpio, this tool does not run.
//! `fixtures/spatial-metric-vectors.json` is the method's test-vector file, vendored verbatim.

pub const QUERY_FRACTION: f64 = 0.10;
/// The read shares reported beside the verdict (1 %, 10 % and 25 % windows).
pub const REPORT_FRACTIONS: [f64; 3] = [0.01, QUERY_FRACTION, 0.25];
pub const PASS_RATIO: f64 = 0.65;
/// The method must not judge fewer row groups by their boxes (a grid is a poor model of a curve
/// sort at small counts).
pub const MIN_ROW_GROUPS: usize = 8;
const ZERO_SKIP: f64 = 1e-9;

/// A row-group box [xmin, ymin, xmax, ymax] with its row and byte counts.
#[derive(Debug, Clone, Copy)]
pub struct Group {
    pub bbox: [f64; 4],
    pub rows: f64,
    pub bytes: f64,
}

#[derive(Debug)]
pub struct Scores {
    pub count: usize,
    /// expected share of the bytes a 10 % window skips
    pub achieved: f64,
    /// the same for the reference grid
    pub achievable: f64,
    /// `min(1, achieved / achievable)`; None when the reference skips nothing
    pub efficiency: Option<f64>,
    /// sum of the box areas over the extent area; None when the extent has no area
    pub area_sum: Option<f64>,
    /// expected read share (1 - skip) at the 1 %, 10 % and 25 % windows
    pub read_profile: [f64; 3],
    /// up to three row groups with the largest boxes, largest first
    pub largest: Vec<usize>,
}

#[derive(Debug)]
pub struct Metric {
    pub row_groups: usize,
    pub file_skip: f64,
    pub ideal_skip: f64,
    pub ratio: f64,
    pub area_sum: Option<f64>,
    pub read_profile: [f64; 3],
    pub largest: Vec<usize>,
}

/// Chance that a coordinate drawn uniformly on [a0, a1] falls in [g0, g1]; a span with no width
/// puts all its probability on a0.
fn axis_share(a0: f64, a1: f64, g0: f64, g1: f64) -> f64 {
    if a1 <= a0 {
        return if g0 <= a0 && a0 <= g1 { 1.0 } else { 0.0 };
    }
    ((a1.min(g1) - a0.max(g0)) / (a1 - a0)).clamp(0.0, 1.0)
}

fn skip_rate(groups: &[Group], fraction: f64) -> f64 {
    let n_rows: f64 = groups.iter().map(|g| g.rows).sum();
    let n_bytes: f64 = groups.iter().map(|g| g.bytes).sum();
    if groups.is_empty() || n_rows <= 0.0 || n_bytes <= 0.0 {
        return 0.0;
    }
    let mut read = 0.0;
    for src in groups {
        let [ax0, ay0, ax1, ay1] = src.bbox;
        let (hw, hh) = (fraction * (ax1 - ax0) / 2.0, fraction * (ay1 - ay0) / 2.0);
        let mut hit = 0.0;
        for dst in groups {
            let [bx0, by0, bx1, by1] = dst.bbox;
            let p =
                axis_share(ax0, ax1, bx0 - hw, bx1 + hw) * axis_share(ay0, ay1, by0 - hh, by1 + hh);
            hit += p * dst.bytes / n_bytes;
        }
        read += src.rows / n_rows * hit;
    }
    let rate = 1.0 - read;
    if rate.abs() <= ZERO_SKIP {
        0.0
    } else {
        rate.clamp(0.0, 1.0)
    }
}

fn extent(groups: &[Group]) -> [f64; 4] {
    groups.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |e, g| {
            [
                e[0].min(g.bbox[0]),
                e[1].min(g.bbox[1]),
                e[2].max(g.bbox[2]),
                e[3].max(g.bbox[3]),
            ]
        },
    )
}

/// The method's reference layout: `ceil(sqrt(n))` columns, `ceil(n / cols)` rows, the last row's
/// cells stretched to span the full width; uniform rows and bytes.
fn reference_grid(ext: &[f64; 4], n: usize) -> Vec<Group> {
    let cols = (n as f64).sqrt().ceil() as usize;
    let rows = n.div_ceil(cols);
    let last = n - cols * (rows - 1);
    let rh = (ext[3] - ext[1]) / rows as f64;
    let mut cells = Vec::with_capacity(n);
    for r in 0..rows {
        let in_row = if r + 1 < rows { cols } else { last };
        let cw = (ext[2] - ext[0]) / in_row as f64;
        for c in 0..in_row {
            let (x0, y0) = (ext[0] + c as f64 * cw, ext[1] + r as f64 * rh);
            cells.push(Group {
                bbox: [x0, y0, x0 + cw, y0 + rh],
                rows: 1.0,
                bytes: 1.0,
            });
        }
    }
    cells
}

fn area(b: &[f64; 4]) -> f64 {
    (b[2] - b[0]) * (b[3] - b[1])
}

/// The method's numbers for any layout, whatever its size (the test vectors score layouts of one
/// to twenty boxes).
pub fn scores(groups: &[Group]) -> Scores {
    let n = groups.len();
    let ext = extent(groups);
    let achieved = skip_rate(groups, QUERY_FRACTION);
    let achievable = if n == 0 {
        0.0
    } else {
        skip_rate(&reference_grid(&ext, n), QUERY_FRACTION)
    };
    let ext_area = if n == 0 { 0.0 } else { area(&ext) };
    let mut ranked: Vec<usize> = (0..n).collect();
    ranked.sort_by(|&a, &b| area(&groups[b].bbox).total_cmp(&area(&groups[a].bbox)));
    ranked.truncate(3);
    Scores {
        count: n,
        achieved,
        achievable,
        efficiency: (achievable > ZERO_SKIP).then(|| (achieved / achievable).min(1.0)),
        area_sum: (ext_area > 0.0)
            .then(|| groups.iter().map(|g| area(&g.bbox)).sum::<f64>() / ext_area),
        read_profile: REPORT_FRACTIONS.map(|f| 1.0 - skip_rate(groups, f)),
        largest: ranked,
    }
}

/// The footer check. `groups` are the row groups of the primary geometry column, in file order.
/// A box with xmin > xmax wraps the antimeridian; it is widened to the full longitude range, so no
/// query can skip it on that axis (the method does not define wrapping boxes; gpio scores them as
/// points on that axis).
pub fn measure(groups: &[Group]) -> Result<Metric, String> {
    let n = groups.len();
    if let Some(i) = groups
        .iter()
        .position(|g| g.bbox.iter().any(|v| !v.is_finite()))
    {
        return Err(format!(
            "row group {i} has non-finite geospatial statistics"
        ));
    }
    if n < MIN_ROW_GROUPS {
        return Err(format!(
            "not judged: {n} row group{}. The spatial-order method judges row-group boxes from {MIN_ROW_GROUPS} row groups up, and sends smaller files to a row check that this tool does not run (as geoparquet-io)",
            if n == 1 { "" } else { "s" }
        ));
    }
    let groups: Vec<Group> = groups
        .iter()
        .map(|g| {
            let mut g = *g;
            if g.bbox[0] > g.bbox[2] {
                g.bbox[0] = -180.0;
                g.bbox[2] = 180.0;
            }
            g.rows = g.rows.max(1.0);
            g.bytes = g.bytes.max(1.0);
            g
        })
        .collect();
    let s = scores(&groups);
    let Some(ratio) = s.efficiency else {
        return Err("not judged: the efficiency is undefined, because no query window can miss any row group (the extent is a single point)".into());
    };
    Ok(Metric {
        row_groups: n,
        file_skip: s.achieved,
        ideal_skip: s.achievable,
        ratio,
        area_sum: s.area_sum,
        read_profile: s.read_profile,
        largest: s.largest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const VECTORS: &str = include_str!("../fixtures/spatial-metric-vectors.json");

    fn layout(v: &Value) -> Vec<Group> {
        let boxes = v["boxes"].as_array().unwrap();
        let weight = |key: &str, i: usize| v.get(key).map_or(1.0, |a| a[i].as_f64().unwrap());
        boxes
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let b: Vec<f64> = b
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_f64().unwrap())
                    .collect();
                // "A layout without byte counts uses s_i = r_i."
                let rows = weight("rows", i);
                Group {
                    bbox: [b[0], b[1], b[2], b[3]],
                    rows,
                    bytes: v.get("sizes").map_or(rows, |a| a[i].as_f64().unwrap()),
                }
            })
            .collect()
    }

    fn close(name: &str, field: &str, got: Option<f64>, want: &Value) {
        match (got, want.as_f64()) {
            (None, None) => {}
            (Some(g), Some(w)) => assert!(
                (g - w).abs() <= 5e-7,
                "{name}: {field} = {g:.6}, expected {w:.6}"
            ),
            (g, _) => panic!("{name}: {field} = {g:?}, expected {want}"),
        }
    }

    #[test]
    fn reproduces_every_layout_vector() {
        let v: Value = serde_json::from_str(VECTORS).unwrap();
        let layouts = v["layouts"].as_object().unwrap();
        assert!(layouts.len() >= 13);
        for (name, case) in layouts {
            let e = &case["expected"];
            let groups = layout(case);
            let s = scores(&groups);
            assert_eq!(s.count as u64, e["count"].as_u64().unwrap(), "{name}");
            close(name, "achieved", Some(s.achieved), &e["achieved"]);
            close(name, "achievable", Some(s.achievable), &e["achievable"]);
            close(name, "efficiency", s.efficiency, &e["efficiency"]);
            close(name, "area_sum", s.area_sum, &e["area_sum"]);
            let below = s.efficiency.map(|x| x < PASS_RATIO);
            assert_eq!(below, e["below_bar"].as_bool(), "{name}: below_bar");
            let verdict = match measure(&groups) {
                Ok(m) if m.ratio >= PASS_RATIO => "pass",
                Ok(_) => "fail",
                Err(_) => "row-check",
            };
            assert_eq!(verdict, e["verdict"].as_str().unwrap(), "{name}: verdict");
        }
    }

    #[test]
    fn bad_input_is_not_judged() {
        let g = |b: [f64; 4]| Group {
            bbox: b,
            rows: 1.0,
            bytes: 1.0,
        };
        let mut boxes: Vec<Group> = (0..9)
            .map(|i| g([i as f64, 0.0, i as f64 + 1.0, 1.0]))
            .collect();
        boxes[4].bbox[2] = f64::INFINITY;
        assert!(measure(&boxes).unwrap_err().contains("non-finite"));
        assert!(
            measure(&[g([0.0; 4]); 9])
                .unwrap_err()
                .contains("undefined")
        );
        assert!(
            measure(&[g([0.0, 0.0, 1.0, 1.0]); 7])
                .unwrap_err()
                .contains("7 row groups")
        );
    }

    #[test]
    fn a_wrapping_box_is_never_skipped_on_longitude() {
        let mut groups: Vec<Group> = (0..8)
            .map(|i| Group {
                bbox: [
                    -180.0 + 45.0 * i as f64,
                    -10.0,
                    -135.0 + 45.0 * i as f64,
                    10.0,
                ],
                rows: 1.0,
                bytes: 1.0,
            })
            .collect();
        let tiled = measure(&groups).unwrap();
        groups[7].bbox = [170.0, -10.0, -170.0, 10.0];
        let wrapped = measure(&groups).unwrap();
        assert!(wrapped.file_skip < tiled.file_skip);
        assert!(wrapped.largest[0] == 7);
    }
}
