//! # Dev Toolbox Font Engine (`font-engine`)
//!
//! | Action | Tool(s) |
//! |---|---|
//! | `inspect` | font-metadata-viewer, otf-inspector |
//! | `char_map` | font-character-map |
//! | `preview` | font-preview |
//! | `size_check` | font-size-checker |
//! | `pairing` | font-pairing-tool |
//! | `convert` | font-converter, woff-to-ttf, ttf-to-woff (WOFF 1.0; WOFF2 → `E_UNSUPPORTED`) |
//! | `subset` | font-subsetter (TrueType/CFF via `subsetter`) |
//!
//! Licensing note: converting or subsetting a font may be restricted by its
//! licence; the `inspect` output surfaces the embedded licence text/URL and
//! the OS/2 embedding permissions (`fsType`).

use engine_shared::{EngineError, EngineResult};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use ttf_parser::{name_id, Face, Tag};

/// Font input: a base64 file, optional name and face index for collections.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct FileInput {
    /// File bytes (base64 on the wire).
    #[serde(with = "engine_shared::b64")]
    pub file: Vec<u8>,
    /// Original file name (used to derive output names).
    pub name: Option<String>,
    /// Face index inside a TrueType/OpenType collection.
    pub index: u32,
}

/// Font container detected from magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// TrueType outlines (`0x00010000` / `true`).
    TrueType,
    /// OpenType with CFF outlines (`OTTO`).
    OpenTypeCff,
    /// TrueType/OpenType collection (`ttcf`).
    Collection,
    /// WOFF 1.0.
    Woff,
    /// WOFF 2.0 (detected only; decoding is not implemented).
    Woff2,
    /// Not recognised.
    Unknown,
}

/// Detects the container from the first four bytes.
pub fn detect(bytes: &[u8]) -> Container {
    if bytes.len() < 4 {
        return Container::Unknown;
    }
    match &bytes[..4] {
        [0, 1, 0, 0] | b"true" => Container::TrueType,
        b"OTTO" => Container::OpenTypeCff,
        b"ttcf" => Container::Collection,
        b"wOFF" => Container::Woff,
        b"wOF2" => Container::Woff2,
        _ => Container::Unknown,
    }
}

fn be32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn be16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}

/// Decodes a WOFF 1.0 container into an SFNT (TTF/OTF) byte stream.
pub fn woff_to_sfnt(bytes: &[u8]) -> EngineResult<Vec<u8>> {
    if bytes.len() < 44 || &bytes[..4] != b"wOFF" {
        return Err(EngineError::unsupported_format("Not a WOFF 1.0 file"));
    }
    let flavor = be32(bytes, 4);
    let num_tables = be16(bytes, 12) as usize;
    if bytes.len() < 44 + num_tables * 20 {
        return Err(EngineError::malformed("WOFF table directory is truncated"));
    }
    let mut tables: Vec<(u32, Vec<u8>, u32)> = Vec::new();
    for i in 0..num_tables {
        let o = 44 + i * 20;
        let tag = be32(bytes, o);
        let offset = be32(bytes, o + 4) as usize;
        let comp_len = be32(bytes, o + 8) as usize;
        let orig_len = be32(bytes, o + 12) as usize;
        let checksum = be32(bytes, o + 16);
        if offset + comp_len > bytes.len() {
            return Err(EngineError::malformed(format!("WOFF table {i} points outside the file")));
        }
        let data = if comp_len == orig_len {
            bytes[offset..offset + comp_len].to_vec()
        } else {
            let mut out = Vec::with_capacity(orig_len);
            flate2::read::ZlibDecoder::new(&bytes[offset..offset + comp_len])
                .read_to_end(&mut out)
                .map_err(|e| {
                    EngineError::malformed(format!("WOFF table {i} inflate failed: {e}"))
                })?;
            if out.len() != orig_len {
                return Err(EngineError::malformed(format!(
                    "WOFF table {i} decompressed to {} bytes, expected {orig_len}",
                    out.len()
                )));
            }
            out
        };
        tables.push((tag, data, checksum));
    }
    Ok(build_sfnt(flavor, tables))
}

/// Splits an SFNT into `(tag, data, checksum)` table records.
fn split_sfnt(bytes: &[u8]) -> EngineResult<Vec<(u32, Vec<u8>, u32)>> {
    if bytes.len() < 12 {
        return Err(EngineError::malformed("Font data is truncated"));
    }
    let n = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let mut tables = Vec::with_capacity(n);
    for i in 0..n {
        let o = 12 + i * 16;
        if o + 16 > bytes.len() {
            return Err(EngineError::malformed("Font table directory is truncated"));
        }
        let (tag, checksum, off, len) = (
            be32(bytes, o),
            be32(bytes, o + 4),
            be32(bytes, o + 8) as usize,
            be32(bytes, o + 12) as usize,
        );
        let data = bytes
            .get(off..off + len)
            .ok_or_else(|| EngineError::malformed("Font table points outside the file"))?;
        tables.push((tag, data.to_vec(), checksum));
    }
    Ok(tables)
}

fn table_checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |acc, c| {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        acc.wrapping_add(u32::from_be_bytes(w))
    })
}

/// Builds a `cmap` table with a single format-12 subtable (Unicode full repertoire, platform 3/10
/// and 0/4).
fn build_cmap12(mapping: &[(u32, u16)]) -> Vec<u8> {
    let mut groups: Vec<(u32, u32, u32)> = Vec::new();
    for &(cp, gid) in mapping {
        match groups.last_mut() {
            Some(g) if g.1 + 1 == cp && g.2 + (g.1 - g.0) + 1 == gid as u32 => g.1 = cp,
            _ => groups.push((cp, cp, gid as u32)),
        }
    }
    let sub_len = 16 + groups.len() * 12;
    let mut out = Vec::with_capacity(4 + 16 + sub_len);
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&2u16.to_be_bytes());
    for (platform, encoding) in [(0u16, 4u16), (3, 10)] {
        out.extend_from_slice(&platform.to_be_bytes());
        out.extend_from_slice(&encoding.to_be_bytes());
        out.extend_from_slice(&20u32.to_be_bytes());
    }
    out.extend_from_slice(&12u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(sub_len as u32).to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&(groups.len() as u32).to_be_bytes());
    for (start, end, gid) in groups {
        out.extend_from_slice(&start.to_be_bytes());
        out.extend_from_slice(&end.to_be_bytes());
        out.extend_from_slice(&gid.to_be_bytes());
    }
    out
}

