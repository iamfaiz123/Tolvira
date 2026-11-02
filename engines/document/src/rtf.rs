//! Minimal RTF reader: control words, groups, destinations, hex/unicode
//! escapes, bold/italic/underline, paragraphs and simple list bullets.

use crate::model::{Block, Doc, Inline};
use engine_shared::{EngineError, EngineResult};

#[derive(Clone, Copy)]
struct State {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    skip: bool,
    uc: usize,
    font_size: u32,
}

/// Parses RTF text (control words, groups, Unicode escapes, basic formatting) into a [`Doc`].
pub fn parse(text: &str) -> EngineResult<Doc> {
    engine_shared::error::require_text(text, "rtf")?;
    if !text.trim_start().starts_with("{\\rtf") {
        return Err(EngineError::unsupported_format("Not an RTF document (expected `{\\rtf`)"));
    }
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut stack: Vec<State> = Vec::new();
    let mut st = State {
        bold: false,
        italic: false,
        underline: false,
        strike: false,
        skip: false,
        uc: 1,
        font_size: 24,
    };
    let mut doc = Doc::default();
    let mut inlines: Vec<Inline> = Vec::new();
    let mut cur = String::new();
    let mut in_list = false;
    let mut pending_unicode_skip = 0usize;
    let mut meta_dest: Option<String> = None;
    let mut meta_buf = String::new();
    let flush_run = |cur: &mut String, inlines: &mut Vec<Inline>, st: &State| {
        if !cur.is_empty() {
            inlines.push(Inline {
                text: std::mem::take(cur),
                bold: st.bold,
                italic: st.italic,
                underline: st.underline,
                strike: st.strike,
                code: false,
                link: None,
            });
        }
    };
    let flush_para = |inlines: &mut Vec<Inline>, doc: &mut Doc, in_list: bool, font_size: u32| {
        let text_empty = inlines.iter().all(|i| i.text.trim().is_empty());
        if text_empty {
            inlines.clear();
            return;
        }
        let mut items = std::mem::take(inlines);
        if in_list {
            // Strip a leading bullet glyph.
            if let Some(first) = items.first_mut() {
                first.text = first
                    .text
                    .trim_start_matches(['•', '·', '\u{b7}', '-', '*'])
                    .trim_start()
                    .to_string();
            }
            doc.blocks.push(Block::ListItem(0, false, items));
        } else if font_size >= 32 && items.iter().all(|i| i.bold || i.text.trim().is_empty()) {
            doc.blocks
                .push(Block::Heading(if font_size >= 40 { 1 } else { 2 }, items));
        } else {
            doc.blocks.push(Block::Paragraph(items));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' => {
                stack.push(st);
                i += 1;
            },
            '}' => {
                if st.skip && meta_dest.is_some() {
                    if let Some(k) = meta_dest.take() {
                        if !meta_buf.trim().is_empty() {
                            doc.metadata
                                .insert(k, serde_json::Value::String(meta_buf.trim().to_string()));
                        }
                        meta_buf.clear();
                    }
                }
                flush_run(&mut cur, &mut inlines, &st);
                st = stack.pop().unwrap_or(st);
                i += 1;
            },
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    break;
                }
                let n = chars[i];
                if n == '\'' {
                    // hex escape
                    let hex: String = chars[i + 1..(i + 3).min(chars.len())].iter().collect();
                    i += 3;
                    if pending_unicode_skip > 0 {
                        pending_unicode_skip -= 1;
                        continue;
                    }
                    if let Ok(b) = u8::from_str_radix(&hex, 16) {
                        if !st.skip {
                            cur.push(cp1252(b));
                        } else if meta_dest.is_some() {
                            meta_buf.push(cp1252(b));
                        }
                    }
                    continue;
                }
                if n == '\\' || n == '{' || n == '}' {
                    if !st.skip {
                        cur.push(n);
                    }
                    i += 1;
                    continue;
                }
                if n == '~' {
                    if !st.skip {
                        cur.push('\u{a0}');
                    }
                    i += 1;
                    continue;
                }
                if n == '-' || n == '_' {
                    i += 1;
                    continue;
                }
                if n == '*' {
                    // \* destination: skip unless known.
                    st.skip = true;
                    i += 1;
                    continue;
                }
                if n == '\n' || n == '\r' {
                    flush_run(&mut cur, &mut inlines, &st);
                    flush_para(&mut inlines, &mut doc, in_list, st.font_size);
                    i += 1;
                    continue;
                }
                // Control word.
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let mut param: Option<i64> = None;
                let pstart = i;
                if i < chars.len() && (chars[i] == '-' || chars[i].is_ascii_digit()) {
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    param = chars[pstart..i].iter().collect::<String>().parse().ok();
                }
                if i < chars.len() && chars[i] == ' ' {
                    i += 1;
                }
                let on = param.unwrap_or(1) != 0;
                match word.as_str() {
                    "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" | "object"
                    | "header" | "footer" | "headerf" | "footerf" | "xmlnstbl" | "listtable"
                    | "listoverridetable" | "themedata" | "datastore" | "latentstyles"
                    | "rsidtbl" | "generator" | "mmathPr" | "fldinst" | "shppict"
                    | "nonshppict" | "colorschememapping" => {
                        st.skip = true;
                        if word == "info" {
                            st.skip = false; // children like \title handled below
                        }
                    },
                    "title" | "author" | "subject" | "company" | "keywords" | "doccomm"
                    | "operator" => {
                        st.skip = true;
                        meta_dest = Some(word.clone());
                        meta_buf.clear();
                    },
                    "par" | "sect" | "page" => {
                        flush_run(&mut cur, &mut inlines, &st);
                        flush_para(&mut inlines, &mut doc, in_list, st.font_size);
                        if word == "page" {
                            doc.blocks.push(Block::PageBreak);
                        }
                        in_list = false;
                    },
                    "pard" => in_list = false,
                    "line" => {
                        if !st.skip {
                            cur.push('\n');
                        }
                    },
                    "tab" => {
                        if !st.skip {
                            cur.push('\t');
                        }
                    },
                    "b" => {
                        flush_run(&mut cur, &mut inlines, &st);
                        st.bold = on;
                    },
                    "i" => {
                        flush_run(&mut cur, &mut inlines, &st);
                        st.italic = on;
                    },
                    "ul" | "ulw" | "uld" | "uldb" => {
                        flush_run(&mut cur, &mut inlines, &st);
                        st.underline = on;
                    },
                    "ulnone" => {
                        flush_run(&mut cur, &mut inlines, &st);
                        st.underline = false;
                    },
                    "strike" => {
                        flush_run(&mut cur, &mut inlines, &st);
                        st.strike = on;
                    },
                    "plain" => {
                        flush_run(&mut cur, &mut inlines, &st);
                        st.bold = false;
                        st.italic = false;
                        st.underline = false;
                        st.strike = false;
                    },
                    "fs" => st.font_size = param.unwrap_or(24).max(1) as u32,
                    "uc" => st.uc = param.unwrap_or(1).max(0) as usize,
                    "u" => {
                        if let Some(p) = param {
                            let code = if p < 0 { p + 65536 } else { p } as u32;
                            if let Some(ch) = char::from_u32(code) {
                                if !st.skip {
                                    cur.push(ch);
                                } else if meta_dest.is_some() {
                                    meta_buf.push(ch);
                                }
                            }
                            pending_unicode_skip = st.uc;
                        }
                    },
                    "ls" | "pntext" | "listtext" => {
                        if word == "ls" {
                            in_list = true;
                        } else {
                            st.skip = true;
                            in_list = true;
                        }
                    },
                    "bullet" => {
                        if !st.skip {
                            cur.push('•');
                        }
                    },
                    "emdash" => {
                        if !st.skip {
                            cur.push('—');
                        }
                    },
                    "endash" => {
                        if !st.skip {
                            cur.push('–');
                        }
                    },
                    "lquote" | "rquote" => {
                        if !st.skip {
                            cur.push('\'');
                        }
                    },
                    "ldblquote" | "rdblquote" => {
                        if !st.skip {
                            cur.push('"');
                        }
                    },
                    "cell" => {
                        if !st.skip {
                            cur.push('\t');
                        }
                    },
                    "row" => {
                        flush_run(&mut cur, &mut inlines, &st);
                        flush_para(&mut inlines, &mut doc, false, st.font_size);
                    },
                    _ => {},
                }
            },
            '\r' | '\n' => i += 1,
            _ => {
                if pending_unicode_skip > 0 {
                    pending_unicode_skip -= 1;
                } else if !st.skip {
                    cur.push(c);
                } else if meta_dest.is_some() {
                    meta_buf.push(c);
                }
                i += 1;
            },
        }
    }
    flush_run(&mut cur, &mut inlines, &st);
    flush_para(&mut inlines, &mut doc, in_list, st.font_size);
    if doc.blocks.is_empty() {
        doc.warnings.push("No text content found".into());
    }
    Ok(doc)
}

