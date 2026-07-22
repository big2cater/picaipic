/**
 * Traditional global Lab color match (追色).
 * Port of the conservative soft-global path from Photo Color Match
 * (color_transfer.py ColorTransferEngine) without segmentation masks.
 */
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// Tunable parameters for traditional color match.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColorMatchParams {
    /// 0..1 blend strength of the global grade (reference uses intensity * 0.65).
    #[serde(default = "default_intensity")]
    pub intensity: f32,
    /// 0..1 restore original luminance after grade.
    #[serde(default = "default_tone_preservation")]
    #[serde(rename = "tonePreservation")]
    pub tone_preservation: f32,
    /// Gray-world white balance on the target before matching.
    #[serde(default = "default_true")]
    #[serde(rename = "autoWb")]
    pub auto_wb: bool,
    /// 0..1 blend original back into highlight regions.
    #[serde(default = "default_protection")]
    #[serde(rename = "highlightProtection")]
    pub highlight_protection: f32,
    /// 0..1 blend original back into shadow regions.
    #[serde(default = "default_protection")]
    #[serde(rename = "shadowProtection")]
    pub shadow_protection: f32,
}

fn default_intensity() -> f32 {
    1.0
}
fn default_tone_preservation() -> f32 {
    0.5
}
fn default_true() -> bool {
    true
}
fn default_protection() -> f32 {
    0.8
}

impl Default for ColorMatchParams {
    fn default() -> Self {
        Self {
            intensity: default_intensity(),
            tone_preservation: default_tone_preservation(),
            auto_wb: true,
            highlight_protection: default_protection(),
            shadow_protection: default_protection(),
        }
    }
}

impl ColorMatchParams {
    pub fn clamped(self) -> Self {
        Self {
            intensity: self.intensity.clamp(0.0, 1.0),
            tone_preservation: self.tone_preservation.clamp(0.0, 1.0),
            auto_wb: self.auto_wb,
            highlight_protection: self.highlight_protection.clamp(0.0, 1.0),
            shadow_protection: self.shadow_protection.clamp(0.0, 1.0),
        }
    }
}

/// Max long edge for Lab median/percentile stats. Full-res sorting on 50MP+ is
/// orders of magnitude slower with nearly identical quantiles.
const STATS_MAX_EDGE: u32 = 1024;

/// Apply conservative global Lab statistics match from `reference` onto `target`.
pub fn apply_traditional_color_match(
    target: &DynamicImage,
    reference: &DynamicImage,
    params: &ColorMatchParams,
) -> DynamicImage {
    let params = params.clone().clamped();
    let (tw, th) = target.dimensions();
    if tw == 0 || th == 0 {
        return target.clone();
    }

    // Full-res working buffer only (result is written in-place). Stats use
    // downsampled copies so we never allocate multiple 50MP f32 planes.
    let mut target_rgb = image_to_rgb_f32(target);

    let mut target_stats_rgb = image_to_rgb_f32(&downsample_for_stats(target, STATS_MAX_EDGE));
    let ref_rgb = image_to_rgb_f32(&downsample_for_stats(reference, STATS_MAX_EDGE));

    // Gray-world scales from the stats sample (mean is robust to downsampling).
    let wb_scales = if params.auto_wb {
        let scales = white_balance_scales(&target_stats_rgb);
        apply_scales_inplace(&mut target_stats_rgb, scales);
        Some(scales)
    } else {
        None
    };

    let maps = compute_lab_channel_maps(&target_stats_rgb, &ref_rgb, 0.18, 0.42);
    let blend = (params.intensity * 0.65).clamp(0.0, 1.0);
    let inv_blend = 1.0 - blend;

    // Single pixel pass: WB → Lab grade blend → highlight/shadow protect → tone.
    for p in target_rgb.iter_mut() {
        let original = *p;
        let mut working = if let Some(scales) = wb_scales {
            apply_scales(original, scales)
        } else {
            original
        };

        if let Some(ref maps) = maps {
            let graded = apply_lab_channel_maps(working, maps);
            working = [
                working[0] * inv_blend + graded[0] * blend,
                working[1] * inv_blend + graded[1] * blend,
                working[2] * inv_blend + graded[2] * blend,
            ];
        }

        protect_one(
            &mut working,
            original,
            params.highlight_protection,
            params.shadow_protection,
        );
        preserve_tone_one(&mut working, original, params.tone_preservation);
        *p = working;
    }

    rgb_f32_to_image(&target_rgb, tw, th)
}

