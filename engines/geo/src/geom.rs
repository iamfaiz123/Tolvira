//! Geodesy and planar geometry helpers. Coordinates are WGS84 lon/lat
//! degrees unless stated otherwise.

use engine_shared::{EngineError, EngineResult};

/// Mean Earth radius in metres (IUGG).
pub const EARTH_RADIUS_M: f64 = 6_371_008.8; // mean radius (IUGG)
/// WGS-84 semi-major axis in metres.
pub const WGS84_A: f64 = 6_378_137.0;
/// WGS-84 flattening.
pub const WGS84_F: f64 = 1.0 / 298.257_223_563;

/// Validates a latitude/longitude pair.
pub fn check_lat_lon(lat: f64, lon: f64) -> EngineResult<()> {
    if !lat.is_finite() || !lon.is_finite() {
        return Err(EngineError::invalid("Coordinates must be finite numbers"));
    }
    if !(-90.0..=90.0).contains(&lat) {
        return Err(EngineError::invalid(format!("Latitude {lat} is out of range (-90..90)")));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(EngineError::invalid(format!("Longitude {lon} is out of range (-180..180)")));
    }
    Ok(())
}

/// Great-circle distance in metres (haversine, mean sphere).
pub fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * a.sqrt().atan2((1.0 - a).sqrt())
}

/// Vincenty inverse on the WGS84 ellipsoid (metres). Falls back to haversine
/// for nearly antipodal points where the iteration does not converge.
pub fn vincenty(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> (f64, bool) {
    let b = WGS84_A * (1.0 - WGS84_F);
    let (u1, u2) = (
        ((1.0 - WGS84_F) * lat1.to_radians().tan()).atan(),
        ((1.0 - WGS84_F) * lat2.to_radians().tan()).atan(),
    );
    let l = (lon2 - lon1).to_radians();
    let (su1, cu1, su2, cu2) = (u1.sin(), u1.cos(), u2.sin(), u2.cos());
    let mut lambda = l;
    let (mut sin_sigma, mut cos_sigma, mut sigma, mut cos_sq_alpha, mut cos2sm) =
        (0.0, 0.0, 0.0, 0.0, 0.0);
    for _ in 0..200 {
        let (sl, cl) = lambda.sin_cos();
        sin_sigma = ((cu2 * sl).powi(2) + (cu1 * su2 - su1 * cu2 * cl).powi(2)).sqrt();
        if sin_sigma == 0.0 {
            return (0.0, true);
        }
        cos_sigma = su1 * su2 + cu1 * cu2 * cl;
        sigma = sin_sigma.atan2(cos_sigma);
        let sin_alpha = cu1 * cu2 * sl / sin_sigma;
        cos_sq_alpha = 1.0 - sin_alpha * sin_alpha;
        cos2sm = if cos_sq_alpha == 0.0 {
            0.0
        } else {
            cos_sigma - 2.0 * su1 * su2 / cos_sq_alpha
        };
        let c = WGS84_F / 16.0 * cos_sq_alpha * (4.0 + WGS84_F * (4.0 - 3.0 * cos_sq_alpha));
        let new_lambda = l
            + (1.0 - c)
                * WGS84_F
                * sin_alpha
                * (sigma
                    + c * sin_sigma * (cos2sm + c * cos_sigma * (-1.0 + 2.0 * cos2sm * cos2sm)));
        if (new_lambda - lambda).abs() < 1e-12 {
            let u_sq = cos_sq_alpha * (WGS84_A * WGS84_A - b * b) / (b * b);
            let aa =
                1.0 + u_sq / 16384.0 * (4096.0 + u_sq * (-768.0 + u_sq * (320.0 - 175.0 * u_sq)));
            let bb = u_sq / 1024.0 * (256.0 + u_sq * (-128.0 + u_sq * (74.0 - 47.0 * u_sq)));
            let delta = bb
                * sin_sigma
                * (cos2sm
                    + bb / 4.0
                        * (cos_sigma * (-1.0 + 2.0 * cos2sm * cos2sm)
                            - bb / 6.0
                                * cos2sm
                                * (-3.0 + 4.0 * sin_sigma * sin_sigma)
                                * (-3.0 + 4.0 * cos2sm * cos2sm)));
            return (b * aa * (sigma - delta), true);
        }
        lambda = new_lambda;
    }
    let _ = (sin_sigma, cos_sigma, sigma, cos_sq_alpha, cos2sm);
    (haversine(lat1, lon1, lat2, lon2), false)
}

/// Initial bearing in degrees (0..360).
pub fn bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    y.atan2(x).to_degrees().rem_euclid(360.0)
}

