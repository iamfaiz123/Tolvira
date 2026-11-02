//! 3D mesh engine: STL (ASCII + binary) and Wavefront OBJ parsing, conversion,
//! inspection (bounds, surface area, signed volume, watertightness), affine
//! transforms, basic repair (degenerate/duplicate removal, normal recomputation),
//! GLB ↔ glTF container split/merge and a tiny software rasteriser for thumbnails.
//!
//! Meshes are triangle soups internally (`Vec<[[f32; 3]; 3]>`); OBJ export
//! re-indexes vertices so the output stays compact.

use engine_shared::error::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Upper bound on triangles to keep memory predictable inside the worker.
const MAX_TRIANGLES: usize = 5_000_000;

/// Mesh input: a base64 file plus optional original file name.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct FileInput {
    /// File bytes (base64 on the wire).
    #[serde(with = "engine_shared::b64")]
    pub file: Vec<u8>,
    /// Original file name (used for format detection and output names).
    pub name: Option<String>,
}

/// A triangle as three `[x, y, z]` vertices.
pub type Tri = [[f32; 3]; 3];

/// Mesh file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Binary STL.
    StlBinary,
    /// ASCII STL.
    StlAscii,
    /// Wavefront OBJ.
    Obj,
    /// Binary glTF container.
    Glb,
    /// glTF JSON.
    Gltf,
}

impl Format {
    fn label(self) -> &'static str {
        match self {
            Format::StlBinary => "stl-binary",
            Format::StlAscii => "stl-ascii",
            Format::Obj => "obj",
            Format::Glb => "glb",
            Format::Gltf => "gltf",
        }
    }
}

/// Detects the format from magic bytes, content and file extension.
pub fn detect(data: &[u8], name: Option<&str>) -> EngineResult<Format> {
    if data.len() >= 12 && &data[0..4] == b"glTF" {
        return Ok(Format::Glb);
    }
    let head = String::from_utf8_lossy(&data[..data.len().min(512)]).to_string();
    let trimmed = head.trim_start();
    if trimmed.starts_with('{') && head.contains("\"asset\"") {
        return Ok(Format::Gltf);
    }
    let ext = name
        .and_then(|n| n.rsplit('.').next())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if trimmed.starts_with("solid") && data.len() >= 84 {
        // Binary STLs occasionally start with "solid"; trust the triangle count instead.
        let n = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        if 84 + n * 50 == data.len() {
            return Ok(Format::StlBinary);
        }
    }
    if trimmed.starts_with("solid") && head.contains("facet") {
        return Ok(Format::StlAscii);
    }
    if trimmed.starts_with("solid") && ext == "stl" {
        return Ok(Format::StlAscii);
    }
    if head.lines().any(|l| {
        l.starts_with("v ")
            || l.starts_with("f ")
            || l.starts_with("o ")
            || l.starts_with("# ")
            || l.starts_with("mtllib")
    }) && ext != "stl"
    {
        return Ok(Format::Obj);
    }
    if data.len() >= 84 {
        let n = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        if 84 + n * 50 == data.len() {
            return Ok(Format::StlBinary);
        }
    }
    if ext == "obj" {
        return Ok(Format::Obj);
    }
    if ext == "stl" {
        return Ok(Format::StlBinary); // let the parser report the exact truncation
    }
    Err(EngineError::unsupported_format(
        "Not a recognised STL, OBJ, GLB or glTF file (supported: binary/ASCII STL, Wavefront OBJ, \
         glTF 2.0 containers)",
    ))
}

/// Parses binary STL (validates the declared triangle count against the file size).
pub fn parse_stl_binary(data: &[u8]) -> EngineResult<Vec<Tri>> {
    if data.len() < 84 {
        return Err(EngineError::malformed("Binary STL is shorter than its 84-byte header"));
    }
    let n = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
    if n > MAX_TRIANGLES {
        return Err(EngineError::limit(format!(
            "STL declares {n} triangles; limit is {MAX_TRIANGLES}"
        )));
    }
    if data.len() < 84 + n * 50 {
        return Err(EngineError::malformed(format!(
            "Binary STL truncated: header declares {n} triangles but only {} bytes of data follow",
            data.len() - 84
        )));
    }
    let f = |o: usize| f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
    let mut tris = Vec::with_capacity(n);
    for i in 0..n {
        let b = 84 + i * 50 + 12;
        let mut t = [[0f32; 3]; 3];
        for v in 0..3 {
            for c in 0..3 {
                let x = f(b + v * 12 + c * 4);
                if !x.is_finite() {
                    return Err(EngineError::malformed(format!(
                        "Triangle {i} contains a non-finite coordinate"
                    )));
                }
                t[v][c] = x;
            }
        }
        tris.push(t);
    }
    Ok(tris)
}

/// Parses ASCII STL facets.
pub fn parse_stl_ascii(text: &str) -> EngineResult<Vec<Tri>> {
    let mut tris = Vec::new();
    let mut cur: Vec<[f32; 3]> = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("vertex") => {
                let mut v = [0f32; 3];
                for c in 0..3 {
                    v[c] = it
                        .next()
                        .and_then(|s| s.parse::<f32>().ok())
                        .filter(|x| x.is_finite())
                        .ok_or_else(|| {
                            EngineError::malformed(format!(
                                "Line {}: vertex needs three numeric coordinates",
                                ln + 1
                            ))
                        })?;
                }
                cur.push(v);
            },
            Some("endloop") => {
                if cur.len() != 3 {
                    return Err(EngineError::malformed(format!(
                        "Line {}: facet loop has {} vertices, expected 3",
                        ln + 1,
                        cur.len()
                    )));
                }
                tris.push([cur[0], cur[1], cur[2]]);
                cur.clear();
                if tris.len() > MAX_TRIANGLES {
                    return Err(EngineError::limit(format!("More than {MAX_TRIANGLES} triangles")));
                }
            },
            _ => {},
        }
    }
    if tris.is_empty() {
        return Err(EngineError::malformed("ASCII STL contains no facets"));
    }
    Ok(tris)
}