/// Bake a single image's color look into an Adobe/Resolve `.cube` 3D LUT.
///
/// This does **not** encode a source→reference color-match pair. It treats the
/// given image as the style/reference look and maps a neutral identity gamut
/// toward that image's Lab statistics, so the LUT can be applied to other photos.
pub fn build_style_cube_from_image(image: &DynamicImage, size: u32) -> Result<String, String> {
    if !(17..=65).contains(&size) {
        return Err(format!(
            "LUT size must be between 17 and 65 inclusive, got {}",
            size
        ));
    }
    let size = size as usize;

    let style = downsample_for_stats(image, STATS_MAX_EDGE);
    let style_rgb = image_to_rgb_f32(&style);
    if style_rgb.len() < 32 {
        return Err("Not enough pixels to estimate style".to_string());
    }

    // Neutral identity population: evenly spaced sRGB lattice samples.
    let mut identity_rgb: Vec<[f32; 3]> = Vec::with_capacity(size * size * size);
    for bi in 0..size {
        let b = (bi as f32) * 255.0 / ((size - 1) as f32);
        for gi in 0..size {
            let g = (gi as f32) * 255.0 / ((size - 1) as f32);
            for ri in 0..size {
                let r = (ri as f32) * 255.0 / ((size - 1) as f32);
                identity_rgb.push([r, g, b]);
            }
        }
    }

    // Full-strength soft Lab grade: identity gamut → style image stats.
    let maps = compute_lab_channel_maps(&identity_rgb, &style_rgb, 0.18, 0.42)
        .ok_or_else(|| "Failed to compute style Lab statistics".to_string())?;
    let blend = 0.65_f32; // same soft global intensity scale as color match at 1.0

    let mut out = String::with_capacity(size * size * size * 32 + 160);
    out.push_str("# PicAiPic style LUT from single image\n");
    out.push_str(&format!("# Size: {size}x{size}x{size}\n"));
    out.push_str("TITLE \"PicAiPic Image Style\"\n");
    out.push_str(&format!("LUT_3D_SIZE {size}\n"));
    out.push('\n');

    // .cube lattice: blue slowest, green, red fastest. Values R G B in 0..1.
    for bi in 0..size {
        let b = (bi as f32) * 255.0 / ((size - 1) as f32);
        for gi in 0..size {
            let g = (gi as f32) * 255.0 / ((size - 1) as f32);
            for ri in 0..size {
                let r = (ri as f32) * 255.0 / ((size - 1) as f32);
                let original = [r, g, b];
                let matched = apply_lab_channel_maps(original, &maps);
                let result = [
                    original[0] * (1.0 - blend) + matched[0] * blend,
                    original[1] * (1.0 - blend) + matched[1] * blend,
                    original[2] * (1.0 - blend) + matched[2] * blend,
                ];
                out.push_str(&format!(
                    "{:.6} {:.6} {:.6}\n",
                    (result[0] / 255.0).clamp(0.0, 1.0),
                    (result[1] / 255.0).clamp(0.0, 1.0),
                    (result[2] / 255.0).clamp(0.0, 1.0),
                ));
            }
        }
    }
    Ok(out)
}

pub fn write_style_cube_from_image(
    image: &DynamicImage,
    size: u32,
    dest_path: &str,
) -> Result<(), String> {
    let text = build_style_cube_from_image(image, size)?;
    if let Some(parent) = std::path::Path::new(dest_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create LUT dir: {}", e))?;
        }
    }
    std::fs::write(dest_path, text).map_err(|e| format!("write LUT: {}", e))
}

