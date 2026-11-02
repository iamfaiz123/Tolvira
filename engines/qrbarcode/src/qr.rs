//! QR code generation (SVG / PNG with styling), payload builders and decoding.

use engine_shared::{EngineError, EngineResult};
use image::{ImageBuffer, Rgba};
use qrcode::{EcLevel, QrCode, Version};
use serde::Deserialize;
use serde_json::{json, Value};

/// Maximum payload accepted (binary capacity of version 40-L).
pub const MAX_PAYLOAD_BYTES: usize = 2953; // QR v40-L binary capacity

/// Visual options shared by every generator.
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Style {
    /// `L`, `M` (default), `Q`, `H`.
    pub error_correction: String,
    /// Pixels per module for raster output (default 8).
    pub module_size: u32,
    /// Quiet zone in modules (default 4, spec minimum).
    pub margin: u32,
    /// Foreground colour (`#rrggbb` or `#rrggbbaa`).
    pub foreground: String,
    /// Background colour; `transparent` allowed for SVG/PNG.
    pub background: String,
    /// `square` (default), `rounded`, `dots`, `diamond`.
    pub module_shape: String,
    /// `square` (default), `rounded`, `circle`.
    pub eye_shape: String,
    /// Optional distinct colour for the finder patterns.
    pub eye_color: String,
    /// Optional second colour for a linear gradient (SVG only).
    pub gradient_to: String,
    /// Force a QR version (1-40); 0 = smallest that fits.
    pub version: u8,
    /// Reserve a clear square in the middle (fraction of width, e.g. 0.2) for a logo.
    pub logo_area: f64,
    /// Output size in pixels for PNG (overrides module_size when > 0).
    pub size: u32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            error_correction: "M".into(),
            module_size: 8,
            margin: 4,
            foreground: "#000000".into(),
            background: "#ffffff".into(),
            module_shape: "square".into(),
            eye_shape: "square".into(),
            eye_color: String::new(),
            gradient_to: String::new(),
            version: 0,
            logo_area: 0.0,
            size: 0,
        }
    }
}

fn ec_level(s: &str) -> EngineResult<EcLevel> {
    Ok(match s.trim().to_ascii_uppercase().as_str() {
        "L" | "LOW" | "7" => EcLevel::L,
        "" | "M" | "MEDIUM" | "15" => EcLevel::M,
        "Q" | "QUARTILE" | "25" => EcLevel::Q,
        "H" | "HIGH" | "30" => EcLevel::H,
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown error correction level `{other}` (L, M, Q, H)"
            )))
        },
    })
}

fn ec_label(e: EcLevel) -> &'static str {
    match e {
        EcLevel::L => "L",
        EcLevel::M => "M",
        EcLevel::Q => "Q",
        EcLevel::H => "H",
    }
}

/// Parses `#rgb`, `#rrggbb`, `#rrggbbaa`, `transparent`, or a handful of names.
pub fn parse_color(s: &str) -> EngineResult<[u8; 4]> {
    let t = s.trim().to_ascii_lowercase();
    match t.as_str() {
        "" | "transparent" | "none" => return Ok([0, 0, 0, 0]),
        "black" => return Ok([0, 0, 0, 255]),
        "white" => return Ok([255, 255, 255, 255]),
        "red" => return Ok([255, 0, 0, 255]),
        "green" => return Ok([0, 128, 0, 255]),
        "blue" => return Ok([0, 0, 255, 255]),
        _ => {},
    }
    let hex = t.trim_start_matches('#');
    let expand = |h: &str| -> Option<Vec<u8>> {
        if !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        match h.len() {
            3 | 4 => h
                .chars()
                .map(|c| u8::from_str_radix(&format!("{c}{c}"), 16).ok())
                .collect(),
            6 | 8 => (0..h.len() / 2)
                .map(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).ok())
                .collect(),
            _ => None,
        }
    };
    let v = expand(hex).ok_or_else(|| {
        EngineError::invalid(format!("Invalid colour `{s}` (use #rrggbb or #rrggbbaa)"))
    })?;
    Ok([v[0], v[1], v[2], v.get(3).copied().unwrap_or(255)])
}

fn css_color(c: [u8; 4]) -> String {
    if c[3] == 0 {
        "none".into()
    } else if c[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
    } else {
        format!("rgba({},{},{},{:.3})", c[0], c[1], c[2], c[3] as f64 / 255.0)
    }
}