/// 16-point compass name for a bearing.
pub fn compass(bearing: f64) -> &'static str {
    const DIRS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    DIRS[((bearing / 22.5).round() as usize) % 16]
}

/// Destination point given start, bearing (deg) and distance (m).
pub fn destination(lat: f64, lon: f64, bearing_deg: f64, distance_m: f64) -> (f64, f64) {
    let d = distance_m / EARTH_RADIUS_M;
    let b = bearing_deg.to_radians();
    let p1 = lat.to_radians();
    let l1 = lon.to_radians();
    let p2 = (p1.sin() * d.cos() + p1.cos() * d.sin() * b.cos()).asin();
    let l2 = l1 + (b.sin() * d.sin() * p1.cos()).atan2(d.cos() - p1.sin() * p2.sin());
    (p2.to_degrees(), ((l2.to_degrees() + 540.0) % 360.0) - 180.0)
}

/// Geographic midpoint of two points.
pub fn midpoint(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> (f64, f64) {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let bx = p2.cos() * dl.cos();
    let by = p2.cos() * dl.sin();
    let lat = (p1.sin() + p2.sin()).atan2(((p1.cos() + bx).powi(2) + by * by).sqrt());
    let lon = lon1.to_radians() + by.atan2(p1.cos() + bx);
    (lat.to_degrees(), ((lon.to_degrees() + 540.0) % 360.0) - 180.0)
}

/// Geodesic polygon area (m²) on the sphere via the spherical excess formula
/// (Chamberlain & Duquette). Ring is a list of (lon, lat).
pub fn spherical_area(ring: &[(f64, f64)]) -> f64 {
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..n {
        let (lon1, lat1) = ring[i];
        let (lon2, lat2) = ring[(i + 1) % n];
        sum +=
            (lon2 - lon1).to_radians() * (2.0 + lat1.to_radians().sin() + lat2.to_radians().sin());
    }
    (sum * EARTH_RADIUS_M * EARTH_RADIUS_M / 2.0).abs()
}

/// Perimeter (m) of a ring using haversine segments.
pub fn ring_length(ring: &[(f64, f64)], closed: bool) -> f64 {
    let mut total = 0.0;
    for w in ring.windows(2) {
        total += haversine(w[0].1, w[0].0, w[1].1, w[1].0);
    }
    if closed && ring.len() > 2 && ring.first() != ring.last() {
        total += haversine(ring[ring.len() - 1].1, ring[ring.len() - 1].0, ring[0].1, ring[0].0);
    }
    total
}

/// Planar centroid of a ring (shoelace) in lon/lat; falls back to the mean for degenerate rings.
pub fn centroid(ring: &[(f64, f64)]) -> (f64, f64) {
    let n = ring.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mut a = 0.0;
    let (mut cx, mut cy) = (0.0, 0.0);
    for i in 0..n {
        let (x0, y0) = ring[i];
        let (x1, y1) = ring[(i + 1) % n];
        let cross = x0 * y1 - x1 * y0;
        a += cross;
        cx += (x0 + x1) * cross;
        cy += (y0 + y1) * cross;
    }
    if a.abs() < 1e-12 {
        let (sx, sy): (f64, f64) =
            ring.iter().fold((0.0, 0.0), |acc, p| (acc.0 + p.0, acc.1 + p.1));
        return (sx / n as f64, sy / n as f64);
    }
    a *= 0.5;
    (cx / (6.0 * a), cy / (6.0 * a))
}

/// Ramer–Douglas–Peucker simplification with tolerance in degrees.
pub fn simplify(points: &[(f64, f64)], tolerance: f64) -> Vec<(f64, f64)> {
    if points.len() <= 2 || tolerance <= 0.0 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((s, e)) = stack.pop() {
        if e <= s + 1 {
            continue;
        }
        let (mut max_d, mut idx) = (0.0, s);
        for i in s + 1..e {
            let d = perpendicular_distance(points[i], points[s], points[e]);
            if d > max_d {
                max_d = d;
                idx = i;
            }
        }
        if max_d > tolerance {
            keep[idx] = true;
            stack.push((s, idx));
            stack.push((idx, e));
        }
    }
    points.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

fn perpendicular_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx == 0.0 && dy == 0.0 {
        return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt();
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    let (px, py) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - px).powi(2) + (p.1 - py).powi(2)).sqrt()
}

