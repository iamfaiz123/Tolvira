//! GeoJSON (RFC 7946) parsing, validation, statistics, bbox, simplification,
//! and conversion to/from CSV, KML and WKT.

use crate::geom;
use engine_shared::{EngineError, EngineResult};
use serde_json::{json, Map, Value};

/// A `(longitude, latitude)` position.
pub type Pos = (f64, f64);

/// GeoJSON geometry.
#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    /// Single position.
    Point(Pos),
    /// Several positions.
    MultiPoint(Vec<Pos>),
    /// Polyline.
    LineString(Vec<Pos>),
    /// Several polylines.
    MultiLineString(Vec<Vec<Pos>>),
    /// Outer ring followed by holes.
    Polygon(Vec<Vec<Pos>>),
    /// Several polygons.
    MultiPolygon(Vec<Vec<Vec<Pos>>>),
    /// `GeometryCollection`.
    Collection(Vec<Geometry>),
}

/// A GeoJSON feature.
#[derive(Debug, Clone)]
pub struct Feature {
    /// Geometry (`null` allowed by the spec).
    pub geometry: Option<Geometry>,
    /// Property object.
    pub properties: Map<String, Value>,
    /// Optional feature id.
    pub id: Option<Value>,
}

/// Result of parsing GeoJSON.
pub struct Parsed {
    /// Features (a bare geometry becomes one feature).
    pub features: Vec<Feature>,
    /// Root object type (`FeatureCollection`, `Feature`, `Geometry`).
    pub kind: &'static str,
    /// Non-fatal problems found while parsing.
    pub warnings: Vec<String>,
}

const MAX_COORDS: usize = 5_000_000;

fn pos(v: &Value, path: &str, warnings: &mut Vec<String>) -> EngineResult<Pos> {
    let arr = v
        .as_array()
        .ok_or_else(|| EngineError::malformed(format!("{path}: position must be an array")))?;
    if arr.len() < 2 {
        return Err(EngineError::malformed(format!("{path}: position needs at least 2 numbers")));
    }
    let x = arr[0]
        .as_f64()
        .ok_or_else(|| EngineError::malformed(format!("{path}: longitude is not a number")))?;
    let y = arr[1]
        .as_f64()
        .ok_or_else(|| EngineError::malformed(format!("{path}: latitude is not a number")))?;
    if !(-180.0..=180.0).contains(&x) || !(-90.0..=90.0).contains(&y) {
        if (-90.0..=90.0).contains(&x) && (-180.0..=180.0).contains(&y) {
            warnings
                .push(format!("{path}: [{x}, {y}] looks like [lat, lon]; GeoJSON uses [lon, lat]"));
        } else {
            return Err(EngineError::invalid(format!(
                "{path}: coordinate [{x}, {y}] is out of WGS84 range"
            )));
        }
    }
    if arr.len() > 3 {
        warnings.push(format!(
            "{path}: positions should have at most 3 elements (lon, lat, elevation)"
        ));
    }
    Ok((x, y))
}

fn positions(v: &Value, path: &str, w: &mut Vec<String>) -> EngineResult<Vec<Pos>> {
    v.as_array()
        .ok_or_else(|| EngineError::malformed(format!("{path}: expected an array of positions")))?
        .iter()
        .enumerate()
        .map(|(i, p)| pos(p, &format!("{path}[{i}]"), w))
        .collect()
}

fn rings(v: &Value, path: &str, w: &mut Vec<String>) -> EngineResult<Vec<Vec<Pos>>> {
    let arr = v
        .as_array()
        .ok_or_else(|| EngineError::malformed(format!("{path}: expected an array of rings")))?;
    let mut out = Vec::new();
    for (i, r) in arr.iter().enumerate() {
        let ring = positions(r, &format!("{path}[{i}]"), w)?;
        if ring.len() < 4 {
            return Err(EngineError::invalid(format!(
                "{path}[{i}]: a linear ring needs at least 4 positions"
            )));
        }
        if ring.first() != ring.last() {
            return Err(EngineError::invalid(format!(
                "{path}[{i}]: ring is not closed (first and last positions differ)"
            )));
        }
        let cw = signed_area(&ring) < 0.0;
        if i == 0 && cw {
            w.push(format!(
                "{path}[0]: exterior ring is clockwise; RFC 7946 recommends counter-clockwise"
            ));
        }
        if i > 0 && !cw {
            w.push(format!(
                "{path}[{i}]: hole is counter-clockwise; RFC 7946 recommends clockwise holes"
            ));
        }
        out.push(ring);
    }
    Ok(out)
}

