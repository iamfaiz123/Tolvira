//! Minimal, dependency-free XLSX and ODS writers.
//!
//! The XLSX writer emits a strict-enough OOXML package (inline strings, a
//! small style sheet with bold headers and date formats, column widths,
//! frozen header row, formulas) that opens in Excel, LibreOffice, Numbers and
//! Google Sheets and round-trips through calamine. It deliberately avoids
//! `rust_xlsxwriter`, which calls the system clock (unavailable in
//! `wasm32-unknown-unknown`).

use crate::model::{column_letter, fmt_number, Cell, Sheet, Workbook};
use engine_shared::{EngineError, EngineResult};
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {},
            c => out.push(c),
        }
    }
    out
}

/// Parses an ISO date / datetime / time string into an Excel serial number
/// (1900 date system). Returns `(serial, kind)` where kind is `date`,
/// `datetime` or `time`.
pub fn iso_to_serial(s: &str) -> Option<(f64, &'static str)> {
    let t = s.trim();
    let (date_part, time_part) = match t.split_once(['T', ' ']) {
        Some((d, tm)) => (Some(d), Some(tm)),
        None if t.contains('-') => (Some(t), None),
        None => (None, Some(t)),
    };
    let mut serial = 0.0;
    let mut kind = "time";
    if let Some(d) = date_part {
        let mut it = d.split('-');
        let y: i64 = it.next()?.parse().ok()?;
        let m: i64 = it.next()?.parse().ok()?;
        let day: i64 = it.next()?.parse().ok()?;
        if !(1..=12).contains(&m) || !(1..=31).contains(&day) || !(1900..=9999).contains(&y) {
            return None;
        }
        serial = days_from_civil(y, m, day) as f64 - days_from_civil(1899, 12, 30) as f64;
        // Excel's phantom 1900-02-29: dates before 1900-03-01 are one day "early".
        if serial < 61.0 {
            serial -= 1.0;
        }
        kind = "date";
    }
    if let Some(tm) = time_part {
        let tm = tm.trim_end_matches('Z');
        let tm = tm.split(['+']).next().unwrap_or(tm);
        let mut it = tm.split(':');
        let h: f64 = it.next()?.parse().ok()?;
        let mi: f64 = it.next().unwrap_or("0").parse().ok()?;
        let sec: f64 = it.next().unwrap_or("0").parse().ok()?;
        if h >= 24.0 || mi >= 60.0 || sec >= 61.0 {
            return None;
        }
        serial += (h * 3600.0 + mi * 60.0 + sec) / 86400.0;
        kind = if kind == "date" {
            if h == 0.0 && mi == 0.0 && sec == 0.0 {
                "date"
            } else {
                "datetime"
            }
        } else {
            "time"
        };
    }
    Some((serial, kind))
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

const STYLES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts>
<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>
<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>
<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
<cellXfs count="5">
<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
<xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>
<xf numFmtId="14" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>
<xf numFmtId="22" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>
<xf numFmtId="21" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>
</cellXfs>
<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>
</styleSheet>"#;

/// Options for XLSX output.
#[derive(Debug, Clone)]
pub struct XlsxOptions {
    /// Render the first row bold.
    pub bold_header: bool,
    /// Freeze the first row.
    pub freeze_header: bool,
    /// Approximate column widths from content.
    pub autofit: bool,
    /// Add an auto-filter to the header row.
    pub auto_filter: bool,
}

impl Default for XlsxOptions {
    fn default() -> Self {
        Self {
            bold_header: true,
            freeze_header: true,
            autofit: true,
            auto_filter: false,
        }
    }
}

fn sheet_xml(sheet: &Sheet, o: &XlsxOptions) -> String {
    let rows = sheet.rows();
    let cols = sheet.cols();
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
    );
    if rows > 0 && cols > 0 {
        xml.push_str(&format!(r#"<dimension ref="A1:{}{}"/>"#, column_letter(cols), rows));
    } else {
        xml.push_str(r#"<dimension ref="A1"/>"#);
    }
    xml.push_str(r#"<sheetViews><sheetView workbookViewId="0""#);
    if rows == 0 || !o.freeze_header {
        xml.push_str("/></sheetViews>");
    } else {
        xml.push_str(r#"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>"#);
    }
    xml.push_str(r#"<sheetFormatPr defaultRowHeight="15"/>"#);
    if o.autofit && cols > 0 {
        xml.push_str("<cols>");
        for c in 0..cols {
            let max_len = sheet
                .cells
                .iter()
                .take(500)
                .map(|r| r.get(c).map(|x| x.to_display().chars().count()).unwrap_or(0))
                .max()
                .unwrap_or(0);
            let width = ((max_len as f64) * 1.1 + 2.0).clamp(8.0, 60.0);
            xml.push_str(&format!(
                r#"<col min="{}" max="{}" width="{:.2}" customWidth="1"/>"#,
                c + 1,
                c + 1,
                width
            ));
        }
        xml.push_str("</cols>");
    }
    xml.push_str("<sheetData>");
    for (ri, row) in sheet.cells.iter().enumerate() {
        if row.iter().all(Cell::is_empty) && !sheet.formulas.keys().any(|(r, _)| *r == ri) {
            continue;
        }
        xml.push_str(&format!(r#"<row r="{}">"#, ri + 1));
        for (ci, cell) in row.iter().enumerate() {
            let reference = format!("{}{}", column_letter(ci + 1), ri + 1);
            let formula = sheet.formulas.get(&(ri, ci));
            let style = if ri == 0 && o.bold_header { r#" s="1""# } else { "" };
            let f_xml = formula
                .map(|f| format!("<f>{}</f>", xml_escape(f.trim_start_matches('='))))
                .unwrap_or_default();
            match cell {
                Cell::Empty => {
                    if formula.is_some() {
                        xml.push_str(&format!(r#"<c r="{reference}"{style}>{f_xml}</c>"#));
                    }
                },
                Cell::Number(n) => xml.push_str(&format!(
                    r#"<c r="{reference}"{style}>{f_xml}<v>{}</v></c>"#,
                    fmt_number_xml(*n)
                )),
                Cell::Bool(b) => xml.push_str(&format!(
                    r#"<c r="{reference}" t="b"{style}>{f_xml}<v>{}</v></c>"#,
                    if *b { 1 } else { 0 }
                )),
                Cell::Text(s) => {
                    if formula.is_some() {
                        xml.push_str(&format!(
                            r#"<c r="{reference}" t="str"{style}>{f_xml}<v>{}</v></c>"#,
                            xml_escape(s)
                        ));
                    } else {
                        xml.push_str(&format!(r#"<c r="{reference}" t="inlineStr"{style}><is><t xml:space="preserve">{}</t></is></c>"#, xml_escape(s)));
                    }
                },
                Cell::Date(d) => match iso_to_serial(d) {
                    Some((serial, kind)) => {
                        let s = if ri == 0 && o.bold_header {
                            1
                        } else {
                            match kind {
                                "date" => 2,
                                "datetime" => 3,
                                _ => 4,
                            }
                        };
                        xml.push_str(&format!(
                            r#"<c r="{reference}" s="{s}">{f_xml}<v>{}</v></c>"#,
                            fmt_number_xml(serial)
                        ));
                    },
                    None => xml.push_str(&format!(
                        r#"<c r="{reference}" t="inlineStr"{style}><is><t>{}</t></is></c>"#,
                        xml_escape(d)
                    )),
                },
                Cell::Error(e) => xml.push_str(&format!(
                    r#"<c r="{reference}" t="e"{style}>{f_xml}<v>{}</v></c>"#,
                    xml_escape(e)
                )),
            }
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData>");
    if o.auto_filter && rows > 1 && cols > 0 {
        xml.push_str(&format!(r#"<autoFilter ref="A1:{}{}"/>"#, column_letter(cols), rows));
    }
    if !sheet.merged.is_empty() {
        xml.push_str(&format!(r#"<mergeCells count="{}">"#, sheet.merged.len()));
        for m in &sheet.merged {
            xml.push_str(&format!(r#"<mergeCell ref="{}"/>"#, xml_escape(m)));
        }
        xml.push_str("</mergeCells>");
    }
    xml.push_str(r#"<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/></worksheet>"#);
    xml
}

fn fmt_number_xml(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn add_file<W: Write + std::io::Seek>(
    z: &mut ZipWriter<W>,
    name: &str,
    data: &[u8],
    stored: bool,
) -> EngineResult<()> {
    let opts = SimpleFileOptions::default().compression_method(if stored {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    });
    z.start_file(name, opts)
        .map_err(|e| EngineError::internal(e.to_string()))?;
    z.write_all(data).map_err(|e| EngineError::internal(e.to_string()))
}

/// Serialises the workbook as XLSX bytes.
pub fn write_xlsx(wb: &Workbook, o: &XlsxOptions) -> EngineResult<Vec<u8>> {
    if wb.sheets.is_empty() {
        return Err(EngineError::empty("sheets"));
    }
    engine_shared::limits::check_count(wb.sheets.len(), 1024, "sheets")?;
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let mut ct = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>"#,
    );
    for i in 0..wb.sheets.len() {
        ct.push_str(&format!(r#"<Override PartName="/xl/worksheets/sheet{}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#, i + 1));
    }
    ct.push_str("</Types>");
    add_file(&mut z, "[Content_Types].xml", ct.as_bytes(), false)?;
    add_file(&mut z, "_rels/.rels", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#, false)?;
    add_file(&mut z, "docProps/app.xml", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>Dev Toolbox Spreadsheet Engine</Application></Properties>"#, false)?;
    add_file(&mut z, "docProps/core.xml", br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>Dev Toolbox</dc:creator></cp:coreProperties>"#, false)?;
    let mut workbook = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><bookViews><workbookView/></bookViews><sheets>"#,
    );
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    let mut used_names: Vec<String> = Vec::new();
    for (i, s) in wb.sheets.iter().enumerate() {
        let name = crate::model::safe_sheet_name(&s.name, &used_names);
        used_names.push(name.clone());
        workbook.push_str(&format!(
            r#"<sheet name="{}" sheetId="{}" {}r:id="rId{}"/>"#,
            xml_escape(&name),
            i + 1,
            if s.hidden { r#"state="hidden" "# } else { "" },
            i + 1
        ));
        rels.push_str(&format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{}.xml"/>"#, i + 1, i + 1));
        add_file(
            &mut z,
            &format!("xl/worksheets/sheet{}.xml", i + 1),
            sheet_xml(s, o).as_bytes(),
            false,
        )?;
    }
    workbook.push_str("</sheets>");
    if !wb.defined_names.is_empty() {
        workbook.push_str("<definedNames>");
        for (n, r) in &wb.defined_names {
            workbook.push_str(&format!(
                r#"<definedName name="{}">{}</definedName>"#,
                xml_escape(n),
                xml_escape(r)
            ));
        }
        workbook.push_str("</definedNames>");
    }
    workbook.push_str("</workbook>");
    rels.push_str(&format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#, wb.sheets.len() + 1));
    add_file(&mut z, "xl/workbook.xml", workbook.as_bytes(), false)?;
    add_file(&mut z, "xl/_rels/workbook.xml.rels", rels.as_bytes(), false)?;
    add_file(&mut z, "xl/styles.xml", STYLES_XML.as_bytes(), false)?;
    let cursor = z.finish().map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(cursor.into_inner())
}

// -------------------------------------------------------------------- ODS --

fn ods_cell(cell: &Cell, formula: Option<&String>) -> String {
    let f_attr = formula
        .map(|f| format!(r#" table:formula="of:={}""#, xml_escape(f.trim_start_matches('='))))
        .unwrap_or_default();
    match cell {
        Cell::Empty => {
            if formula.is_some() {
                format!("<table:table-cell{f_attr}/>")
            } else {
                "<table:table-cell/>".into()
            }
        },
        Cell::Number(n) => format!(
            r#"<table:table-cell office:value-type="float" office:value="{}"{f_attr}><text:p>{}</text:p></table:table-cell>"#,
            fmt_number_xml(*n),
            fmt_number(*n)
        ),
        Cell::Bool(b) => format!(
            r#"<table:table-cell office:value-type="boolean" office:boolean-value="{b}"{f_attr}><text:p>{}</text:p></table:table-cell>"#,
            if *b { "TRUE" } else { "FALSE" }
        ),
        Cell::Text(s) => format!(
            r#"<table:table-cell office:value-type="string"{f_attr}><text:p>{}</text:p></table:table-cell>"#,
            xml_escape(s)
        ),
        Cell::Date(d) => {
            let kind = iso_to_serial(d).map(|x| x.1).unwrap_or("string");
            match kind {
                "time" => {
                    let parts: Vec<&str> = d.split(':').collect();
                    format!(
                        r#"<table:table-cell office:value-type="time" office:time-value="PT{}H{}M{}S"{f_attr}><text:p>{}</text:p></table:table-cell>"#,
                        parts.first().unwrap_or(&"0"),
                        parts.get(1).unwrap_or(&"0"),
                        parts.get(2).unwrap_or(&"0"),
                        xml_escape(d)
                    )
                },
                "date" | "datetime" => format!(
                    r#"<table:table-cell office:value-type="date" office:date-value="{}"{f_attr}><text:p>{}</text:p></table:table-cell>"#,
                    xml_escape(d),
                    xml_escape(d)
                ),
                _ => format!(
                    r#"<table:table-cell office:value-type="string"{f_attr}><text:p>{}</text:p></table:table-cell>"#,
                    xml_escape(d)
                ),
            }
        },
        Cell::Error(e) => format!(
            r#"<table:table-cell office:value-type="string"{f_attr}><text:p>{}</text:p></table:table-cell>"#,
            xml_escape(e)
        ),
    }
}

/// Serialises the workbook as an OpenDocument Spreadsheet.
pub fn write_ods(wb: &Workbook) -> EngineResult<Vec<u8>> {
    if wb.sheets.is_empty() {
        return Err(EngineError::empty("sheets"));
    }
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    // The mimetype member must be first and stored.
    add_file(&mut z, "mimetype", b"application/vnd.oasis.opendocument.spreadsheet", true)?;
    add_file(&mut z, "META-INF/manifest.xml", br#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2"><manifest:file-entry manifest:full-path="/" manifest:version="1.2" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/><manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/></manifest:manifest>"#, false)?;
    add_file(&mut z, "styles.xml", br#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" office:version="1.2"><office:styles/></office:document-styles>"#, false)?;
    add_file(&mut z, "meta.xml", br#"<?xml version="1.0" encoding="UTF-8"?><office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" office:version="1.2"><office:meta><meta:generator>Dev Toolbox Spreadsheet Engine</meta:generator></office:meta></office:document-meta>"#, false)?;
    let mut content = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2" office:version="1.2"><office:body><office:spreadsheet>"#,
    );
    let mut used: Vec<String> = Vec::new();
    for s in &wb.sheets {
        let name = crate::model::safe_sheet_name(&s.name, &used);
        used.push(name.clone());
        content.push_str(&format!(r#"<table:table table:name="{}">"#, xml_escape(&name)));
        content.push_str(&format!(
            r#"<table:table-column table:number-columns-repeated="{}"/>"#,
            s.cols().max(1)
        ));
        for (ri, row) in s.cells.iter().enumerate() {
            content.push_str("<table:table-row>");
            for (ci, c) in row.iter().enumerate() {
                content.push_str(&ods_cell(c, s.formulas.get(&(ri, ci))));
            }
            content.push_str("</table:table-row>");
        }
        if s.cells.is_empty() {
            content.push_str("<table:table-row><table:table-cell/></table:table-row>");
        }
        content.push_str("</table:table>");
    }
    content.push_str("</office:spreadsheet></office:body></office:document-content>");
    add_file(&mut z, "content.xml", content.as_bytes(), false)?;
    let cursor = z.finish().map_err(|e| EngineError::internal(e.to_string()))?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serials() {
        assert_eq!(iso_to_serial("1900-01-01"), Some((1.0, "date")));
        assert_eq!(iso_to_serial("2024-01-01"), Some((45292.0, "date")));
        assert_eq!(iso_to_serial("2024-01-01T12:00:00").unwrap().0, 45292.5);
        assert_eq!(iso_to_serial("06:00:00"), Some((0.25, "time")));
        assert_eq!(iso_to_serial("2024-13-01"), None);
    }

    #[test]
    fn xlsx_roundtrip() {
        let mut s = Sheet::new("Data");
        s.set(0, 0, Cell::Text("name".into()));
        s.set(0, 1, Cell::Text("qty".into()));
        s.set(0, 2, Cell::Text("when".into()));
        s.set(1, 0, Cell::Text("a<b&c".into()));
        s.set(1, 1, Cell::Number(2.5));
        s.set(1, 2, Cell::Date("2024-03-05".into()));
        s.set(2, 0, Cell::Bool(true));
        s.set(2, 1, Cell::Number(3.0));
        s.formulas.insert((3, 1), "SUM(B2:B3)".into());
        s.set(3, 1, Cell::Number(5.5));
        let wb = Workbook {
            sheets: vec![s],
            format: "xlsx".into(),
            defined_names: vec![],
            has_macros: false,
        };
        let bytes = write_xlsx(&wb, &XlsxOptions::default()).unwrap();
        let back = Workbook::read(&bytes, None, true).unwrap();
        assert_eq!(back.format, "xlsx");
        let sh = &back.sheets[0];
        assert_eq!(sh.name, "Data");
        assert_eq!(sh.get(1, 0), &Cell::Text("a<b&c".into()));
        assert_eq!(sh.get(1, 1), &Cell::Number(2.5));
        assert_eq!(sh.get(1, 2), &Cell::Date("2024-03-05".into()));
        assert_eq!(sh.get(2, 0), &Cell::Bool(true));
        assert_eq!(sh.formulas.get(&(3, 1)).map(String::as_str), Some("SUM(B2:B3)"));
        let ods = write_ods(&wb).unwrap();
        let back = Workbook::read(&ods, None, true).unwrap();
        assert_eq!(back.format, "ods");
        assert_eq!(back.sheets[0].get(1, 1), &Cell::Number(2.5));
        assert_eq!(back.sheets[0].get(1, 0), &Cell::Text("a<b&c".into()));
    }
}