// ------------------------------------------------------------------ UTM --

/// UTM coordinates.
pub struct Utm {
    /// Zone number (1–60).
    pub zone: u8,
    /// Latitude band letter (C–X).
    pub band: char,
    /// `N` or `S`.
    pub hemisphere: char,
    /// Easting in metres.
    pub easting: f64,
    /// Northing in metres.
    pub northing: f64,
}

/// UTM latitude band letter.
pub fn lat_band(lat: f64) -> char {
    const BANDS: &[u8] = b"CDEFGHJKLMNPQRSTUVWX";
    let idx = ((lat + 80.0) / 8.0).floor().clamp(0.0, 19.0) as usize;
    BANDS[idx] as char
}

/// Converts latitude/longitude to UTM (WGS-84).
pub fn to_utm(lat: f64, lon: f64) -> EngineResult<Utm> {
    check_lat_lon(lat, lon)?;
    if !(-80.0..=84.0).contains(&lat) {
        return Err(EngineError::invalid(
            "UTM is defined between 80°S and 84°N (use UPS for the poles)",
        ));
    }
    let mut zone = ((lon + 180.0) / 6.0).floor() as u8 + 1;
    // Norway / Svalbard exceptions.
    if (56.0..64.0).contains(&lat) && (3.0..12.0).contains(&lon) {
        zone = 32;
    }
    if (72.0..84.0).contains(&lat) {
        zone = match lon {
            l if (0.0..9.0).contains(&l) => 31,
            l if (9.0..21.0).contains(&l) => 33,
            l if (21.0..33.0).contains(&l) => 35,
            l if (33.0..42.0).contains(&l) => 37,
            _ => zone,
        };
    }
    let lon0 = ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0).to_radians();
    let (e, n) = transverse_mercator(lat.to_radians(), lon.to_radians(), lon0, lat < 0.0);
    Ok(Utm {
        zone,
        band: lat_band(lat),
        hemisphere: if lat < 0.0 { 'S' } else { 'N' },
        easting: e,
        northing: n,
    })
}

