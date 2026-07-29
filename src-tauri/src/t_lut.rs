/**
 * 3D LUT (.cube) parse/apply + photo-style grading helpers.
 * Inspired by PhotonCamera CubeLutParser / ColorRecipe layering,
 * implemented in pure Rust for PicAiPic host tools.
 */
use image::{DynamicImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::t_config;

/// Parsed Adobe/Resolve-style 3D LUT.
#[derive(Debug, Clone)]
pub struct CubeLut {
    pub size: usize,
    /// Interleaved RGB, length size^3 * 3, values in 0..1.
    /// Index order: blue slowest, green, red fastest (same as .cube file order).
    pub data: Vec<f32>,
    #[allow(dead_code)]
    pub title: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LutLibraryEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub category: String,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PhotoStyleParams {
    /// -100..100 (CSS-like brightness offset)
    #[serde(default)]
    pub brightness: i32,
    /// -100..100
    #[serde(default)]
    pub contrast: f32,
    /// percent, 100 = neutral
    #[serde(default = "default_sat")]
    pub saturation: f32,
    /// degrees
    #[serde(default)]
    pub hue: i32,
    /// -100..100 highlight recovery/boost (simple)
    #[serde(default)]
    pub highlights: f32,
    /// -100..100 shadow lift/crush
    #[serde(default)]
    pub shadows: f32,
    /// 0..100 fade to gray mid
    #[serde(default)]
    pub fade: f32,
    /// 0..100 vignette strength
    #[serde(default)]
    pub vignette: f32,
    /// 0..100 film grain
    #[serde(default)]
    pub grain: f32,
    /// optional CSS-like filter tag
    #[serde(default)]
    pub filter: Option<String>,
    /// optional library lut id
    #[serde(default)]
    #[serde(rename = "lutId")]
    pub lut_id: Option<String>,
    /// optional direct cube path (import/preview)
    #[serde(default)]
    #[serde(rename = "lutPath")]
    pub lut_path: Option<String>,
    /// 0..100 LUT mix
    #[serde(default = "default_lut_intensity")]
    #[serde(rename = "lutIntensity")]
    pub lut_intensity: f32,
}

fn default_sat() -> f32 {
    100.0
}
fn default_lut_intensity() -> f32 {
    100.0
}

// --- library paths ---

pub fn lut_library_dir() -> Result<PathBuf, String> {
    let dir = t_config::get_app_data_dir()?.join("luts");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create lut dir: {}", e))?;
    Ok(dir)
}

fn lut_index_path() -> Result<PathBuf, String> {
    Ok(lut_library_dir()?.join("index.json"))
}

pub fn load_lut_index() -> Result<Vec<LutLibraryEntry>, String> {
    let path = lut_index_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read lut index: {}", e))?;
    let list: Vec<LutLibraryEntry> =
        serde_json::from_str(&text).map_err(|e| format!("parse lut index: {}", e))?;
    Ok(list)
}

fn save_lut_index(list: &[LutLibraryEntry]) -> Result<(), String> {
    let path = lut_index_path()?;
    let text = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("write lut index: {}", e))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn sanitize_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "lut".to_string()
    } else {
        s.chars().take(48).collect()
    }
}

/// Import a .cube file into the app LUT library. Returns the new entry.
pub fn import_lut_file(
    source_path: &str,
    display_name: Option<String>,
) -> Result<LutLibraryEntry, String> {
    let src = Path::new(source_path);
    if !src.is_file() {
        return Err("LUT source file not found".to_string());
    }
    // Validate cube
    let cube = parse_cube_file(src)?;
    let _ = cube;

    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("lut");
    let name = display_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| stem.to_string());

    let id = format!(
        "lut_{}_{}",
        now_secs(),
        &uuid::Uuid::new_v4().to_string()[..8]
    );
    let file_name = format!(
        "{}_{}.cube",
        sanitize_stem(&name),
        &id[id.len().saturating_sub(8)..]
    );
    let dest = lut_library_dir()?.join(&file_name);
    std::fs::copy(src, &dest).map_err(|e| format!("copy lut: {}", e))?;

    let entry = LutLibraryEntry {
        id,
        name,
        file_name,
        favorite: false,
        category: String::new(),
        created_at: now_secs(),
    };
    let mut list = load_lut_index()?;
    list.insert(0, entry.clone());
    save_lut_index(&list)?;
    Ok(entry)
}

