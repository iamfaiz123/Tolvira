//! # Dev Toolbox GIS / Geo Engine (`geo-engine`)
//!
//! All coordinates are WGS84 (EPSG:4326) longitude/latitude in degrees;
//! GeoJSON order is `[lon, lat]`, everything user-facing here uses `lat, lon`
//! explicitly named. Distances use the mean-sphere haversine by default and
//! Vincenty on the WGS84 ellipsoid when `ellipsoid: true`. Areas use the
//! spherical excess formula (accurate to ~0.3% vs ellipsoidal).
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `format` | geojson-formatter (pretty/minify, precision, bbox) |
//! | `validate` | geojson-validator |
//! | `to_csv` / `from_csv` | geojson-to-csv, csv-to-geojson |
//! | `to_kml` / `from_kml` | geojson-to-kml, kml-to-geojson |
//! | `wkt_parse` / `wkt_from_geojson` | wkt-viewer |
//! | `convert_coordinate` | coordinate-converter (DD ↔ DMS ↔ UTM ↔ geohash) |
//! | `distance` | distance-between-coordinates (+ bearing, midpoint) |
//! | `area` | area-from-coordinates (polygon area, perimeter, centroid) |
//! | `simplify` | geojson-simplifier (Douglas–Peucker) |
//! | `bbox` | geojson-bbox |
//! | `geohash` | geohash-encoder-decoder |
//! | `utm` | utm-latlon-converter |
//! | `destination` | (bonus) point at bearing/distance |

pub mod geojson;
pub mod geom;

use engine_shared::{EngineError, EngineResult};
use geojson::{Feature, Geometry};
use serde::Deserialize;
use serde_json::{json, Map, Value};

/// GeoJSON supplied as text or as an already-parsed JSON value.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct GeoInput {
    /// GeoJSON text (or JSON value under `json`).
    pub text: Option<String>,
    /// GeoJSON value (alternative to `text`).
    pub json: Option<Value>,
}

fn load(input: &GeoInput) -> EngineResult<geojson::Parsed> {
    if let Some(v) = &input.json {
        if !v.is_null() {
            return geojson::parse_value(v);
        }
    }
    geojson::parse(input.text.as_deref().unwrap_or(""))
}

fn summary(p: &geojson::Parsed) -> Value {
    let mut types: Map<String, Value> = Map::new();
    let mut coords = 0usize;
    let mut prop_keys: Vec<String> = Vec::new();
    for f in &p.features {
        if let Some(g) = &f.geometry {
            let t = geojson::geometry_type(g);
            types.insert(t.into(), json!(types.get(t).and_then(Value::as_u64).unwrap_or(0) + 1));
            coords += geojson::count_coords(g);
        }
        for k in f.properties.keys() {
            if !prop_keys.contains(k) {
                prop_keys.push(k.clone());
            }
        }
    }
    json!({
        "kind": p.kind,
        "features": p.features.len(),
        "geometry_types": types,
        "coordinates": coords,
        "property_keys": prop_keys,
        "bbox": geojson::bbox(&p.features),
        "warnings": p.warnings
    })
}