fn downsample_for_stats(img: &DynamicImage, max_edge: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let edge = w.max(h);
    if edge <= max_edge || max_edge < 64 {
        return img.clone();
    }
    let scale = max_edge as f32 / edge as f32;
    let nw = ((w as f32) * scale).round().max(1.0) as u32;
    let nh = ((h as f32) * scale).round().max(1.0) as u32;
    img.resize(nw, nh, image::imageops::FilterType::Triangle)
}

fn image_to_rgb_f32(img: &DynamicImage) -> Vec<[f32; 3]> {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = Vec::with_capacity((w * h) as usize);
    for p in rgba.pixels() {
        out.push([p[0] as f32, p[1] as f32, p[2] as f32]);
    }
    out
}

fn rgb_f32_to_image(pixels: &[[f32; 3]], w: u32, h: u32) -> DynamicImage {
    let mut rgba = RgbaImage::new(w, h);
    for (i, px) in pixels.iter().enumerate() {
        let x = (i as u32) % w;
        let y = (i as u32) / w;
        rgba.put_pixel(
            x,
            y,
            Rgba([
                px[0].round().clamp(0.0, 255.0) as u8,
                px[1].round().clamp(0.0, 255.0) as u8,
                px[2].round().clamp(0.0, 255.0) as u8,
                255,
            ]),
        );
    }
    DynamicImage::ImageRgba8(rgba)
}

fn white_balance_scales(pixels: &[[f32; 3]]) -> [f32; 3] {
    if pixels.is_empty() {
        return [1.0, 1.0, 1.0];
    }
    let n = pixels.len() as f32;
    let mut sum = [0.0f32; 3];
    for p in pixels.iter() {
        sum[0] += p[0];
        sum[1] += p[1];
        sum[2] += p[2];
    }
    let mean = [sum[0] / n, sum[1] / n, sum[2] / n];
    let avg = (mean[0] + mean[1] + mean[2]) / 3.0;
    [
        avg / mean[0].max(1.0),
        avg / mean[1].max(1.0),
        avg / mean[2].max(1.0),
    ]
}

fn apply_scales(p: [f32; 3], scale: [f32; 3]) -> [f32; 3] {
    [
        (p[0] * scale[0]).clamp(0.0, 255.0),
        (p[1] * scale[1]).clamp(0.0, 255.0),
        (p[2] * scale[2]).clamp(0.0, 255.0),
    ]
}

fn apply_scales_inplace(pixels: &mut [[f32; 3]], scale: [f32; 3]) {
    for p in pixels.iter_mut() {
        *p = apply_scales(*p, scale);
    }
}

/// Per-channel Lab median/percentile map shared by image apply and LUT bake.
#[derive(Clone, Copy)]
struct LabChannelMap {
    t_med: f32,
    scale: f32,
    shift: f32,
    strength: f32,
}

fn compute_lab_channel_maps(
    image: &[[f32; 3]],
    reference: &[[f32; 3]],
    l_strength: f32,
    chroma_strength: f32,
) -> Option<[LabChannelMap; 3]> {
    if image.len() < 32 || reference.len() < 32 {
        return None;
    }

    let image_lab: Vec<[f32; 3]> = image
        .iter()
        .map(|p| rgb_to_opencv_lab(p[0], p[1], p[2]))
        .collect();
    let ref_lab: Vec<[f32; 3]> = reference
        .iter()
        .map(|p| rgb_to_opencv_lab(p[0], p[1], p[2]))
        .collect();
    let strengths = [l_strength, chroma_strength, chroma_strength];
    let mut maps = [LabChannelMap {
        t_med: 0.0,
        scale: 1.0,
        shift: 0.0,
        strength: 0.0,
    }; 3];

    for c in 0..3 {
        let t_vals: Vec<f32> = image_lab.iter().map(|p| p[c]).collect();
        let r_vals: Vec<f32> = ref_lab.iter().map(|p| p[c]).collect();
        // One sort per channel yields median + 16/84 percentiles (was 3 full sorts each).
        let (t_med, t_p16, t_p84) = channel_quantiles(&t_vals);
        let (r_med, r_p16, r_p84) = channel_quantiles(&r_vals);
        let t_width = (t_p84 - t_p16) + 1e-6;
        let r_width = (r_p84 - r_p16) + 1e-6;
        let scale = (r_width / t_width).clamp(0.84, 1.18);
        let shift_limit = if c == 0 { 16.0 } else { 20.0 };
        let shift = (r_med - t_med).clamp(-shift_limit, shift_limit);
        maps[c] = LabChannelMap {
            t_med,
            scale,
            shift,
            strength: strengths[c],
        };
    }
    Some(maps)
}