fn transverse_mercator(phi: f64, lambda: f64, lambda0: f64, south: bool) -> (f64, f64) {
    // Karney-style series (accurate to ~mm).
    let k0 = 0.9996;
    let f = WGS84_F;
    let n = f / (2.0 - f);
    let aa = WGS84_A / (1.0 + n) * (1.0 + n * n / 4.0 + n.powi(4) / 64.0);
    let alpha = [
        n / 2.0 - 2.0 * n * n / 3.0 + 5.0 * n.powi(3) / 16.0,
        13.0 * n * n / 48.0 - 3.0 * n.powi(3) / 5.0,
        61.0 * n.powi(3) / 240.0,
    ];
    let t = (phi.sin().atanh()
        - 2.0 * n.sqrt() / (1.0 + n) * (2.0 * n.sqrt() / (1.0 + n) * phi.sin()).atanh())
    .sinh();
    let xi_p = (t / (lambda - lambda0).cos()).atan();
    let eta_p = ((lambda - lambda0).sin() / (1.0 + t * t).sqrt()).atanh();
    let mut xi = xi_p;
    let mut eta = eta_p;
    for (j, a) in alpha.iter().enumerate() {
        let k = 2.0 * (j as f64 + 1.0);
        xi += a * (k * xi_p).sin() * (k * eta_p).cosh();
        eta += a * (k * xi_p).cos() * (k * eta_p).sinh();
    }
    let e = 500_000.0 + k0 * aa * eta;
    let n_ = k0 * aa * xi + if south { 10_000_000.0 } else { 0.0 };
    (e, n_)
}

/// Converts UTM to latitude/longitude (WGS-84).
pub fn from_utm(zone: u8, south: bool, easting: f64, northing: f64) -> EngineResult<(f64, f64)> {
    if !(1..=60).contains(&zone) {
        return Err(EngineError::invalid("UTM zone must be 1-60"));
    }
    if !(100_000.0..=900_000.0).contains(&easting) || !(0.0..=10_000_000.0).contains(&northing) {
        return Err(EngineError::invalid("Easting must be 100000-900000 and northing 0-10000000"));
    }
    let k0 = 0.9996;
    let f = WGS84_F;
    let n = f / (2.0 - f);
    let aa = WGS84_A / (1.0 + n) * (1.0 + n * n / 4.0 + n.powi(4) / 64.0);
    let beta = [
        n / 2.0 - 2.0 * n * n / 3.0 + 37.0 * n.powi(3) / 96.0,
        n * n / 48.0 + n.powi(3) / 15.0,
        17.0 * n.powi(3) / 480.0,
    ];
    let delta = [
        2.0 * n - 2.0 * n * n / 3.0 - 2.0 * n.powi(3),
        7.0 * n * n / 3.0 - 8.0 * n.powi(3) / 5.0,
        56.0 * n.powi(3) / 15.0,
    ];
    let xi = (northing - if south { 10_000_000.0 } else { 0.0 }) / (k0 * aa);
    let eta = (easting - 500_000.0) / (k0 * aa);
    let (mut xi_p, mut eta_p) = (xi, eta);
    for (j, b) in beta.iter().enumerate() {
        let k = 2.0 * (j as f64 + 1.0);
        xi_p -= b * (k * xi).sin() * (k * eta).cosh();
        eta_p -= b * (k * xi).cos() * (k * eta).sinh();
    }
    let chi = (xi_p.sin() / eta_p.cosh()).asin();
    let mut phi = chi;
    for (j, d) in delta.iter().enumerate() {
        let k = 2.0 * (j as f64 + 1.0);
        phi += d * (k * chi).sin();
    }
    let lambda0 = (zone as f64 - 1.0) * 6.0 - 180.0 + 3.0;
    let lambda = lambda0 + (eta_p.sinh() / xi_p.cos()).atan().to_degrees();
    Ok((phi.to_degrees(), lambda))
}

// -------------------------------------------------------------- geohash --

const GH32: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// Encodes a position as a geohash of `precision` characters.
pub fn geohash_encode(lat: f64, lon: f64, precision: usize) -> EngineResult<String> {
    check_lat_lon(lat, lon)?;
    let precision = precision.clamp(1, 12);
    let (mut lat_r, mut lon_r) = ((-90.0f64, 90.0f64), (-180.0f64, 180.0f64));
    let mut out = String::new();
    let mut bits = 0u8;
    let mut bit = 0;
    let mut even = true;
    while out.len() < precision {
        if even {
            let mid = (lon_r.0 + lon_r.1) / 2.0;
            if lon >= mid {
                bits = (bits << 1) | 1;
                lon_r.0 = mid;
            } else {
                bits <<= 1;
                lon_r.1 = mid;
            }
        } else {
            let mid = (lat_r.0 + lat_r.1) / 2.0;
            if lat >= mid {
                bits = (bits << 1) | 1;
                lat_r.0 = mid;
            } else {
                bits <<= 1;
                lat_r.1 = mid;
            }
        }
        even = !even;
        bit += 1;
        if bit == 5 {
            out.push(GH32[bits as usize] as char);
            bits = 0;
            bit = 0;
        }
    }
    Ok(out)
}