/// Every action understood by the geo engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum GeoAction {
    /// Pretty-prints / minifies / rounds GeoJSON.
    Format {
        /// GeoJSON input.
        #[serde(flatten)]
        input: GeoInput,
        /// Emit compact JSON.
        #[serde(default)]
        minify: bool,
        /// Coordinate decimals.
        #[serde(default)]
        precision: Option<u32>,
        /// Add `bbox` members.
        #[serde(default)]
        add_bbox: bool,
    },
    /// Validates GeoJSON structure and geometry rules.
    Validate {
        /// GeoJSON input.
        #[serde(flatten)]
        input: GeoInput,
    },
    /// GeoJSON → CSV.
    ToCsv {
        /// GeoJSON input.
        #[serde(flatten)]
        input: GeoInput,
        /// `auto` (default), `latlon` (lat/lon columns for points) or `wkt`.
        #[serde(default)]
        geometry_as: String,
    },
    /// CSV → GeoJSON (points from lat/lon columns or geometries from a WKT column).
    FromCsv {
        /// CSV text.
        text: String,
        /// Latitude column name (auto-detected when empty).
        #[serde(default)]
        lat_column: String,
        /// Longitude column name (auto-detected when empty).
        #[serde(default)]
        lon_column: String,
        /// WKT column name (auto-detected when empty).
        #[serde(default)]
        wkt_column: String,
        /// CSV parser options.
        #[serde(default)]
        options: tabular_core::table::CsvOptions,
    },
    /// GeoJSON → KML.
    ToKml {
        /// GeoJSON input.
        #[serde(flatten)]
        input: GeoInput,
        /// Document name.
        #[serde(default)]
        name: String,
        /// Property used as placemark name.
        #[serde(default)]
        name_property: Option<String>,
        /// Property used as placemark description.
        #[serde(default)]
        description_property: Option<String>,
    },
    /// KML → GeoJSON.
    FromKml {
        /// KML text.
        text: String,
    },
    /// WKT → GeoJSON.
    WktParse {
        /// WKT text.
        wkt: String,
    },
    /// GeoJSON → WKT.
    WktFromGeojson {
        /// GeoJSON input.
        #[serde(flatten)]
        input: GeoInput,
    },
    /// Parses a coordinate in any notation (decimal, DMS, UTM, geohash) and converts to all others.
    ConvertCoordinate {
        /// Coordinate text.
        coordinate: String,
    },
    /// Distance and bearing between two points.
    Distance {
        /// Start coordinate.
        from: String,
        /// End coordinate.
        to: String,
        /// Use the Vincenty ellipsoid formula (otherwise haversine).
        #[serde(default)]
        ellipsoid: bool,
    },
    /// Area, perimeter and centroid of a polygon.
    Area {
        /// Ring as `[[lon, lat], …]`.
        #[serde(default)]
        coordinates: Vec<Vec<f64>>,
        /// GeoJSON polygon (alternative to `coordinates`).
        #[serde(flatten)]
        input: GeoInput,
    },
    /// Simplifies geometries (Douglas–Peucker).
    Simplify {
        /// GeoJSON input.
        #[serde(flatten)]
        input: GeoInput,
        /// Tolerance in degrees.
        #[serde(default)]
        tolerance: Option<f64>,
        /// Tolerance in metres (converted to degrees).
        #[serde(default)]
        tolerance_m: Option<f64>,
        /// Coordinate decimals for the output.
        #[serde(default)]
        precision: Option<u32>,
    },
    /// Bounding box of the input.
    Bbox {
        /// GeoJSON input.
        #[serde(flatten)]
        input: GeoInput,
    },
    /// Geohash encode/decode.
    Geohash {
        /// Geohash to decode.
        #[serde(default)]
        hash: Option<String>,
        /// Coordinate to encode.
        #[serde(default)]
        coordinate: Option<String>,
        /// Geohash length.
        #[serde(default)]
        precision: Option<usize>,
    },
    /// UTM encode/decode.
    Utm {
        /// Coordinate to convert to UTM.
        #[serde(default)]
        coordinate: Option<String>,
        /// UTM zone (for decoding).
        #[serde(default)]
        zone: Option<u8>,
        /// `N` or `S` (for decoding).
        #[serde(default)]
        hemisphere: Option<String>,
        /// Easting in metres (for decoding).
        #[serde(default)]
        easting: Option<f64>,
        /// Northing in metres (for decoding).
        #[serde(default)]
        northing: Option<f64>,
    },
    /// Destination point from a start, bearing and distance.
    Destination {
        /// Start coordinate.
        from: String,
        /// Bearing in degrees.
        bearing: f64,
        /// Distance in metres.
        distance_m: f64,
    },
}