pub fn delete_lut_entry(lut_id: &str) -> Result<bool, String> {
    let mut list = load_lut_index()?;
    let Some(pos) = list.iter().position(|e| e.id == lut_id) else {
        return Ok(false);
    };
    let entry = list.remove(pos);
    let path = lut_library_dir()?.join(&entry.file_name);
    let _ = std::fs::remove_file(path);
    save_lut_index(&list)?;
    Ok(true)
}

pub fn update_lut_entry(
    lut_id: &str,
    name: Option<String>,
    favorite: Option<bool>,
    category: Option<String>,
) -> Result<LutLibraryEntry, String> {
    let mut list = load_lut_index()?;
    let entry = list
        .iter_mut()
        .find(|e| e.id == lut_id)
        .ok_or_else(|| "LUT not found".to_string())?;
    if let Some(n) = name {
        let n = n.trim();
        if !n.is_empty() {
            entry.name = n.to_string();
        }
    }
    if let Some(f) = favorite {
        entry.favorite = f;
    }
    if let Some(c) = category {
        entry.category = c.trim().to_string();
    }
    let out = entry.clone();
    save_lut_index(&list)?;
    Ok(out)
}

pub fn resolve_lut_path(lut_id: &str) -> Result<PathBuf, String> {
    let list = load_lut_index()?;
    let entry = list
        .iter()
        .find(|e| e.id == lut_id)
        .ok_or_else(|| "LUT not found".to_string())?;
    let path = lut_library_dir()?.join(&entry.file_name);
    if !path.is_file() {
        return Err("LUT file missing on disk".to_string());
    }
    Ok(path)
}

// --- cube parse / sample ---

pub fn parse_cube_file(path: &Path) -> Result<CubeLut, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read cube: {}", e))?;
    parse_cube_text(&text)
}

pub fn parse_cube_text(text: &str) -> Result<CubeLut, String> {
    let mut title = String::new();
    let mut size: usize = 0;
    let mut domain_min = [0.0f32; 3];
    let mut domain_max = [1.0f32; 3];
    let mut data: Vec<f32> = Vec::new();
    let mut temp: Vec<[f32; 3]> = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("TITLE") {
            title = extract_title(line);
            continue;
        }
        if line.starts_with("LUT_3D_SIZE") {
            size = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if size > 0 {
                data = Vec::with_capacity(size * size * size * 3);
                for v in temp.drain(..) {
                    push_norm(&mut data, v, domain_min, domain_max);
                }
            }
            continue;
        }
        if line.starts_with("DOMAIN_MIN") {
            if let Some(v) = parse_triple(line.trim_start_matches("DOMAIN_MIN")) {
                domain_min = v;
            }
            continue;
        }
        if line.starts_with("DOMAIN_MAX") {
            if let Some(v) = parse_triple(line.trim_start_matches("DOMAIN_MAX")) {
                domain_max = v;
            }
            continue;
        }
        if let Some(v) = parse_triple(line) {
            if size > 0 {
                push_norm(&mut data, v, domain_min, domain_max);
            } else {
                temp.push(v);
            }
        }
    }

    if size < 2 || size > 128 {
        return Err("Invalid LUT_3D_SIZE".to_string());
    }
    let expected = size * size * size * 3;
    if data.len() < expected {
        return Err(format!(
            "cube data short: got {} expected {}",
            data.len(),
            expected
        ));
    }
    if data.len() > expected {
        data.truncate(expected);
    }
    Ok(CubeLut { size, data, title })
}

