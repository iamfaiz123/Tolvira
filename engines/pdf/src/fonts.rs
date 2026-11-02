//! Standard-14 font support: WinAnsi encoding, glyph widths and word wrapping.
//!
//! Generated PDFs use the standard Type1 fonts (Helvetica, Times, Courier)
//! which every viewer ships, so no font embedding is required. Text is
//! encoded with `WinAnsiEncoding`; characters outside that repertoire are
//! replaced with `?` (the engine reports how many were substituted).

use crate::font_metrics;
use lopdf::{Dictionary, Document, Object, ObjectId};

/// A resolved standard font face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdFont {
    /// PostScript base font name (e.g. `Helvetica-Bold`).
    pub base_font: &'static str,
    widths: &'static [u16; 256],
}

impl StdFont {
    /// Resolves a family (`helvetica` | `times` | `courier`, aliases
    /// `sans`/`arial`, `serif`, `mono`) and style into a face.
    pub fn resolve(family: &str, bold: bool, italic: bool) -> StdFont {
        let fam = family.trim().to_ascii_lowercase();
        let fam = match fam.as_str() {
            "times" | "times-roman" | "serif" | "times new roman" => "times",
            "courier" | "mono" | "monospace" | "courier new" => "courier",
            _ => "helvetica",
        };
        let (base_font, widths): (&'static str, &'static [u16; 256]) = match (fam, bold, italic) {
            ("times", false, false) => ("Times-Roman", &font_metrics::TIMES_ROMAN),
            ("times", true, false) => ("Times-Bold", &font_metrics::TIMES_BOLD),
            ("times", false, true) => ("Times-Italic", &font_metrics::TIMES_ITALIC),
            ("times", true, true) => ("Times-BoldItalic", &font_metrics::TIMES_BOLDITALIC),
            ("courier", false, false) => ("Courier", &font_metrics::COURIER),
            ("courier", true, false) => ("Courier-Bold", &font_metrics::COURIER_BOLD),
            ("courier", false, true) => ("Courier-Oblique", &font_metrics::COURIER_OBLIQUE),
            ("courier", true, true) => ("Courier-BoldOblique", &font_metrics::COURIER_BOLDOBLIQUE),
            (_, false, false) => ("Helvetica", &font_metrics::HELVETICA),
            (_, true, false) => ("Helvetica-Bold", &font_metrics::HELVETICA_BOLD),
            (_, false, true) => ("Helvetica-Oblique", &font_metrics::HELVETICA_OBLIQUE),
            (_, true, true) => ("Helvetica-BoldOblique", &font_metrics::HELVETICA_BOLDOBLIQUE),
        };
        StdFont { base_font, widths }
    }

    /// Width of a single WinAnsi byte in 1/1000 em.
    pub fn glyph_width(&self, byte: u8) -> f64 {
        let w = self.widths[byte as usize];
        if w == 0 {
            // Unknown glyph: use the width of `?` so layout stays stable.
            self.widths[b'?' as usize] as f64
        } else {
            w as f64
        }
    }

    /// Width of an encoded byte string at `size` points.
    pub fn text_width(&self, encoded: &[u8], size: f64) -> f64 {
        encoded.iter().map(|&b| self.glyph_width(b)).sum::<f64>() * size / 1000.0
    }

    /// Width of a Unicode string at `size` points (after WinAnsi encoding).
    pub fn str_width(&self, text: &str, size: f64) -> f64 {
        self.text_width(&encode_winansi(text).0, size)
    }

    /// Creates the font dictionary object for this face inside `doc`.
    pub fn add_to_document(&self, doc: &mut Document) -> ObjectId {
        let mut dict = Dictionary::new();
        dict.set("Type", Object::Name(b"Font".to_vec()));
        dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        dict.set("BaseFont", Object::Name(self.base_font.as_bytes().to_vec()));
        dict.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
        doc.add_object(Object::Dictionary(dict))
    }
}

/// Encodes text as WinAnsi bytes. Returns the bytes and the number of
/// characters that had to be substituted with `?`.
pub fn encode_winansi(text: &str) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(text.len());
    let mut substituted = 0;
    for ch in text.chars() {
        match winansi_byte(ch) {
            Some(b) => out.push(b),
            None => {
                substituted += 1;
                out.push(b'?');
            },
        }
    }
    (out, substituted)
}