fn build_sfnt(flavor: u32, mut tables: Vec<(u32, Vec<u8>, u32)>) -> Vec<u8> {
    tables.sort_by_key(|t| t.0);
    let n = tables.len() as u16;
    let mut entry_selector = 0u16;
    while (1u16 << (entry_selector + 1)) <= n {
        entry_selector += 1;
    }
    let search_range = (1u16 << entry_selector) * 16;
    let mut out = Vec::new();
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&(n * 16 - search_range).to_be_bytes());
    let mut offset = 12 + tables.len() * 16;
    let mut body = Vec::new();
    for (tag, data, checksum) in &tables {
        out.extend_from_slice(&tag.to_be_bytes());
        out.extend_from_slice(&checksum.to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
        let pad = (4 - data.len() % 4) % 4;
        body.extend(std::iter::repeat_n(0u8, pad));
        offset += data.len() + pad;
    }
    out.extend(body);
    out
}

/// Wraps an SFNT into a WOFF 1.0 container (zlib-compressing tables that shrink).
pub fn sfnt_to_woff(bytes: &[u8]) -> EngineResult<Vec<u8>> {
    if bytes.len() < 12 || !matches!(detect(bytes), Container::TrueType | Container::OpenTypeCff) {
        return Err(EngineError::unsupported_format(
            "WOFF packaging needs a single TrueType/OpenType font (not a collection)",
        ));
    }
    let flavor = be32(bytes, 0);
    let num_tables = be16(bytes, 4) as usize;
    if bytes.len() < 12 + num_tables * 16 {
        return Err(EngineError::malformed("SFNT table directory is truncated"));
    }
    let mut entries = Vec::new();
    let mut total_sfnt = 12 + num_tables * 16;
    for i in 0..num_tables {
        let o = 12 + i * 16;
        let tag = be32(bytes, o);
        let checksum = be32(bytes, o + 4);
        let off = be32(bytes, o + 8) as usize;
        let len = be32(bytes, o + 12) as usize;
        if off + len > bytes.len() {
            return Err(EngineError::malformed(format!("Table {i} points outside the file")));
        }
        let data = &bytes[off..off + len];
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        enc.write_all(data)
            .map_err(|e| EngineError::internal(e.to_string()))?;
        let comp = enc.finish().map_err(|e| EngineError::internal(e.to_string()))?;
        let stored = if comp.len() < data.len() { comp } else { data.to_vec() };
        total_sfnt += len + (4 - len % 4) % 4;
        entries.push((tag, stored, len as u32, checksum));
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"wOFF");
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]); // length (patched)
    out.extend_from_slice(&(num_tables as u16).to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&(total_sfnt as u32).to_be_bytes());
    out.extend_from_slice(&[0, 1, 0, 0]); // version 1.0
    out.extend_from_slice(&[0; 20]); // meta/priv
    let mut offset = 44 + num_tables * 20;
    let mut body = Vec::new();
    for (tag, stored, orig_len, checksum) in &entries {
        out.extend_from_slice(&tag.to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(stored.len() as u32).to_be_bytes());
        out.extend_from_slice(&orig_len.to_be_bytes());
        out.extend_from_slice(&checksum.to_be_bytes());
        body.extend_from_slice(stored);
        let pad = (4 - stored.len() % 4) % 4;
        body.extend(std::iter::repeat_n(0u8, pad));
        offset += stored.len() + pad;
    }
    out.extend(body);
    let total = out.len() as u32;
    out[8..12].copy_from_slice(&total.to_be_bytes());
    Ok(out)
}

/// Returns SFNT bytes for parsing (decoding WOFF when needed) and the container kind.
fn sfnt_bytes(bytes: &[u8]) -> EngineResult<(Vec<u8>, Container)> {
    engine_shared::error::require_bytes(bytes, "font")?;
    engine_shared::limits::check_size(bytes.len(), 64 * 1024 * 1024, "font")?;
    match detect(bytes) {
        Container::Woff => Ok((woff_to_sfnt(bytes)?, Container::Woff)),
        Container::Woff2 => Err(EngineError::unsupported(
            "WOFF2 uses Brotli plus glyf/loca transforms that are not implemented; convert WOFF2 \
             → TTF with the browser FontFace API or the backend (woff2_decompress) first",
        )),
        Container::Unknown => Err(EngineError::unsupported_format(
            "Not a TrueType/OpenType/WOFF font (unknown magic bytes)",
        )),
        c => Ok((bytes.to_vec(), c)),
    }
}

fn parse_face<'a>(data: &'a [u8], index: u32) -> EngineResult<Face<'a>> {
    Face::parse(data, index).map_err(|e| EngineError::malformed(format!("Cannot parse font: {e}")))
}

fn name(face: &Face, id: u16) -> Option<String> {
    let names = face.names();
    // Prefer Windows/Unicode English, then any Unicode, then Mac Roman.
    let mut best: Option<String> = None;
    for n in names.into_iter() {
        if n.name_id != id {
            continue;
        }
        if let Some(s) = n.to_string() {
            if n.is_unicode() && n.language_id == 0x409 {
                return Some(s);
            }
            if best.is_none() || n.is_unicode() {
                best = Some(s);
            }
        }
    }
    best
}