fn extract_title(line: &str) -> String {
    if let Some(start) = line.find('"') {
        if let Some(end) = line.rfind('"') {
            if end > start {
                return line[start + 1..end].to_string();
            }
        }
    }
    line.split_whitespace()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_triple(s: &str) -> Option<[f32; 3]> {
    let mut parts = s.split_whitespace();
    let a = parts.next()?.parse().ok()?;
    let b = parts.next()?.parse().ok()?;
    let c = parts.next()?.parse().ok()?;
    Some([a, b, c])
}

fn push_norm(data: &mut Vec<f32>, v: [f32; 3], min: [f32; 3], max: [f32; 3]) {
    for i in 0..3 {
        let n = if (max[i] - min[i]).abs() < 1e-8 {
            v[i]
        } else {
            (v[i] - min[i]) / (max[i] - min[i])
        };
        data.push(n.clamp(0.0, 1.0));
    }
}

fn sample_lut(lut: &CubeLut, r: f32, g: f32, b: f32) -> [f32; 3] {
    let n = (lut.size - 1) as f32;
    let rf = r.clamp(0.0, 1.0) * n;
    let gf = g.clamp(0.0, 1.0) * n;
    let bf = b.clamp(0.0, 1.0) * n;

    let r0 = rf.floor() as usize;
    let g0 = gf.floor() as usize;
    let b0 = bf.floor() as usize;
    let r1 = (r0 + 1).min(lut.size - 1);
    let g1 = (g0 + 1).min(lut.size - 1);
    let b1 = (b0 + 1).min(lut.size - 1);
    let rd = rf - r0 as f32;
    let gd = gf - g0 as f32;
    let bd = bf - b0 as f32;

    let c000 = lut_at(lut, r0, g0, b0);
    let c100 = lut_at(lut, r1, g0, b0);
    let c010 = lut_at(lut, r0, g1, b0);
    let c110 = lut_at(lut, r1, g1, b0);
    let c001 = lut_at(lut, r0, g0, b1);
    let c101 = lut_at(lut, r1, g0, b1);
    let c011 = lut_at(lut, r0, g1, b1);
    let c111 = lut_at(lut, r1, g1, b1);

    let mut out = [0.0f32; 3];
    for i in 0..3 {
        let c00 = c000[i] * (1.0 - rd) + c100[i] * rd;
        let c10 = c010[i] * (1.0 - rd) + c110[i] * rd;
        let c01 = c001[i] * (1.0 - rd) + c101[i] * rd;
        let c11 = c011[i] * (1.0 - rd) + c111[i] * rd;
        let c0 = c00 * (1.0 - gd) + c10 * gd;
        let c1 = c01 * (1.0 - gd) + c11 * gd;
        out[i] = (c0 * (1.0 - bd) + c1 * bd).clamp(0.0, 1.0);
    }
    out
}

fn lut_at(lut: &CubeLut, r: usize, g: usize, b: usize) -> [f32; 3] {
    // cube order: for b for g for r
    let idx = ((b * lut.size + g) * lut.size + r) * 3;
    [lut.data[idx], lut.data[idx + 1], lut.data[idx + 2]]
}

pub fn apply_cube_lut(img: &DynamicImage, lut: &CubeLut, intensity: f32) -> DynamicImage {
    let intensity = (intensity / 100.0).clamp(0.0, 1.0);
    if intensity <= 0.0 {
        return img.clone();
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbaImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let r = p[0] as f32 / 255.0;
        let g = p[1] as f32 / 255.0;
        let b = p[2] as f32 / 255.0;
        let mapped = sample_lut(lut, r, g, b);
        let rr = r * (1.0 - intensity) + mapped[0] * intensity;
        let gg = g * (1.0 - intensity) + mapped[1] * intensity;
        let bb = b * (1.0 - intensity) + mapped[2] * intensity;
        out.put_pixel(
            x,
            y,
            Rgba([
                (rr * 255.0).round().clamp(0.0, 255.0) as u8,
                (gg * 255.0).round().clamp(0.0, 255.0) as u8,
                (bb * 255.0).round().clamp(0.0, 255.0) as u8,
                p[3],
            ]),
        );
    }
    DynamicImage::ImageRgba8(out)
}

// --- photo style effects ---

pub fn apply_photo_style(img: DynamicImage, style: &PhotoStyleParams) -> DynamicImage {
    let mut img = img;

    // 1) basic adjustments (reuse CSS-like semantics for brightness/contrast/sat/hue)
    // highlights/shadows applied as a simple luma remap after base adjust.
    img = apply_basic_adjust(
        img,
        style.brightness,
        style.contrast,
        style.saturation,
        style.hue,
        style.filter.as_deref(),
    );

    img = apply_highlights_shadows(img, style.highlights, style.shadows);

    // 2) LUT
    if style.lut_intensity > 0.0 {
        let lut_path = if let Some(path) = style
            .lut_path
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            Some(PathBuf::from(path))
        } else if let Some(id) = style
            .lut_id
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            resolve_lut_path(id).ok()
        } else {
            None
        };
        if let Some(path) = lut_path {
            if let Ok(cube) = parse_cube_file(&path) {
                img = apply_cube_lut(&img, &cube, style.lut_intensity);
            }
        }
    }

    // 3) effects
    if style.fade > 0.0 {
        img = apply_fade(img, style.fade);
    }
    if style.vignette > 0.0 {
        img = apply_vignette(img, style.vignette);
    }
    if style.grain > 0.0 {
        img = apply_grain(img, style.grain);
    }

    img
}