fn apply_lab_channel_maps(rgb: [f32; 3], maps: &[LabChannelMap; 3]) -> [f32; 3] {
    let mut lab = rgb_to_opencv_lab(rgb[0], rgb[1], rgb[2]);
    for c in 0..3 {
        let m = maps[c];
        let mapped = (lab[c] - m.t_med) * m.scale + m.t_med + m.shift;
        lab[c] = lab[c] * (1.0 - m.strength) + mapped * m.strength;
    }
    // Full OpenCV 8-bit Lab range (was 72..186 on a/b, which crushed saturated colors).
    lab[0] = lab[0].clamp(0.0, 255.0);
    lab[1] = lab[1].clamp(0.0, 255.0);
    lab[2] = lab[2].clamp(0.0, 255.0);
    let (r, g, b) = opencv_lab_to_rgb(lab[0], lab[1], lab[2]);
    [r, g, b]
}

fn protect_one(result: &mut [f32; 3], original: [f32; 3], highlight_prot: f32, shadow_prot: f32) {
    if highlight_prot <= 0.0 && shadow_prot <= 0.0 {
        return;
    }
    let gray = 0.299 * original[0] + 0.587 * original[1] + 0.114 * original[2];
    let highlight_mask = ((gray - 190.0) / 35.0).clamp(0.0, 1.0);
    let shadow_mask = ((70.0 - gray) / 35.0).clamp(0.0, 1.0);
    let protection = (highlight_mask * highlight_prot).max(shadow_mask * shadow_prot);
    if protection > 0.0 {
        let inv = 1.0 - protection;
        result[0] = result[0] * inv + original[0] * protection;
        result[1] = result[1] * inv + original[1] * protection;
        result[2] = result[2] * inv + original[2] * protection;
    }
}

fn preserve_tone_one(result: &mut [f32; 3], original: [f32; 3], strength: f32) {
    if strength <= 0.0 {
        return;
    }
    let orig_gray = 0.299 * original[0] + 0.587 * original[1] + 0.114 * original[2];
    let res_gray = 0.299 * result[0] + 0.587 * result[1] + 0.114 * result[2];
    let ratio = (orig_gray / res_gray.max(1.0)).clamp(0.6, 1.8);
    let factor = 1.0 - strength + strength * ratio;
    result[0] = (result[0] * factor).clamp(0.0, 255.0);
    result[1] = (result[1] * factor).clamp(0.0, 255.0);
    result[2] = (result[2] * factor).clamp(0.0, 255.0);
}

// --- OpenCV-style 8-bit Lab (sRGB) ---
// L: 0..255 maps L* 0..100; a/b: 0..255 with 128 = 0.

fn srgb_u8_to_linear(c: f32) -> f32 {
    let x = (c / 255.0).clamp(0.0, 1.0);
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_u8(c: f32) -> f32 {
    let x = c.clamp(0.0, 1.0);
    let s = if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).clamp(0.0, 255.0)
}