fn unicode_block(cp: u32) -> &'static str {
    match cp {
        0x0000..=0x007F => "Basic Latin",
        0x0080..=0x00FF => "Latin-1 Supplement",
        0x0100..=0x017F => "Latin Extended-A",
        0x0180..=0x024F => "Latin Extended-B",
        0x0250..=0x02AF => "IPA Extensions",
        0x02B0..=0x02FF => "Spacing Modifier Letters",
        0x0300..=0x036F => "Combining Diacritical Marks",
        0x0370..=0x03FF => "Greek and Coptic",
        0x0400..=0x04FF => "Cyrillic",
        0x0500..=0x052F => "Cyrillic Supplement",
        0x0530..=0x058F => "Armenian",
        0x0590..=0x05FF => "Hebrew",
        0x0600..=0x06FF => "Arabic",
        0x0900..=0x097F => "Devanagari",
        0x0980..=0x09FF => "Bengali",
        0x0A00..=0x0A7F => "Gurmukhi",
        0x0A80..=0x0AFF => "Gujarati",
        0x0B80..=0x0BFF => "Tamil",
        0x0C00..=0x0C7F => "Telugu",
        0x0C80..=0x0CFF => "Kannada",
        0x0D00..=0x0D7F => "Malayalam",
        0x0E00..=0x0E7F => "Thai",
        0x10A0..=0x10FF => "Georgian",
        0x1100..=0x11FF => "Hangul Jamo",
        0x1E00..=0x1EFF => "Latin Extended Additional",
        0x1F00..=0x1FFF => "Greek Extended",
        0x2000..=0x206F => "General Punctuation",
        0x2070..=0x209F => "Superscripts and Subscripts",
        0x20A0..=0x20CF => "Currency Symbols",
        0x2100..=0x214F => "Letterlike Symbols",
        0x2150..=0x218F => "Number Forms",
        0x2190..=0x21FF => "Arrows",
        0x2200..=0x22FF => "Mathematical Operators",
        0x2300..=0x23FF => "Miscellaneous Technical",
        0x2500..=0x257F => "Box Drawing",
        0x2580..=0x259F => "Block Elements",
        0x25A0..=0x25FF => "Geometric Shapes",
        0x2600..=0x26FF => "Miscellaneous Symbols",
        0x2700..=0x27BF => "Dingbats",
        0x3000..=0x303F => "CJK Symbols and Punctuation",
        0x3040..=0x309F => "Hiragana",
        0x30A0..=0x30FF => "Katakana",
        0x4E00..=0x9FFF => "CJK Unified Ideographs",
        0xAC00..=0xD7AF => "Hangul Syllables",
        0xE000..=0xF8FF => "Private Use Area",
        0xFB00..=0xFB4F => "Alphabetic Presentation Forms",
        0xFE70..=0xFEFF => "Arabic Presentation Forms-B",
        0xFF00..=0xFFEF => "Halfwidth and Fullwidth Forms",
        0x1F300..=0x1F5FF => "Miscellaneous Symbols and Pictographs",
        0x1F600..=0x1F64F => "Emoticons",
        0x1F900..=0x1F9FF => "Supplemental Symbols and Pictographs",
        _ => "Other",
    }
}

fn codepoints(face: &Face) -> Vec<u32> {
    let mut cps = Vec::new();
    if let Some(cmap) = face.tables().cmap {
        for sub in cmap.subtables {
            if !sub.is_unicode() {
                continue;
            }
            sub.codepoints(|cp| cps.push(cp));
        }
    }
    cps.sort_unstable();
    cps.dedup();
    cps
}

fn classify(face: &Face, family: &str) -> (&'static str, Vec<&'static str>) {
    let lower = family.to_lowercase();
    let mono = face.is_monospaced()
        || lower.contains("mono")
        || lower.contains("code")
        || lower.contains("courier")
        || lower.contains("consol");
    // ttf-parser does not expose sFamilyClass / PANOSE, so read them from the raw OS/2 table.
    let os2_raw = face.raw_face().table(Tag::from_bytes(b"OS/2")).unwrap_or(&[]);
    let family_class = if os2_raw.len() >= 32 {
        u16::from_be_bytes([os2_raw[30], os2_raw[31]]) >> 8
    } else {
        0
    };
    let panose: &[u8] = if os2_raw.len() >= 42 {
        &os2_raw[32..42]
    } else {
        &[0; 10]
    };
    let panose_serif = panose[0] == 2 && (2..=10).contains(&panose[1]);
    let panose_sans = panose[0] == 2 && (11..=15).contains(&panose[1]);
    let kind = if mono {
        "monospace"
    } else if lower.contains("script")
        || lower.contains("hand")
        || lower.contains("brush")
        || family_class == 10
    {
        "handwriting"
    } else if lower.contains("display")
        || lower.contains("black")
        || lower.contains("poster")
        || (9..=10).contains(&family_class)
        || family_class == 12
    {
        "display"
    } else if lower.contains("sans")
        || lower.contains("grotesk")
        || lower.contains("gothic")
        || panose_sans
        || family_class == 8
    {
        "sans-serif"
    } else if lower.contains("serif")
        || lower.contains("roman")
        || lower.contains("garamond")
        || lower.contains("times")
        || panose_serif
        || (1..=7).contains(&family_class)
    {
        "serif"
    } else {
        "sans-serif"
    };
    let traits: Vec<&'static str> = [
        (face.is_italic() || face.is_oblique(), "italic"),
        (face.is_bold(), "bold"),
        (face.is_variable(), "variable"),
        (face.tables().cff.is_some(), "cff-outlines"),
        (face.tables().glyf.is_some(), "truetype-outlines"),
        (face.tables().gsub.is_some(), "opentype-features"),
    ]
    .iter()
    .filter(|(c, _)| *c)
    .map(|(_, t)| *t)
    .collect();
    (kind, traits)
}

