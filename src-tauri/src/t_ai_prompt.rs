/**
 * AI image prompt import from PNG text chunks and JPEG EXIF/COM
 * (A1111 / NovelAI / InvokeAI / ComfyUI).
 * Used during library scan to fill empty afiles.comments only.
 */
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Default on: import prompts into empty comments during scan.
static IMPORT_AI_PROMPTS: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(true));

const MAX_COMMENT_CHARS: usize = 4000;
/// Bound ancillary-chunk scan so huge PNGs cannot stall indexing.
const MAX_PNG_SCAN_BYTES: u64 = 4 * 1024 * 1024;
/// Bound JPEG marker walk for COM / late APP1.
const MAX_JPEG_SCAN_BYTES: u64 = 2 * 1024 * 1024;
const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_SIG: &[u8] = b"\xFF\xD8";

pub fn set_import_enabled(enabled: bool) {
    IMPORT_AI_PROMPTS.store(enabled, Ordering::Relaxed);
}

pub fn is_import_enabled() -> bool {
    IMPORT_AI_PROMPTS.load(Ordering::Relaxed)
}

/// Extract a prompt when import is enabled.
/// Tries PNG text chunks first, then JPEG UserComment / COM / ImageDescription-like fields.
/// `header` may be the scan pre-read buffer (up to 128 KiB).
/// `exif_user_comment` / `exif_image_description` avoid a second EXIF open when the scanner already has them.
pub fn extract_prompt_for_path(
    file_path: &str,
    header: Option<&[u8]>,
    exif_user_comment: Option<&str>,
    exif_image_description: Option<&str>,
) -> Option<String> {
    if !is_import_enabled() {
        return None;
    }

    if looks_like_png(file_path, header) {
        if let Ok(chunks) = extract_png_text_chunks(file_path, header) {
            if !chunks.is_empty() {
                if let Some(p) = parse_ai_prompt(&chunks) {
                    return Some(truncate_comment(&p));
                }
            }
        }
    }

    if looks_like_jpeg(file_path, header) {
        if let Some(p) = extract_jpeg_prompt(
            file_path,
            header,
            exif_user_comment,
            exif_image_description,
        ) {
            return Some(truncate_comment(&p));
        }
    }

    // Non-JPEG that still carries EXIF description/user comment (rare HEIC/TIFF-like paths).
    if let Some(p) =
        parse_jpeg_prompt_fields(exif_user_comment, exif_image_description, &[] as &[&str])
    {
        return Some(truncate_comment(&p));
    }

    None
}

fn looks_like_png(file_path: &str, header: Option<&[u8]>) -> bool {
    if let Some(h) = header {
        if is_png_bytes(h) {
            return true;
        }
    }
    Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("png"))
        .unwrap_or(false)
}

fn looks_like_jpeg(file_path: &str, header: Option<&[u8]>) -> bool {
    if let Some(h) = header {
        if is_jpeg_bytes(h) {
            return true;
        }
    }
    Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "jpe" | "jfif"
            )
        })
        .unwrap_or(false)
}

pub fn is_png_bytes(data: &[u8]) -> bool {
    data.len() >= PNG_SIG.len() && data.starts_with(PNG_SIG)
}

pub fn is_jpeg_bytes(data: &[u8]) -> bool {
    data.len() >= JPEG_SIG.len() && data.starts_with(JPEG_SIG)
}