/// Maps a char to its WinAnsi (cp1252) byte.
pub fn winansi_byte(ch: char) -> Option<u8> {
    let cp = ch as u32;
    match cp {
        0x20..=0x7E => Some(cp as u8),
        0xA0..=0xFF => Some(cp as u8),
        0x09 => Some(b' '),
        0x20AC => Some(0x80),
        0x201A => Some(0x82),
        0x0192 => Some(0x83),
        0x201E => Some(0x84),
        0x2026 => Some(0x85),
        0x2020 => Some(0x86),
        0x2021 => Some(0x87),
        0x02C6 => Some(0x88),
        0x2030 => Some(0x89),
        0x0160 => Some(0x8A),
        0x2039 => Some(0x8B),
        0x0152 => Some(0x8C),
        0x017D => Some(0x8E),
        0x2018 => Some(0x91),
        0x2019 => Some(0x92),
        0x201C => Some(0x93),
        0x201D => Some(0x94),
        0x2022 => Some(0x95),
        0x2013 => Some(0x96),
        0x2014 => Some(0x97),
        0x02DC => Some(0x98),
        0x2122 => Some(0x99),
        0x0161 => Some(0x9A),
        0x203A => Some(0x9B),
        0x0153 => Some(0x9C),
        0x017E => Some(0x9E),
        0x0178 => Some(0x9F),
        _ => None,
    }
}

/// Greedy word wrap of a single paragraph into lines that fit `max_width`
/// points. Words longer than a line are broken by character.
pub fn wrap_line(font: StdFont, size: f64, text: &str, max_width: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let space_w = font.str_width(" ", size);
    let mut current_w = 0.0;

    for word in text.split(' ') {
        if word.is_empty() {
            // Preserve consecutive spaces modestly.
            if !current.is_empty() {
                current.push(' ');
                current_w += space_w;
            }
            continue;
        }
        let word_w = font.str_width(word, size);
        let needed = if current.is_empty() {
            word_w
        } else {
            current_w + space_w + word_w
        };
        if needed <= max_width || max_width <= 0.0 {
            if !current.is_empty() {
                current.push(' ');
                current_w += space_w;
            }
            current.push_str(word);
            current_w += word_w;
        } else if word_w > max_width {
            // Break the long word character by character.
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_w = 0.0;
            }
            for ch in word.chars() {
                let cw = font.str_width(&ch.to_string(), size);
                if current_w + cw > max_width && !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    current_w = 0.0;
                }
                current.push(ch);
                current_w += cw;
            }
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
            current_w = word_w;
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_faces() {
        assert_eq!(StdFont::resolve("Helvetica", false, false).base_font, "Helvetica");
        assert_eq!(StdFont::resolve("serif", true, true).base_font, "Times-BoldItalic");
        assert_eq!(StdFont::resolve("mono", false, true).base_font, "Courier-Oblique");
        assert_eq!(StdFont::resolve("unknown", true, false).base_font, "Helvetica-Bold");
    }

    #[test]
    fn widths_match_afm() {
        let f = StdFont::resolve("helvetica", false, false);
        // Helvetica 'A' = 667, space = 278
        assert_eq!(f.glyph_width(b'A'), 667.0);
        assert!((f.str_width("A ", 10.0) - 9.45).abs() < 1e-9);
        let c = StdFont::resolve("courier", false, false);
        assert_eq!(c.glyph_width(b'i'), 600.0);
        assert_eq!(c.glyph_width(b'W'), 600.0);
    }

    #[test]
    fn encoding() {
        let (bytes, subs) = encode_winansi("Café €5 – 漢");
        assert_eq!(bytes, vec![b'C', b'a', b'f', 0xE9, b' ', 0x80, b'5', b' ', 0x96, b' ', b'?']);
        assert_eq!(subs, 1);
    }

    #[test]
    fn wrapping() {
        let f = StdFont::resolve("courier", false, false);
        // Courier 10pt: 6pt per char -> 60pt holds 10 chars
        let lines = wrap_line(f, 10.0, "aaaa bbbb cccc dddd", 60.0);
        assert_eq!(lines, vec!["aaaa bbbb", "cccc dddd"]);
        let long = wrap_line(f, 10.0, "abcdefghijklmnop", 60.0);
        assert_eq!(long, vec!["abcdefghij", "klmnop"]);
        assert_eq!(wrap_line(f, 10.0, "", 60.0), vec![""]);
    }
}