/// Signed planar area of a ring (positive = counter-clockwise).
pub fn signed_area(ring: &[Pos]) -> f64 {
    let n = ring.len();
    let mut a = 0.0;
    for i in 0..n {
        let (x0, y0) = ring[i];
        let (x1, y1) = ring[(i + 1) % n];
        a += x0 * y1 - x1 * y0;
    }
    a / 2.0
}

/// Parses a geometry object, recording warnings (winding, duplicate closing points…).
pub fn parse_geometry(v: &Value, path: &str, w: &mut Vec<String>) -> EngineResult<Geometry> {
    let obj = v
        .as_object()
        .ok_or_else(|| EngineError::malformed(format!("{path}: geometry must be an object")))?;
    let t = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::malformed(format!("{path}: geometry has no `type`")))?;
    let coords = || {
        obj.get("coordinates")
            .ok_or_else(|| EngineError::malformed(format!("{path}: {t} has no `coordinates`")))
    };
    Ok(match t {
        "Point" => Geometry::Point(pos(coords()?, path, w)?),
        "MultiPoint" => Geometry::MultiPoint(positions(coords()?, path, w)?),
        "LineString" => {
            let p = positions(coords()?, path, w)?;
            if p.len() < 2 {
                return Err(EngineError::invalid(format!(
                    "{path}: LineString needs at least 2 positions"
                )));
            }
            Geometry::LineString(p)
        },
        "MultiLineString" => Geometry::MultiLineString(
            coords()?
                .as_array()
                .ok_or_else(|| EngineError::malformed(format!("{path}: expected array")))?
                .iter()
                .enumerate()
                .map(|(i, l)| positions(l, &format!("{path}[{i}]"), w))
                .collect::<EngineResult<_>>()?,
        ),
        "Polygon" => Geometry::Polygon(rings(coords()?, path, w)?),
        "MultiPolygon" => Geometry::MultiPolygon(
            coords()?
                .as_array()
                .ok_or_else(|| EngineError::malformed(format!("{path}: expected array")))?
                .iter()
                .enumerate()
                .map(|(i, p)| rings(p, &format!("{path}[{i}]"), w))
                .collect::<EngineResult<_>>()?,
        ),
        "GeometryCollection" => Geometry::Collection(
            obj.get("geometries")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    EngineError::malformed(format!(
                        "{path}: GeometryCollection has no `geometries`"
                    ))
                })?
                .iter()
                .enumerate()
                .map(|(i, g)| parse_geometry(g, &format!("{path}.geometries[{i}]"), w))
                .collect::<EngineResult<_>>()?,
        ),
        other => {
            return Err(EngineError::invalid(format!("{path}: unknown geometry type `{other}`")))
        },
    })
}

/// Parses GeoJSON text.
pub fn parse(text: &str) -> EngineResult<Parsed> {
    engine_shared::error::require_text(text, "geojson")?;
    engine_shared::limits::check_size(text.len(), 100 * 1024 * 1024, "geojson")?;
    let v: Value = serde_json::from_str(text)
        .map_err(|e| EngineError::malformed(format!("Invalid JSON: {e}")))?;
    parse_value(&v)
}

/// Parses an already-decoded GeoJSON value.
pub fn parse_value(v: &Value) -> EngineResult<Parsed> {
    let mut warnings = Vec::new();
    let obj = v
        .as_object()
        .ok_or_else(|| EngineError::malformed("GeoJSON root must be an object"))?;
    let t = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::malformed("Root object has no `type`"))?;
    if obj.contains_key("crs") {
        warnings.push("`crs` member is obsolete in RFC 7946 (WGS84 is assumed)".into());
    }
    let features = match t {
        "FeatureCollection" => {
            let arr = obj.get("features").and_then(Value::as_array).ok_or_else(|| {
                EngineError::malformed("FeatureCollection has no `features` array")
            })?;
            arr.iter()
                .enumerate()
                .map(|(i, f)| parse_feature(f, &format!("features[{i}]"), &mut warnings))
                .collect::<EngineResult<Vec<_>>>()?
        },
        "Feature" => vec![parse_feature(v, "feature", &mut warnings)?],
        _ => vec![Feature {
            geometry: Some(parse_geometry(v, "geometry", &mut warnings)?),
            properties: Map::new(),
            id: None,
        }],
    };
    let kind = match t {
        "FeatureCollection" => "FeatureCollection",
        "Feature" => "Feature",
        _ => "Geometry",
    };
    let total: usize = features
        .iter()
        .map(|f| f.geometry.as_ref().map(count_coords).unwrap_or(0))
        .sum();
    if total > MAX_COORDS {
        return Err(EngineError::limit(format!(
            "GeoJSON has {total} coordinates; limit is {MAX_COORDS}"
        )));
    }
    warnings.dedup();
    if warnings.len() > 50 {
        let n = warnings.len();
        warnings.truncate(50);
        warnings.push(format!("… {} more warnings", n - 50));
    }
    Ok(Parsed { features, kind, warnings })
}