/// Decode EXIF UserComment payload (charset header + text).
pub fn decode_exif_user_comment(data: &[u8]) -> Option<String> {
    if data.is_empty() {
        return None;
    }
    // Standard EXIF UserComment: 8-byte charset code + text body.
    if data.len() >= 8 {
        let charset = &data[..8];
        let body = &data[8..];
        if charset.starts_with(b"ASCII") || charset.iter().all(|&b| b == 0) {
            let s = String::from_utf8_lossy(body)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if !s.is_empty() {
                return Some(s);
            }
        } else if charset.starts_with(b"UNICODE") {
            // UTF-16 BE or LE; try LE first (common on Windows tooling).
            if let Some(s) = decode_utf16_bytes(body) {
                return Some(s);
            }
        } else if charset.starts_with(b"JIS") {
            let s = String::from_utf8_lossy(body)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    // Fallback: whole buffer as UTF-8/Latin-1-ish lossy text.
    let s = String::from_utf8_lossy(data)
        .trim_end_matches('\0')
        .trim()
        .to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn decode_utf16_bytes(data: &[u8]) -> Option<String> {
    if data.len() < 2 {
        return None;
    }
    let has_bom_le = data.starts_with(&[0xFF, 0xFE]);
    let has_bom_be = data.starts_with(&[0xFE, 0xFF]);
    let (be, start) = if has_bom_be {
        (true, 2)
    } else if has_bom_le {
        (false, 2)
    } else {
        // Heuristic: many zero high bytes → BE; many zero low bytes → LE.
        let zero_odd = data.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
        let zero_even = data.iter().step_by(2).filter(|&&b| b == 0).count();
        (zero_odd >= zero_even, 0)
    };
    let units: Vec<u16> = data[start..]
        .chunks_exact(2)
        .map(|c| {
            if be {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        })
        .take_while(|&u| u != 0)
        .collect();
    let s = String::from_utf16_lossy(&units).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn extract_jpeg_prompt(
    file_path: &str,
    header: Option<&[u8]>,
    exif_user_comment: Option<&str>,
    exif_image_description: Option<&str>,
) -> Option<String> {
    let mut com_notes: Vec<String> = Vec::new();
    if let Some(h) = header {
        if is_jpeg_bytes(h) {
            collect_jpeg_com_from_bytes(h, &mut com_notes);
        }
    }
    if com_notes.is_empty() {
        if let Ok(extra) = extract_jpeg_com_from_file(file_path) {
            com_notes.extend(extra);
        }
    }

    // If scanner did not pass EXIF strings, try a lightweight EXIF UserComment read.
    let owned_user_comment = if exif_user_comment.is_none() {
        extract_jpeg_user_comment_from_file(file_path)
    } else {
        None
    };
    let user_comment = exif_user_comment.or(owned_user_comment.as_deref());

    let com_refs: Vec<&str> = com_notes.iter().map(|s| s.as_str()).collect();
    parse_jpeg_prompt_fields(user_comment, exif_image_description, &com_refs)
}

/// Parse prompt candidates from JPEG-side metadata strings.
pub fn parse_jpeg_prompt_fields(
    user_comment: Option<&str>,
    image_description: Option<&str>,
    com_comments: &[&str],
) -> Option<String> {
    let mut candidates: Vec<&str> = Vec::new();
    if let Some(s) = user_comment {
        candidates.push(s);
    }
    for c in com_comments {
        candidates.push(c);
    }
    if let Some(s) = image_description {
        candidates.push(s);
    }

    for raw in candidates {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        // A1111 often dumps full parameters into UserComment.
        if let Some(p) = parse_a1111_parameters(raw) {
            return Some(p);
        }
        if let Some(p) = parse_novelai_or_json_prompt(raw) {
            return Some(p);
        }
        if let Some(p) = parse_json_prompt_field(raw) {
            return Some(p);
        }
        if let Some(p) = parse_comfy_prompt(raw) {
            return Some(p);
        }
        if looks_like_prompt_text(raw) {
            return Some(raw.to_string());
        }
    }
    None
}

fn collect_jpeg_com_from_bytes(data: &[u8], out: &mut Vec<String>) {
    if !is_jpeg_bytes(data) {
        return;
    }
    let mut i = 2usize;
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            break;
        }
        let marker = data[i + 1];
        i += 2;
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        // Standalone markers without length
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if i + 2 > data.len() {
            break;
        }
        let len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if len < 2 || i + len > data.len() {
            break;
        }
        let payload = &data[i + 2..i + len];
        i += len;
        if marker == 0xFE {
            // COM
            let s = String::from_utf8_lossy(payload)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if !s.is_empty() {
                out.push(s);
            }
        }
    }
}

fn extract_jpeg_com_from_file(file_path: &str) -> Result<Vec<String>, String> {
    let mut file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut sig = [0u8; 2];
    file.read_exact(&mut sig).map_err(|e| e.to_string())?;
    if &sig != JPEG_SIG {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut read_total: u64 = 2;
    loop {
        if read_total >= MAX_JPEG_SCAN_BYTES {
            break;
        }
        let mut marker = [0u8; 2];
        if file.read_exact(&mut marker).is_err() {
            break;
        }
        read_total += 2;
        if marker[0] != 0xFF {
            break;
        }
        let m = marker[1];
        if m == 0xD9 || m == 0xDA {
            break;
        }
        if m == 0x01 || (0xD0..=0xD7).contains(&m) {
            continue;
        }
        let mut len_buf = [0u8; 2];
        if file.read_exact(&mut len_buf).is_err() {
            break;
        }
        read_total += 2;
        let len = u16::from_be_bytes(len_buf) as u64;
        if len < 2 {
            break;
        }
        let payload_len = (len - 2) as usize;
        if read_total + payload_len as u64 > MAX_JPEG_SCAN_BYTES {
            let _ = file.seek(SeekFrom::Current(payload_len as i64));
            break;
        }
        if m == 0xFE {
            let mut payload = vec![0u8; payload_len];
            if file.read_exact(&mut payload).is_err() {
                break;
            }
            read_total += payload_len as u64;
            let s = String::from_utf8_lossy(&payload)
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if !s.is_empty() {
                out.push(s);
            }
        } else {
            if file.seek(SeekFrom::Current(payload_len as i64)).is_err() {
                break;
            }
            read_total += payload_len as u64;
        }
    }
    Ok(out)
}

fn extract_jpeg_user_comment_from_file(file_path: &str) -> Option<String> {
    let exif = crate::t_image::read_exif_permissive(file_path)?;
    extract_user_comment_from_exif(&exif)
}

/// Pull UserComment text from a parsed EXIF object without aggressive punctuation stripping.
pub fn extract_user_comment_from_exif(exif: &exif::Exif) -> Option<String> {
    use exif::{In, Tag, Value};
    let field = exif
        .get_field(Tag::UserComment, In::PRIMARY)
        .or_else(|| exif.fields().find(|f| f.tag == Tag::UserComment))?;
    match &field.value {
        Value::Undefined(bytes, _) => decode_exif_user_comment(bytes),
        Value::Byte(bytes) => decode_exif_user_comment(bytes),
        Value::Ascii(lines) => {
            let mut bytes = Vec::new();
            for line in lines {
                bytes.extend(line.iter().cloned().take_while(|&b| b != 0));
            }
            let s = String::from_utf8_lossy(&bytes).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        _ => {
            let s = field.display_value().to_string();
            let s = s.trim().trim_matches('"').trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }
}

/// Walk PNG ancillary text chunks (tEXt / iTXt / zTXt) with a read budget.
pub fn extract_png_text_chunks(
    file_path: &str,
    header: Option<&[u8]>,
) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();

    if let Some(h) = header {
        if is_png_bytes(h) {
            parse_png_text_from_bytes(h, &mut map);
            // Header often holds A1111 parameters; if we already have a useful key, stop.
            if has_prompt_keys(&map) {
                return Ok(map);
            }
        }
    }

    let mut file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let mut sig = [0u8; 8];
    file.read_exact(&mut sig).map_err(|e| e.to_string())?;
    if &sig != PNG_SIG {
        return Ok(map);
    }

    let mut read_total: u64 = 8;
    loop {
        if read_total >= MAX_PNG_SCAN_BYTES {
            break;
        }
        let mut len_buf = [0u8; 4];
        if file.read_exact(&mut len_buf).is_err() {
            break;
        }
        read_total += 4;
        let data_len = u32::from_be_bytes(len_buf) as u64;
        let mut type_buf = [0u8; 4];
        if file.read_exact(&mut type_buf).is_err() {
            break;
        }
        read_total += 4;
        let chunk_type = match std::str::from_utf8(&type_buf) {
            Ok(s) => s.to_string(),
            Err(_) => break,
        };

        if chunk_type == "IEND" {
            break;
        }

        let next_budget = MAX_PNG_SCAN_BYTES.saturating_sub(read_total);
        if data_len > next_budget {
            // Skip remaining without loading unbounded data.
            let _ = file.seek(SeekFrom::Current(data_len as i64 + 4));
            break;
        }

        let mut data = vec![0u8; data_len as usize];
        if data_len > 0 && file.read_exact(&mut data).is_err() {
            break;
        }
        read_total += data_len;
        // CRC
        let mut crc = [0u8; 4];
        if file.read_exact(&mut crc).is_err() {
            break;
        }
        read_total += 4;

        if let Some((k, v)) = decode_text_chunk(&chunk_type, &data) {
            map.entry(k).or_insert(v);
        }
    }

    Ok(map)
}

fn parse_png_text_from_bytes(bytes: &[u8], map: &mut HashMap<String, String>) {
    if !is_png_bytes(bytes) || bytes.len() < 8 {
        return;
    }
    let mut i = 8usize;
    while i + 12 <= bytes.len() {
        let data_len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]])
            as usize;
        i += 4;
        if i + 4 > bytes.len() {
            break;
        }
        let chunk_type = match std::str::from_utf8(&bytes[i..i + 4]) {
            Ok(s) => s,
            Err(_) => break,
        };
        i += 4;
        if chunk_type == "IEND" {
            break;
        }
        if i + data_len + 4 > bytes.len() {
            // Incomplete chunk in header buffer — stop; full file walk will continue.
            break;
        }
        let data = &bytes[i..i + data_len];
        i += data_len + 4; // data + CRC
        if let Some((k, v)) = decode_text_chunk(chunk_type, data) {
            map.entry(k).or_insert(v);
        }
    }
}

fn decode_text_chunk(chunk_type: &str, data: &[u8]) -> Option<(String, String)> {
    match chunk_type {
        "tEXt" => decode_text_plain(data),
        "zTXt" => decode_text_ztxt(data),
        "iTXt" => decode_text_itxt(data),
        _ => None,
    }
}

fn decode_text_plain(data: &[u8]) -> Option<(String, String)> {
    let nul = data.iter().position(|&b| b == 0)?;
    let key = std::str::from_utf8(&data[..nul]).ok()?.to_string();
    let value = String::from_utf8_lossy(&data[nul + 1..]).into_owned();
    if key.is_empty() || value.trim().is_empty() {
        return None;
    }
    Some((key, value))
}

fn decode_text_ztxt(data: &[u8]) -> Option<(String, String)> {
    let nul = data.iter().position(|&b| b == 0)?;
    let key = std::str::from_utf8(&data[..nul]).ok()?.to_string();
    if nul + 2 > data.len() {
        return None;
    }
    let compression = data[nul + 1];
    if compression != 0 {
        return None; // only deflate
    }
    let compressed = &data[nul + 2..];
    let value = inflate_to_string(compressed)?;
    if key.is_empty() || value.trim().is_empty() {
        return None;
    }
    Some((key, value))
}

fn decode_text_itxt(data: &[u8]) -> Option<(String, String)> {
    // keyword\0 compression_flag compression_method language\0 translated\0 text
    let mut rest = data;
    let nul = rest.iter().position(|&b| b == 0)?;
    let key = std::str::from_utf8(&rest[..nul]).ok()?.to_string();
    rest = &rest[nul + 1..];
    if rest.len() < 2 {
        return None;
    }
    let compression_flag = rest[0];
    let _compression_method = rest[1];
    rest = &rest[2..];
    // language tag
    let nul = rest.iter().position(|&b| b == 0)?;
    rest = &rest[nul + 1..];
    // translated keyword
    let nul = rest.iter().position(|&b| b == 0)?;
    rest = &rest[nul + 1..];
    let value = if compression_flag == 1 {
        inflate_to_string(rest)?
    } else {
        String::from_utf8_lossy(rest).into_owned()
    };
    if key.is_empty() || value.trim().is_empty() {
        return None;
    }
    Some((key, value))
}

fn inflate_to_string(data: &[u8]) -> Option<String> {
    use flate2::read::ZlibDecoder;
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(String::from_utf8_lossy(&out).into_owned())
}

fn has_prompt_keys(map: &HashMap<String, String>) -> bool {
    const KEYS: &[&str] = &[
        "parameters",
        "prompt",
        "Prompt",
        "Comment",
        "comment",
        "Description",
        "workflow",
        "Workflow",
        "sd-metadata",
        "invokeai_metadata",
        "invokeai_graph",
        "Software",
    ];
    KEYS.iter().any(|k| map.contains_key(*k))
}

/// Parse known AI tool text chunks into a single human-readable prompt.
pub fn parse_ai_prompt(chunks: &HashMap<String, String>) -> Option<String> {
    // 1) Automatic1111 / Forge style
    if let Some(params) = get_ci(chunks, "parameters") {
        if let Some(p) = parse_a1111_parameters(params) {
            return Some(p);
        }
    }

    // 2) NovelAI / generic Comment
    for key in ["Comment", "comment", "Description", "description"] {
        if let Some(raw) = chunks.get(key) {
            if let Some(p) = parse_novelai_or_json_prompt(raw) {
                return Some(p);
            }
            if looks_like_prompt_text(raw) {
                return Some(raw.trim().to_string());
            }
        }
    }

    // 3) InvokeAI metadata
    for key in ["invokeai_metadata", "sd-metadata", "invokeai_graph"] {
        if let Some(raw) = get_ci(chunks, key) {
            if let Some(p) = parse_json_prompt_field(raw) {
                return Some(p);
            }
        }
    }

    // 4) Explicit prompt keys (Invoke / misc)
    for key in ["prompt", "Prompt", "positive_prompt", "Positive Prompt"] {
        if let Some(raw) = chunks.get(key) {
            if let Some(p) = parse_json_prompt_field(raw).or_else(|| {
                let t = raw.trim();
                if t.is_empty() {
                    None
                } else if t.starts_with('{') {
                    None
                } else {
                    Some(t.to_string())
                }
            }) {
                return Some(p);
            }
        }
    }

    // 5) ComfyUI workflow / prompt JSON
    for key in ["workflow", "Workflow", "prompt", "Prompt"] {
        if let Some(raw) = chunks.get(key) {
            if let Some(p) = parse_comfy_prompt(raw) {
                return Some(p);
            }
        }
    }

    None
}

fn get_ci<'a>(map: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    map.get(key)
        .map(|s| s.as_str())
        .or_else(|| {
            map.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.as_str())
        })
}