/// Parses OBJ vertices/faces (polygons are triangulated); returns triangles plus OBJ statistics.
pub fn parse_obj(text: &str) -> EngineResult<(Vec<Tri>, Value)> {
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris = Vec::new();
    let (mut objects, mut groups, mut polygons, mut quads) = (0usize, 0usize, 0usize, 0usize);
    for (ln, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let mut v = [0f32; 3];
                for c in 0..3 {
                    v[c] = it
                        .next()
                        .and_then(|s| s.parse::<f32>().ok())
                        .filter(|x| x.is_finite())
                        .ok_or_else(|| {
                            EngineError::malformed(format!(
                                "Line {}: 'v' needs three numeric coordinates",
                                ln + 1
                            ))
                        })?;
                }
                verts.push(v);
            },
            Some("f") => {
                let mut idx = Vec::new();
                for tok in it {
                    let i: i64 = tok.split('/').next().unwrap_or("").parse().map_err(|_| {
                        EngineError::malformed(format!("Line {}: bad face index '{tok}'", ln + 1))
                    })?;
                    let resolved = if i < 0 { verts.len() as i64 + i } else { i - 1 };
                    if resolved < 0 || resolved as usize >= verts.len() {
                        return Err(EngineError::malformed(format!(
                            "Line {}: face references vertex {i} but only {} vertices are defined",
                            ln + 1,
                            verts.len()
                        )));
                    }
                    idx.push(resolved as usize);
                }
                if idx.len() < 3 {
                    return Err(EngineError::malformed(format!(
                        "Line {}: face needs at least 3 vertices",
                        ln + 1
                    )));
                }
                polygons += 1;
                if idx.len() == 4 {
                    quads += 1;
                }
                for k in 1..idx.len() - 1 {
                    tris.push([verts[idx[0]], verts[idx[k]], verts[idx[k + 1]]]);
                }
                if tris.len() > MAX_TRIANGLES {
                    return Err(EngineError::limit(format!("More than {MAX_TRIANGLES} triangles")));
                }
            },
            Some("o") => objects += 1,
            Some("g") => groups += 1,
            _ => {},
        }
    }
    if verts.is_empty() {
        return Err(EngineError::malformed("OBJ contains no 'v' vertex lines"));
    }
    let extra = json!({
        "vertices": verts.len(),
        "faces": polygons,
        "quads": quads,
        "objects": objects,
        "groups": groups,
        "points_only": tris.is_empty()
    });
    Ok((tris, extra))
}