/// Returns (lat, lon, lat_error, lon_error).
pub fn geohash_decode(hash: &str) -> EngineResult<(f64, f64, f64, f64)> {
    let h = hash.trim().to_ascii_lowercase();
    if h.is_empty() || h.len() > 12 {
        return Err(EngineError::invalid("Geohash must be 1-12 characters"));
    }
    let (mut lat_r, mut lon_r) = ((-90.0f64, 90.0f64), (-180.0f64, 180.0f64));
    let mut even = true;
    for c in h.bytes() {
        let idx = GH32.iter().position(|x| *x == c).ok_or_else(|| {
            EngineError::invalid(format!("Invalid geohash character `{}`", c as char))
        })?;
        for i in (0..5).rev() {
            let bit = (idx >> i) & 1;
            if even {
                let mid = (lon_r.0 + lon_r.1) / 2.0;
                if bit == 1 {
                    lon_r.0 = mid
                } else {
                    lon_r.1 = mid
                }
            } else {
                let mid = (lat_r.0 + lat_r.1) / 2.0;
                if bit == 1 {
                    lat_r.0 = mid
                } else {
                    lat_r.1 = mid
                }
            }
            even = !even;
        }
    }
    Ok((
        (lat_r.0 + lat_r.1) / 2.0,
        (lon_r.0 + lon_r.1) / 2.0,
        (lat_r.1 - lat_r.0) / 2.0,
        (lon_r.1 - lon_r.0) / 2.0,
    ))
}

/// Eight neighbouring geohashes as `(direction, hash)`.
pub fn geohash_neighbors(hash: &str) -> EngineResult<Vec<(String, String)>> {
    let (lat, lon, lat_err, lon_err) = geohash_decode(hash)?;
    let p = hash.trim().len();
    let mut out = Vec::new();
    for (name, dlat, dlon) in [
        ("n", 1.0, 0.0),
        ("ne", 1.0, 1.0),
        ("e", 0.0, 1.0),
        ("se", -1.0, 1.0),
        ("s", -1.0, 0.0),
        ("sw", -1.0, -1.0),
        ("w", 0.0, -1.0),
        ("nw", 1.0, -1.0),
    ] {
        let nlat = (lat + dlat * lat_err * 2.0).clamp(-90.0, 90.0);
        let mut nlon = lon + dlon * lon_err * 2.0;
        if nlon > 180.0 {
            nlon -= 360.0;
        }
        if nlon < -180.0 {
            nlon += 360.0;
        }
        out.push((name.to_string(), geohash_encode(nlat, nlon, p)?));
    }
    Ok(out)
}

// ------------------------------------------------------------------ DMS --

/// Formats a coordinate in degrees/minutes/seconds.
pub fn to_dms(value: f64, is_lat: bool) -> String {
    let hemi = if is_lat {
        if value >= 0.0 {
            'N'
        } else {
            'S'
        }
    } else if value >= 0.0 {
        'E'
    } else {
        'W'
    };
    let v = value.abs();
    let d = v.floor();
    let m_full = (v - d) * 60.0;
    let m = m_full.floor();
    let s = (m_full - m) * 60.0;
    format!("{}°{:02}'{:05.2}\"{}", d as i64, m as i64, s, hemi)
}