fn parse_a1111_parameters(raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    // Positive prompt is everything before "Negative prompt:" (case-insensitive).
    let lower = text.to_ascii_lowercase();
    let has_a1111_markers = lower.contains("negative prompt:")
        || lower.contains("\nsteps:")
        || lower.starts_with("steps:")
        || lower.contains("\nsampler:")
        || lower.contains("cfg scale:");
    let positive = if let Some(idx) = lower.find("negative prompt:") {
        text[..idx].trim()
    } else if let Some(idx) = lower.find("\nsteps:") {
        // Some exports omit Negative prompt line
        text[..idx].trim()
    } else {
        text
    };
    if positive.is_empty() {
        return None;
    }
    // Reject pure JSON dumped as parameters
    if positive.starts_with('{') && positive.contains("\"class_type\"") {
        return None;
    }
    // Require A1111 markers or prompt-like text so short camera strings
    // (e.g. "DSC_0001", "OLYMPUS") are not treated as generation prompts.
    if !has_a1111_markers && !looks_like_prompt_text(positive) {
        return None;
    }
    Some(positive.to_string())
}

fn parse_novelai_or_json_prompt(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if t.starts_with('{') {
        return parse_json_prompt_field(t);
    }
    if looks_like_prompt_text(t) {
        return Some(t.to_string());
    }
    None
}