fn rgb_to_opencv_lab(r: f32, g: f32, b: f32) -> [f32; 3] {
    let rl = srgb_u8_to_linear(r);
    let gl = srgb_u8_to_linear(g);
    let bl = srgb_u8_to_linear(b);

    // sRGB D65 -> XYZ
    let x = rl * 0.4124564 + gl * 0.3575761 + bl * 0.1804375;
    let y = rl * 0.2126729 + gl * 0.7151522 + bl * 0.0721750;
    let z = rl * 0.0193339 + gl * 0.1191920 + bl * 0.9503041;

    // D65 white
    let xr = x / 0.95047;
    let yr = y / 1.00000;
    let zr = z / 1.08883;

    let fx = lab_f(xr);
    let fy = lab_f(yr);
    let fz = lab_f(zr);

    let l_star = 116.0 * fy - 16.0;
    let a_star = 500.0 * (fx - fy);
    let b_star = 200.0 * (fy - fz);

    [
        (l_star * 255.0 / 100.0).clamp(0.0, 255.0),
        (a_star + 128.0).clamp(0.0, 255.0),
        (b_star + 128.0).clamp(0.0, 255.0),
    ]
}

fn opencv_lab_to_rgb(l8: f32, a8: f32, b8: f32) -> (f32, f32, f32) {
    let l_star = l8 * 100.0 / 255.0;
    let a_star = a8 - 128.0;
    let b_star = b8 - 128.0;

    let fy = (l_star + 16.0) / 116.0;
    let fx = fy + a_star / 500.0;
    let fz = fy - b_star / 200.0;

    let xr = lab_f_inv(fx);
    let yr = lab_f_inv(fy);
    let zr = lab_f_inv(fz);

    let x = xr * 0.95047;
    let y = yr * 1.00000;
    let z = zr * 1.08883;

    let rl = x * 3.2404542 + y * -1.5371385 + z * -0.4985314;
    let gl = x * -0.9692660 + y * 1.8760108 + z * 0.0415560;
    let bl = x * 0.0556434 + y * -0.2040259 + z * 1.0572252;

    (
        linear_to_srgb_u8(rl),
        linear_to_srgb_u8(gl),
        linear_to_srgb_u8(bl),
    )
}

fn lab_f(t: f32) -> f32 {
    const DELTA: f32 = 6.0 / 29.0;
    if t > DELTA * DELTA * DELTA {
        t.cbrt()
    } else {
        t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
    }
}

fn lab_f_inv(t: f32) -> f32 {
    const DELTA: f32 = 6.0 / 29.0;
    if t > DELTA {
        t * t * t
    } else {
        3.0 * DELTA * DELTA * (t - 4.0 / 29.0)
    }
}