/// Names, metrics, classification, glyph/character coverage and tables of a font.
pub fn inspect(input: &FileInput) -> EngineResult<Value> {
    let (data, container) = sfnt_bytes(&input.file)?;
    let faces = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
    let face = parse_face(&data, input.index)?;
    let upm = face.units_per_em() as f64;
    let family = name(&face, name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| name(&face, name_id::FAMILY))
        .unwrap_or_default();
    let (kind, traits) = classify(&face, &family);
    let cps = codepoints(&face);
    let os2 = face.tables().os2;
    let fs_type = os2
        .map(|o| o.permissions())
        .map(|p| format!("{p:?}"))
        .unwrap_or_else(|| "unknown".into());
    let tables: Vec<String> = face
        .raw_face()
        .table_records
        .into_iter()
        .map(|r| r.tag.to_string())
        .collect();
    let axes: Vec<Value> = face
        .variation_axes()
        .into_iter()
        .map(|a| {
            json!({
                "tag": a.tag.to_string(),
                "min": a.min_value,
                "default": a.def_value,
                "max": a.max_value,
                "hidden": a.hidden
            })
        })
        .collect();
    let per_em = |v: i16| (v as f64 / upm * 1000.0).round() / 1000.0;
    Ok(json!({
        "container": match container { Container::TrueType => "ttf", Container::OpenTypeCff => "otf", Container::Collection => "ttc", Container::Woff => "woff", _ => "unknown" },
        "faces_in_file": faces,
        "face_index": input.index,
        "names": {
            "family": family,
            "subfamily": name(&face, name_id::TYPOGRAPHIC_SUBFAMILY).or_else(|| name(&face, name_id::SUBFAMILY)),
            "full_name": name(&face, name_id::FULL_NAME),
            "postscript_name": name(&face, name_id::POST_SCRIPT_NAME),
            "version": name(&face, name_id::VERSION),
            "copyright": name(&face, name_id::COPYRIGHT_NOTICE),
            "trademark": name(&face, name_id::TRADEMARK),
            "manufacturer": name(&face, name_id::MANUFACTURER),
            "designer": name(&face, name_id::DESIGNER),
            "description": name(&face, name_id::DESCRIPTION),
            "vendor_url": name(&face, name_id::VENDOR_URL),
            "designer_url": name(&face, name_id::DESIGNER_URL),
            "license": name(&face, name_id::LICENSE),
            "license_url": name(&face, name_id::LICENSE_URL),
            "unique_id": name(&face, name_id::UNIQUE_ID),
        },
        "classification": {
            "category": kind,
            "traits": traits,
            "weight": face.weight().to_number(),
            "weight_name": format!("{:?}", face.weight()),
            "width": format!("{:?}", face.width()),
            "italic_angle": face.italic_angle(),
            "monospaced": face.is_monospaced()
        },
        "metrics": {
            "units_per_em": face.units_per_em(),
            "ascender": face.ascender(),
            "descender": face.descender(),
            "line_gap": face.line_gap(),
            "cap_height": face.capital_height(),
            "x_height": face.x_height(),
            "ascender_em": per_em(face.ascender()),
            "descender_em": per_em(face.descender()),
            "x_height_em": face.x_height().map(per_em),
            "cap_height_em": face.capital_height().map(per_em),
            "underline": face.underline_metrics().map(|u| json!({ "position": u.position, "thickness": u.thickness })),
            "bbox": {
                "x_min": face.global_bounding_box().x_min,
                "y_min": face.global_bounding_box().y_min,
                "x_max": face.global_bounding_box().x_max,
                "y_max": face.global_bounding_box().y_max
            }
        },
        "glyphs": {
            "count": face.number_of_glyphs(),
            "mapped_codepoints": cps.len(),
            "blocks": block_counts(&cps)
        },
        "embedding_permissions": fs_type,
        "variable": { "is_variable": face.is_variable(), "axes": axes },
        "tables": tables,
        "features": {
            "gsub": face.tables().gsub.is_some(),
            "gpos": face.tables().gpos.is_some(),
            "kern": face.tables().kern.is_some(),
            "color": face.tables().colr.is_some() || face.tables().svg.is_some() || face.tables().sbix.is_some() || face.tables().cbdt.is_some(),
            "hinting": tables.iter().any(|t| t == "fpgm" || t == "prep")
        },
        "file_bytes": input.file.len(),
    }))
}