fn coordinate_json(lat: f64, lon: f64) -> EngineResult<Value> {
    let utm = geom::to_utm(lat, lon).ok();
    Ok(json!({
        "latitude": lat,
        "longitude": lon,
        "decimal": format!("{lat}, {lon}"),
        "dms": format!("{} {}", geom::to_dms(lat, true), geom::to_dms(lon, false)),
        "dmm": format!("{}° {:.4}' {}, {}° {:.4}' {}", lat.abs().floor(), (lat.abs() - lat.abs().floor()) * 60.0, if lat >= 0.0 { "N" } else { "S" }, lon.abs().floor(), (lon.abs() - lon.abs().floor()) * 60.0, if lon >= 0.0 { "E" } else { "W" }),
        "geojson": [lon, lat],
        "wkt": format!("POINT ({lon} {lat})"),
        "geohash": geom::geohash_encode(lat, lon, 9)?,
        "utm": utm.map(|u| json!({ "zone": u.zone, "band": u.band, "hemisphere": u.hemisphere, "easting": (u.easting * 100.0).round() / 100.0, "northing": (u.northing * 100.0).round() / 100.0, "display": format!("{}{} {:.0} {:.0}", u.zone, u.band, u.easting, u.northing) })),
        "plus_code_note": "Open Location Codes are not implemented",
        "links": {
            "openstreetmap": format!("https://www.openstreetmap.org/?mlat={lat}&mlon={lon}#map=15/{lat}/{lon}"),
            "google_maps": format!("https://www.google.com/maps?q={lat},{lon}")
        },
        "hemisphere": format!("{}{}", if lat >= 0.0 { "N" } else { "S" }, if lon >= 0.0 { "E" } else { "W" }),
    }))
}