fn apply_basic_adjust(
    img: DynamicImage,
    brightness: i32,
    contrast: f32,
    saturation_percent: f32,
    hue: i32,
    filter: Option<&str>,
) -> DynamicImage {
    // Match existing editor semantics as closely as practical.
    let mut img = img;
    if hue != 0 {
        img = img.huerotate(hue);
    }
    let b = brightness.clamp(-100, 100);
    let c = contrast.clamp(-100.0, 100.0);
    let sat = (saturation_percent / 100.0).clamp(0.0, 3.0);
    let need_pixel = b != 0 || c != 0.0 || (sat - 1.0).abs() > 1e-4;
    if need_pixel {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut out = RgbaImage::new(w, h);
        let bright_mul = (100 + b) as f32 / 100.0;
        let contrast_factor = (259.0 * (c + 255.0)) / (255.0 * (259.0 - c));
        for (x, y, p) in rgba.enumerate_pixels() {
            let mut r = p[0] as f32;
            let mut g = p[1] as f32;
            let mut bch = p[2] as f32;
            r *= bright_mul;
            g *= bright_mul;
            bch *= bright_mul;
            r = contrast_factor * (r - 128.0) + 128.0;
            g = contrast_factor * (g - 128.0) + 128.0;
            bch = contrast_factor * (bch - 128.0) + 128.0;
            if (sat - 1.0).abs() > 1e-4 {
                let luma = 0.299 * r + 0.587 * g + 0.114 * bch;
                r = luma + (r - luma) * sat;
                g = luma + (g - luma) * sat;
                bch = luma + (bch - luma) * sat;
            }
            out.put_pixel(
                x,
                y,
                Rgba([
                    r.round().clamp(0.0, 255.0) as u8,
                    g.round().clamp(0.0, 255.0) as u8,
                    bch.round().clamp(0.0, 255.0) as u8,
                    p[3],
                ]),
            );
        }
        img = DynamicImage::ImageRgba8(out);
    }
    match filter.unwrap_or("") {
        "grayscale" => img = img.grayscale().into_rgba8().into(),
        "invert" => {
            let mut rgba = img.to_rgba8();
            for p in rgba.pixels_mut() {
                p[0] = 255 - p[0];
                p[1] = 255 - p[1];
                p[2] = 255 - p[2];
            }
            img = DynamicImage::ImageRgba8(rgba);
        }
        "sepia" => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = RgbaImage::new(w, h);
            for (x, y, p) in rgba.enumerate_pixels() {
                let r = p[0] as f32;
                let g = p[1] as f32;
                let b = p[2] as f32;
                let nr = (0.393 * r + 0.769 * g + 0.189 * b).min(255.0);
                let ng = (0.349 * r + 0.686 * g + 0.168 * b).min(255.0);
                let nb = (0.272 * r + 0.534 * g + 0.131 * b).min(255.0);
                out.put_pixel(x, y, Rgba([nr as u8, ng as u8, nb as u8, p[3]]));
            }
            img = DynamicImage::ImageRgba8(out);
        }
        _ => {}
    }
    img
}