fn block_counts(cps: &[u32]) -> Vec<Value> {
    let mut counts: Vec<(&'static str, usize)> = Vec::new();
    for cp in cps {
        let b = unicode_block(*cp);
        match counts.iter_mut().find(|(n, _)| *n == b) {
            Some((_, c)) => *c += 1,
            None => counts.push((b, 1)),
        }
    }
    counts
        .iter()
        .map(|(n, c)| json!({ "block": n, "glyphs": c }))
        .collect()
}

/// Paged list of mapped characters with Unicode block statistics and CSS `unicode-range`.
pub fn char_map(
    input: &FileInput,
    block: Option<&str>,
    offset: usize,
    limit: usize,
) -> EngineResult<Value> {
    let (data, _) = sfnt_bytes(&input.file)?;
    let face = parse_face(&data, input.index)?;
    let cps = codepoints(&face);
    let filtered: Vec<u32> = match block {
        Some(b) if !b.is_empty() => cps
            .iter()
            .copied()
            .filter(|cp| unicode_block(*cp).eq_ignore_ascii_case(b))
            .collect(),
        _ => cps.clone(),
    };
    let limit = if limit == 0 { 500 } else { limit.min(5000) };
    let page: Vec<Value> = filtered
        .iter()
        .skip(offset)
        .take(limit)
        .map(|cp| {
            let ch = char::from_u32(*cp);
            let gid = ch.and_then(|c| face.glyph_index(c));
            json!({
                "codepoint": format!("U+{:04X}", cp),
                "char": ch.map(|c| c.to_string()),
                "glyph_id": gid.map(|g| g.0),
                "advance": gid.and_then(|g| face.glyph_hor_advance(g)),
                "name": gid.and_then(|g| face.glyph_name(g)),
                "block": unicode_block(*cp)
            })
        })
        .collect();
    let mut ranges = Vec::new();
    if let Some(&first) = filtered.first() {
        let (mut s, mut prev) = (first, first);
        for &cp in filtered.iter().skip(1) {
            if cp != prev + 1 {
                ranges.push(format!("U+{s:04X}-U+{prev:04X}"));
                s = cp;
            }
            prev = cp;
        }
        ranges.push(format!("U+{s:04X}-U+{prev:04X}"));
    }
    let sample_text = "The quick brown fox jumps over the lazy dog 0123456789";
    let missing: Vec<char> = sample_text
        .chars()
        .filter(|c| !c.is_whitespace() && face.glyph_index(*c).is_none())
        .collect();
    Ok(json!({
        "total": cps.len(),
        "filtered": filtered.len(),
        "offset": offset,
        "limit": limit,
        "glyphs": page,
        "blocks": block_counts(&cps),
        "unicode_ranges": ranges,
        "css_unicode_range": ranges.iter().take(40).cloned().collect::<Vec<_>>().join(", "),
        "basic_latin_complete": missing.is_empty(),
        "missing_from_pangram": missing
    }))
}

/// Renders text with the font (ab_glyph) to a PNG.
pub fn preview(
    input: &FileInput,
    text: &str,
    size_px: f32,
    color: [u8; 4],
    background: [u8; 4],
    line_height: f32,
    max_width: u32,
) -> EngineResult<Value> {
    let (data, _) = sfnt_bytes(&input.file)?;
    let text = if text.trim().is_empty() {
        "The quick brown fox jumps over the lazy dog\nABCDEFGHIJKLM nopqrstuvwxyz 0123456789"
    } else {
        text
    };
    if !(4.0..=512.0).contains(&size_px) {
        return Err(EngineError::invalid("size_px must be between 4 and 512"));
    }
    use ab_glyph::{Font, FontRef, ScaleFont};
    let font = FontRef::try_from_slice_and_index(&data, input.index)
        .map_err(|e| EngineError::malformed(format!("Cannot load font for rendering: {e}")))?;
    let scaled = font.as_scaled(ab_glyph::PxScale::from(size_px));
    let lh = (size_px * if line_height <= 0.0 { 1.3 } else { line_height }).round();
    let lines: Vec<&str> = text.lines().take(40).collect();
    let padding = (size_px * 0.4).round() as u32;
    let mut width = 0f32;
    for l in &lines {
        let w: f32 = l.chars().map(|c| scaled.h_advance(font.glyph_id(c))).sum();
        width = width.max(w);
    }
    let w = (width.ceil() as u32 + padding * 2)
        .clamp(1, if max_width == 0 { 4096 } else { max_width.max(64) });
    let h = (lh as u32) * lines.len().max(1) as u32 + padding * 2;
    engine_shared::limits::check_dimensions(w, h)?;
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba(background));
    let mut missing = Vec::new();
    for (li, line) in lines.iter().enumerate() {
        let mut x = padding as f32;
        let baseline = padding as f32 + lh * li as f32 + scaled.ascent();
        let mut prev: Option<ab_glyph::GlyphId> = None;
        for c in line.chars() {
            let gid = font.glyph_id(c);
            if gid.0 == 0 && !c.is_whitespace() && !missing.contains(&c) {
                missing.push(c);
            }
            if let Some(p) = prev {
                x += scaled.kern(p, gid);
            }
            let glyph = gid.with_scale_and_position(size_px, ab_glyph::point(x, baseline));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, cov| {
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;
                    if px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                        let p = img.get_pixel_mut(px as u32, py as u32);
                        let a = cov.clamp(0.0, 1.0) * color[3] as f32 / 255.0;
                        for k in 0..3 {
                            p[k] = (p[k] as f32 * (1.0 - a) + color[k] as f32 * a).round() as u8;
                        }
                        p[3] = (p[3] as f32).max(a * 255.0) as u8;
                    }
                });
            }
            x += scaled.h_advance(gid);
            prev = Some(gid);
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    let png = out.into_inner();
    Ok(json!({
        "png": engine_shared::b64::encode(&png),
        "mime": "image/png",
        "width": w,
        "height": h,
        "size_px": size_px,
        "lines": lines.len(),
        "missing_glyphs": missing,
        "bytes": png.len()
    }))
}

/// Readability check of font sizes based on x-height.
pub fn size_check(input: &FileInput, sizes: &[f32], context: &str) -> EngineResult<Value> {
    let (data, _) = sfnt_bytes(&input.file)?;
    let face = parse_face(&data, input.index)?;
    let upm = face.units_per_em() as f64;
    let x_height = face.x_height().map(|x| x as f64 / upm).unwrap_or(0.5);
    let cap = face.capital_height().map(|x| x as f64 / upm).unwrap_or(0.7);
    let sizes: Vec<f32> = if sizes.is_empty() {
        vec![10.0, 12.0, 14.0, 16.0, 18.0, 24.0, 32.0]
    } else {
        sizes.to_vec()
    };
    let min_x_px = match context {
        "mobile" => 7.5,
        "print" => 5.0,
        "signage" => 20.0,
        _ => 7.0,
    };
    let rows: Vec<Value> = sizes
        .iter()
        .map(|s| {
            let xh = *s as f64 * x_height;
            json!({
                "size_px": s,
                "size_pt": (*s as f64 * 0.75 * 100.0).round() / 100.0,
                "size_rem": (*s as f64 / 16.0 * 1000.0).round() / 1000.0,
                "x_height_px": (xh * 100.0).round() / 100.0,
                "cap_height_px": (*s as f64 * cap * 100.0).round() / 100.0,
                "line_height_px": ((face.ascender() as f64 - face.descender() as f64 + face.line_gap() as f64) / upm * *s as f64).round(),
                "readable": xh >= min_x_px,
                "verdict": if xh < min_x_px * 0.8 { "too small" } else if xh < min_x_px { "borderline" } else { "ok" }
            })
        })
        .collect();
    let rec_body = (min_x_px / x_height).ceil();
    Ok(json!({
        "x_height_ratio": (x_height * 1000.0).round() / 1000.0,
        "cap_height_ratio": (cap * 1000.0).round() / 1000.0,
        "context": if context.is_empty() { "screen" } else { context },
        "minimum_x_height_px": min_x_px,
        "recommended_minimum_body_px": rec_body,
        "sizes": rows,
        "note": "Readability threshold uses x-height: fonts with a small x-height need a larger \
                 nominal size"
    }))
}