/// Decodes the input into triangles (glTF geometry returns `E_UNSUPPORTED`).
pub fn load(input: &FileInput) -> EngineResult<(Format, Vec<Tri>, Value)> {
    engine_shared::error::require_bytes(&input.file, "3D model file")?;
    let fmt = detect(&input.file, input.name.as_deref())?;
    match fmt {
        Format::StlBinary => Ok((fmt, parse_stl_binary(&input.file)?, json!({}))),
        Format::StlAscii => {
            Ok((fmt, parse_stl_ascii(&String::from_utf8_lossy(&input.file))?, json!({})))
        },
        Format::Obj => {
            let (t, extra) = parse_obj(&String::from_utf8_lossy(&input.file))?;
            Ok((fmt, t, extra))
        },
        Format::Glb | Format::Gltf => Err(EngineError::unsupported(
            "glTF geometry decoding (accessors/buffer views) is not implemented; use glb_to_gltf \
             / gltf_inspect for container operations or the browser three.js viewer",
        )),
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}
/// Unit normal of a triangle (zero vector for degenerate triangles).
pub fn normal(t: &Tri) -> [f32; 3] {
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let l = norm(n);
    if l > 0.0 {
        [n[0] / l, n[1] / l, n[2] / l]
    } else {
        [0.0, 0.0, 0.0]
    }
}
fn area(t: &Tri) -> f64 {
    norm(cross(sub(t[1], t[0]), sub(t[2], t[0]))) as f64 / 2.0
}
fn signed_volume(t: &Tri) -> f64 {
    dot(t[0], cross(t[1], t[2])) as f64 / 6.0
}
fn key(v: [f32; 3]) -> [u32; 3] {
    [v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]
}

/// Geometry statistics computed by [`stats`].
pub struct Stats {
    /// Axis-aligned bounding box `(min, max)`.
    pub bounds: ([f32; 3], [f32; 3]),
    /// Surface area.
    pub area: f64,
    /// Signed volume (negative when normals point inwards).
    pub volume: f64,
    /// Number of distinct vertices.
    pub unique_vertices: usize,
    /// Number of distinct edges.
    pub edges: usize,
    /// Edges used by exactly one triangle (holes).
    pub boundary_edges: usize,
    /// Edges used by more than two triangles.
    pub non_manifold_edges: usize,
    /// Zero-area triangles.
    pub degenerate: usize,
}

/// Computes bounds, area, volume, vertex/edge counts and manifold checks.
pub fn stats(tris: &[Tri]) -> Stats {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    let mut verts: HashMap<[u32; 3], u32> = HashMap::new();
    let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
    let (mut area_sum, mut vol, mut degenerate) = (0f64, 0f64, 0usize);
    for t in tris {
        let mut ids = [0u32; 3];
        for (k, v) in t.iter().enumerate() {
            for c in 0..3 {
                lo[c] = lo[c].min(v[c]);
                hi[c] = hi[c].max(v[c]);
            }
            let n = verts.len() as u32;
            ids[k] = *verts.entry(key(*v)).or_insert(n);
        }
        if area(t) <= 0.0 || ids[0] == ids[1] || ids[1] == ids[2] || ids[0] == ids[2] {
            degenerate += 1;
        }
        for k in 0..3 {
            let (a, b) = (ids[k], ids[(k + 1) % 3]);
            *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
        area_sum += area(t);
        vol += signed_volume(t);
    }
    if tris.is_empty() {
        lo = [0.0; 3];
        hi = [0.0; 3];
    }
    Stats {
        bounds: (lo, hi),
        area: area_sum,
        volume: vol,
        unique_vertices: verts.len(),
        edges: edges.len(),
        boundary_edges: edges.values().filter(|&&c| c == 1).count(),
        non_manifold_edges: edges.values().filter(|&&c| c > 2).count(),
        degenerate,
    }
}

fn r(x: f64) -> f64 {
    (x * 10000.0).round() / 10000.0
}

/// Reports format, triangle/vertex counts, bounds, area, volume, watertightness and warnings.
pub fn inspect(input: &FileInput, unit: &str) -> EngineResult<Value> {
    let fmt = detect(
        engine_shared::error::require_bytes(&input.file, "3D model file")?,
        input.name.as_deref(),
    )?;
    if matches!(fmt, Format::Glb | Format::Gltf) {
        return gltf_inspect(input);
    }
    let (_, tris, extra) = load(input)?;
    let s = stats(&tris);
    let size = [
        s.bounds.1[0] - s.bounds.0[0],
        s.bounds.1[1] - s.bounds.0[1],
        s.bounds.1[2] - s.bounds.0[2],
    ];
    let unit = if unit.is_empty() { "mm" } else { unit };
    let watertight = s.boundary_edges == 0 && s.non_manifold_edges == 0 && !tris.is_empty();
    let mut warnings: Vec<&str> = Vec::new();
    if !watertight && !tris.is_empty() {
        warnings.push("Mesh is not watertight – volume is an estimate");
    }
    if s.degenerate > 0 {
        warnings.push("Contains degenerate (zero-area) triangles; run repair");
    }
    // Cubic-centimetre factor per unit (volume is in unit³); unknown units get no conversion.
    let cm3_factor = match unit {
        "mm" => Some(1.0 / 1000.0),
        "cm" => Some(1.0),
        "m" => Some(1_000_000.0),
        "in" | "inch" => Some(16.387_064),
        _ => None,
    };
    let (vol_cm3, mass_pla_g) = match cm3_factor {
        Some(f) => (s.volume.abs() * f, s.volume.abs() * f * 1.24),
        None => (f64::NAN, f64::NAN),
    };
    Ok(json!({
        "format": fmt.label(),
        "bytes": input.file.len(),
        "unit": unit,
        "triangles": tris.len(),
        "unique_vertices": s.unique_vertices,
        "edges": s.edges,
        "bounds": {
            "min": s.bounds.0,
            "max": s.bounds.1,
            "size": size,
            "center": [
                (s.bounds.0[0] + s.bounds.1[0]) / 2.0,
                (s.bounds.0[1] + s.bounds.1[1]) / 2.0,
                (s.bounds.0[2] + s.bounds.1[2]) / 2.0
            ]
        },
        "surface_area": r(s.area),
        "volume": r(s.volume.abs()),
        "volume_signed": r(s.volume),
        "inverted_normals_suspected": s.volume < 0.0 && watertight,
        "volume_cm3": if vol_cm3.is_nan() { Value::Null } else { json!(r(vol_cm3)) },
        "estimated_mass_pla_grams": if mass_pla_g.is_nan() { Value::Null } else { json!(r(mass_pla_g)) },
        "watertight": watertight,
        "boundary_edges": s.boundary_edges,
        "non_manifold_edges": s.non_manifold_edges,
        "degenerate_triangles": s.degenerate,
        "volume_reliable": watertight,
        "obj": if extra.as_object().map(|o| o.is_empty()).unwrap_or(true) { Value::Null } else { extra },
        "warnings": warnings,
    }))
}

/// Serialises triangles as binary STL.
pub fn write_stl_binary(tris: &[Tri], name: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(84 + tris.len() * 50);
    let mut header = format!("Dev Toolbox STL: {name}").into_bytes();
    header.resize(80, 0);
    out.extend(header);
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for t in tris {
        for c in normal(t).iter().chain(t.iter().flatten()) {
            out.extend_from_slice(&c.to_le_bytes());
        }
        out.extend_from_slice(&[0, 0]);
    }
    out
}

/// Serialises triangles as ASCII STL.
pub fn write_stl_ascii(tris: &[Tri], name: &str) -> String {
    let mut s = format!("solid {name}\n");
    for t in tris {
        let n = normal(t);
        s.push_str(&format!("  facet normal {:e} {:e} {:e}\n    outer loop\n", n[0], n[1], n[2]));
        for v in t {
            s.push_str(&format!("      vertex {:e} {:e} {:e}\n", v[0], v[1], v[2]));
        }
        s.push_str("    endloop\n  endfacet\n");
    }
    s.push_str(&format!("endsolid {name}\n"));
    s
}

/// Serialises triangles as OBJ with de-duplicated vertices.
pub fn write_obj(tris: &[Tri], name: &str) -> String {
    let mut verts: HashMap<[u32; 3], usize> = HashMap::new();
    let mut order: Vec<[f32; 3]> = Vec::new();
    let mut faces = Vec::with_capacity(tris.len());
    for t in tris {
        let mut f = [0usize; 3];
        for k in 0..3 {
            let n = order.len();
            f[k] = *verts.entry(key(t[k])).or_insert_with(|| {
                order.push(t[k]);
                n
            });
        }
        faces.push(f);
    }
    let mut s = format!("# Dev Toolbox OBJ export\no {name}\n");
    for v in &order {
        s.push_str(&format!("v {} {} {}\n", v[0], v[1], v[2]));
    }
    for f in &faces {
        s.push_str(&format!("f {} {} {}\n", f[0] + 1, f[1] + 1, f[2] + 1));
    }
    s
}

fn base_name(input: &FileInput) -> String {
    input
        .name
        .as_deref()
        .and_then(|n| n.rsplit('/').next())
        .map(|n| {
            n.rsplit_once('.')
                .map(|(b, _)| b.to_string())
                .unwrap_or_else(|| n.to_string())
        })
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| "model".into())
}

fn emit(tris: &[Tri], to: &str, base: &str, input_len: usize) -> EngineResult<Value> {
    let (bytes, ext, mime) = match to.to_ascii_lowercase().as_str() {
        "stl" | "stl-binary" | "stl_binary" => (write_stl_binary(tris, base), "stl", "model/stl"),
        "stl-ascii" | "stl_ascii" => (write_stl_ascii(tris, base).into_bytes(), "stl", "model/stl"),
        "obj" => (write_obj(tris, base).into_bytes(), "obj", "model/obj"),
        other => {
            return Err(EngineError::invalid(format!(
                "Unsupported output format '{other}' (use stl, stl-ascii or obj)"
            )))
        },
    };
    Ok(json!({
        "file": engine_shared::b64::encode(&bytes),
        "filename": format!("{base}.{ext}"),
        "mime": mime,
        "bytes": bytes.len(),
        "input_bytes": input_len,
        "triangles": tris.len()
    }))
}

/// Converts between STL (binary/ASCII) and OBJ.
pub fn convert(input: &FileInput, to: &str) -> EngineResult<Value> {
    let (fmt, tris, _) = load(input)?;
    let mut v = emit(
        &tris,
        if to.is_empty() { "stl" } else { to },
        &base_name(input),
        input.file.len(),
    )?;
    v["source_format"] = json!(fmt.label());
    Ok(v)
}

/// Applies scale → rotate (degrees about X, Y, Z in that order) → translate, optionally centring
/// first.
pub fn transform(
    input: &FileInput,
    scale: [f64; 3],
    rotate_deg: [f64; 3],
    translate: [f64; 3],
    center: bool,
    to: &str,
) -> EngineResult<Value> {
    let (fmt, mut tris, _) = load(input)?;
    for s in scale {
        if !s.is_finite() || s == 0.0 {
            return Err(EngineError::invalid("Scale factors must be finite and non-zero"));
        }
    }
    if rotate_deg.iter().chain(translate.iter()).any(|x| !x.is_finite()) {
        return Err(EngineError::invalid("Rotation and translation must be finite numbers"));
    }
    let st = stats(&tris);
    let c = if center {
        [
            (st.bounds.0[0] + st.bounds.1[0]) / 2.0,
            (st.bounds.0[1] + st.bounds.1[1]) / 2.0,
            (st.bounds.0[2] + st.bounds.1[2]) / 2.0,
        ]
    } else {
        [0.0; 3]
    };
    let (rx, ry, rz) = (
        rotate_deg[0].to_radians(),
        rotate_deg[1].to_radians(),
        rotate_deg[2].to_radians(),
    );
    let ((sx, cx), (sy, cy), (sz, cz)) = (rx.sin_cos(), ry.sin_cos(), rz.sin_cos());
    let apply = |v: [f32; 3]| -> [f32; 3] {
        let p = [
            (v[0] - c[0]) as f64 * scale[0],
            (v[1] - c[1]) as f64 * scale[1],
            (v[2] - c[2]) as f64 * scale[2],
        ];
        let p = [p[0], p[1] * cx - p[2] * sx, p[1] * sx + p[2] * cx];
        let p = [p[0] * cy + p[2] * sy, p[1], -p[0] * sy + p[2] * cy];
        let p = [p[0] * cz - p[1] * sz, p[0] * sz + p[1] * cz, p[2]];
        [
            (p[0] + translate[0]) as f32,
            (p[1] + translate[1]) as f32,
            (p[2] + translate[2]) as f32,
        ]
    };
    let mirrored = scale.iter().filter(|s| **s < 0.0).count() % 2 == 1;
    for t in tris.iter_mut() {
        *t = [apply(t[0]), apply(t[1]), apply(t[2])];
        if mirrored {
            t.swap(1, 2); // keep outward-facing winding after a reflection
        }
    }
    let out_fmt = if to.is_empty() {
        if fmt == Format::Obj {
            "obj"
        } else {
            "stl"
        }
    } else {
        to
    };
    let mut v = emit(&tris, out_fmt, &base_name(input), input.file.len())?;
    let ns = stats(&tris);
    v["bounds"] = json!({ "min": ns.bounds.0, "max": ns.bounds.1 });
    v["volume"] = json!(r(ns.volume.abs()));
    Ok(v)
}

/// Removes degenerate and duplicate triangles, flips an inside-out mesh and re-emits with
/// recomputed normals.
pub fn repair(input: &FileInput, to: &str) -> EngineResult<Value> {
    let (fmt, tris, _) = load(input)?;
    let before = stats(&tris);
    let mut seen: HashMap<[[u32; 3]; 3], ()> = HashMap::new();
    let mut out = Vec::with_capacity(tris.len());
    let (mut degenerate, mut duplicates) = (0usize, 0usize);
    for t in &tris {
        if area(t) <= 0.0 {
            degenerate += 1;
            continue;
        }
        let mut k = [key(t[0]), key(t[1]), key(t[2])];
        k.sort_unstable();
        if seen.insert(k, ()).is_some() {
            duplicates += 1;
            continue;
        }
        out.push(*t);
    }
    let mut flipped = false;
    let mid = stats(&out);
    if mid.volume < 0.0 && mid.boundary_edges == 0 {
        for t in out.iter_mut() {
            t.swap(1, 2);
        }
        flipped = true;
    }
    let after = stats(&out);
    let out_fmt = if to.is_empty() {
        if fmt == Format::Obj {
            "obj"
        } else {
            "stl"
        }
    } else {
        to
    };
    let mut v = emit(&out, out_fmt, &format!("{}-repaired", base_name(input)), input.file.len())?;
    v["removed_degenerate"] = json!(degenerate);
    v["removed_duplicates"] = json!(duplicates);
    v["flipped_inside_out"] = json!(flipped);
    v["normals_recomputed"] = json!(true);
    v["boundary_edges_before"] = json!(before.boundary_edges);
    v["boundary_edges_after"] = json!(after.boundary_edges);
    v["watertight"] = json!(after.boundary_edges == 0 && after.non_manifold_edges == 0);
    v["note"] =
        json!("Hole filling and self-intersection repair are out of scope for the basic repair");
    Ok(v)
}

/// Software-rasterised thumbnail: orthographic projection with a 3/4 view, flat shading, depth
/// buffer.
pub fn thumbnail(
    input: &FileInput,
    width: u32,
    height: u32,
    background: &str,
) -> EngineResult<Value> {
    let (_, tris, _) = load(input)?;
    if tris.is_empty() {
        return Err(EngineError::empty("mesh triangles"));
    }
    let (w, h) = (width.clamp(16, 2048), height.clamp(16, 2048));
    let bg = parse_hex(if background.is_empty() { "#f4f5f7" } else { background })?;
    let st = stats(&tris);
    let c = [
        (st.bounds.0[0] + st.bounds.1[0]) / 2.0,
        (st.bounds.0[1] + st.bounds.1[1]) / 2.0,
        (st.bounds.0[2] + st.bounds.1[2]) / 2.0,
    ];
    let (ay, ax) = (-35f32.to_radians(), 25f32.to_radians());
    let rot = |v: [f32; 3]| -> [f32; 3] {
        let p = sub(v, c);
        let p = [p[0] * ay.cos() + p[2] * ay.sin(), p[1], -p[0] * ay.sin() + p[2] * ay.cos()];
        [p[0], p[1] * ax.cos() - p[2] * ax.sin(), p[1] * ax.sin() + p[2] * ax.cos()]
    };
    let proj: Vec<Tri> = tris.iter().map(|t| [rot(t[0]), rot(t[1]), rot(t[2])]).collect();
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for t in &proj {
        for v in t {
            for k in 0..2 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-6);
    let scale = (w.min(h) as f32 * 0.86) / span;
    let (ox, oy) = (
        w as f32 / 2.0 - (lo[0] + hi[0]) / 2.0 * scale,
        h as f32 / 2.0 + (lo[1] + hi[1]) / 2.0 * scale,
    );
    let mut img = vec![bg; (w * h) as usize];
    let mut depth = vec![f32::MIN; (w * h) as usize];
    let light = [0.3f32, 0.5, 0.81];
    for t in &proj {
        let n = normal(t);
        let shade = (dot(n, light).abs() * 0.75 + 0.25).min(1.0);
        let col = [
            (70.0 + 150.0 * shade) as u8,
            (110.0 + 130.0 * shade) as u8,
            (170.0 + 80.0 * shade) as u8,
            255,
        ];
        let p: Vec<[f32; 3]> = t
            .iter()
            .map(|v| [v[0] * scale + ox, oy - v[1] * scale, v[2]])
            .collect();
        let (minx, maxx) = (
            p.iter().map(|v| v[0]).fold(f32::MAX, f32::min).floor().max(0.0) as u32,
            p.iter()
                .map(|v| v[0])
                .fold(f32::MIN, f32::max)
                .ceil()
                .min(w as f32 - 1.0) as u32,
        );
        let (miny, maxy) = (
            p.iter().map(|v| v[1]).fold(f32::MAX, f32::min).floor().max(0.0) as u32,
            p.iter()
                .map(|v| v[1])
                .fold(f32::MIN, f32::max)
                .ceil()
                .min(h as f32 - 1.0) as u32,
        );
        let det =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        if det.abs() < 1e-9 || minx > maxx || miny > maxy {
            continue;
        }
        for y in miny..=maxy {
            for x in minx..=maxx {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let l1 = ((p[1][0] - px) * (p[2][1] - py) - (p[2][0] - px) * (p[1][1] - py)) / det;
                let l2 = ((p[2][0] - px) * (p[0][1] - py) - (p[0][0] - px) * (p[2][1] - py)) / det;
                let l3 = 1.0 - l1 - l2;
                if l1 >= 0.0 && l2 >= 0.0 && l3 >= 0.0 {
                    let z = l1 * p[0][2] + l2 * p[1][2] + l3 * p[2][2];
                    let i = (y * w + x) as usize;
                    if z > depth[i] {
                        depth[i] = z;
                        img[i] = col;
                    }
                }
            }
        }
    }
    let mut png = Vec::new();
    let buf: Vec<u8> = img.iter().flatten().copied().collect();
    image::write_buffer_with_format(
        &mut std::io::Cursor::new(&mut png),
        &buf,
        w,
        h,
        image::ExtendedColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| EngineError::internal(format!("PNG encode failed: {e}")))?;
    Ok(json!({
        "png": engine_shared::b64::encode(&png),
        "mime": "image/png",
        "width": w,
        "height": h,
        "triangles": tris.len(),
        "bytes": png.len(),
        "view": "orthographic 3/4, flat shaded"
    }))
}

fn parse_hex(s: &str) -> EngineResult<[u8; 4]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(EngineError::invalid(format!(
            "Background must be a #RRGGBB colour, got '{s}'"
        )));
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
    Ok([p(0), p(2), p(4), 255])
}

// ---------- glTF container handling ----------

fn glb_chunks(data: &[u8]) -> EngineResult<(Value, Option<Vec<u8>>)> {
    if data.len() < 20 || &data[0..4] != b"glTF" {
        return Err(EngineError::unsupported_format("Not a GLB container"));
    }
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if version != 2 {
        return Err(EngineError::unsupported(format!(
            "GLB version {version} is not supported (only glTF 2.0)"
        )));
    }
    let total = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
    if total > data.len() {
        return Err(EngineError::malformed(format!(
            "GLB header declares {total} bytes but file has {}",
            data.len()
        )));
    }
    let (mut o, mut json_chunk, mut bin) = (12usize, None, None);
    while o + 8 <= total {
        let len = u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize;
        let ty = &data[o + 4..o + 8];
        let body = data
            .get(o + 8..o + 8 + len)
            .ok_or_else(|| EngineError::malformed("GLB chunk is truncated"))?;
        match ty {
            b"JSON" => {
                json_chunk = Some(serde_json::from_slice::<Value>(body).map_err(|e| {
                    EngineError::malformed(format!("GLB JSON chunk is invalid: {e}"))
                })?)
            },
            b"BIN\0" => bin = Some(body.to_vec()),
            _ => {},
        }
        o += 8 + len;
    }
    Ok((json_chunk.ok_or_else(|| EngineError::malformed("GLB has no JSON chunk"))?, bin))
}

fn gltf_json(input: &FileInput) -> EngineResult<(Value, Option<Vec<u8>>, bool)> {
    engine_shared::error::require_bytes(&input.file, "glTF file")?;
    if input.file.starts_with(b"glTF") {
        let (j, b) = glb_chunks(&input.file)?;
        return Ok((j, b, true));
    }
    if !String::from_utf8_lossy(&input.file[..input.file.len().min(64)])
        .trim_start()
        .starts_with('{')
    {
        return Err(EngineError::unsupported_format(
            "Not a glTF document (expected a GLB container or JSON starting with '{')",
        ));
    }
    let j: Value = serde_json::from_slice(&input.file)
        .map_err(|e| EngineError::malformed(format!("glTF JSON is invalid: {e}")))?;
    if j.get("asset").is_none() {
        return Err(EngineError::unsupported_format(
            "JSON is not a glTF document (missing 'asset')",
        ));
    }
    Ok((j, None, false))
}

/// Reports the structure of a glTF/GLB file (scenes, meshes, materials, buffers, extensions).
pub fn gltf_inspect(input: &FileInput) -> EngineResult<Value> {
    let (j, bin, is_glb) = gltf_json(input)?;
    let count = |k: &str| j.get(k).and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    let prims: usize = j
        .get("meshes")
        .and_then(|m| m.as_array())
        .map(|m| {
            m.iter()
                .map(|x| {
                    x.get("primitives")
                        .and_then(|p| p.as_array())
                        .map(|p| p.len())
                        .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0);
    let vertex_est: u64 = j
        .get("accessors")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter(|x| x.get("type").and_then(|t| t.as_str()) == Some("VEC3"))
                .filter_map(|x| x.get("count").and_then(|c| c.as_u64()))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    let external: Vec<String> = ["buffers", "images"]
        .iter()
        .flat_map(|k| j.get(*k).and_then(|v| v.as_array()).cloned().unwrap_or_default())
        .filter_map(|b| {
            b.get("uri")
                .and_then(|u| u.as_str())
                .filter(|u| !u.starts_with("data:"))
                .map(String::from)
        })
        .collect();
    Ok(json!({
        "format": if is_glb { "glb" } else { "gltf" },
        "bytes": input.file.len(),
        "asset": j.get("asset").cloned().unwrap_or(Value::Null),
        "scenes": count("scenes"),
        "nodes": count("nodes"),
        "meshes": count("meshes"),
        "primitives": prims,
        "materials": count("materials"),
        "textures": count("textures"),
        "images": count("images"),
        "animations": count("animations"),
        "skins": count("skins"),
        "accessors": count("accessors"),
        "buffers": count("buffers"),
        "binary_chunk_bytes": bin.as_ref().map(|b| b.len()),
        "max_vec3_accessor_count": vertex_est,
        "extensions_used": j.get("extensionsUsed").cloned().unwrap_or(json!([])),
        "extensions_required": j.get("extensionsRequired").cloned().unwrap_or(json!([])),
        "external_resources": external,
        "self_contained": is_glb || external.is_empty(),
    }))
}

/// GLB → glTF JSON with the binary chunk embedded as a data: URI (self-contained, viewer-ready).
pub fn glb_to_gltf(input: &FileInput) -> EngineResult<Value> {
    let (mut j, bin, is_glb) = gltf_json(input)?;
    if !is_glb {
        return Err(EngineError::invalid("Input is already a .gltf JSON document"));
    }
    if let (Some(bin), Some(buffers)) =
        (bin.as_ref(), j.get_mut("buffers").and_then(|b| b.as_array_mut()))
    {
        if let Some(first) = buffers.first_mut() {
            if first.get("uri").is_none() {
                first["uri"] = json!(format!(
                    "data:application/octet-stream;base64,{}",
                    engine_shared::b64::encode(bin)
                ));
                first["byteLength"] = json!(bin.len());
            }
        }
    }
    let text =
        serde_json::to_string_pretty(&j).map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(json!({
        "file": engine_shared::b64::encode(text.as_bytes()),
        "filename": format!("{}.gltf", base_name(input)),
        "mime": "model/gltf+json",
        "bytes": text.len(),
        "input_bytes": input.file.len(),
        "embedded_binary_bytes": bin.map(|b| b.len()).unwrap_or(0)
    }))
}

/// Minimal optimiser: re-packs a GLB/glTF with compact JSON and strips `extras`; reports savings.
pub fn gltf_optimize(input: &FileInput) -> EngineResult<Value> {
    let (mut j, bin, _) = gltf_json(input)?;
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("extras");
                m.values_mut().for_each(strip);
            },
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {},
        }
    }
    strip(&mut j);
    let mut bin = bin.unwrap_or_default();
    if bin.is_empty() {
        if let Some(uri) = j
            .get("buffers")
            .and_then(|b| b.as_array())
            .and_then(|b| b.first())
            .and_then(|b| b.get("uri"))
            .and_then(|u| u.as_str())
        {
            if let Some(b64) = uri.strip_prefix("data:application/octet-stream;base64,") {
                bin = engine_shared::b64::decode(b64)
                    .map_err(|_| EngineError::malformed("Buffer data URI is not valid base64"))?;
                if let Some(first) = j
                    .get_mut("buffers")
                    .and_then(|b| b.as_array_mut())
                    .and_then(|b| b.first_mut())
                {
                    first.as_object_mut().map(|o| o.remove("uri"));
                }
            } else {
                return Err(EngineError::unsupported(
                    "glTF references an external .bin buffer; supply a GLB or a self-contained \
                     glTF",
                ));
            }
        }
    }
    let mut jb = serde_json::to_vec(&j).map_err(|e| EngineError::internal(e.to_string()))?;
    while !jb.len().is_multiple_of(4) {
        jb.push(b' ');
    }
    let bin_pad = (4 - bin.len() % 4) % 4;
    let total = 12 + 8 + jb.len() + if bin.is_empty() { 0 } else { 8 + bin.len() + bin_pad };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(jb.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend(jb);
    if !bin.is_empty() {
        out.extend_from_slice(&((bin.len() + bin_pad) as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend(bin);
        out.extend(std::iter::repeat_n(0u8, bin_pad));
    }
    Ok(json!({
        "file": engine_shared::b64::encode(&out),
        "filename": format!("{}-optimized.glb", base_name(input)),
        "mime": "model/gltf-binary",
        "bytes": out.len(),
        "input_bytes": input.file.len(),
        "saved_percent": r((1.0 - out.len() as f64 / input.file.len() as f64) * 100.0),
        "note": "Structural repack only (compact JSON, extras stripped, binary embedded); mesh \
                 decimation / Draco / meshopt compression are not applied"
    }))
}

/// Every action understood by the 3D engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize, Debug)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum MeshAction {
    /// Mesh inspection.
    Inspect {
        /// Input mesh.
        #[serde(flatten)]
        input: FileInput,
        /// `mm` (default), `cm`, `m` or `in` for volume/mass figures.
        #[serde(default)]
        unit: String,
    },
    /// Format conversion.
    Convert {
        /// Input mesh.
        #[serde(flatten)]
        input: FileInput,
        /// Target format (`stl`, `stl-ascii`, `obj`).
        #[serde(default)]
        to: String,
    },
    /// Scale / rotate / translate.
    Transform {
        /// Input mesh.
        #[serde(flatten)]
        input: FileInput,
        /// Scale factors per axis.
        #[serde(default = "one3")]
        scale: [f64; 3],
        /// Rotation in degrees about X, Y, Z (applied in that order).
        #[serde(default)]
        rotate: [f64; 3],
        /// Translation per axis.
        #[serde(default)]
        translate: [f64; 3],
        /// Centre the mesh on the origin before transforming.
        #[serde(default)]
        center: bool,
        /// `stl`, `stl-ascii` or `obj` (default: same as input).
        #[serde(default)]
        to: String,
    },
    /// Basic repair (degenerate/duplicate removal, winding flip).
    Repair {
        /// Input mesh.
        #[serde(flatten)]
        input: FileInput,
        /// `stl`, `stl-ascii` or `obj` (default: same as input).
        #[serde(default)]
        to: String,
    },
    /// Software-rendered PNG thumbnail.
    Thumbnail {
        /// Input mesh.
        #[serde(flatten)]
        input: FileInput,
        /// Image width.
        #[serde(default = "d256")]
        width: u32,
        /// Image height.
        #[serde(default = "d256")]
        height: u32,
        /// Background colour (`#rrggbb`).
        #[serde(default)]
        background: String,
    },
    /// glTF structure report.
    GltfInspect {
        /// Input glTF/GLB.
        #[serde(flatten)]
        input: FileInput,
    },
    /// GLB → glTF JSON with the binary chunk embedded.
    GlbToGltf {
        /// Input GLB.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Repacks as a compact GLB (extras stripped, buffer embedded).
    GltfOptimize {
        /// Input glTF/GLB.
        #[serde(flatten)]
        input: FileInput,
    },
}
fn one3() -> [f64; 3] {
    [1.0; 3]
}
fn d256() -> u32 {
    256
}

/// Dispatches a [`MeshAction`] to its implementation.
pub fn handle_action(action: MeshAction) -> EngineResult<Value> {
    match action {
        MeshAction::Inspect { input, unit } => inspect(&input, &unit),
        MeshAction::Convert { input, to } => convert(&input, &to),
        MeshAction::Transform {
            input,
            scale,
            rotate,
            translate,
            center,
            to,
        } => transform(&input, scale, rotate, translate, center, &to),
        MeshAction::Repair { input, to } => repair(&input, &to),
        MeshAction::Thumbnail { input, width, height, background } => {
            thumbnail(&input, width, height, &background)
        },
        MeshAction::GltfInspect { input } => gltf_inspect(&input),
        MeshAction::GlbToGltf { input } => glb_to_gltf(&input),
        MeshAction::GltfOptimize { input } => gltf_optimize(&input),
    }
}

engine_shared::export_engine!(handle_action, MeshAction);

/// Unit cube (12 triangles, outward normals) used by tests.
pub fn cube(size: f32) -> Vec<Tri> {
    let s = size;
    let p = |x: f32, y: f32, z: f32| [x * s, y * s, z * s];
    let q = |a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]| [[a, b, c], [a, c, d]];
    let mut t = Vec::new();
    t.extend(q(p(0., 0., 0.), p(0., 1., 0.), p(1., 1., 0.), p(1., 0., 0.))); // -z
    t.extend(q(p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.))); // +z
    t.extend(q(p(0., 0., 0.), p(1., 0., 0.), p(1., 0., 1.), p(0., 0., 1.))); // -y
    t.extend(q(p(0., 1., 0.), p(0., 1., 1.), p(1., 1., 1.), p(1., 1., 0.))); // +y
    t.extend(q(p(0., 0., 0.), p(0., 0., 1.), p(0., 1., 1.), p(0., 1., 0.))); // -x
    t.extend(q(p(1., 0., 0.), p(1., 1., 0.), p(1., 1., 1.), p(1., 0., 1.))); // +x
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fi(bytes: Vec<u8>, name: &str) -> FileInput {
        FileInput { file: bytes, name: Some(name.into()) }
    }

    #[test]
    fn cube_roundtrip_and_stats() {
        let c = cube(10.0);
        let bin = write_stl_binary(&c, "cube");
        let i = inspect(&fi(bin.clone(), "cube.stl"), "mm").unwrap();
        assert_eq!(i["format"], "stl-binary");
        assert_eq!(i["triangles"], 12);
        assert_eq!(i["volume"], 1000.0);
        assert_eq!(i["surface_area"], 600.0);
        assert_eq!(i["watertight"], true);
        let ascii = write_stl_ascii(&c, "cube");
        assert_eq!(parse_stl_ascii(&ascii).unwrap().len(), 12);
        let obj = convert(&fi(bin.clone(), "cube.stl"), "obj").unwrap();
        let obj_bytes = engine_shared::b64::decode(obj["file"].as_str().unwrap()).unwrap();
        let (t, extra) = parse_obj(&String::from_utf8_lossy(&obj_bytes)).unwrap();
        assert_eq!(t.len(), 12);
        assert_eq!(extra["vertices"], 8);
        let back = convert(&fi(obj_bytes, "cube.obj"), "stl").unwrap();
        assert_eq!(back["triangles"], 12);
        let tr = transform(
            &fi(bin.clone(), "cube.stl"),
            [2.0, 2.0, 2.0],
            [0.0, 0.0, 90.0],
            [5.0, 0.0, 0.0],
            true,
            "",
        )
        .unwrap();
        assert_eq!(tr["volume"], 8000.0);
        let th = thumbnail(&fi(bin.clone(), "cube.stl"), 64, 64, "").unwrap();
        assert_eq!(th["width"], 64);
        let mut broken = c.clone();
        broken.push([[0.0; 3], [0.0; 3], [1.0, 1.0, 1.0]]);
        broken.push(c[0]);
        let rep = repair(&fi(write_stl_binary(&broken, "b"), "b.stl"), "").unwrap();
        assert_eq!(rep["removed_degenerate"], 1);
        assert_eq!(rep["removed_duplicates"], 1);
        assert_eq!(rep["triangles"], 12);
    }

    #[test]
    fn gltf_and_errors() {
        let j = json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 4 }],
            "meshes": [{ "primitives": [{}] }],
            "extras": { "x": 1 }
        });
        let mut jb = serde_json::to_vec(&j).unwrap();
        while !jb.len().is_multiple_of(4) {
            jb.push(b' ');
        }
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&((12 + 8 + jb.len() + 8 + 4) as u32).to_le_bytes());
        glb.extend_from_slice(&(jb.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend(jb);
        glb.extend_from_slice(&4u32.to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(&[1, 2, 3, 4]);
        let i = inspect(&fi(glb.clone(), "m.glb"), "").unwrap();
        assert_eq!(i["format"], "glb");
        assert_eq!(i["primitives"], 1);
        assert_eq!(i["binary_chunk_bytes"], 4);
        let g = glb_to_gltf(&fi(glb.clone(), "m.glb")).unwrap();
        assert_eq!(g["filename"], "m.gltf");
        let gltf = engine_shared::b64::decode(g["file"].as_str().unwrap()).unwrap();
        let o = gltf_optimize(&fi(gltf, "m.gltf")).unwrap();
        let (j2, bin, _) = gltf_json(&fi(
            engine_shared::b64::decode(o["file"].as_str().unwrap()).unwrap(),
            "o.glb",
        ))
        .unwrap();
        assert!(j2.get("extras").is_none());
        assert_eq!(bin.unwrap(), vec![1, 2, 3, 4]);
        assert!(convert(&fi(glb, "m.glb"), "obj").is_err());
        assert!(inspect(&fi(b"hello world".to_vec(), "x.bin"), "").is_err());
        assert!(parse_obj("v 0 0 0\nv 1 0 0\nf 1 2 9\n").is_err());
        let mut trunc = write_stl_binary(&cube(1.0), "c");
        trunc.truncate(200);
        assert!(parse_stl_binary(&trunc).is_err());
        assert!(inspect(&fi(vec![], "c.stl"), "").is_err());
    }
}