fn cp1252(b: u8) -> char {
    match b {
        0x80 => '€',
        0x82 => '‚',
        0x83 => 'ƒ',
        0x84 => '„',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => 'ˆ',
        0x89 => '‰',
        0x8A => 'Š',
        0x8B => '‹',
        0x8C => 'Œ',
        0x8E => 'Ž',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201c}',
        0x94 => '\u{201d}',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x98 => '˜',
        0x99 => '™',
        0x9A => 'š',
        0x9B => '›',
        0x9C => 'œ',
        0x9E => 'ž',
        0x9F => 'Ÿ',
        _ => b as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rtf() {
        let rtf = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Calibri;}}{\info{\title My Doc}{\author Ada}}\pard\fs40\b Heading\b0\par\pard\fs24 Plain \b bold\b0  and \i italic\i0  caf\'e9 \u8364? end.\par{\pard\ls1 {\pntext\'b7\tab}First item\par}Next\line line\par}";
        let doc = parse(rtf).unwrap();
        assert_eq!(doc.metadata["title"], "My Doc");
        assert_eq!(doc.metadata["author"], "Ada");
        assert!(
            matches!(&doc.blocks[0], Block::Heading(1, i) if i[0].text == "Heading"),
            "{:?}",
            doc.blocks[0]
        );
        let text = doc.to_text();
        assert!(text.contains("Plain bold and italic café € end."), "{text}");
        assert!(
            matches!(&doc.blocks[2], Block::ListItem(0, false, i) if crate::model::plain(i) == "First item"),
            "{:?}",
            doc.blocks[2]
        );
        assert!(text.contains("Next\nline"));
        let html = doc.to_html(false, "");
        assert!(html.contains("<strong>bold</strong>"));
        assert!(parse("plain text").is_err());
        assert!(parse("").is_err());
    }
}