/// Parses many coordinate notations: `40.7128, -74.0060`, `40°42'46"N 74°0'22"W`,
/// `N40 42.767 W74 0.360`, `40.7128N 74.0060W`.
pub fn parse_coordinate(input: &str) -> EngineResult<(f64, f64)> {
    let t = input
        .trim()
        .replace(['\u{2019}', '\u{2032}'], "'")
        .replace(['\u{201d}', '\u{2033}'], "\"")
        .replace("''", "\"");
    engine_shared::error::require_text(&t, "coordinate")?;
    // Simple decimal pair.
    let simple: Vec<&str> = t
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    if simple.len() == 2 {
        if let (Ok(a), Ok(b)) = (simple[0].parse::<f64>(), simple[1].parse::<f64>()) {
            check_lat_lon(a, b)?;
            return Ok((a, b));
        }
    }
    // Tokenise into two components by hemisphere letters.
    let upper = t.to_ascii_uppercase();
    let mut comps: Vec<(f64, Option<char>)> = Vec::new();
    let mut cur = String::new();
    let mut cur_hemi: Option<char> = None;
    let flush = |cur: &mut String,
                 hemi: &mut Option<char>,
                 comps: &mut Vec<(f64, Option<char>)>|
     -> EngineResult<()> {
        let s = cur.trim().to_string();
        if s.is_empty() {
            return Ok(());
        }
        let v = dms_to_decimal(&s)?;
        comps.push((v, hemi.take()));
        cur.clear();
        Ok(())
    };
    for c in upper.chars() {
        match c {
            'N' | 'S' | 'E' | 'W' => {
                if cur.trim().is_empty() {
                    // Prefix form: `N40 42.767`
                    cur_hemi = Some(c);
                } else if cur_hemi.is_some() {
                    // Previous component used the prefix form; this letter starts the next one.
                    flush(&mut cur, &mut cur_hemi, &mut comps)?;
                    cur_hemi = Some(c);
                } else {
                    // Suffix form: `40°42'46"N`
                    cur_hemi = Some(c);
                    flush(&mut cur, &mut cur_hemi, &mut comps)?;
                }
            },
            ',' | ';' => flush(&mut cur, &mut cur_hemi, &mut comps)?,
            _ => cur.push(c),
        }
    }
    flush(&mut cur, &mut cur_hemi, &mut comps)?;
    if comps.len() != 2 {
        return Err(EngineError::invalid(format!(
            "Cannot parse `{input}` as a coordinate pair (try `lat, lon` or `40°42'46\"N \
             74°0'22\"W`)"
        )));
    }
    let mut lat = None;
    let mut lon = None;
    for (v, h) in &comps {
        match h {
            Some('N') => lat = Some(*v),
            Some('S') => lat = Some(-v),
            Some('E') => lon = Some(*v),
            Some('W') => lon = Some(-v),
            _ => {},
        }
    }
    let (lat, lon) = match (lat, lon) {
        (Some(a), Some(b)) => (a, b),
        (None, None) => (comps[0].0, comps[1].0),
        (Some(a), None) => (a, comps.iter().find(|c| c.1.is_none()).map(|c| c.0).unwrap_or(0.0)),
        (None, Some(b)) => (comps.iter().find(|c| c.1.is_none()).map(|c| c.0).unwrap_or(0.0), b),
    };
    check_lat_lon(lat, lon)?;
    Ok((lat, lon))
}