/// Module matrix with helpers for finder-pattern detection.
pub struct Matrix {
    /// Modules per side.
    pub width: usize,
    /// Row-major modules (`true` = dark).
    pub modules: Vec<bool>,
    /// Symbol version (1–40).
    pub version: i16,
    /// Error-correction level.
    pub ec: EcLevel,
}

impl Matrix {
    /// Module at `(x, y)`; `false` outside the matrix.
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.modules[y * self.width + x]
    }
    /// Is the module part of one of the three finder patterns (7×7 incl. border)?
    pub fn in_eye(&self, x: usize, y: usize) -> bool {
        let w = self.width;
        (x < 7 && y < 7) || (x >= w - 7 && y < 7) || (x < 7 && y >= w - 7)
    }
    /// Whether `(x, y)` lies inside the central logo clearance area of `fraction` width.
    pub fn in_logo_area(&self, x: usize, y: usize, fraction: f64) -> bool {
        if fraction <= 0.0 {
            return false;
        }
        let side = ((self.width as f64) * fraction.min(0.35)).round() as usize;
        let start = (self.width - side) / 2;
        x >= start && x < start + side && y >= start && y < start + side
    }
}

/// Encodes bytes into a QR matrix according to the style (error correction, minimum version).
pub fn build(data: &[u8], style: &Style) -> EngineResult<Matrix> {
    if data.is_empty() {
        return Err(EngineError::empty("data"));
    }
    if data.len() > MAX_PAYLOAD_BYTES {
        return Err(EngineError::limit(format!(
            "Payload is {} bytes; QR codes hold at most {MAX_PAYLOAD_BYTES} bytes (version 40, \
             level L)",
            data.len()
        )));
    }
    let ec = ec_level(&style.error_correction)?;
    let ec = if style.logo_area > 0.0 && matches!(ec, EcLevel::L | EcLevel::M) {
        EcLevel::H
    } else {
        ec
    };
    let code = if style.version > 0 {
        if style.version > 40 {
            return Err(EngineError::invalid("QR version must be 1-40"));
        }
        QrCode::with_version(data, Version::Normal(style.version as i16), ec).map_err(|e| {
            EngineError::invalid(format!("Data does not fit QR version {}: {e}", style.version))
        })?
    } else {
        QrCode::with_error_correction_level(data, ec).map_err(|e| {
            EngineError::limit(format!(
                "Data too long for a QR code at level {}: {e}",
                ec_label(ec)
            ))
        })?
    };
    let version = match code.version() {
        Version::Normal(v) => v,
        Version::Micro(v) => -v,
    };
    Ok(Matrix {
        width: code.width(),
        modules: code
            .to_colors()
            .into_iter()
            .map(|c| c == qrcode::Color::Dark)
            .collect(),
        version,
        ec,
    })
}

fn eye_path(x0: f64, y0: f64, shape: &str, fg: &str) -> String {
    // Outer 7×7 ring (1 module thick) + inner 3×3 block.
    match shape {
        "rounded" => format!(
            r#"<rect x="{}" y="{}" width="6" height="6" rx="1.8" fill="none" stroke="{fg}" stroke-width="1"/><rect x="{}" y="{}" width="3" height="3" rx="0.8" fill="{fg}"/>"#,
            x0 + 0.5,
            y0 + 0.5,
            x0 + 2.0,
            y0 + 2.0
        ),
        "circle" => format!(
            r#"<circle cx="{}" cy="{}" r="3" fill="none" stroke="{fg}" stroke-width="1"/><circle cx="{}" cy="{}" r="1.5" fill="{fg}"/>"#,
            x0 + 3.5,
            y0 + 3.5,
            x0 + 3.5,
            y0 + 3.5
        ),
        _ => format!(
            r#"<path fill="{fg}" fill-rule="evenodd" d="M{x} {y}h7v7h-7z M{x1} {y1}h5v5h-5z"/><rect x="{x2}" y="{y2}" width="3" height="3" fill="{fg}"/>"#,
            x = x0,
            y = y0,
            x1 = x0 + 1.0,
            y1 = y0 + 1.0,
            x2 = x0 + 2.0,
            y2 = y0 + 2.0
        ),
    }
}