fn apply_highlights_shadows(img: DynamicImage, highlights: f32, shadows: f32) -> DynamicImage {
    let h = (highlights / 100.0).clamp(-1.0, 1.0);
    let s = (shadows / 100.0).clamp(-1.0, 1.0);
    if h.abs() < 1e-4 && s.abs() < 1e-4 {
        return img;
    }
    let rgba = img.to_rgba8();
    let (w, hh) = rgba.dimensions();
    let mut out = RgbaImage::new(w, hh);
    for (x, y, p) in rgba.enumerate_pixels() {
        let mut r = p[0] as f32 / 255.0;
        let mut g = p[1] as f32 / 255.0;
        let mut b = p[2] as f32 / 255.0;
        let luma = 0.299 * r + 0.587 * g + 0.114 * b;
        // shadow mask near 0, highlight mask near 1
        let sm = (1.0 - luma).powf(1.5);
        let hm = luma.powf(1.5);
        let delta = s * 0.35 * sm + h * 0.35 * hm;
        r = (r + delta).clamp(0.0, 1.0);
        g = (g + delta).clamp(0.0, 1.0);
        b = (b + delta).clamp(0.0, 1.0);
        out.put_pixel(
            x,
            y,
            Rgba([
                (r * 255.0).round() as u8,
                (g * 255.0).round() as u8,
                (b * 255.0).round() as u8,
                p[3],
            ]),
        );
    }
    DynamicImage::ImageRgba8(out)
}

fn apply_fade(img: DynamicImage, fade: f32) -> DynamicImage {
    let t = (fade / 100.0).clamp(0.0, 1.0) * 0.55;
    if t <= 0.0 {
        return img;
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbaImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        // lift blacks toward mid gray
        let r = p[0] as f32 * (1.0 - t) + 128.0 * t;
        let g = p[1] as f32 * (1.0 - t) + 128.0 * t;
        let b = p[2] as f32 * (1.0 - t) + 128.0 * t;
        out.put_pixel(
            x,
            y,
            Rgba([
                r.round().clamp(0.0, 255.0) as u8,
                g.round().clamp(0.0, 255.0) as u8,
                b.round().clamp(0.0, 255.0) as u8,
                p[3],
            ]),
        );
    }
    DynamicImage::ImageRgba8(out)
}

fn apply_vignette(img: DynamicImage, amount: f32) -> DynamicImage {
    let a = (amount / 100.0).clamp(0.0, 1.0);
    if a <= 0.0 {
        return img;
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let cx = (w as f32 - 1.0) * 0.5;
    let cy = (h as f32 - 1.0) * 0.5;
    let max_d = (cx * cx + cy * cy).sqrt().max(1.0);
    let mut out = RgbaImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let dx = x as f32 - cx;
        let dy = y as f32 - cy;
        let d = (dx * dx + dy * dy).sqrt() / max_d;
        // smooth edge darkening
        let v = 1.0 - a * d.powf(1.8);
        out.put_pixel(
            x,
            y,
            Rgba([
                ((p[0] as f32) * v).round().clamp(0.0, 255.0) as u8,
                ((p[1] as f32) * v).round().clamp(0.0, 255.0) as u8,
                ((p[2] as f32) * v).round().clamp(0.0, 255.0) as u8,
                p[3],
            ]),
        );
    }
    DynamicImage::ImageRgba8(out)
}

fn apply_grain(img: DynamicImage, amount: f32) -> DynamicImage {
    let a = (amount / 100.0).clamp(0.0, 1.0);
    if a <= 0.0 {
        return img;
    }
    let strength = a * 28.0;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbaImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        // cheap deterministic noise
        let n = hash_noise(x, y);
        let delta = (n - 0.5) * 2.0 * strength;
        out.put_pixel(
            x,
            y,
            Rgba([
                (p[0] as f32 + delta).round().clamp(0.0, 255.0) as u8,
                (p[1] as f32 + delta).round().clamp(0.0, 255.0) as u8,
                (p[2] as f32 + delta).round().clamp(0.0, 255.0) as u8,
                p[3],
            ]),
        );
    }
    DynamicImage::ImageRgba8(out)
}

fn hash_noise(x: u32, y: u32) -> f32 {
    let mut n = x
        .wrapping_mul(374761393)
        .wrapping_add(y.wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    n ^= n >> 16;
    (n as f32) / (u32::MAX as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_identity_like_cube() {
        let mut text = String::from("TITLE \"t\"\nLUT_3D_SIZE 2\n\n");
        // 2^3 entries, identity-ish
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    text.push_str(&format!(
                        "{:.1} {:.1} {:.1}\n",
                        r as f32, g as f32, b as f32
                    ));
                }
            }
        }
        let cube = parse_cube_text(&text).expect("parse");
        assert_eq!(cube.size, 2);
        assert_eq!(cube.data.len(), 2 * 2 * 2 * 3);
        let s = sample_lut(&cube, 1.0, 0.0, 0.0);
        assert!((s[0] - 1.0).abs() < 1e-3);
    }
}