/// Heuristic pairing suggestions (Google Fonts) based on the font's classification.
pub fn pairing(input: &FileInput, role: &str) -> EngineResult<Value> {
    let (data, _) = sfnt_bytes(&input.file)?;
    let face = parse_face(&data, input.index)?;
    let family = name(&face, name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| name(&face, name_id::FAMILY))
        .unwrap_or_else(|| "Font".into());
    let (kind, _) = classify(&face, &family);
    let role = if role.is_empty() {
        if kind == "display" || kind == "handwriting" {
            "heading"
        } else {
            "body"
        }
    } else {
        role
    };
    let suggestions: Vec<(&str, &str, &str)> = match (kind, role) {
        ("serif", "heading") => vec![
            ("Inter", "sans-serif", "Neutral sans for body copy under a serif headline"),
            ("Source Sans 3", "sans-serif", "Humanist sans pairs with transitional serifs"),
            ("IBM Plex Sans", "sans-serif", "Geometric-humanist, good for UI"),
        ],
        ("serif", _) => vec![
            ("Playfair Display", "display", "High-contrast display serif for headlines"),
            ("Montserrat", "sans-serif", "Geometric sans headline over a serif body"),
            ("Oswald", "sans-serif", "Condensed sans for strong headings"),
        ],
        ("sans-serif", "heading") => vec![
            ("Merriweather", "serif", "Sturdy serif body under a sans heading"),
            ("Lora", "serif", "Calligraphic serif with good screen readability"),
            ("Source Serif 4", "serif", "Companion to Source Sans"),
        ],
        ("sans-serif", _) => vec![
            ("Playfair Display", "display", "Contrast: display serif headings"),
            ("Poppins", "sans-serif", "Geometric sans headings over a humanist body"),
            ("Space Grotesk", "sans-serif", "Techy grotesk for headings"),
        ],
        ("monospace", _) => vec![
            ("Inter", "sans-serif", "UI text next to code"),
            ("IBM Plex Sans", "sans-serif", "Same superfamily as Plex Mono"),
            ("Source Sans 3", "sans-serif", "Pairs with Source Code Pro"),
        ],
        ("display", _) | ("handwriting", _) => vec![
            ("Inter", "sans-serif", "Quiet body text lets the display face shine"),
            ("Lato", "sans-serif", "Warm humanist body"),
            ("Merriweather", "serif", "Serif body for editorial feel"),
        ],
        _ => vec![("Inter", "sans-serif", "Safe neutral pairing")],
    };
    let css_fallback = match kind {
        "serif" => "Georgia, 'Times New Roman', serif",
        "monospace" => "ui-monospace, SFMono-Regular, Menlo, monospace",
        "handwriting" => "cursive",
        "display" => "Impact, sans-serif",
        _ => "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
    };
    Ok(json!({
        "font": family,
        "category": kind,
        "role": role,
        "principles": [
            "Pair contrasting categories (serif + sans) or one superfamily",
            "Match x-heights so mixed lines look even",
            "Limit to two families; use weight/size for hierarchy",
            "Keep body text 16px+ on screen"
        ],
        "suggestions": suggestions.iter().map(|(n, c, why)| json!({ "family": n, "category": c, "why": why, "google_fonts": format!("\
            https://fonts.google.com/specimen/{}", n.replace(' ', "+")) })).collect::<Vec<_>>(),
        "css": format!("--font-{role}: '{family}', {css_fallback};\n--font-{}: '{}', {};", if role == "heading" { "body" } else { "heading" }, suggestions[0].0, match suggestions[0].1 { "serif" | "display" => "Georgia, serif", _ => "system-ui, sans-serif" }),
        "x_height_ratio": face.x_height().map(|x| (x as f64 / face.units_per_em() as f64 * 1000.0).round() / 1000.0),
    }))
}

/// Converts between TTF/OTF and WOFF 1.0 (WOFF 2.0 returns `E_UNSUPPORTED`).
pub fn convert(input: &FileInput, to: &str) -> EngineResult<Value> {
    engine_shared::error::require_bytes(&input.file, "font")?;
    let container = detect(&input.file);
    let base = input
        .name
        .as_deref()
        .and_then(|n| n.rsplit('/').next())
        .and_then(|n| n.rsplit_once('.').map(|(b, _)| b.to_string()))
        .unwrap_or_else(|| "font".into());
    let to = to.trim().to_ascii_lowercase();
    let (bytes, ext, mime) = match (container, to.as_str()) {
        (Container::Woff, "ttf" | "otf" | "sfnt" | "") => {
            let sfnt = woff_to_sfnt(&input.file)?;
            let ext = if detect(&sfnt) == Container::OpenTypeCff {
                "otf"
            } else {
                "ttf"
            };
            (sfnt, ext, if ext == "otf" { "font/otf" } else { "font/ttf" })
        },
        (Container::TrueType | Container::OpenTypeCff, "woff" | "") => {
            (sfnt_to_woff(&input.file)?, "woff", "font/woff")
        },
        (Container::TrueType, "ttf") | (Container::OpenTypeCff, "otf") => (
            input.file.clone(),
            if container == Container::TrueType { "ttf" } else { "otf" },
            "font/ttf",
        ),
        (Container::Woff2, _) | (_, "woff2") => {
            return Err(EngineError::unsupported(
                "WOFF2 (Brotli + glyph transforms) is not implemented in WASM; use the browser \
                 FontFace API / backend woff2 tools",
            ))
        },
        (Container::TrueType, "otf") | (Container::OpenTypeCff, "ttf") => {
            return Err(EngineError::unsupported(
                "Outline conversion between TrueType (glyf) and CFF requires re-hinting and curve \
                 conversion; use FontForge on the backend",
            ))
        },
        (Container::Collection, _) => {
            return Err(EngineError::unsupported(
                "Font collections (.ttc) must be split first; pass `index` to inspect/subset a \
                 single face",
            ))
        },
        (Container::Unknown, _) => {
            return Err(EngineError::unsupported_format("Not a recognised font file"))
        },
        (_, other) => {
            return Err(EngineError::invalid(format!(
                "Unknown target format `{other}` (ttf, otf, woff)"
            )))
        },
    };
    // Sanity: the result must still parse.
    let check = if ext == "woff" {
        woff_to_sfnt(&bytes)?
    } else {
        bytes.clone()
    };
    parse_face(&check, 0)?;
    Ok(json!({
        "file": engine_shared::b64::encode(&bytes),
        "filename": format!("{base}.{ext}"),
        "mime": mime,
        "bytes": bytes.len(),
        "input_bytes": input.file.len(),
        "ratio": ((bytes.len() as f64 / input.file.len() as f64) * 1000.0).round() / 10.0
    }))
}