fn parse_json_prompt_field(raw: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    // Common keys across NovelAI / Invoke / SD metadata
    const KEYS: &[&str] = &[
        "prompt",
        "positive_prompt",
        "positivePrompt",
        "Positive Prompt",
        "caption",
        "text",
    ];
    if let Some(obj) = v.as_object() {
        for k in KEYS {
            if let Some(s) = obj.get(*k).and_then(|x| x.as_str()) {
                let s = s.trim();
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        // Nested image / extra fields
        if let Some(s) = obj
            .get("image")
            .and_then(|i| i.get("prompt"))
            .and_then(|x| x.as_str())
        {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn parse_comfy_prompt(raw: &str) -> Option<String> {
    let t = raw.trim();
    if !t.starts_with('{') && !t.starts_with('[') {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(t).ok()?;
    let mut texts: Vec<String> = Vec::new();
    collect_comfy_texts(&v, &mut texts);
    // Prefer longer CLIP-like strings; skip short UI labels
    texts.retain(|s| s.chars().count() >= 8);
    if texts.is_empty() {
        return None;
    }
    texts.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    // De-dupe while preserving order
    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::new();
    for s in texts {
        if seen.insert(s.clone()) {
            unique.push(s);
        }
    }
    // Join up to two distinct prompts (positive-ish)
    let joined = unique.into_iter().take(2).collect::<Vec<_>>().join("\n---\n");
    if joined.trim().is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn collect_comfy_texts(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(map) => {
            let class = map
                .get("class_type")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            let is_clip = class.contains("CLIPText")
                || class.contains("TextEncode")
                || class.eq_ignore_ascii_case("CLIPTextEncode");
            if let Some(inputs) = map.get("inputs").and_then(|i| i.as_object()) {
                if let Some(text) = inputs.get("text").and_then(|t| t.as_str()) {
                    let text = text.trim();
                    if !text.is_empty() && (is_clip || text.chars().count() >= 24) {
                        out.push(text.to_string());
                    }
                }
            }
            // Also accept top-level "text" / "prompt" string fields in node maps
            for key in ["text", "prompt", "positive", "positive_prompt"] {
                if let Some(s) = map.get(key).and_then(|x| x.as_str()) {
                    let s = s.trim();
                    if s.chars().count() >= 24 {
                        out.push(s.to_string());
                    }
                }
            }
            for val in map.values() {
                collect_comfy_texts(val, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_comfy_texts(item, out);
            }
        }
        _ => {}
    }
}

fn looks_like_prompt_text(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 8 {
        return false;
    }
    // Heuristic: weight tags, commas, or multi-word English-ish prompt
    t.contains(',')
        || t.contains(":")
        || t.contains("masterpiece")
        || t.contains("best quality")
        || t.split_whitespace().count() >= 4
}

fn truncate_comment(s: &str) -> String {
    let count = s.chars().count();
    if count <= MAX_COMMENT_CHARS {
        return s.to_string();
    }
    let truncated: String = s.chars().take(MAX_COMMENT_CHARS.saturating_sub(1)).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a1111_positive_only() {
        let raw = "a cat, masterpiece\nNegative prompt: bad hands\nSteps: 20, Sampler: Euler a";
        let p = parse_a1111_parameters(raw).unwrap();
        assert!(p.contains("a cat"));
        assert!(!p.to_ascii_lowercase().contains("negative"));
        assert!(!p.contains("Steps:"));
    }

    #[test]
    fn parse_from_parameters_chunk() {
        let mut m = HashMap::new();
        m.insert(
            "parameters".into(),
            "sunset beach, 4k\nNegative prompt: blur\nSteps: 28".into(),
        );
        let p = parse_ai_prompt(&m).unwrap();
        assert_eq!(p, "sunset beach, 4k");
    }

    #[test]
    fn novelai_json_prompt() {
        let mut m = HashMap::new();
        m.insert(
            "Comment".into(),
            r#"{"prompt":"1girl, school uniform","steps":28}"#.into(),
        );
        let p = parse_ai_prompt(&m).unwrap();
        assert!(p.contains("1girl"));
    }

    #[test]
    fn comfy_clip_text() {
        let json = r#"{
          "3": {
            "class_type": "CLIPTextEncode",
            "inputs": { "text": "a majestic mountain landscape at dawn, detailed", "clip": ["4", 0] }
          },
          "6": {
            "class_type": "CLIPTextEncode",
            "inputs": { "text": "blurry, low quality", "clip": ["4", 0] }
          }
        }"#;
        let mut m = HashMap::new();
        m.insert("workflow".into(), json.into());
        let p = parse_ai_prompt(&m).unwrap();
        assert!(p.contains("majestic mountain") || p.contains("blurry"));
    }

    #[test]
    fn garbage_returns_none() {
        let mut m = HashMap::new();
        m.insert("Software".into(), "paint.net".into());
        assert!(parse_ai_prompt(&m).is_none());
    }

    #[test]
    fn truncate_long() {
        let long: String = "x".repeat(5000);
        let t = truncate_comment(&long);
        assert!(t.chars().count() <= MAX_COMMENT_CHARS);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn synthetic_png_text_chunk() {
        // Minimal PNG: signature + tEXt + IEND
        let mut png = PNG_SIG.to_vec();
        // tEXt: keyword "parameters\0" + value
        let mut text_data = b"parameters\0a red fox, detailed fur\nNegative prompt: blur\nSteps: 10".to_vec();
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&(text_data.len() as u32).to_be_bytes());
        chunk.extend_from_slice(b"tEXt");
        chunk.append(&mut text_data);
        chunk.extend_from_slice(&0u32.to_be_bytes()); // fake CRC ok for our parser
        png.extend_from_slice(&chunk);
        // IEND
        png.extend_from_slice(&0u32.to_be_bytes());
        png.extend_from_slice(b"IEND");
        png.extend_from_slice(&0u32.to_be_bytes());

        let mut map = HashMap::new();
        parse_png_text_from_bytes(&png, &mut map);
        assert!(map.contains_key("parameters"));
        let p = parse_ai_prompt(&map).unwrap();
        assert!(p.contains("red fox"));
    }

    #[test]
    fn import_flag_toggle() {
        set_import_enabled(false);
        assert!(!is_import_enabled());
        set_import_enabled(true);
        assert!(is_import_enabled());
    }

    #[test]
    fn jpeg_user_comment_a1111() {
        let raw = "a blue car on a rainy street, detailed\nNegative prompt: blur\nSteps: 20";
        let p = parse_jpeg_prompt_fields(Some(raw), None, &[]).unwrap();
        assert!(p.contains("blue car"));
        assert!(!p.to_ascii_lowercase().contains("negative"));
    }

    #[test]
    fn jpeg_com_marker_prompt() {
        let com = "masterpiece, best quality, 1girl standing in garden";
        let p = parse_jpeg_prompt_fields(None, None, &[com]).unwrap();
        assert!(p.contains("1girl"));
    }

    #[test]
    fn jpeg_image_description_fallback() {
        let desc = "soft portrait lighting, film grain, analog photo look";
        let p = parse_jpeg_prompt_fields(None, Some(desc), &[]).unwrap();
        assert!(p.contains("film grain"));
    }

    #[test]
    fn jpeg_camera_description_ignored() {
        // Short camera-ish description should not be treated as a prompt.
        assert!(parse_jpeg_prompt_fields(None, Some("DSC_0001"), &[]).is_none());
        assert!(parse_jpeg_prompt_fields(None, Some("OLYMPUS"), &[]).is_none());
    }

    #[test]
    fn decode_user_comment_ascii_header() {
        let mut data = b"ASCII\0\0\0".to_vec();
        data.extend_from_slice(b"a red fox in snow, detailed fur\nNegative prompt: blur\nSteps: 10");
        let s = decode_exif_user_comment(&data).unwrap();
        assert!(s.contains("red fox"));
        let p = parse_a1111_parameters(&s).unwrap();
        assert!(p.contains("red fox"));
        assert!(!p.contains("Steps:"));
    }

    #[test]
    fn synthetic_jpeg_com_chunk() {
        // SOI + COM + EOI
        let text = b"sunset over mountains, golden hour light, 4k detail";
        let mut jpeg = JPEG_SIG.to_vec();
        jpeg.push(0xFF);
        jpeg.push(0xFE); // COM
        let len = (text.len() + 2) as u16;
        jpeg.extend_from_slice(&len.to_be_bytes());
        jpeg.extend_from_slice(text);
        jpeg.push(0xFF);
        jpeg.push(0xD9); // EOI
        let mut com = Vec::new();
        collect_jpeg_com_from_bytes(&jpeg, &mut com);
        assert_eq!(com.len(), 1);
        let p = parse_jpeg_prompt_fields(None, None, &[com[0].as_str()]).unwrap();
        assert!(p.contains("sunset"));
    }
}