fn parse_feature(v: &Value, path: &str, w: &mut Vec<String>) -> EngineResult<Feature> {
    let obj = v
        .as_object()
        .ok_or_else(|| EngineError::malformed(format!("{path}: feature must be an object")))?;
    if obj.get("type").and_then(Value::as_str) != Some("Feature") {
        return Err(EngineError::malformed(format!("{path}: type must be `Feature`")));
    }
    let geometry = match obj.get("geometry") {
        None => {
            w.push(format!("{path}: feature has no `geometry` member (should be null if absent)"));
            None
        },
        Some(Value::Null) => None,
        Some(g) => Some(parse_geometry(g, &format!("{path}.geometry"), w)?),
    };
    let properties = match obj.get("properties") {
        Some(Value::Object(m)) => m.clone(),
        Some(Value::Null) | None => {
            if !obj.contains_key("properties") {
                w.push(format!("{path}: feature has no `properties` member"));
            }
            Map::new()
        },
        Some(_) => {
            return Err(EngineError::malformed(format!(
                "{path}: properties must be an object or null"
            )))
        },
    };
    Ok(Feature {
        geometry,
        properties,
        id: obj.get("id").cloned(),
    })
}

/// Number of positions in a geometry.
pub fn count_coords(g: &Geometry) -> usize {
    match g {
        Geometry::Point(_) => 1,
        Geometry::MultiPoint(p) | Geometry::LineString(p) => p.len(),
        Geometry::MultiLineString(l) | Geometry::Polygon(l) => l.iter().map(Vec::len).sum(),
        Geometry::MultiPolygon(p) => p.iter().flatten().map(Vec::len).sum(),
        Geometry::Collection(c) => c.iter().map(count_coords).sum(),
    }
}

/// GeoJSON type name of a geometry.
pub fn geometry_type(g: &Geometry) -> &'static str {
    match g {
        Geometry::Point(_) => "Point",
        Geometry::MultiPoint(_) => "MultiPoint",
        Geometry::LineString(_) => "LineString",
        Geometry::MultiLineString(_) => "MultiLineString",
        Geometry::Polygon(_) => "Polygon",
        Geometry::MultiPolygon(_) => "MultiPolygon",
        Geometry::Collection(_) => "GeometryCollection",
    }
}

/// Collects every position of a geometry.
pub fn all_positions(g: &Geometry, out: &mut Vec<Pos>) {
    match g {
        Geometry::Point(p) => out.push(*p),
        Geometry::MultiPoint(p) | Geometry::LineString(p) => out.extend(p),
        Geometry::MultiLineString(l) | Geometry::Polygon(l) => l.iter().for_each(|r| out.extend(r)),
        Geometry::MultiPolygon(p) => p.iter().flatten().for_each(|r| out.extend(r)),
        Geometry::Collection(c) => c.iter().for_each(|g| all_positions(g, out)),
    }
}

/// Bounding box `[west, south, east, north]` of features (`None` when there are no positions).
pub fn bbox(features: &[Feature]) -> Option<[f64; 4]> {
    let mut pts = Vec::new();
    for f in features {
        if let Some(g) = &f.geometry {
            all_positions(g, &mut pts);
        }
    }
    if pts.is_empty() {
        return None;
    }
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for (x, y) in pts {
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    }
    Some(b)
}

fn round(v: f64, precision: Option<u32>) -> Value {
    match precision {
        Some(p) => {
            let f = 10f64.powi(p as i32);
            json!((v * f).round() / f)
        },
        None => json!(v),
    }
}

fn pos_json(p: &Pos, precision: Option<u32>) -> Value {
    json!([round(p.0, precision), round(p.1, precision)])
}