/// Subsets glyphs to the given text / Unicode ranges and rebuilds the `cmap` table.
pub fn subset(
    input: &FileInput,
    text: &str,
    unicodes: &[String],
    keep_names: bool,
    output: &str,
) -> EngineResult<Value> {
    let (data, _) = sfnt_bytes(&input.file)?;
    let face = parse_face(&data, input.index)?;
    let mut chars: Vec<char> = text.chars().collect();
    for spec in unicodes {
        for part in spec.split(',') {
            let p = part.trim().trim_start_matches("U+").trim_start_matches("u+");
            if p.is_empty() {
                continue;
            }
            let (a, b) = p
                .split_once('-')
                .map(|(a, b)| (a, b.trim_start_matches("U+")))
                .unwrap_or((p, p));
            let (a, b) = (
                u32::from_str_radix(a, 16)
                    .map_err(|_| EngineError::invalid(format!("Bad unicode range `{part}`")))?,
                u32::from_str_radix(b, 16)
                    .map_err(|_| EngineError::invalid(format!("Bad unicode range `{part}`")))?,
            );
            if b < a || b - a > 65536 {
                return Err(EngineError::invalid(format!("Bad unicode range `{part}`")));
            }
            chars.extend((a..=b).filter_map(char::from_u32));
        }
    }
    chars.sort_unstable();
    chars.dedup();
    if chars.is_empty() {
        return Err(EngineError::empty("text or unicodes"));
    }
    let mut gids: Vec<u16> = vec![0];
    let mut missing = Vec::new();
    for c in &chars {
        match face.glyph_index(*c) {
            Some(g) => gids.push(g.0),
            None => missing.push(c.to_string()),
        }
    }
    gids.sort_unstable();
    gids.dedup();
    let mapper = subsetter::GlyphRemapper::new_from_glyphs(&gids);
    let out = subsetter::subset(&data, input.index, &mapper).map_err(|e| {
        EngineError::unsupported(format!(
            "Subsetting failed: {e:?} (CFF2/variable fonts are not supported)"
        ))
    })?;
    // The subsetter drops `cmap` (it targets PDF embedding); rebuild one so the font stays usable on the web.
    let mut mapping: Vec<(u32, u16)> = chars
        .iter()
        .filter_map(|c| {
            face.glyph_index(*c)
                .and_then(|g| mapper.get(g.0))
                .map(|new| (*c as u32, new))
        })
        .collect();
    mapping.sort_unstable();
    mapping.dedup();
    let mut tables = split_sfnt(&out)?;
    tables.retain(|t| {
        t.0 != u32::from_be_bytes(*b"cmap") && (keep_names || t.0 != u32::from_be_bytes(*b"name"))
    });
    let cmap = build_cmap12(&mapping);
    tables.push((u32::from_be_bytes(*b"cmap"), cmap.clone(), table_checksum(&cmap)));
    let out = build_sfnt(be32(&out, 0), tables);
    let (bytes, ext, mime) = if output.eq_ignore_ascii_case("woff") {
        (sfnt_to_woff(&out)?, "woff", "font/woff")
    } else {
        let ext = if detect(&out) == Container::OpenTypeCff {
            "otf"
        } else {
            "ttf"
        };
        (out, ext, "font/ttf")
    };
    let base = input
        .name
        .as_deref()
        .and_then(|n| n.rsplit('/').next())
        .and_then(|n| n.rsplit_once('.').map(|(b, _)| b.to_string()))
        .unwrap_or_else(|| "font".into());
    Ok(json!({
        "file": engine_shared::b64::encode(&bytes),
        "filename": format!("{base}-subset.{ext}"),
        "mime": mime,
        "bytes": bytes.len(),
        "input_bytes": input.file.len(),
        "reduction_percent": ((1.0 - bytes.len() as f64 / input.file.len() as f64) * 1000.0).round() / 10.0,
        "glyphs_kept": gids.len(),
        "glyphs_before": face.number_of_glyphs(),
        "characters": chars.len(),
        "missing_characters": missing,
        "note": "Glyph IDs are renumbered; OpenType layout (GSUB/GPOS) tables are dropped by the \
                 subsetter"
    }))
}

fn parse_color(s: &str, default: [u8; 4]) -> EngineResult<[u8; 4]> {
    let t = s.trim().trim_start_matches('#');
    if t.is_empty() {
        return Ok(default);
    }
    if !(t.len() == 6 || t.len() == 8) || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(EngineError::invalid("Colours must be #rrggbb or #rrggbbaa"));
    }
    // Validated as hex digits above, so parsing cannot fail.
    let v: Vec<u8> = (0..t.len() / 2)
        .map(|i| u8::from_str_radix(&t[i * 2..i * 2 + 2], 16).unwrap_or(0))
        .collect();
    Ok([v[0], v[1], v[2], v.get(3).copied().unwrap_or(255)])
}