/// Renders the matrix as an SVG string (viewBox in module units).
pub fn to_svg(m: &Matrix, style: &Style) -> EngineResult<String> {
    let fg = parse_color(&style.foreground)?;
    let bg = parse_color(&style.background)?;
    let eye_fg = if style.eye_color.is_empty() {
        fg
    } else {
        parse_color(&style.eye_color)?
    };
    let margin = style.margin as f64;
    let total = m.width as f64 + margin * 2.0;
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total} {total}" shape-rendering="crispEdges" width="{px}" height="{px}">"#,
        px = (total * style.module_size.max(1) as f64) as u32
    );
    let fill_ref = if style.gradient_to.is_empty() {
        css_color(fg)
    } else {
        let to = parse_color(&style.gradient_to)?;
        svg.push_str(&format!(r#"<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient></defs>"#, css_color(fg), css_color(to)));
        "url(#g)".to_string()
    };
    if bg[3] > 0 {
        svg.push_str(&format!(r#"<rect width="100%" height="100%" fill="{}"/>"#, css_color(bg)));
    }
    let shape = style.module_shape.as_str();
    let mut path = String::new();
    let mut extra = String::new();
    for y in 0..m.width {
        for x in 0..m.width {
            if !m.get(x, y) || m.in_eye(x, y) || m.in_logo_area(x, y, style.logo_area) {
                continue;
            }
            let (px, py) = (x as f64 + margin, y as f64 + margin);
            match shape {
                "dots" | "circle" => extra.push_str(&format!(
                    r#"<circle cx="{}" cy="{}" r="0.45" fill="{fill_ref}"/>"#,
                    px + 0.5,
                    py + 0.5
                )),
                "rounded" => extra.push_str(&format!(
                    r#"<rect x="{}" y="{}" width="0.9" height="0.9" rx="0.3" fill="{fill_ref}"/>"#,
                    px + 0.05,
                    py + 0.05
                )),
                "diamond" => extra.push_str(&format!(
                    r#"<path d="M{} {}l0.5 0.5l-0.5 0.5l-0.5 -0.5z" fill="{fill_ref}"/>"#,
                    px + 0.5,
                    py
                )),
                _ => path.push_str(&format!("M{px} {py}h1v1h-1z")),
            }
        }
    }
    if !path.is_empty() {
        svg.push_str(&format!(r#"<path d="{path}" fill="{fill_ref}"/>"#));
    }
    svg.push_str(&extra);
    let eye_fill = if style.eye_color.is_empty() && !style.gradient_to.is_empty() {
        fill_ref.clone()
    } else {
        css_color(eye_fg)
    };
    let w = m.width as f64;
    for (ex, ey) in [(0.0, 0.0), (w - 7.0, 0.0), (0.0, w - 7.0)] {
        svg.push_str(&eye_path(ex + margin, ey + margin, &style.eye_shape, &eye_fill));
    }
    if shape != "square" {
        svg = svg.replace(r#" shape-rendering="crispEdges""#, "");
    }
    svg.push_str("</svg>");
    Ok(svg)
}

/// Renders the matrix to PNG bytes.
pub fn to_png(m: &Matrix, style: &Style) -> EngineResult<Vec<u8>> {
    let fg = parse_color(&style.foreground)?;
    let bg = parse_color(&style.background)?;
    let eye_fg = if style.eye_color.is_empty() {
        fg
    } else {
        parse_color(&style.eye_color)?
    };
    let total_modules = m.width as u32 + style.margin * 2;
    let scale = if style.size > 0 {
        (style.size / total_modules).max(1)
    } else {
        style.module_size.max(1)
    };
    let px = total_modules * scale;
    engine_shared::limits::check_dimensions(px, px)?;
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(px, px, Rgba(bg));
    let shape = style.module_shape.as_str();
    for y in 0..m.width {
        for x in 0..m.width {
            if !m.get(x, y) || m.in_logo_area(x, y, style.logo_area) {
                continue;
            }
            let color = if m.in_eye(x, y) { eye_fg } else { fg };
            let (ox, oy) = ((x as u32 + style.margin) * scale, (y as u32 + style.margin) * scale);
            let is_eye = m.in_eye(x, y);
            for dy in 0..scale {
                for dx in 0..scale {
                    let draw = if is_eye || shape == "square" {
                        true
                    } else {
                        // Normalised coords in [0,1)
                        let (u, v) = (
                            (dx as f64 + 0.5) / scale as f64 - 0.5,
                            (dy as f64 + 0.5) / scale as f64 - 0.5,
                        );
                        match shape {
                            "dots" | "circle" => u * u + v * v <= 0.45 * 0.45,
                            "rounded" => {
                                let r = 0.3;
                                let (au, av) = (u.abs(), v.abs());
                                let lim = 0.45;
                                if au <= lim - r || av <= lim - r {
                                    au <= lim && av <= lim
                                } else {
                                    (au - (lim - r)).powi(2) + (av - (lim - r)).powi(2) <= r * r
                                }
                            },
                            "diamond" => u.abs() + v.abs() <= 0.5,
                            _ => true,
                        }
                    };
                    if draw {
                        img.put_pixel(ox + dx, oy + dy, Rgba(color));
                    }
                }
            }
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| EngineError::internal(format!("PNG encode failed: {e}")))?;
    Ok(out.into_inner())
}

/// Matrix as JSON (width, rows of 0/1 strings, version, EC level).
pub fn matrix_json(m: &Matrix) -> Value {
    json!({
        "version": m.version,
        "modules": m.width,
        "error_correction": ec_label(m.ec),
        "rows": (0..m.width).map(|y| (0..m.width).map(|x| if m.get(x, y) { '1' } else { '0' }).collect::<String>()).collect::<Vec<_>>()
    })
}

/// Produces every output form for a payload.
pub fn generate(
    data: &str,
    style: &Style,
    formats: &[String],
    include_matrix: bool,
) -> EngineResult<Value> {
    let m = build(data.as_bytes(), style)?;
    let want = |f: &str| formats.is_empty() || formats.iter().any(|x| x.eq_ignore_ascii_case(f));
    let mut out = json!({
        "payload": data,
        "payload_bytes": data.len(),
        "version": m.version,
        "modules": m.width,
        "error_correction": ec_label(m.ec),
        "size_px": (m.width as u32 + style.margin * 2) * style.module_size.max(1),
        "payload_type": classify_payload(data)["type"],
    });
    if want("svg") {
        out["svg"] = json!(to_svg(&m, style)?);
    }
    if want("png") {
        let png = to_png(&m, style)?;
        out["png"] = json!(engine_shared::b64::encode(&png));
        out["png_data_url"] =
            json!(format!("data:image/png;base64,{}", engine_shared::b64::encode(&png)));
    }
    if want("ascii") {
        let mut s = String::new();
        for y in (0..m.width).step_by(2) {
            for x in 0..m.width {
                let top = m.get(x, y);
                let bottom = y + 1 < m.width && m.get(x, y + 1);
                s.push(match (top, bottom) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    _ => ' ',
                });
            }
            s.push('\n');
        }
        out["ascii"] = json!(s);
    }
    if include_matrix || want("matrix") {
        out["matrix"] = matrix_json(&m);
    }
    if style.logo_area > 0.0 {
        out["logo_area"] = json!({
            "fraction": style.logo_area.min(0.35),
            "note": "Error correction raised to H; place the logo centred over the cleared square"
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------- payloads --

fn esc_wifi(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace(':', "\\:")
        .replace('"', "\\\"")
}

/// Builds a `WIFI:` payload with proper escaping.
pub fn wifi_payload(
    ssid: &str,
    password: &str,
    security: &str,
    hidden: bool,
    eap: Option<&str>,
    identity: Option<&str>,
) -> EngineResult<String> {
    engine_shared::error::require_text(ssid, "ssid")?;
    let sec = match security.trim().to_ascii_uppercase().as_str() {
        "" | "WPA" | "WPA2" | "WPA3" | "WPA/WPA2" | "WPA2/WPA3" => "WPA",
        "WEP" => "WEP",
        "NONE" | "NOPASS" | "OPEN" => "nopass",
        "WPA2-EAP" | "WPA3-EAP" | "EAP" => "WPA2-EAP",
        other => {
            return Err(EngineError::invalid(format!(
                "Unknown Wi-Fi security `{other}` (WPA, WPA2, WPA3, WEP, nopass, WPA2-EAP)"
            )))
        },
    };
    if sec != "nopass" && password.is_empty() {
        return Err(EngineError::invalid(
            "Password is required for secured networks (use security `nopass` for open networks)",
        ));
    }
    if sec == "WPA"
        && !password.is_empty()
        && !(8..=63).contains(&password.chars().count())
        && password.len() != 64
    {
        return Err(EngineError::invalid(
            "WPA passwords must be 8-63 characters (or a 64-hex PSK)",
        ));
    }
    let mut s = format!("WIFI:T:{sec};S:{};", esc_wifi(ssid));
    if sec != "nopass" {
        s.push_str(&format!("P:{};", esc_wifi(password)));
    }
    if let Some(e) = eap.filter(|e| !e.is_empty()) {
        s.push_str(&format!("E:{};", esc_wifi(e)));
    }
    if let Some(i) = identity.filter(|i| !i.is_empty()) {
        s.push_str(&format!("I:{};", esc_wifi(i)));
    }
    if hidden {
        s.push_str("H:true;");
    }
    s.push(';');
    Ok(s)
}

/// Contact fields for a vCard QR.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct VCard {
    /// Given name.
    pub first_name: String,
    /// Family name.
    pub last_name: String,
    /// Formatted full name (derived when empty).
    pub full_name: String,
    /// Organisation.
    pub organization: String,
    /// Job title.
    pub title: String,
    /// Work phone.
    pub phone: String,
    /// Mobile phone.
    pub mobile: String,
    /// Fax.
    pub fax: String,
    /// E-mail.
    pub email: String,
    /// Website URL.
    pub website: String,
    /// Street address.
    pub street: String,
    /// City.
    pub city: String,
    /// State / region.
    pub state: String,
    /// Postal code.
    pub zip: String,
    /// Country.
    pub country: String,
    /// Free-text note.
    pub note: String,
    /// Birthday (`YYYY-MM-DD`).
    pub birthday: String,
    /// `3.0` (default, best reader support) or `4.0`.
    pub version: String,
    /// Emit MECARD instead of vCard (more compact, Japanese phones).
    pub mecard: bool,
}

fn esc_vcard(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(';', "\\;")
        .replace('\n', "\\n")
}

/// Builds a vCard 3.0 payload.
pub fn vcard_payload(v: &VCard) -> EngineResult<String> {
    let fn_ = if v.full_name.is_empty() {
        format!("{} {}", v.first_name, v.last_name).trim().to_string()
    } else {
        v.full_name.clone()
    };
    if fn_.is_empty() && v.organization.is_empty() {
        return Err(EngineError::invalid("A name or organization is required"));
    }
    if !v.email.is_empty() && !v.email.contains('@') {
        return Err(EngineError::invalid("Invalid email address"));
    }
    if v.mecard {
        let esc = |s: &str| s.replace('\\', "\\\\").replace(':', "\\:").replace(';', "\\;");
        let mut s = format!("MECARD:N:{},{};", esc(&v.last_name), esc(&v.first_name));
        for (k, val) in [
            ("TEL", &v.phone),
            ("TEL", &v.mobile),
            ("EMAIL", &v.email),
            ("URL", &v.website),
            ("ORG", &v.organization),
            ("NOTE", &v.note),
            ("BDAY", &v.birthday),
        ] {
            if !val.is_empty() {
                s.push_str(&format!("{k}:{};", esc(val)));
            }
        }
        if !v.street.is_empty() || !v.city.is_empty() {
            s.push_str(&format!(
                "ADR:,,{},{},{},{},{};",
                esc(&v.street),
                esc(&v.city),
                esc(&v.state),
                esc(&v.zip),
                esc(&v.country)
            ));
        }
        s.push(';');
        return Ok(s);
    }
    let version = if v.version == "4.0" { "4.0" } else { "3.0" };
    let mut lines = vec!["BEGIN:VCARD".to_string(), format!("VERSION:{version}")];
    lines.push(format!("N:{};{};;;", esc_vcard(&v.last_name), esc_vcard(&v.first_name)));
    lines.push(format!("FN:{}", esc_vcard(&fn_)));
    if !v.organization.is_empty() {
        lines.push(format!("ORG:{}", esc_vcard(&v.organization)));
    }
    if !v.title.is_empty() {
        lines.push(format!("TITLE:{}", esc_vcard(&v.title)));
    }
    let tel = |kind: &str, n: &str| {
        if version == "4.0" {
            format!("TEL;TYPE={kind};VALUE=uri:tel:{}", n.replace(' ', ""))
        } else {
            format!("TEL;TYPE={kind}:{n}")
        }
    };
    if !v.phone.is_empty() {
        lines.push(tel("WORK,VOICE", &v.phone));
    }
    if !v.mobile.is_empty() {
        lines.push(tel("CELL", &v.mobile));
    }
    if !v.fax.is_empty() {
        lines.push(tel("FAX", &v.fax));
    }
    if !v.email.is_empty() {
        lines.push(format!("EMAIL;TYPE=INTERNET:{}", v.email));
    }
    if !v.website.is_empty() {
        lines.push(format!("URL:{}", v.website));
    }
    if [&v.street, &v.city, &v.state, &v.zip, &v.country]
        .iter()
        .any(|s| !s.is_empty())
    {
        lines.push(format!(
            "ADR;TYPE=WORK:;;{};{};{};{};{}",
            esc_vcard(&v.street),
            esc_vcard(&v.city),
            esc_vcard(&v.state),
            esc_vcard(&v.zip),
            esc_vcard(&v.country)
        ));
    }
    if !v.birthday.is_empty() {
        lines.push(format!("BDAY:{}", v.birthday.replace('-', "")));
    }
    if !v.note.is_empty() {
        lines.push(format!("NOTE:{}", esc_vcard(&v.note)));
    }
    lines.push("END:VCARD".into());
    Ok(lines.join("\r\n"))
}

/// Validates and normalises a URL payload.
pub fn url_payload(url: &str) -> EngineResult<String> {
    let t = url.trim();
    engine_shared::error::require_text(t, "url")?;
    if t.contains(char::is_whitespace) {
        return Err(EngineError::invalid("URL must not contain whitespace"));
    }
    let with_scheme = if t.contains("://") || t.starts_with("mailto:") || t.starts_with("tel:") {
        t.to_string()
    } else {
        format!("https://{t}")
    };
    if !with_scheme
        .split("://")
        .nth(1)
        .map(|h| h.contains('.') || h.starts_with("localhost"))
        .unwrap_or(true)
    {
        return Err(EngineError::invalid(format!("`{t}` does not look like a valid URL")));
    }
    Ok(with_scheme)
}

/// Builds a `mailto:` payload.
pub fn email_payload(to: &str, subject: &str, body: &str) -> EngineResult<String> {
    engine_shared::error::require_text(to, "to")?;
    if !to.contains('@') {
        return Err(EngineError::invalid("Invalid email address"));
    }
    let mut s = format!("mailto:{to}");
    let mut params = Vec::new();
    if !subject.is_empty() {
        params.push(format!("subject={}", pct(subject)));
    }
    if !body.is_empty() {
        params.push(format!("body={}", pct(body)));
    }
    if !params.is_empty() {
        s.push('?');
        s.push_str(&params.join("&"));
    }
    Ok(s)
}

fn pct(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Builds an `SMSTO:` payload.
pub fn sms_payload(number: &str, message: &str) -> EngineResult<String> {
    engine_shared::error::require_text(number, "number")?;
    Ok(if message.is_empty() {
        format!("SMSTO:{number}")
    } else {
        format!("SMSTO:{number}:{message}")
    })
}

/// Builds a `geo:` payload.
pub fn geo_payload(lat: f64, lon: f64, query: &str) -> EngineResult<String> {
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(EngineError::invalid("Latitude must be -90..90 and longitude -180..180"));
    }
    Ok(if query.is_empty() {
        format!("geo:{lat},{lon}")
    } else {
        format!("geo:{lat},{lon}?q={}", pct(query))
    })
}

/// Builds an iCalendar `VEVENT` payload.
pub fn event_payload(
    title: &str,
    start: &str,
    end: &str,
    location: &str,
    description: &str,
) -> EngineResult<String> {
    engine_shared::error::require_text(title, "title")?;
    engine_shared::error::require_text(start, "start")?;
    let norm = |d: &str| d.replace(['-', ':'], "");
    let mut lines = vec![
        "BEGIN:VEVENT".to_string(),
        format!("SUMMARY:{}", esc_vcard(title)),
        format!("DTSTART:{}", norm(start)),
    ];
    if !end.is_empty() {
        lines.push(format!("DTEND:{}", norm(end)));
    }
    if !location.is_empty() {
        lines.push(format!("LOCATION:{}", esc_vcard(location)));
    }
    if !description.is_empty() {
        lines.push(format!("DESCRIPTION:{}", esc_vcard(description)));
    }
    lines.push("END:VEVENT".into());
    Ok(lines.join("\r\n"))
}

/// Recognises common payload conventions.
pub fn classify_payload(s: &str) -> Value {
    let t = s.trim();
    let upper = t.to_ascii_uppercase();
    if upper.starts_with("WIFI:") {
        let mut fields = serde_json::Map::new();
        let body = &t[5..];
        let mut cur = String::new();
        let mut parts = Vec::new();
        let mut chars = body.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            } else if c == ';' {
                parts.push(std::mem::take(&mut cur));
            } else {
                cur.push(c);
            }
        }
        for p in parts {
            if let Some((k, v)) = p.split_once(':') {
                let key = match k {
                    "T" => "security",
                    "S" => "ssid",
                    "P" => "password",
                    "H" => "hidden",
                    "E" => "eap",
                    "I" => "identity",
                    other => other,
                };
                fields.insert(key.to_string(), Value::String(v.to_string()));
            }
        }
        return json!({ "type": "wifi", "fields": fields });
    }
    if upper.starts_with("BEGIN:VCARD") {
        let mut fields = serde_json::Map::new();
        for line in t.lines() {
            if let Some((k, v)) = line.split_once(':') {
                let key = k.split(';').next().unwrap_or(k).to_ascii_lowercase();
                if !matches!(key.as_str(), "begin" | "end" | "version") {
                    fields.entry(key).or_insert(Value::String(v.trim().to_string()));
                }
            }
        }
        return json!({ "type": "vcard", "fields": fields });
    }
    if upper.starts_with("MECARD:") {
        return json!({ "type": "mecard" });
    }
    if upper.starts_with("BEGIN:VEVENT") || upper.starts_with("BEGIN:VCALENDAR") {
        return json!({ "type": "event" });
    }
    if upper.starts_with("MAILTO:") {
        return json!({ "type": "email", "address": t[7..].split('?').next().unwrap_or("") });
    }
    if upper.starts_with("TEL:") {
        return json!({ "type": "phone", "number": &t[4..] });
    }
    if upper.starts_with("SMSTO:") || upper.starts_with("SMS:") {
        return json!({ "type": "sms" });
    }
    if upper.starts_with("GEO:") {
        return json!({ "type": "geo" });
    }
    if upper.starts_with("HTTP://") || upper.starts_with("HTTPS://") {
        return json!({ "type": "url", "url": t });
    }
    if upper.starts_with("UPI://") {
        return json!({ "type": "upi" });
    }
    if upper.starts_with("BITCOIN:") || upper.starts_with("ETHEREUM:") {
        return json!({ "type": "crypto" });
    }
    if upper.starts_with("OTPAUTH://") {
        return json!({ "type": "otp" });
    }
    if t.starts_with('{') && serde_json::from_str::<Value>(t).is_ok() {
        return json!({ "type": "json" });
    }
    if t.chars().all(|c| c.is_ascii_digit()) && !t.is_empty() {
        return json!({ "type": "numeric" });
    }
    json!({ "type": "text" })
}

// ------------------------------------------------------------------ decode --

/// Decodes every QR code found in an image.
pub fn decode(bytes: &[u8]) -> EngineResult<Value> {
    engine_shared::error::require_bytes(bytes, "image")?;
    engine_shared::limits::check_size(bytes.len(), engine_shared::limits::MAX_FILE_BYTES, "image")?;
    let img = image::load_from_memory(bytes)
        .map_err(|e| EngineError::unsupported_format(format!("Cannot decode image: {e}")))?;
    engine_shared::limits::check_dimensions(img.width(), img.height())?;
    let gray = img.to_luma8();
    let mut attempts: Vec<image::GrayImage> = vec![gray.clone()];
    // Downscale very large images for speed; upscale tiny ones for robustness.
    let max_side = gray.width().max(gray.height());
    if max_side > 1600 {
        let f = 1600.0 / max_side as f64;
        attempts.push(image::imageops::resize(
            &gray,
            (gray.width() as f64 * f) as u32,
            (gray.height() as f64 * f) as u32,
            image::imageops::FilterType::Triangle,
        ));
    }
    if max_side < 300 {
        attempts.push(image::imageops::resize(
            &gray,
            gray.width() * 3,
            gray.height() * 3,
            image::imageops::FilterType::Nearest,
        ));
    }
    // Inverted (light-on-dark) codes.
    let mut inverted = gray.clone();
    image::imageops::invert(&mut inverted);
    attempts.push(inverted);
    let mut results: Vec<Value> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for (ai, g) in attempts.iter().enumerate() {
        let mut prepared = rqrr::PreparedImage::prepare(g.clone());
        let grids = prepared.detect_grids();
        for grid in grids {
            match grid.decode() {
                Ok((meta, content)) => {
                    let b = grid.bounds;
                    if results.iter().any(|r| r["content"] == content) {
                        continue;
                    }
                    let ec = ["M", "L", "H", "Q"]
                        .get(meta.ecc_level as usize)
                        .copied()
                        .unwrap_or("?");
                    results.push(json!({
                        "content": content,
                        "payload": classify_payload(&content),
                        "version": meta.version.0,
                        "modules": meta.version.to_size(),
                        "error_correction": ec,
                        "mask": meta.mask,
                        "bounds": b.iter().map(|p| json!({ "x": p.x, "y": p.y })).collect::<Vec<_>>(),
                        "inverted": ai == attempts.len() - 1,
                    }));
                },
                Err(e) => failures.push(e.to_string()),
            }
        }
        if !results.is_empty() {
            break;
        }
    }
    if results.is_empty() {
        return Err(EngineError::not_found(if failures.is_empty() {
            "No QR code found in the image (try a sharper, higher-contrast photo with the full \
             quiet zone visible)"
                .to_string()
        } else {
            format!(
                "QR code located but could not be decoded ({}); the image may be blurred, skewed \
                 or damaged beyond the error-correction capacity",
                failures[0]
            )
        }));
    }
    Ok(json!({
        "count": results.len(),
        "codes": results,
        "image": { "width": img.width(), "height": img.height() }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_decode_roundtrip() {
        let style = Style { module_size: 6, ..Default::default() };
        let g =
            generate("https://example.com/path?x=1", &style, &["svg".into(), "png".into()], false)
                .unwrap();
        assert_eq!(g["payload_type"], "url");
        assert!(g["svg"].as_str().unwrap().starts_with("<svg"));
        let png = engine_shared::b64::decode(g["png"].as_str().unwrap()).unwrap();
        let d = decode(&png).unwrap();
        assert_eq!(d["codes"][0]["content"], "https://example.com/path?x=1");
        assert_eq!(d["codes"][0]["payload"]["type"], "url");
        // Styled (dots, inverted colours) still decodes.
        let styled = Style {
            module_size: 8,
            module_shape: "dots".into(),
            foreground: "#ffffff".into(),
            background: "#102030".into(),
            ..Default::default()
        };
        let g = generate("hello world", &styled, &["png".into()], false).unwrap();
        let png = engine_shared::b64::decode(g["png"].as_str().unwrap()).unwrap();
        assert_eq!(decode(&png).unwrap()["codes"][0]["content"], "hello world");
        assert!(generate("", &style, &[], false).is_err());
        assert!(generate(&"x".repeat(3000), &style, &[], false).is_err());
        assert!(decode(b"not an image").is_err());
        let blank = image::RgbaImage::from_pixel(64, 64, Rgba([255, 255, 255, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        blank.write_to(&mut buf, image::ImageFormat::Png).unwrap();
        assert_eq!(
            decode(&buf.into_inner()).unwrap_err().code,
            engine_shared::error::ErrorCode::NotFound
        );
    }

    #[test]
    fn payloads() {
        assert_eq!(
            wifi_payload("My;Net", "pass:word", "WPA2", true, None, None).unwrap(),
            "WIFI:T:WPA;S:My\\;Net;P:pass\\:word;H:true;;"
        );
        assert!(wifi_payload("x", "", "WPA", false, None, None).is_err());
        assert!(wifi_payload("x", "short", "WPA", false, None, None).is_err());
        assert_eq!(
            wifi_payload("open", "", "nopass", false, None, None).unwrap(),
            "WIFI:T:nopass;S:open;;"
        );
        let v = vcard_payload(&VCard {
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            email: "ada@example.com".into(),
            phone: "+44 1".into(),
            ..Default::default()
        })
        .unwrap();
        assert!(v.starts_with("BEGIN:VCARD\r\nVERSION:3.0\r\nN:Lovelace;Ada;;;\r\nFN:Ada Lovelace"));
        assert!(vcard_payload(&VCard::default()).is_err());
        assert_eq!(url_payload("example.com").unwrap(), "https://example.com");
        assert!(url_payload("not a url").is_err());
        assert_eq!(
            email_payload("a@b.c", "Hi there", "").unwrap(),
            "mailto:a@b.c?subject=Hi%20there"
        );
        assert_eq!(geo_payload(1.5, 2.5, "").unwrap(), "geo:1.5,2.5");
        assert!(geo_payload(100.0, 0.0, "").is_err());
        assert_eq!(classify_payload("WIFI:T:WPA;S:a\\;b;P:c;;")["fields"]["ssid"], "a;b");
        assert_eq!(classify_payload("BEGIN:VCARD\nFN:X\nEND:VCARD")["fields"]["fn"], "X");
        assert_eq!(classify_payload("12345")["type"], "numeric");
    }
}