/// Serialises a geometry, optionally rounding coordinates to `precision` decimals.
pub fn geometry_json(g: &Geometry, precision: Option<u32>) -> Value {
    match g {
        Geometry::Point(p) => json!({ "type": "Point", "coordinates": pos_json(p, precision) }),
        Geometry::MultiPoint(p) => {
            json!({
                "type": "MultiPoint",
                "coordinates": p.iter().map(|x| pos_json(x, precision)).collect::<Vec<_>>()
            })
        },
        Geometry::LineString(p) => {
            json!({
                "type": "LineString",
                "coordinates": p.iter().map(|x| pos_json(x, precision)).collect::<Vec<_>>()
            })
        },
        Geometry::MultiLineString(l) => {
            json!({
                "type": "MultiLineString",
                "coordinates": l.iter().map(|r| r.iter().map(|x| pos_json(x, precision)).collect::<Vec<_>>()).collect::<Vec<_>>()
            })
        },
        Geometry::Polygon(l) => {
            json!({
                "type": "Polygon",
                "coordinates": l.iter().map(|r| r.iter().map(|x| pos_json(x, precision)).collect::<Vec<_>>()).collect::<Vec<_>>()
            })
        },
        Geometry::MultiPolygon(p) => {
            json!({
                "type": "MultiPolygon",
                "coordinates": p.iter().map(|poly| poly.iter().map(|r| r.iter().map(|x| pos_json(x, precision)).collect::<Vec<_>>()).collect::<Vec<_>>()).collect::<Vec<_>>()
            })
        },
        Geometry::Collection(c) => {
            json!({
                "type": "GeometryCollection",
                "geometries": c.iter().map(|g| geometry_json(g, precision)).collect::<Vec<_>>()
            })
        },
    }
}

/// Serialises a feature (optionally with `bbox`).
pub fn feature_json(f: &Feature, precision: Option<u32>, with_bbox: bool) -> Value {
    let mut v = json!({
        "type": "Feature",
        "geometry": f.geometry.as_ref().map(|g| geometry_json(g, precision)).unwrap_or(Value::Null),
        "properties": f.properties
    });
    if let Some(id) = &f.id {
        v["id"] = id.clone();
    }
    if with_bbox {
        if let Some(b) = bbox(std::slice::from_ref(f)) {
            v["bbox"] = json!(b);
        }
    }
    v
}

/// Serialises a `FeatureCollection` (optionally with `bbox`).
pub fn collection_json(features: &[Feature], precision: Option<u32>, with_bbox: bool) -> Value {
    let mut v = json!({
        "type": "FeatureCollection",
        "features": features.iter().map(|f| feature_json(f, precision, with_bbox)).collect::<Vec<_>>()
    });
    if with_bbox {
        if let Some(b) = bbox(features) {
            v["bbox"] = json!(b);
        }
    }
    v
}

/// Per-feature measurements (lengths in metres, areas in m²).
pub fn measure(g: &Geometry) -> Value {
    match g {
        Geometry::Point(p) => json!({ "type": "Point", "lon": p.0, "lat": p.1 }),
        Geometry::MultiPoint(p) => json!({ "type": "MultiPoint", "points": p.len() }),
        Geometry::LineString(p) => {
            json!({
                "type": "LineString",
                "length_m": geom::ring_length(p, false).round(),
                "vertices": p.len()
            })
        },
        Geometry::MultiLineString(l) => {
            json!({
                "type": "MultiLineString",
                "length_m": l.iter().map(|r| geom::ring_length(r, false)).sum::<f64>().round(),
                "parts": l.len()
            })
        },
        Geometry::Polygon(rings) => {
            let outer = geom::spherical_area(&rings[0]);
            let holes: f64 = rings.iter().skip(1).map(|r| geom::spherical_area(r)).sum();
            let c = geom::centroid(&rings[0]);
            json!({
                "type": "Polygon",
                "area_m2": (outer - holes).round(),
                "area_km2": ((outer - holes) / 1e6 * 1000.0).round() / 1000.0,
                "area_hectares": ((outer - holes) / 1e4 * 100.0).round() / 100.0,
                "area_acres": ((outer - holes) / 4046.856 * 100.0).round() / 100.0,
                "perimeter_m": geom::ring_length(&rings[0], true).round(),
                "holes": rings.len() - 1,
                "centroid": [c.0, c.1],
                "vertices": rings.iter().map(Vec::len).sum::<usize>()
            })
        },
        Geometry::MultiPolygon(polys) => {
            let area: f64 = polys
                .iter()
                .map(|rings| {
                    geom::spherical_area(&rings[0])
                        - rings.iter().skip(1).map(|r| geom::spherical_area(r)).sum::<f64>()
                })
                .sum();
            json!({
                "type": "MultiPolygon",
                "area_m2": area.round(),
                "area_km2": (area / 1e6 * 1000.0).round() / 1000.0,
                "polygons": polys.len()
            })
        },
        Geometry::Collection(c) => {
            json!({
                "type": "GeometryCollection",
                "geometries": c.iter().map(measure).collect::<Vec<_>>()
            })
        },
    }
}