fn dms_to_decimal(s: &str) -> EngineResult<f64> {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c == '°' || c == '\'' || c == '"' || c == 'D' || c == 'M' || c == 'S' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let parts: Vec<f64> = cleaned
        .split_whitespace()
        .map(|p| {
            p.parse::<f64>()
                .map_err(|_| EngineError::invalid(format!("`{p}` is not a number in `{s}`")))
        })
        .collect::<EngineResult<_>>()?;
    if parts.is_empty() || parts.len() > 3 {
        return Err(EngineError::invalid(format!("Cannot parse `{s}`")));
    }
    let sign = if parts[0] < 0.0 { -1.0 } else { 1.0 };
    let d = parts[0].abs();
    let m = parts.get(1).copied().unwrap_or(0.0);
    let sec = parts.get(2).copied().unwrap_or(0.0);
    if m < 0.0 || m >= 60.0 || sec < 0.0 || sec >= 60.0 {
        return Err(EngineError::invalid("Minutes and seconds must be 0-59.99"));
    }
    Ok(sign * (d + m / 60.0 + sec / 3600.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_and_bearings() {
        // London → Paris ≈ 343.5 km
        let d = haversine(51.5074, -0.1278, 48.8566, 2.3522);
        assert!((d / 1000.0 - 343.5).abs() < 1.0, "{d}");
        let (v, ok) = vincenty(51.5074, -0.1278, 48.8566, 2.3522);
        assert!(ok && (v / 1000.0 - 343.9).abs() < 1.0, "{v}");
        let b = bearing(51.5074, -0.1278, 48.8566, 2.3522);
        assert!((b - 148.1).abs() < 1.0, "{b}");
        assert_eq!(compass(b), "SSE");
        let (lat, lon) = destination(51.5074, -0.1278, 90.0, 100_000.0);
        assert!((lat - 51.5).abs() < 0.02 && lon > 1.3);
        let ring = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let area = spherical_area(&ring);
        assert!((area / 1e9 - 12.36).abs() < 0.2, "{area}");
        let len = ring_length(&ring, true);
        assert!((len / 1000.0 - 444.0).abs() < 3.0, "{len}");
        let c = centroid(&ring);
        assert!((c.0 - 0.5).abs() < 1e-9 && (c.1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn utm_geohash_dms() {
        let u = to_utm(40.7128, -74.0060).unwrap();
        assert_eq!(u.zone, 18);
        assert_eq!(u.band, 'T');
        assert!((u.easting - 583_959.4).abs() < 2.0, "{}", u.easting);
        assert!((u.northing - 4_507_351.0).abs() < 2.0, "{}", u.northing);
        let (lat, lon) = from_utm(18, false, u.easting, u.northing).unwrap();
        assert!((lat - 40.7128).abs() < 1e-5 && (lon + 74.0060).abs() < 1e-5);
        assert_eq!(to_utm(60.0, 5.0).unwrap().zone, 32);
        assert!(to_utm(85.0, 0.0).is_err());
        let gh = geohash_encode(57.64911, 10.40744, 11).unwrap();
        assert_eq!(gh, "u4pruydqqvj");
        let (la, lo, _, _) = geohash_decode("u4pruydqqvj").unwrap();
        assert!((la - 57.64911).abs() < 1e-4 && (lo - 10.40744).abs() < 1e-4);
        assert_eq!(geohash_neighbors("u4pruyd").unwrap().len(), 8);
        assert!(geohash_decode("a").is_err());
        assert_eq!(to_dms(40.7128, true), "40°42'46.08\"N");
        assert_eq!(
            parse_coordinate("40°42'46\"N 74°0'22\"W")
                .map(|(a, b)| ((a * 1000.0).round() / 1000.0, (b * 1000.0).round() / 1000.0))
                .unwrap(),
            (40.713, -74.006)
        );
        assert_eq!(parse_coordinate("40.7128, -74.0060").unwrap(), (40.7128, -74.0060));
        assert_eq!(
            parse_coordinate("N40 42.767 W74 0.360")
                .map(|(a, b)| ((a * 1000.0).round() / 1000.0, (b * 1000.0).round() / 1000.0))
                .unwrap(),
            (40.713, -74.006)
        );
        assert!(parse_coordinate("hello").is_err());
        assert!(parse_coordinate("95, 10").is_err());
        let pts: Vec<(f64, f64)> = (0..100)
            .map(|i| (i as f64 * 0.01, ((i as f64) * 0.3).sin() * 0.001))
            .collect();
        let s = simplify(&pts, 0.0005);
        assert!(s.len() < 40 && s.len() >= 2, "{}", s.len());
    }
}