/// Dispatches a [`GeoAction`] to its implementation.
pub fn handle_action(action: GeoAction) -> EngineResult<Value> {
    match action {
        GeoAction::Format { input, minify, precision, add_bbox } => {
            let p = load(&input)?;
            let v = match p.kind {
                "Geometry" => match p.features.first().and_then(|f| f.geometry.as_ref()) {
                    Some(g) => geojson::geometry_json(g, precision),
                    None => Value::Null,
                },
                "Feature" => geojson::feature_json(&p.features[0], precision, add_bbox),
                _ => geojson::collection_json(&p.features, precision, add_bbox),
            };
            let text = if minify {
                serde_json::to_string(&v)
            } else {
                serde_json::to_string_pretty(&v)
            }
            .map_err(|e| EngineError::internal(e.to_string()))?;
            Ok(json!({ "text": text, "bytes": text.len(), "summary": summary(&p) }))
        },
        GeoAction::Validate { input } => {
            let text = input.text.clone().unwrap_or_default();
            match load(&input) {
                Ok(p) => {
                    let mut features = Vec::new();
                    for (i, f) in p.features.iter().enumerate().take(1000) {
                        features.push(json!({
                            "index": i,
                            "type": f.geometry.as_ref().map(geojson::geometry_type),
                            "measure": f.geometry.as_ref().map(geojson::measure),
                            "properties": f.properties.len()
                        }));
                    }
                    Ok(json!({
                        "valid": true,
                        "rfc7946_warnings": p.warnings,
                        "summary": summary(&p),
                        "features": features
                    }))
                },
                Err(e) if e.code == engine_shared::error::ErrorCode::EmptyInput => Err(e),
                Err(e) => {
                    let pos = if text.is_empty() {
                        None
                    } else {
                        serde_json::from_str::<Value>(&text)
                            .err()
                            .map(|je| json!({ "line": je.line(), "column": je.column() }))
                    };
                    Ok(json!({
                        "valid": false,
                        "error": e.message,
                        "error_code": e.code.as_str(),
                        "position": pos
                    }))
                },
            }
        },
        GeoAction::ToCsv { input, geometry_as } => {
            let p = load(&input)?;
            let mut headers: Vec<String> = Vec::new();
            for f in &p.features {
                for k in f.properties.keys() {
                    if !headers.contains(k) {
                        headers.push(k.clone());
                    }
                }
            }
            let mode = if geometry_as.is_empty() {
                "auto"
            } else {
                geometry_as.as_str()
            };
            let all_points = p
                .features
                .iter()
                .all(|f| matches!(f.geometry, Some(Geometry::Point(_)) | None));
            let use_latlon = mode == "latlon" || (mode == "auto" && all_points);
            let mut cols = headers.clone();
            if use_latlon {
                cols.push("latitude".into());
                cols.push("longitude".into());
            } else {
                cols.push("geometry_type".into());
                cols.push("wkt".into());
            }
            let rows: Vec<Vec<String>> = p
                .features
                .iter()
                .map(|f| {
                    let mut row: Vec<String> = headers
                        .iter()
                        .map(|h| f.properties.get(h).map(geojson::value_text).unwrap_or_default())
                        .collect();
                    match (&f.geometry, use_latlon) {
                        (Some(Geometry::Point(pt)), true) => {
                            row.push(pt.1.to_string());
                            row.push(pt.0.to_string());
                        },
                        (Some(g), true) => {
                            let c = geom::centroid(&{
                                let mut v = Vec::new();
                                geojson::all_positions(g, &mut v);
                                v
                            });
                            row.push(c.1.to_string());
                            row.push(c.0.to_string());
                        },
                        (Some(g), false) => {
                            row.push(geojson::geometry_type(g).into());
                            row.push(geojson::to_wkt(g));
                        },
                        (None, _) => {
                            row.push(String::new());
                            row.push(String::new());
                        },
                    }
                    row
                })
                .collect();
            let t = tabular_core::table::Table {
                headers: cols,
                rows,
                issues: vec![],
                delimiter: b',',
            };
            Ok(json!({
                "csv": t.to_csv(&tabular_core::table::CsvOptions::default())?,
                "rows": t.rows.len(),
                "headers": t.headers,
                "geometry_columns": if use_latlon { "latitude, longitude" } else { "geometry_type, wkt" }
            }))
        },
        GeoAction::FromCsv {
            text,
            lat_column,
            lon_column,
            wkt_column,
            options,
        } => {
            let t = tabular_core::table::Table::parse(&text, &options)?;
            let find = |cands: &[&str], given: &str| -> Option<usize> {
                if !given.is_empty() {
                    return t.column_index(given).ok();
                }
                t.headers.iter().position(|h| {
                    cands.iter().any(|c| h.to_lowercase().replace([' ', '_'], "") == *c)
                })
            };
            let wkt_idx = find(&["wkt", "geometry", "geom", "thegeom"], &wkt_column);
            let lat_idx = find(&["latitude", "lat", "y"], &lat_column);
            let lon_idx = find(&["longitude", "lon", "lng", "long", "x"], &lon_column);
            if wkt_idx.is_none() && (lat_idx.is_none() || lon_idx.is_none()) {
                return Err(EngineError::invalid(format!(
                    "Could not find latitude/longitude (or WKT) columns in {:?}; pass lat_column \
                     / lon_column",
                    t.headers
                )));
            }
            let mut features = Vec::new();
            let mut skipped = Vec::new();
            for (i, row) in t.rows.iter().enumerate() {
                let geometry = if let Some(wi) = wkt_idx.filter(|wi| !row[*wi].trim().is_empty()) {
                    match geojson::parse_wkt(&row[wi]) {
                        Ok(g) => Some(g),
                        Err(e) => {
                            skipped.push(json!({ "row": i + 1, "reason": e.message }));
                            continue;
                        },
                    }
                } else if let (Some(la), Some(lo)) = (lat_idx, lon_idx) {
                    match (
                        tabular_core::table::parse_number(&row[la]),
                        tabular_core::table::parse_number(&row[lo]),
                    ) {
                        (Some(lat), Some(lon)) if geom::check_lat_lon(lat, lon).is_ok() => {
                            Some(Geometry::Point((lon, lat)))
                        },
                        _ => {
                            skipped.push(json!({
                                "row": i + 1,
                                "reason": format!("Invalid coordinates `{}`, `{}`", row[la], row[lo])
                            }));
                            continue;
                        },
                    }
                } else {
                    skipped.push(json!({ "row": i + 1, "reason": "No geometry" }));
                    continue;
                };
                let mut props = Map::new();
                for (ci, h) in t.headers.iter().enumerate() {
                    if Some(ci) == wkt_idx || Some(ci) == lat_idx || Some(ci) == lon_idx {
                        continue;
                    }
                    let ty = t.column_type(ci);
                    props.insert(h.clone(), tabular_core::table::cell_to_value(&row[ci], ty));
                }
                features.push(Feature { geometry, properties: props, id: None });
            }
            if features.is_empty() {
                return Err(EngineError::invalid("No rows produced a valid geometry"));
            }
            let v = geojson::collection_json(&features, None, false);
            Ok(json!({
                "geojson": v,
                "text": serde_json::to_string_pretty(&v).unwrap_or_default(),
                "features": features.len(),
                "skipped": skipped,
                "bbox": geojson::bbox(&features)
            }))
        },
        GeoAction::ToKml {
            input,
            name,
            name_property,
            description_property,
        } => {
            let p = load(&input)?;
            let kml = geojson::to_kml(
                &p.features,
                if name.is_empty() { "Dev Toolbox export" } else { &name },
                name_property.as_deref(),
                description_property.as_deref(),
            );
            Ok(json!({
                "kml": kml,
                "placemarks": p.features.len(),
                "filename": "export.kml",
                "mime": "application/vnd.google-earth.kml+xml"
            }))
        },
        GeoAction::FromKml { text } => {
            let features = geojson::from_kml(&text)?;
            let v = geojson::collection_json(&features, None, false);
            Ok(json!({
                "geojson": v,
                "text": serde_json::to_string_pretty(&v).unwrap_or_default(),
                "features": features.len(),
                "bbox": geojson::bbox(&features)
            }))
        },
        GeoAction::WktParse { wkt } => {
            let g = geojson::parse_wkt(&wkt)?;
            let f = Feature {
                geometry: Some(g.clone()),
                properties: Map::new(),
                id: None,
            };
            Ok(json!({
                "type": geojson::geometry_type(&g),
                "geojson": geojson::geometry_json(&g, None),
                "feature": geojson::feature_json(&f, None, true),
                "bbox": geojson::bbox(std::slice::from_ref(&f)),
                "measure": geojson::measure(&g),
                "coordinates": geojson::count_coords(&g),
                "normalized_wkt": geojson::to_wkt(&g)
            }))
        },
        GeoAction::WktFromGeojson { input } => {
            let p = load(&input)?;
            let items: Vec<Value> = p
                .features
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    json!({
                        "index": i,
                        "wkt": f.geometry.as_ref().map(geojson::to_wkt),
                        "properties": f.properties
                    })
                })
                .collect();
            Ok(json!({
                "wkt": items.iter().filter_map(|i| i["wkt"].as_str().map(str::to_string)).collect::<Vec<_>>().join("\n"),
                "items": items
            }))
        },
        GeoAction::ConvertCoordinate { coordinate } => {
            let (lat, lon) = geom::parse_coordinate(&coordinate)?;
            let mut v = coordinate_json(lat, lon)?;
            v["input"] = json!(coordinate);
            Ok(v)
        },
        GeoAction::Distance { from, to, ellipsoid } => {
            let (lat1, lon1) = geom::parse_coordinate(&from)?;
            let (lat2, lon2) = geom::parse_coordinate(&to)?;
            let hav = geom::haversine(lat1, lon1, lat2, lon2);
            let (vin, converged) = geom::vincenty(lat1, lon1, lat2, lon2);
            let d = if ellipsoid { vin } else { hav };
            let b = geom::bearing(lat1, lon1, lat2, lon2);
            let back = geom::bearing(lat2, lon2, lat1, lon1);
            let mid = geom::midpoint(lat1, lon1, lat2, lon2);
            Ok(json!({
                "from": { "latitude": lat1, "longitude": lon1 },
                "to": { "latitude": lat2, "longitude": lon2 },
                "method": if ellipsoid { "vincenty (WGS84 ellipsoid)" } else { "\
                    haversine (mean sphere R=6371.0088 km)" },
                "distance": {
                    "meters": (d * 1000.0).round() / 1000.0,
                    "kilometers": (d / 1000.0 * 1_000_000.0).round() / 1_000_000.0,
                    "miles": (d / 1609.344 * 1_000_000.0).round() / 1_000_000.0,
                    "nautical_miles": (d / 1852.0 * 1_000_000.0).round() / 1_000_000.0,
                    "feet": (d / 0.3048).round()
                },
                "alternative": {
                    "haversine_m": hav.round(),
                    "vincenty_m": vin.round(),
                    "vincenty_converged": converged,
                    "difference_m": ((vin - hav) * 100.0).round() / 100.0
                },
                "initial_bearing": (b * 100.0).round() / 100.0,
                "compass": geom::compass(b),
                "final_bearing": ((back + 180.0) % 360.0 * 100.0).round() / 100.0,
                "midpoint": {
                    "latitude": (mid.0 * 1e6).round() / 1e6,
                    "longitude": (mid.1 * 1e6).round() / 1e6
                },
                "travel_time_estimates": {
                    "walking_5kmh": fmt_hours(d / 5000.0),
                    "cycling_18kmh": fmt_hours(d / 18000.0),
                    "driving_80kmh": fmt_hours(d / 80000.0),
                    "flight_850kmh": fmt_hours(d / 850_000.0)
                },
            }))
        },
        GeoAction::Area { coordinates, input } => {
            let ring: Vec<(f64, f64)> = if !coordinates.is_empty() {
                let pts: Vec<(f64, f64)> = coordinates
                    .iter()
                    .map(|c| {
                        if c.len() < 2 {
                            return Err(EngineError::invalid("Each coordinate needs [lat, lon]"));
                        }
                        geom::check_lat_lon(c[0], c[1])?;
                        Ok((c[1], c[0]))
                    })
                    .collect::<EngineResult<_>>()?;
                pts
            } else {
                let p = load(&input)?;
                let areal = p.features.iter().filter_map(|f| f.geometry.as_ref()).find(|g| {
                    matches!(
                        g,
                        Geometry::Polygon(_) | Geometry::MultiPolygon(_) | Geometry::LineString(_)
                    )
                });
                match areal {
                    Some(Geometry::Polygon(r)) => r[0].clone(),
                    Some(Geometry::MultiPolygon(p)) => p[0][0].clone(),
                    Some(Geometry::LineString(l)) => l.clone(),
                    _ => {
                        return Err(EngineError::invalid(
                            "GeoJSON contains no Polygon/LineString to measure",
                        ))
                    },
                }
            };
            let mut ring = ring;
            if ring.len() >= 3 && ring.first() == ring.last() {
                ring.pop();
            }
            if ring.len() < 3 {
                return Err(EngineError::invalid(
                    "A polygon needs at least 3 distinct coordinates",
                ));
            }
            let area = geom::spherical_area(&ring);
            let perim = geom::ring_length(&ring, true);
            let c = geom::centroid(&ring);
            let b = geojson::bbox(&[Feature {
                geometry: Some(Geometry::LineString(ring.clone())),
                properties: Map::new(),
                id: None,
            }])
            .ok_or_else(|| EngineError::empty("polygon ring"))?;
            Ok(json!({
                "vertices": ring.len(),
                "area": {
                    "square_meters": area.round(),
                    "square_kilometers": (area / 1e6 * 1e4).round() / 1e4,
                    "hectares": (area / 1e4 * 100.0).round() / 100.0,
                    "acres": (area / 4046.8564224 * 100.0).round() / 100.0,
                    "square_feet": (area * 10.7639).round(),
                    "square_miles": (area / 2_589_988.11 * 1e4).round() / 1e4
                },
                "perimeter": {
                    "meters": perim.round(),
                    "kilometers": (perim / 1000.0 * 1000.0).round() / 1000.0,
                    "miles": (perim / 1609.344 * 1000.0).round() / 1000.0
                },
                "centroid": {
                    "latitude": (c.1 * 1e6).round() / 1e6,
                    "longitude": (c.0 * 1e6).round() / 1e6
                },
                "bbox": { "west": b[0], "south": b[1], "east": b[2], "north": b[3] },
                "winding": if geojson::signed_area(&ring) > 0.0 { "counter-clockwise" } else { "clockwise" },
                "method": "spherical excess on a sphere of R=6371.0088 km (±0.3% vs ellipsoid)",
            }))
        },
        GeoAction::Simplify { input, tolerance, tolerance_m, precision } => {
            let p = load(&input)?;
            let tol = match (tolerance, tolerance_m) {
                (Some(t), _) => t,
                (None, Some(m)) => m / 111_320.0,
                (None, None) => 0.0001,
            };
            if tol <= 0.0 {
                return Err(EngineError::invalid("tolerance must be > 0"));
            }
            let before: usize = p
                .features
                .iter()
                .filter_map(|f| f.geometry.as_ref())
                .map(geojson::count_coords)
                .sum();
            let features: Vec<Feature> = p
                .features
                .iter()
                .map(|f| Feature {
                    geometry: f.geometry.as_ref().map(|g| geojson::simplify_geometry(g, tol)),
                    properties: f.properties.clone(),
                    id: f.id.clone(),
                })
                .collect();
            let after: usize = features
                .iter()
                .filter_map(|f| f.geometry.as_ref())
                .map(geojson::count_coords)
                .sum();
            let v = match p.kind {
                "Geometry" => match features.first().and_then(|f| f.geometry.as_ref()) {
                    Some(g) => geojson::geometry_json(g, precision),
                    None => Value::Null,
                },
                "Feature" => geojson::feature_json(&features[0], precision, false),
                _ => geojson::collection_json(&features, precision, false),
            };
            Ok(json!({
                "geojson": v,
                "text": serde_json::to_string(&v).unwrap_or_default(),
                "tolerance_degrees": tol,
                "tolerance_m_approx": (tol * 111_320.0).round(),
                "coordinates_before": before,
                "coordinates_after": after,
                "reduction_percent": if before > 0 { ((1.0 - after as f64 / before as f64) * 1000.0).round() / 10.0 } else { 0.0 }
            }))
        },
        GeoAction::Bbox { input } => {
            let p = load(&input)?;
            let b = geojson::bbox(&p.features).ok_or_else(|| EngineError::empty("geometries"))?;
            let w = geom::haversine(b[1], b[0], b[1], b[2]);
            let h = geom::haversine(b[1], b[0], b[3], b[0]);
            Ok(json!({
                "bbox": b,
                "west": b[0],
                "south": b[1],
                "east": b[2],
                "north": b[3],
                "center": { "latitude": (b[1] + b[3]) / 2.0, "longitude": (b[0] + b[2]) / 2.0 },
                "width_km": (w / 1000.0 * 100.0).round() / 100.0,
                "height_km": (h / 1000.0 * 100.0).round() / 100.0,
                "polygon": {
                    "type": "Polygon",
                    "coordinates": [
                        [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]], [b[0], b[1]]]
                    ]
                },
                "leaflet_bounds": [[b[1], b[0]], [b[3], b[2]]],
                "crosses_antimeridian": b[0] > b[2],
            }))
        },
        GeoAction::Geohash { hash, coordinate, precision } => {
            if let Some(h) = hash.filter(|h| !h.trim().is_empty()) {
                let (lat, lon, lat_err, lon_err) = geom::geohash_decode(&h)?;
                let neighbors: Map<String, Value> = geom::geohash_neighbors(&h)?
                    .into_iter()
                    .map(|(k, v)| (k, Value::String(v)))
                    .collect();
                return Ok(json!({
                    "geohash": h.trim().to_lowercase(),
                    "latitude": lat,
                    "longitude": lon,
                    "error": {
                        "latitude": lat_err,
                        "longitude": lon_err,
                        "approx_meters": (lat_err.max(lon_err) * 111_320.0).round()
                    },
                    "bounds": {
                        "south": lat - lat_err,
                        "north": lat + lat_err,
                        "west": lon - lon_err,
                        "east": lon + lon_err
                    },
                    "precision": h.trim().len(),
                    "neighbors": neighbors
                }));
            }
            let c = coordinate
                .filter(|c| !c.trim().is_empty())
                .ok_or_else(|| EngineError::empty("hash or coordinate"))?;
            let (lat, lon) = geom::parse_coordinate(&c)?;
            let p = precision.unwrap_or(9);
            let h = geom::geohash_encode(lat, lon, p)?;
            let (_, _, lat_err, lon_err) = geom::geohash_decode(&h)?;
            Ok(json!({
                "geohash": h,
                "latitude": lat,
                "longitude": lon,
                "precision": p.clamp(1, 12),
                "cell_size_approx_m": {
                    "height": (lat_err * 2.0 * 111_320.0).round(),
                    "width": (lon_err * 2.0 * 111_320.0 * lat.to_radians().cos()).round()
                },
                "all_precisions": (1..=12).map(|k| geom::geohash_encode(lat, lon, k).unwrap_or_default()).collect::<Vec<_>>(),
                "neighbors": geom::geohash_neighbors(&h)?.into_iter().map(|(k, v)| (k, Value::String(v))).collect::<Map<_,
                _>>()
            }))
        },
        GeoAction::Utm {
            coordinate,
            zone,
            hemisphere,
            easting,
            northing,
        } => {
            if let (Some(z), Some(e), Some(n)) = (zone, easting, northing) {
                let south = hemisphere
                    .as_deref()
                    .map(|h| h.to_ascii_uppercase().starts_with('S'))
                    .unwrap_or(false);
                let (lat, lon) = geom::from_utm(z, south, e, n)?;
                let mut v = coordinate_json(lat, lon)?;
                v["input"] = json!({
                    "zone": z,
                    "hemisphere": if south { "S" } else { "N" },
                    "easting": e,
                    "northing": n
                });
                return Ok(v);
            }
            let c = coordinate
                .filter(|c| !c.trim().is_empty())
                .ok_or_else(|| EngineError::empty("coordinate or zone/easting/northing"))?;
            let (lat, lon) = geom::parse_coordinate(&c)?;
            let u = geom::to_utm(lat, lon)?;
            Ok(json!({
                "latitude": lat,
                "longitude": lon,
                "zone": u.zone,
                "band": u.band,
                "hemisphere": u.hemisphere,
                "easting": (u.easting * 1000.0).round() / 1000.0,
                "northing": (u.northing * 1000.0).round() / 1000.0,
                "display": format!("{}{} {:.0}mE {:.0}mN", u.zone, u.band, u.easting, u.northing),
                "epsg": if u.hemisphere == 'N' { 32600 + u.zone as u32 } else { 32700 + u.zone as u32 },
                "central_meridian": (u.zone as f64 - 1.0) * 6.0 - 180.0 + 3.0,
                "datum": "WGS84"
            }))
        },
        GeoAction::Destination { from, bearing, distance_m } => {
            let (lat, lon) = geom::parse_coordinate(&from)?;
            if distance_m < 0.0 || distance_m > 40_075_000.0 {
                return Err(EngineError::invalid("distance_m must be between 0 and 40,075,000"));
            }
            let (dlat, dlon) = geom::destination(lat, lon, bearing, distance_m);
            Ok(json!({
                "from": { "latitude": lat, "longitude": lon },
                "bearing": bearing.rem_euclid(360.0),
                "distance_m": distance_m,
                "destination": {
                    "latitude": (dlat * 1e6).round() / 1e6,
                    "longitude": (dlon * 1e6).round() / 1e6
                },
                "geojson_line": {
                    "type": "LineString",
                    "coordinates": [[lon, lat], [dlon, dlat]]
                }
            }))
        },
    }
}

fn fmt_hours(h: f64) -> String {
    if h < 1.0 / 60.0 {
        return format!("{:.0} s", h * 3600.0);
    }
    let total_min = (h * 60.0).round() as u64;
    if total_min < 60 {
        format!("{total_min} min")
    } else if total_min < 60 * 48 {
        format!("{}h {:02}m", total_min / 60, total_min % 60)
    } else {
        format!("{:.1} days", h / 24.0)
    }
}

engine_shared::export_engine!(handle_action, GeoAction);