/// Douglas–Peucker simplification with a tolerance in degrees.
pub fn simplify_geometry(g: &Geometry, tolerance: f64) -> Geometry {
    let simp_ring = |r: &Vec<Pos>| -> Vec<Pos> {
        let closed = r.first() == r.last() && r.len() > 3;
        let mut s = geom::simplify(r, tolerance);
        if closed {
            if s.len() < 4 {
                // Keep a valid (possibly tiny) ring.
                s = vec![r[0], r[r.len() / 3], r[2 * r.len() / 3], r[0]];
            } else if s.first() != s.last() {
                s.push(s[0]);
            }
        }
        s
    };
    match g {
        Geometry::LineString(p) => Geometry::LineString(geom::simplify(p, tolerance)),
        Geometry::MultiLineString(l) => {
            Geometry::MultiLineString(l.iter().map(|r| geom::simplify(r, tolerance)).collect())
        },
        Geometry::Polygon(r) => Geometry::Polygon(r.iter().map(simp_ring).collect()),
        Geometry::MultiPolygon(p) => Geometry::MultiPolygon(
            p.iter().map(|poly| poly.iter().map(simp_ring).collect()).collect(),
        ),
        Geometry::Collection(c) => {
            Geometry::Collection(c.iter().map(|g| simplify_geometry(g, tolerance)).collect())
        },
        other => other.clone(),
    }
}

// -------------------------------------------------------------------- WKT --