/// Sort once and return (median, p16, p84).
fn channel_quantiles(values: &[f32]) -> (f32, f32, f32) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = v.len() / 2;
    let median = if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) * 0.5
    } else {
        v[mid]
    };
    let p_at = |p: f32| {
        let idx = ((v.len() as f32 - 1.0) * p.clamp(0.0, 1.0)).round() as usize;
        v[idx.min(v.len() - 1)]
    };
    (median, p_at(0.16), p_at(0.84))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn solid(w: u32, h: u32, r: u8, g: u8, b: u8) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |_, _| Rgb([r, g, b])))
    }

    fn mean_rgb(img: &DynamicImage) -> [f32; 3] {
        let rgba = img.to_rgba8();
        let n = (rgba.width() * rgba.height()).max(1) as f32;
        let mut s = [0.0f32; 3];
        for p in rgba.pixels() {
            s[0] += p[0] as f32;
            s[1] += p[1] as f32;
            s[2] += p[2] as f32;
        }
        [s[0] / n, s[1] / n, s[2] / n]
    }

    #[test]
    fn same_image_stays_close() {
        let img = solid(32, 32, 120, 90, 70);
        let out = apply_traditional_color_match(
            &img,
            &img,
            &ColorMatchParams {
                intensity: 1.0,
                tone_preservation: 0.0,
                auto_wb: false,
                highlight_protection: 0.0,
                shadow_protection: 0.0,
            },
        );
        let a = mean_rgb(&img);
        let b = mean_rgb(&out);
        for i in 0..3 {
            assert!(
                (a[i] - b[i]).abs() < 8.0,
                "channel {i}: {} vs {}",
                a[i],
                b[i]
            );
        }
    }

    #[test]
    fn warm_reference_pulls_mean_warmer() {
        let target = solid(48, 48, 100, 100, 100);
        let reference = solid(48, 48, 180, 120, 80);
        let out = apply_traditional_color_match(
            &target,
            &reference,
            &ColorMatchParams {
                intensity: 1.0,
                tone_preservation: 0.0,
                auto_wb: false,
                highlight_protection: 0.0,
                shadow_protection: 0.0,
            },
        );
        let t = mean_rgb(&target);
        let o = mean_rgb(&out);
        // R should rise relative to B (warmer).
        assert!(
            o[0] - o[2] > t[0] - t[2] + 2.0,
            "expected warmer cast: {o:?} from {t:?}"
        );
    }

    #[test]
    fn highlight_protection_keeps_bright_pixels() {
        let mut target = RgbImage::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let v = if x < 8 { 40 } else { 240 };
                target.put_pixel(x, y, Rgb([v, v, v]));
            }
        }
        let target = DynamicImage::ImageRgb8(target);
        let reference = solid(16, 16, 40, 180, 40);
        let unprotected = apply_traditional_color_match(
            &target,
            &reference,
            &ColorMatchParams {
                intensity: 1.0,
                tone_preservation: 0.0,
                auto_wb: false,
                highlight_protection: 0.0,
                shadow_protection: 0.0,
            },
        );
        let protected = apply_traditional_color_match(
            &target,
            &reference,
            &ColorMatchParams {
                intensity: 1.0,
                tone_preservation: 0.0,
                auto_wb: false,
                highlight_protection: 1.0,
                shadow_protection: 0.0,
            },
        );
        // Sample a highlight pixel (right half).
        let o = target.get_pixel(12, 8);
        let p = protected.get_pixel(12, 8);
        let u = unprotected.get_pixel(12, 8);
        let dist_p = (o[0] as i32 - p[0] as i32).abs()
            + (o[1] as i32 - p[1] as i32).abs()
            + (o[2] as i32 - p[2] as i32).abs();
        let dist_u = (o[0] as i32 - u[0] as i32).abs()
            + (o[1] as i32 - u[1] as i32).abs()
            + (o[2] as i32 - u[2] as i32).abs();
        assert!(dist_p <= dist_u, "protected={dist_p} unprotected={dist_u}");
    }

    #[test]
    fn style_cube_export_has_size_header() {
        let style = solid(24, 24, 180, 120, 80);
        let cube = build_style_cube_from_image(&style, 17).expect("cube");
        assert!(cube.contains("LUT_3D_SIZE 17"));
        assert!(cube.contains("PicAiPic style LUT from single image") || cube.contains("TITLE"));
        let data_lines = cube
            .lines()
            .filter(|l| {
                !l.is_empty()
                    && !l.starts_with('#')
                    && !l.starts_with("TITLE")
                    && !l.starts_with("LUT_")
            })
            .count();
        assert_eq!(data_lines, 17 * 17 * 17);
    }

    #[test]
    fn style_cube_rejects_out_of_range_size() {
        let style = solid(16, 16, 120, 90, 70);
        assert!(build_style_cube_from_image(&style, 8).is_err());
        assert!(build_style_cube_from_image(&style, 100).is_err());
    }

    #[test]
    fn saturated_reference_keeps_chroma() {
        // Neutral gray target pulled toward a highly saturated red reference.
        // Old a/b clamp(72,186) crushed chroma; full 0..255 Lab must keep a red cast.
        let target = solid(48, 48, 128, 128, 128);
        let reference = solid(48, 48, 255, 20, 20);
        let out = apply_traditional_color_match(
            &target,
            &reference,
            &ColorMatchParams {
                intensity: 1.0,
                tone_preservation: 0.0,
                auto_wb: false,
                highlight_protection: 0.0,
                shadow_protection: 0.0,
            },
        );
        let o = mean_rgb(&out);
        assert!(
            o[0] > o[1] + 8.0 && o[0] > o[2] + 8.0,
            "expected red cast retained from saturated reference, got {o:?}"
        );
    }
}