/// Every action understood by the font engine (`{"action": "...", "payload": {...}}`).
#[derive(Deserialize)]
#[serde(tag = "action", content = "payload", rename_all = "snake_case")]
pub enum FontAction {
    /// Font metadata and metrics.
    Inspect {
        /// Input font.
        #[serde(flatten)]
        input: FileInput,
    },
    /// Character map.
    CharMap {
        /// Input font.
        #[serde(flatten)]
        input: FileInput,
        /// Restrict to a Unicode block name.
        #[serde(default)]
        block: Option<String>,
        /// First character to return.
        #[serde(default)]
        offset: usize,
        /// Maximum characters returned.
        #[serde(default)]
        limit: usize,
    },
    /// Renders sample text to PNG.
    Preview {
        /// Input font.
        #[serde(flatten)]
        input: FileInput,
        /// Text to render (default pangram).
        #[serde(default)]
        text: String,
        /// Font size in pixels.
        #[serde(default)]
        size_px: Option<f32>,
        /// Text colour (`#rrggbb`).
        #[serde(default)]
        color: String,
        /// Background colour (`#rrggbb`, transparent when empty).
        #[serde(default)]
        background: String,
        /// Line-height multiplier.
        #[serde(default)]
        line_height: f32,
        /// Wrap width in pixels.
        #[serde(default)]
        max_width: u32,
    },
    /// Size readability check.
    SizeCheck {
        /// Input font.
        #[serde(flatten)]
        input: FileInput,
        /// Sizes in pixels to check.
        #[serde(default)]
        sizes: Vec<f32>,
        /// `screen` (default), `print` or `mobile`.
        #[serde(default)]
        context: String,
    },
    /// Pairing suggestions.
    Pairing {
        /// Input font.
        #[serde(flatten)]
        input: FileInput,
        /// Role of the font (`heading` or `body`).
        #[serde(default)]
        role: String,
    },
    /// Format conversion.
    Convert {
        /// Input font.
        #[serde(flatten)]
        input: FileInput,
        /// Target format (`ttf`, `otf`, `woff`).
        #[serde(default)]
        to: String,
    },
    /// Glyph subsetting.
    Subset {
        /// Input font.
        #[serde(flatten)]
        input: FileInput,
        /// Characters to keep.
        #[serde(default)]
        text: String,
        /// Unicode ranges to keep (`U+0020-007E`).
        #[serde(default)]
        unicodes: Vec<String>,
        /// Keep the `name` table.
        #[serde(default = "default_true")]
        keep_names: bool,
        /// Output container (`ttf` default, `woff`).
        #[serde(default)]
        output: String,
    },
}

fn default_true() -> bool {
    true
}

/// Dispatches a [`FontAction`] to its implementation.
pub fn handle_action(action: FontAction) -> EngineResult<Value> {
    match action {
        FontAction::Inspect { input } => inspect(&input),
        FontAction::CharMap { input, block, offset, limit } => {
            char_map(&input, block.as_deref(), offset, limit)
        },
        FontAction::Preview {
            input,
            text,
            size_px,
            color,
            background,
            line_height,
            max_width,
        } => preview(
            &input,
            &text,
            size_px.unwrap_or(32.0),
            parse_color(&color, [17, 17, 17, 255])?,
            parse_color(&background, [255, 255, 255, 255])?,
            line_height,
            max_width,
        ),
        FontAction::SizeCheck { input, sizes, context } => size_check(&input, &sizes, &context),
        FontAction::Pairing { input, role } => pairing(&input, &role),
        FontAction::Convert { input, to } => convert(&input, &to),
        FontAction::Subset {
            input,
            text,
            unicodes,
            keep_names,
            output,
        } => subset(&input, &text, &unicodes, keep_names, &output),
    }
}

engine_shared::export_engine!(handle_action, FontAction);

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &[u8] = include_bytes!("../../image/assets/DejaVuSans-subset.ttf");

    fn input() -> FileInput {
        FileInput {
            file: FONT.to_vec(),
            name: Some("DejaVuSans.ttf".into()),
            index: 0,
        }
    }

    #[test]
    fn inspect_and_map() {
        let i = inspect(&input()).unwrap();
        assert_eq!(i["container"], "ttf");
        assert!(i["names"]["family"].as_str().unwrap().contains("DejaVu"));
        assert!(i["glyphs"]["count"].as_u64().unwrap() > 100);
        assert_eq!(i["classification"]["category"], "sans-serif");
        let m = char_map(&input(), Some("Basic Latin"), 0, 10).unwrap();
        assert_eq!(m["glyphs"].as_array().unwrap().len(), 10);
        assert_eq!(m["basic_latin_complete"], true);
        let p =
            preview(&input(), "Hello", 24.0, [0, 0, 0, 255], [255, 255, 255, 255], 1.3, 0).unwrap();
        assert!(p["width"].as_u64().unwrap() > 40);
        assert!(p["missing_glyphs"].as_array().unwrap().is_empty());
        let s = size_check(&input(), &[8.0, 16.0], "mobile").unwrap();
        assert_eq!(s["sizes"][1]["readable"], true);
        assert!(
            pairing(&input(), "").unwrap()["suggestions"]
                .as_array()
                .unwrap()
                .len()
                >= 3
        );
    }

    #[test]
    fn convert_and_subset() {
        let w = convert(&input(), "woff").unwrap();
        assert_eq!(w["filename"], "DejaVuSans.woff");
        let woff = engine_shared::b64::decode(w["file"].as_str().unwrap()).unwrap();
        assert_eq!(detect(&woff), Container::Woff);
        assert!(woff.len() < FONT.len());
        let back = convert(
            &FileInput {
                file: woff.clone(),
                name: Some("x.woff".into()),
                index: 0,
            },
            "ttf",
        )
        .unwrap();
        let ttf = engine_shared::b64::decode(back["file"].as_str().unwrap()).unwrap();
        let face = Face::parse(&ttf, 0).unwrap();
        assert_eq!(face.number_of_glyphs(), Face::parse(FONT, 0).unwrap().number_of_glyphs());
        let i = inspect(&FileInput { file: woff, name: None, index: 0 }).unwrap();
        assert_eq!(i["container"], "woff");
        let s = subset(&input(), "Hello World", &["U+0030-0039".into()], true, "").unwrap();
        assert!(s["bytes"].as_u64().unwrap() < FONT.len() as u64, "{}", s["bytes"]);
        let sub = engine_shared::b64::decode(s["file"].as_str().unwrap()).unwrap();
        let f = Face::parse(&sub, 0).unwrap();
        assert!(
            f.glyph_index('H').is_some()
                && f.glyph_index('7').is_some()
                && f.glyph_index('Z').is_none()
        );
        assert!(convert(&input(), "woff2").is_err());
        assert!(subset(&input(), "", &[], true, "").is_err());
        assert!(inspect(&FileInput {
            file: b"not a font".to_vec(),
            name: None,
            index: 0
        })
        .is_err());
        assert!(inspect(&FileInput {
            file: b"wOF2garbage".to_vec(),
            name: None,
            index: 0
        })
        .is_err());
    }
}