fn wkt_positions(p: &[Pos]) -> String {
    p.iter()
        .map(|(x, y)| format!("{x} {y}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Serialises a geometry as WKT.
pub fn to_wkt(g: &Geometry) -> String {
    match g {
        Geometry::Point(p) => format!("POINT ({} {})", p.0, p.1),
        Geometry::MultiPoint(p) => format!(
            "MULTIPOINT ({})",
            p.iter()
                .map(|(x, y)| format!("({x} {y})"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Geometry::LineString(p) => format!("LINESTRING ({})", wkt_positions(p)),
        Geometry::MultiLineString(l) => format!(
            "MULTILINESTRING ({})",
            l.iter()
                .map(|r| format!("({})", wkt_positions(r)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Geometry::Polygon(r) => format!(
            "POLYGON ({})",
            r.iter()
                .map(|ring| format!("({})", wkt_positions(ring)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Geometry::MultiPolygon(p) => format!(
            "MULTIPOLYGON ({})",
            p.iter()
                .map(|poly| format!(
                    "({})",
                    poly.iter()
                        .map(|ring| format!("({})", wkt_positions(ring)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Geometry::Collection(c) => {
            format!("GEOMETRYCOLLECTION ({})", c.iter().map(to_wkt).collect::<Vec<_>>().join(", "))
        },
    }
}

struct WktParser<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> WktParser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && self.s.as_bytes()[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.s.as_bytes().get(self.pos).copied()
    }
    fn expect(&mut self, c: u8) -> EngineResult<()> {
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(EngineError::malformed(format!(
                "WKT: expected `{}` at offset {}",
                c as char, self.pos
            )))
        }
    }
    fn word(&mut self) -> String {
        self.skip_ws();
        let start = self.pos;
        while self.pos < self.s.len() && self.s.as_bytes()[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
        }
        self.s[start..self.pos].to_ascii_uppercase()
    }
    fn number(&mut self) -> EngineResult<f64> {
        self.skip_ws();
        let start = self.pos;
        while self.pos < self.s.len()
            && (self.s.as_bytes()[self.pos].is_ascii_digit()
                || matches!(self.s.as_bytes()[self.pos], b'-' | b'+' | b'.' | b'e' | b'E'))
        {
            self.pos += 1;
        }
        self.s[start..self.pos].parse::<f64>().map_err(|_| {
            EngineError::malformed(format!("WKT: expected a number at offset {start}"))
        })
    }
    fn position(&mut self) -> EngineResult<Pos> {
        let x = self.number()?;
        let y = self.number()?;
        // Optional Z / M values.
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == b'-' || c == b'+' || c == b'.')
        {
            self.number()?;
        }
        Ok((x, y))
    }
    fn position_list(&mut self) -> EngineResult<Vec<Pos>> {
        self.expect(b'(')?;
        let mut out = Vec::new();
        loop {
            // MULTIPOINT allows parenthesised points.
            if self.peek() == Some(b'(') {
                self.expect(b'(')?;
                out.push(self.position()?);
                self.expect(b')')?;
            } else {
                out.push(self.position()?);
            }
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b')') => {
                    self.pos += 1;
                    break;
                },
                _ => {
                    return Err(EngineError::malformed(format!(
                        "WKT: expected `,` or `)` at offset {}",
                        self.pos
                    )))
                },
            }
        }
        Ok(out)
    }
    fn ring_list(&mut self) -> EngineResult<Vec<Vec<Pos>>> {
        self.expect(b'(')?;
        let mut out = Vec::new();
        loop {
            out.push(self.position_list()?);
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b')') => {
                    self.pos += 1;
                    break;
                },
                _ => {
                    return Err(EngineError::malformed(format!(
                        "WKT: expected `,` or `)` at offset {}",
                        self.pos
                    )))
                },
            }
        }
        Ok(out)
    }
    fn geometry(&mut self) -> EngineResult<Geometry> {
        let kind = self.word();
        // Optional Z/M/ZM suffix word.
        let save = self.pos;
        let suffix = self.word();
        if !matches!(suffix.as_str(), "Z" | "M" | "ZM") {
            self.pos = save;
        }
        if self.word() == "EMPTY" {
            return Err(EngineError::invalid(format!("WKT: {kind} EMPTY cannot be represented")));
        }
        self.pos = save;
        let suffix2 = self.word();
        if !matches!(suffix2.as_str(), "Z" | "M" | "ZM") {
            self.pos = save;
        }
        Ok(match kind.as_str() {
            "POINT" => {
                self.expect(b'(')?;
                let p = self.position()?;
                self.expect(b')')?;
                Geometry::Point(p)
            },
            "MULTIPOINT" => Geometry::MultiPoint(self.position_list()?),
            "LINESTRING" => Geometry::LineString(self.position_list()?),
            "MULTILINESTRING" => Geometry::MultiLineString(self.ring_list()?),
            "POLYGON" => Geometry::Polygon(self.ring_list()?),
            "MULTIPOLYGON" => {
                self.expect(b'(')?;
                let mut polys = Vec::new();
                loop {
                    polys.push(self.ring_list()?);
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b')') => {
                            self.pos += 1;
                            break;
                        },
                        _ => return Err(EngineError::malformed("WKT: bad MULTIPOLYGON")),
                    }
                }
                Geometry::MultiPolygon(polys)
            },
            "GEOMETRYCOLLECTION" => {
                self.expect(b'(')?;
                let mut gs = Vec::new();
                loop {
                    gs.push(self.geometry()?);
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b')') => {
                            self.pos += 1;
                            break;
                        },
                        _ => return Err(EngineError::malformed("WKT: bad GEOMETRYCOLLECTION")),
                    }
                }
                Geometry::Collection(gs)
            },
            other => {
                return Err(EngineError::invalid(format!("WKT: unknown geometry type `{other}`")))
            },
        })
    }
}

/// Parses WKT (`POINT`, `LINESTRING`, `POLYGON`, `MULTI*`, `GEOMETRYCOLLECTION`).
pub fn parse_wkt(text: &str) -> EngineResult<Geometry> {
    let t = text.trim();
    engine_shared::error::require_text(t, "wkt")?;
    // Strip optional SRID prefix (EWKT).
    let t = t
        .split_once(';')
        .filter(|(a, _)| a.to_ascii_uppercase().starts_with("SRID"))
        .map(|(_, b)| b)
        .unwrap_or(t);
    let mut p = WktParser { s: t, pos: 0 };
    let g = p.geometry()?;
    if p.peek().is_some() {
        return Err(EngineError::malformed(format!(
            "WKT: unexpected trailing content at offset {}",
            p.pos
        )));
    }
    let mut w = Vec::new();
    // Validate ranges through the GeoJSON path.
    let _ = parse_geometry(&geometry_json(&g, None), "wkt", &mut w)?;
    Ok(g)
}

// -------------------------------------------------------------------- KML --

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn kml_coords(p: &[Pos]) -> String {
    p.iter()
        .map(|(x, y)| format!("{x},{y},0"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn kml_geometry(g: &Geometry) -> String {
    match g {
        Geometry::Point(p) => {
            format!("<Point><coordinates>{},{},0</coordinates></Point>", p.0, p.1)
        },
        Geometry::MultiPoint(p) => format!(
            "<MultiGeometry>{}</MultiGeometry>",
            p.iter()
                .map(|x| kml_geometry(&Geometry::Point(*x)))
                .collect::<String>()
        ),
        Geometry::LineString(p) => format!(
            "<LineString><tessellate>1</tessellate><coordinates>{}</coordinates></LineString>",
            kml_coords(p)
        ),
        Geometry::MultiLineString(l) => format!(
            "<MultiGeometry>{}</MultiGeometry>",
            l.iter()
                .map(|r| kml_geometry(&Geometry::LineString(r.clone())))
                .collect::<String>()
        ),
        Geometry::Polygon(rings) => {
            let mut s = String::from("<Polygon><tessellate>1</tessellate>");
            for (i, r) in rings.iter().enumerate() {
                let tag = if i == 0 { "outerBoundaryIs" } else { "innerBoundaryIs" };
                s.push_str(&format!(
                    "<{tag}><LinearRing><coordinates>{}</coordinates></LinearRing></{tag}>",
                    kml_coords(r)
                ));
            }
            s.push_str("</Polygon>");
            s
        },
        Geometry::MultiPolygon(p) => format!(
            "<MultiGeometry>{}</MultiGeometry>",
            p.iter()
                .map(|poly| kml_geometry(&Geometry::Polygon(poly.clone())))
                .collect::<String>()
        ),
        Geometry::Collection(c) => format!(
            "<MultiGeometry>{}</MultiGeometry>",
            c.iter().map(kml_geometry).collect::<String>()
        ),
    }
}

/// Renders features as a KML document.
pub fn to_kml(
    features: &[Feature],
    name: &str,
    name_property: Option<&str>,
    description_property: Option<&str>,
) -> String {
    let mut kml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml \
                           xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n<name>{}</name>\n", xml_escape(name));
    for (i, f) in features.iter().enumerate() {
        let name = name_property
            .and_then(|k| f.properties.get(k))
            .or_else(|| {
                f.properties
                    .get("name")
                    .or_else(|| f.properties.get("Name"))
                    .or_else(|| f.properties.get("title"))
            })
            .map(value_text)
            .unwrap_or_else(|| format!("Feature {}", i + 1));
        let desc = description_property
            .and_then(|k| f.properties.get(k))
            .or_else(|| f.properties.get("description"))
            .map(value_text);
        kml.push_str("<Placemark>\n");
        kml.push_str(&format!("<name>{}</name>\n", xml_escape(&name)));
        if let Some(d) = desc {
            kml.push_str(&format!("<description>{}</description>\n", xml_escape(&d)));
        }
        let extra: Vec<(&String, &Value)> = f
            .properties
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "name" | "Name" | "title" | "description"))
            .collect();
        if !extra.is_empty() {
            kml.push_str("<ExtendedData>");
            for (k, v) in extra {
                kml.push_str(&format!(
                    "<Data name=\"{}\"><value>{}</value></Data>",
                    xml_escape(k),
                    xml_escape(&value_text(v))
                ));
            }
            kml.push_str("</ExtendedData>\n");
        }
        if let Some(g) = &f.geometry {
            kml.push_str(&kml_geometry(g));
            kml.push('\n');
        }
        kml.push_str("</Placemark>\n");
    }
    kml.push_str("</Document>\n</kml>\n");
    kml
}

/// Property value as display text.
pub fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn parse_kml_coords(text: &str) -> EngineResult<Vec<Pos>> {
    let mut out = Vec::new();
    for tuple in text.split_whitespace() {
        let parts: Vec<&str> = tuple.split(',').collect();
        if parts.len() < 2 {
            return Err(EngineError::malformed(format!("KML: bad coordinate tuple `{tuple}`")));
        }
        let x: f64 = parts[0]
            .trim()
            .parse()
            .map_err(|_| EngineError::malformed(format!("KML: bad longitude `{}`", parts[0])))?;
        let y: f64 = parts[1]
            .trim()
            .parse()
            .map_err(|_| EngineError::malformed(format!("KML: bad latitude `{}`", parts[1])))?;
        out.push((x, y));
    }
    Ok(out)
}

/// Minimal KML reader: Placemarks with Point / LineString / Polygon /
/// MultiGeometry, name, description and ExtendedData.
pub fn from_kml(text: &str) -> EngineResult<Vec<Feature>> {
    use quick_xml::events::Event;
    engine_shared::error::require_text(text, "kml")?;
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut features = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut current: Option<Feature> = None;
    let mut geoms: Vec<Geometry> = Vec::new();
    let mut rings: Vec<Vec<Pos>> = Vec::new();
    let mut data_name: Option<String> = None;
    let mut in_kml = false;
    let mut coords_buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if name == "kml" {
                    in_kml = true;
                }
                if name == "Placemark" {
                    current =
                        Some(Feature {
                            geometry: None,
                            properties: Map::new(),
                            id: e.attributes().flatten().find(|a| a.key.as_ref() == b"id").map(
                                |a| Value::String(String::from_utf8_lossy(&a.value).to_string()),
                            ),
                        });
                    geoms.clear();
                }
                if name == "Polygon" {
                    rings.clear();
                }
                if name == "Data" || name == "SimpleData" {
                    data_name = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"name")
                        .map(|a| String::from_utf8_lossy(&a.value).to_string());
                }
                stack.push(name);
            },
            Ok(Event::Text(t)) => {
                let txt = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                let Some(top) = stack.last().cloned() else {
                    continue;
                };
                if let Some(f) = current.as_mut() {
                    match top.as_str() {
                        "name"
                            if stack.iter().rev().nth(1).map(String::as_str)
                                == Some("Placemark") =>
                        {
                            f.properties.insert("name".into(), Value::String(txt));
                        },
                        "description"
                            if stack.iter().rev().nth(1).map(String::as_str)
                                == Some("Placemark") =>
                        {
                            f.properties.insert("description".into(), Value::String(txt));
                        },
                        "value" | "SimpleData" => {
                            if let Some(k) = data_name.clone() {
                                f.properties.insert(k, Value::String(txt));
                            }
                        },
                        "coordinates" => coords_buf.push_str(&txt),
                        _ => {},
                    }
                }
            },
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                stack.pop();
                match name.as_str() {
                    "coordinates" => {
                        let pts = parse_kml_coords(&coords_buf)?;
                        coords_buf.clear();
                        // Determine parent geometry type.
                        let parent = stack.last().map(String::as_str).unwrap_or("");
                        match parent {
                            "Point" => geoms.push(Geometry::Point(
                                *pts.first()
                                    .ok_or_else(|| EngineError::malformed("KML: empty Point"))?,
                            )),
                            "LineString" | "LinearRing"
                                if stack
                                    .iter()
                                    .any(|s| s == "outerBoundaryIs" || s == "innerBoundaryIs") =>
                            {
                                rings.push(pts)
                            },
                            "LineString" => geoms.push(Geometry::LineString(pts)),
                            "LinearRing" => rings.push(pts),
                            _ => geoms.push(Geometry::MultiPoint(pts)),
                        }
                    },
                    "Polygon" => {
                        if !rings.is_empty() {
                            geoms.push(Geometry::Polygon(std::mem::take(&mut rings)));
                        }
                    },
                    "Placemark" => {
                        if let Some(mut f) = current.take() {
                            f.geometry = match geoms.len() {
                                0 => None,
                                1 => Some(geoms.remove(0)),
                                _ => Some(Geometry::Collection(std::mem::take(&mut geoms))),
                            };
                            features.push(f);
                        }
                    },
                    _ => {},
                }
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(EngineError::malformed(format!("KML XML error: {e}"))),
            _ => {},
        }
    }
    if !in_kml {
        return Err(EngineError::unsupported_format("Not a KML document (no <kml> root)"));
    }
    if features.is_empty() {
        return Err(EngineError::malformed("KML has no Placemarks"));
    }
    Ok(features)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FC: &str = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A","pop":5},"geometry":{"type":"Point","coordinates":[10.5,45.2]}},{"type":"Feature","properties":{"name":"B"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}}]}"#;

    #[test]
    fn parse_and_convert() {
        let p = parse(FC).unwrap();
        assert_eq!(p.features.len(), 2);
        assert_eq!(p.kind, "FeatureCollection");
        assert_eq!(bbox(&p.features).unwrap(), [0.0, 0.0, 10.5, 45.2]);
        let m = measure(p.features[1].geometry.as_ref().unwrap());
        assert!(m["area_km2"].as_f64().unwrap() > 12000.0);
        let wkt = to_wkt(p.features[1].geometry.as_ref().unwrap());
        assert_eq!(wkt, "POLYGON ((0 0, 1 0, 1 1, 0 1, 0 0))");
        let g = parse_wkt(
            "SRID=4326;MULTIPOLYGON Z (((0 0 1, 1 0 1, 1 1 1, 0 0 1)), ((5 5 0, 6 5 0, 6 6 0, 5 5 \
             0)))",
        )
        .unwrap();
        assert!(matches!(g, Geometry::MultiPolygon(ref p) if p.len() == 2));
        assert!(parse_wkt("CIRCLE (1 2)").is_err());
        assert!(parse_wkt("POINT (200 1)").is_err());
        let kml = to_kml(&p.features, "Doc", None, None);
        assert!(kml.contains("<Placemark>\n<name>A</name>"));
        assert!(kml.contains("<Data name=\"pop\"><value>5</value></Data>"));
        let back = from_kml(&kml).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].properties["name"], "A");
        assert!(matches!(back[1].geometry, Some(Geometry::Polygon(_))));
        assert!(from_kml("<foo/>").is_err());
        let bad = parse(
            r#"{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1]]]},"properties":{}}"#,
        );
        assert!(bad.is_err());
        let w = parse(r#"{"type":"Point","coordinates":[45.0,100.0]}"#).unwrap();
        assert!(w.warnings[0].contains("[lat, lon]"));
        assert!(parse(r#"{"type":"Point","coordinates":[500,100]}"#).is_err());
        let simple = simplify_geometry(
            &Geometry::LineString(vec![
                (0.0, 0.0),
                (0.5, 0.0001),
                (1.0, 0.0),
                (1.5, 0.0001),
                (2.0, 0.0),
            ]),
            0.01,
        );
        assert!(matches!(simple, Geometry::LineString(ref l) if l.len() == 2));
    }
}
