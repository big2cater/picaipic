/**
 * AI Engine module
 * Handles ONNX Runtime sessions and model inference.
 */
use crate::t_common;
use image::{DynamicImage, GenericImageView};
use ndarray::{Array, Array4};
use ort::{
    inputs,
    session::{Session, builder::GraphOptimizationLevel},
    value::Value,
};
use reqwest::header::{CONTENT_RANGE, RANGE, USER_AGENT};
use serde::Serialize;
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
use tokenizers::{Tokenizer, TruncationDirection, TruncationParams, TruncationStrategy};
use tokio::io::AsyncWriteExt;

/// CLIP-aligned bilingual text tower max length (bundled int8 + cloud pack).
const MULTILINGUAL_TEXT_MAX_LEN: usize = 128;
/// Product CLIP embedding width (vision + aligned text projection).
const CLIP_EMBED_DIM: usize = 512;

pub struct AiEngine {
    text_model: Option<Session>,
    vision_model: Option<Session>,
    tokenizer: Option<Tokenizer>,
    text_model_kind: ImageSearchTextModel,
}

const AI_INTRA_THREADS: usize = 2;

fn ai_intra_threads() -> usize {
    std::env::var("PICAIPIC_AI_INTRA_THREADS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|&value| (1..=64).contains(&value))
        .unwrap_or(AI_INTRA_THREADS)
}
// CLIP-B/32-aligned bilingual text (Track C) — self-hosted dynamic int8 on picaipic-binaries.
// Asset names on release tag `models` (not the local install filenames).
const MULTILINGUAL_TEXT_MODEL_URL: &str = "https://github.com/big2cater/picaipic-binaries/releases/download/models/clip-vit-b32-multilingual-v1-text-int8.onnx";
const MULTILINGUAL_TOKENIZER_URL: &str = "https://github.com/big2cater/picaipic-binaries/releases/download/models/clip-vit-b32-multilingual-v1-text-tokenizer.json";
const MULTILINGUAL_RELEASE_API_URL: &str =
    "https://api.github.com/repos/big2cater/picaipic-binaries/releases/tags/models";
/// Expected sha256 of self-hosted int8 text tower (Phase 0 quantize_dynamic).
const MULTILINGUAL_TEXT_MODEL_SHA256: &str =
    "50357311fe7b8e06afcaab355e0147291bbc47db869b8ec0671b3e4b2bfa248e";
const MULTILINGUAL_TOKENIZER_SHA256: &str =
    "bf1b59b7b11c95f194f51708d918eea378e09d05f84c0e1656dc5180e8117088";
static MULTILINGUAL_MODEL_DOWNLOAD_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageSearchTextModel {
    Default,
    Multilingual,
}

impl ImageSearchTextModel {
    pub fn from_i64(value: i64) -> Self {
        match value {
            1 => Self::Multilingual,
            _ => Self::Default,
        }
    }

    pub fn as_i64(self) -> i64 {
        match self {
            Self::Default => 0,
            Self::Multilingual => 1,
        }
    }
}

/// Old sentence-embedding packs without CLIP projection (wrong space). Kept for error matching.
pub const ERR_MULTILINGUAL_TEXT_ONLY_DISABLED: &str = "MULTILINGUAL_TEXT_ONLY_DISABLED: text tower is not CLIP-B/32-aligned (missing sentence_embedding 512-d). Stay on default model or re-download the bilingual pack.";

pub fn assert_text_model_activatable(model: ImageSearchTextModel) -> Result<(), String> {
    match model {
        ImageSearchTextModel::Default | ImageSearchTextModel::Multilingual => Ok(()),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSearchModelStatus {
    pub active_model: i64,
    pub multilingual_available: bool,
}

#[derive(Debug, Clone)]
struct TextModelPaths {
    model: PathBuf,
    tokenizer: PathBuf,
}

impl AiEngine {
    pub fn new() -> Self {
        Self {
            text_model: None,
            vision_model: None,
            tokenizer: None,
            text_model_kind: ImageSearchTextModel::Default,
        }
    }

    pub fn load_models(&mut self, app: &AppHandle) -> Result<(), String> {
        if self.text_model.is_some() && self.vision_model.is_some() {
            return Ok(());
        }

        println!("Loading AI Models...");

        let resource_dir = Self::resource_model_dir(app)?;
        let vision_model_path = resource_dir.join(t_common::AI_VISION_MODEL);

        // Load Vision Model
        if self.vision_model.is_none() {
            let vision_model = Self::load_session(&vision_model_path, "vision")?;
            self.vision_model = Some(vision_model);
        }

        if self.text_model.is_none() {
            // Product default: bundled resources text is CLIP-aligned bilingual int8 (EN+CN).
            // Observation period: optional app-data Multilingual re-download may override via settings.
            // Legacy EN-only CLIP text is no longer shipped in resources (kept on picaipic-binaries / probe backup).
            if Self::is_multilingual_model_available(app) {
                match self.set_text_model(app, ImageSearchTextModel::Multilingual) {
                    Ok(()) => println!("Text tower: app-data bilingual (CLIP-aligned)"),
                    Err(e) => {
                        eprintln!(
                            "App-data bilingual text failed ({e}); trying bundled bilingual text"
                        );
                        self.set_text_model(app, ImageSearchTextModel::Default)?;
                        println!("Text tower: bundled bilingual (CLIP-aligned int8)");
                    }
                }
            } else {
                self.set_text_model(app, ImageSearchTextModel::Default)?;
                println!("Text tower: bundled bilingual (CLIP-aligned int8)");
            }
        }

        println!("AI Models Loaded Successfully!");
        Ok(())
    }

    fn load_session(path: &Path, model_name: &str) -> Result<Session, String> {
        Session::builder()
            .map_err(|e| e.to_string())?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| e.to_string())?
            .with_intra_threads(ai_intra_threads())
            .map_err(|e| e.to_string())?
            .commit_from_file(path)
            .map_err(|e| format!("Failed to load {} model from {:?}: {}", model_name, path, e))
    }

    fn resource_model_dir(app: &AppHandle) -> Result<PathBuf, String> {
        #[cfg(debug_assertions)]
        {
            let manifest_dir = env!("CARGO_MANIFEST_DIR");
            let dev_path = std::path::PathBuf::from(manifest_dir).join("resources/models");
            if dev_path.exists() {
                return Ok(dev_path);
            }
        }

        app.path()
            .resolve("models", tauri::path::BaseDirectory::Resource)
            .map_err(|e| format!("Failed to resolve resource path: {}", e))
    }

    /// CLIP-aligned bilingual text pack (no vision). Prefer int8 subdir when present.
    fn multilingual_model_dir(_app: &AppHandle) -> Result<PathBuf, String> {
        crate::t_config::get_app_data_dir().map(|dir| {
            dir.join("models")
                .join("image-search")
                .join("clip-vit-b32-multilingual-v1-text")
        })
    }

    /// Prefer `…-int8` install when both exist (smaller download / Phase 0 preferred).
    fn multilingual_install_dir(app: &AppHandle) -> Result<PathBuf, String> {
        let base = Self::multilingual_model_dir(app)?;
        let int8 = base.with_file_name(format!(
            "{}-int8",
            base.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("clip-vit-b32-multilingual-v1-text")
        ));
        let int8_model = int8.join(t_common::AI_TEXT_MODEL);
        let int8_tok = int8.join(t_common::AI_TOKENIZER);
        if int8_model.is_file() && int8_tok.is_file() {
            return Ok(int8);
        }
        Ok(base)
    }

    fn text_model_paths(
        app: &AppHandle,
        model: ImageSearchTextModel,
    ) -> Result<TextModelPaths, String> {
        let model_dir = match model {
            ImageSearchTextModel::Default => Self::resource_model_dir(app)?,
            ImageSearchTextModel::Multilingual => Self::multilingual_install_dir(app)?,
        };

        Ok(TextModelPaths {
            model: model_dir.join(t_common::AI_TEXT_MODEL),
            tokenizer: model_dir.join(t_common::AI_TOKENIZER),
        })
    }

    pub fn is_multilingual_model_available(app: &AppHandle) -> bool {
        Self::text_model_paths(app, ImageSearchTextModel::Multilingual)
            .map(|paths| paths.model.exists() && paths.tokenizer.exists())
            .unwrap_or(false)
    }

    pub fn model_status(&self, app: &AppHandle) -> ImageSearchModelStatus {
        ImageSearchModelStatus {
            active_model: self.text_model_kind.as_i64(),
            multilingual_available: Self::is_multilingual_model_available(app),
        }
    }

    pub fn set_text_model(
        &mut self,
        app: &AppHandle,
        model: ImageSearchTextModel,
    ) -> Result<(), String> {
        assert_text_model_activatable(model)?;
        if self.text_model.is_some() && self.text_model_kind == model {
            return Ok(());
        }

        let paths = Self::text_model_paths(app, model)?;
        if !paths.model.exists() || !paths.tokenizer.exists() {
            return Err(format!(
                "Image search model files are missing for {:?}",
                model
            ));
        }

        // Bundled Default and Multilingual are both CLIP-aligned bilingual text (int8).
        // Use multilingual max length; sentence_embedding path is required for both.
        let max_len = MULTILINGUAL_TEXT_MAX_LEN;

        let mut tokenizer = Tokenizer::from_file(&paths.tokenizer)
            .map_err(|e| format!("Failed to load tokenizer from {:?}: {}", paths.tokenizer, e))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: max_len,
                strategy: TruncationStrategy::LongestFirst,
                stride: 0,
                direction: TruncationDirection::Right,
            }))
            .map_err(|e| format!("Failed to set tokenizer truncation: {}", e))?;
        let text_model = Self::load_session(&paths.model, "text")?;

        // Require CLIP-aligned 512-d sentence_embedding (not DistilBERT 768 token stream).
        Self::assert_clip_aligned_text_session(&text_model)?;

        // Trial encode smoke; restore previous session on failure.
        let prev_tok = self.tokenizer.take();
        let prev_sess = self.text_model.take();
        let prev_kind = self.text_model_kind;
        self.tokenizer = Some(tokenizer);
        self.text_model = Some(text_model);
        self.text_model_kind = model;

        if let Err(e) = self.smoke_multilingual_text_tower() {
            self.tokenizer = prev_tok;
            self.text_model = prev_sess;
            self.text_model_kind = prev_kind;
            return Err(e);
        }

        Ok(())
    }

    fn assert_clip_aligned_text_session(session: &Session) -> Result<(), String> {
        let has_sentence = session.outputs.iter().any(|o| {
            let n = o.name.to_ascii_lowercase();
            n == "sentence_embedding" || n == "sentence_embeddings" || n == "text_embeds"
        });
        if !has_sentence {
            return Err(format!(
                "{} (no sentence_embedding/text_embeds output)",
                ERR_MULTILINGUAL_TEXT_ONLY_DISABLED
            ));
        }
        Ok(())
    }

    /// Encode short EN+ZH probes and require 512-d finite vectors.
    fn smoke_multilingual_text_tower(&mut self) -> Result<(), String> {
        for q in ["a photo of a bird", "一只鸟"] {
            let emb = self.encode_text(q)?;
            if emb.len() != CLIP_EMBED_DIM {
                return Err(format!(
                    "Multilingual smoke failed: dim {} != {} for query {:?}",
                    emb.len(),
                    CLIP_EMBED_DIM,
                    q
                ));
            }
            if !emb.iter().all(|x| x.is_finite()) {
                return Err(format!(
                    "Multilingual smoke failed: non-finite embedding for {:?}",
                    q
                ));
            }
        }
        Ok(())
    }

    pub fn is_loaded(&self) -> bool {
        self.text_model.is_some() && self.vision_model.is_some() && self.tokenizer.is_some()
    }

    /// Normalize free-text for CLIP: short bare labels → `a photo of a {label}`.
    /// Leaves longer phrases / already-templated prompts / CJK free-text alone.
    pub fn normalize_clip_text_query(text: &str) -> String {
        let t = text.trim();
        if t.is_empty() {
            return String::new();
        }
        let lower = t.to_ascii_lowercase();
        // Already CLIP-style or descriptive English.
        if lower.starts_with("a photo of")
            || lower.starts_with("a close ")
            || lower.starts_with("an image of")
            || lower.starts_with("a picture of")
        {
            return t.to_string();
        }
        // Multi-word / long free text: keep as-is (smart tags already short-templated).
        let word_count = t.split_whitespace().count();
        if word_count > 3 || t.chars().count() > 32 {
            return t.to_string();
        }
        // Bare short EN label (letters/digits/hyphen/space only): wrap.
        let is_simple_latin = t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || c == '-' || c == '\'');
        if is_simple_latin && word_count >= 1 {
            // "a bird" / "an insect" already have an article → "a photo of a bird".
            if lower.starts_with("a ") || lower.starts_with("an ") || lower.starts_with("the ") {
                return format!("a photo of {t}");
            }
            // Bare "bird" / "insect": choose a/an for the first letter.
            let first = lower.chars().next().unwrap_or('x');
            let art = if matches!(first, 'a' | 'e' | 'i' | 'o' | 'u') {
                "an"
            } else {
                "a"
            };
            return format!("a photo of {art} {t}");
        }
        // CJK / mixed: do not invent English wrappers.
        t.to_string()
    }

    pub fn encode_text(&mut self, text: &str) -> Result<Vec<f32>, String> {
        if !self.is_loaded() {
            return Err("AI models not loaded".to_string());
        }

        let text = Self::normalize_clip_text_query(text);
        let tokenizer = self.tokenizer.as_ref().unwrap();
        let encoding = tokenizer
            .encode(text.as_str(), true)
            .map_err(|e| format!("Tokenization error: {}", e))?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        let input_ids_array = Array::from_shape_vec(
            (1, input_ids.len()),
            input_ids.iter().map(|&x| x as i64).collect(),
        )
        .map_err(|e| e.to_string())?;

        let input_ids_value = Value::from_array(input_ids_array).map_err(|e| e.to_string())?;
        let attention_mask_array = Array::from_shape_vec(
            (1, attention_mask.len()),
            attention_mask.iter().map(|&x| x as i64).collect(),
        )
        .map_err(|e| e.to_string())?;
        let attention_mask_value =
            Value::from_array(attention_mask_array).map_err(|e| e.to_string())?;

        let uses_attention_mask = self
            .text_model
            .as_ref()
            .unwrap()
            .inputs
            .iter()
            .any(|input| input.name == "attention_mask");

        let outputs = if uses_attention_mask {
            self.text_model.as_mut().unwrap().run(inputs![
                "input_ids" => input_ids_value,
                "attention_mask" => attention_mask_value,
            ])
        } else {
            self.text_model.as_mut().unwrap().run(inputs![
                "input_ids" => input_ids_value,
            ])
        }
        .map_err(|e| format!("Inference error: {}", e))?;

        // Prefer projected CLIP-space vectors. Never use token_embeddings (often 768).
        let (embedding, first_token_only) = if let Some(vals) = outputs.get("sentence_embedding") {
            (vals, false)
        } else if let Some(vals) = outputs.get("sentence_embeddings") {
            (vals, false)
        } else if let Some(vals) = outputs.get("text_embeds") {
            (vals, false)
        } else if let Some(vals) = outputs.get("pooler_output") {
            (vals, false)
        } else if let Some(vals) = outputs.get("last_hidden_state") {
            (vals, true)
        } else {
            (&outputs[0], true)
        };

        let emb = Self::extract_text_embedding(embedding, first_token_only)?;
        if emb.len() != CLIP_EMBED_DIM {
            return Err(format!(
                "Text embedding dim {} != {} (use sentence_embedding, not token_embeddings)",
                emb.len(),
                CLIP_EMBED_DIM
            ));
        }
        Ok(emb)
    }

    fn extract_text_embedding(
        embedding: &ort::value::DynValue,
        first_token_only: bool,
    ) -> Result<Vec<f32>, String> {
        let (shape, embedding_data) = embedding
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract tensor: {}", e))?;

        if first_token_only && shape.len() >= 3 {
            let hidden_size = shape
                .last()
                .copied()
                .filter(|dim| *dim > 0)
                .ok_or_else(|| format!("Invalid text embedding shape: {}", shape))?
                as usize;
            if embedding_data.len() < hidden_size {
                return Err(format!(
                    "Text embedding data is shorter than shape {}",
                    shape
                ));
            }
            return Ok(embedding_data[..hidden_size].to_vec());
        }

        Ok(embedding_data.to_vec())
    }

    /// Path encode (legacy entry). Prefer `load_image_for_clip_embed` +
    /// `encode_image_from_dynamic` so I/O stays outside the engine lock.
    #[allow(dead_code)]
    pub fn encode_image(&mut self, image_path: &str) -> Result<Vec<f32>, String> {
        if !self.is_loaded() {
            return Err("AI models not loaded".to_string());
        }
        // file_type 1: JPEG scaled / open+cap; orientation default 1
        let (img, _) = crate::t_image::load_image_for_clip_embed(image_path, 1, 1)?;
        self.encode_image_from_dynamic(img)
    }

    pub fn encode_image_from_bytes(&mut self, image_bytes: &[u8]) -> Result<Vec<f32>, String> {
        if !self.is_loaded() {
            return Err("AI models not loaded".to_string());
        }

        let img = image::load_from_memory(image_bytes)
            .map_err(|e| format!("Failed to load image from memory: {}", e))?;
        self.encode_image_from_dynamic(img)
    }

    /// Encode a pre-decoded image (preferred: decode/I/O outside AiEngine mutex).
    pub fn encode_image_from_dynamic(&mut self, img: DynamicImage) -> Result<Vec<f32>, String> {
        if !self.is_loaded() {
            return Err("AI models not loaded".to_string());
        }
        self.encode_images_from_dynamic(vec![img])?
            .into_iter()
            .next()
            .ok_or_else(|| "Vision model returned no embedding".to_string())
    }

    pub fn encode_images_from_dynamic(
        &mut self,
        images: Vec<DynamicImage>,
    ) -> Result<Vec<Vec<f32>>, String> {
        if !self.is_loaded() {
            return Err("AI models not loaded".to_string());
        }
        if images.is_empty() {
            return Ok(Vec::new());
        }
        let batch_size = images.len();
        let image_input = Self::preprocess_dynamic_images(images)?;
        self.encode_preprocessed_images_profiled(image_input, batch_size)
            .map(|(embeddings, _)| embeddings)
    }

    pub(crate) fn encode_preprocessed_images_profiled(
        &mut self,
        image_input: Array4<f32>,
        batch_size: usize,
    ) -> Result<(Vec<Vec<f32>>, Duration), String> {
        if !self.is_loaded() {
            return Err("AI models not loaded".to_string());
        }
        let inference_started = Instant::now();
        let embeddings = self.run_vision_model_batch(image_input, batch_size)?;
        let inference_elapsed = inference_started.elapsed();
        Ok((embeddings, inference_elapsed))
    }

    fn run_vision_model_batch(
        &mut self,
        image_input: Array4<f32>,
        batch_size: usize,
    ) -> Result<Vec<Vec<f32>>, String> {
        let image_input_value = Value::from_array(image_input).map_err(|e| e.to_string())?;

        let outputs = self
            .vision_model
            .as_mut()
            .unwrap()
            .run(inputs![
                "pixel_values" => image_input_value,
            ])
            .map_err(|e| format!("Inference error: {}", e))?;

        let embedding = if let Some(vals) = outputs.get("pooler_output") {
            vals
        } else if let Some(vals) = outputs.get("image_embeds") {
            vals
        } else {
            &outputs[0]
        };

        let (_, embedding_data) = embedding
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract tensor: {}", e))?;

        if embedding_data.len() != batch_size * CLIP_EMBED_DIM {
            return Err(format!(
                "Vision embedding shape has {} values; expected {}",
                embedding_data.len(),
                batch_size * CLIP_EMBED_DIM
            ));
        }
        Ok(embedding_data
            .chunks_exact(CLIP_EMBED_DIM)
            .map(|chunk| chunk.to_vec())
            .collect())
    }

    /// Cap longest edge before the final 224 square (defense in depth if caller
    /// did not already apply `t_common::EMBED_SOURCE_MAX_EDGE`).
    fn downscale_for_embed(img: DynamicImage) -> DynamicImage {
        let max_edge = t_common::EMBED_SOURCE_MAX_EDGE;
        let (w, h) = img.dimensions();
        if w.max(h) <= max_edge {
            return img;
        }
        img.thumbnail(max_edge, max_edge)
    }

    pub(crate) fn preprocess_dynamic_images(
        images: Vec<DynamicImage>,
    ) -> Result<Array4<f32>, String> {
        let mean = [0.48145466, 0.4578275, 0.40821073];
        let std = [0.26862954, 0.26130258, 0.27577711];
        let mut array = Array::zeros((images.len(), 3, 224, 224));
        let plane_len = 224 * 224;
        let data = array
            .as_slice_mut()
            .ok_or_else(|| "CLIP input array is not contiguous".to_string())?;
        for (batch, source) in images.into_iter().enumerate() {
            let img = Self::downscale_for_embed(source)
                .resize_exact(224, 224, image::imageops::FilterType::Triangle)
                .to_rgb8();
            let batch_offset = batch * 3 * plane_len;
            for (index, pixel) in img.pixels().enumerate() {
                data[batch_offset + index] = (pixel[0] as f32 / 255.0 - mean[0]) / std[0];
                data[batch_offset + plane_len + index] =
                    (pixel[1] as f32 / 255.0 - mean[1]) / std[1];
                data[batch_offset + 2 * plane_len + index] =
                    (pixel[2] as f32 / 255.0 - mean[2]) / std[2];
            }
        }
        Ok(array)
    }
}

#[cfg(test)]
mod image_preprocess_tests {
    use super::AiEngine;
    use image::{DynamicImage, Rgb, RgbImage};

    #[test]
    fn batch_preprocess_keeps_nchw_channel_layout() {
        let red = DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 2, Rgb([255, 0, 0])));
        let green = DynamicImage::ImageRgb8(RgbImage::from_pixel(2, 4, Rgb([0, 255, 0])));
        let input = AiEngine::preprocess_dynamic_images(vec![red, green]).unwrap();

        assert_eq!(input.shape(), &[2, 3, 224, 224]);
        assert!((input[[0, 0, 0, 0]] - ((1.0 - 0.48145466) / 0.26862954)).abs() < 1e-6);
        assert!((input[[0, 1, 0, 0]] - ((0.0 - 0.4578275) / 0.26130258)).abs() < 1e-6);
        assert!((input[[1, 1, 223, 223]] - ((1.0 - 0.4578275) / 0.26130258)).abs() < 1e-6);
        assert!((input[[1, 2, 223, 223]] - ((0.0 - 0.40821073) / 0.27577711)).abs() < 1e-6);
    }
}

pub struct AiState(pub Mutex<AiEngine>);

async fn get_remote_file_size(client: &reqwest::Client, url: &str) -> Option<u64> {
    let response = client
        .get(url)
        .header(RANGE, "bytes=0-0")
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;

    if let Some(content_range) = response.headers().get(CONTENT_RANGE) {
        let content_range = content_range.to_str().ok()?;
        if let Some((_, total)) = content_range.rsplit_once('/') {
            if total != "*" {
                return total.parse::<u64>().ok();
            }
        }
    }

    response.content_length()
}

async fn get_release_asset_total_size(
    client: &reqwest::Client,
    files: &[(&str, &str, &str)],
) -> Option<u64> {
    let response = client
        .get(MULTILINGUAL_RELEASE_API_URL)
        .header(USER_AGENT, "PicAiPic")
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&response.text().await.ok()?).ok()?;
    let assets = value.get("assets")?.as_array()?;
    let mut total_size = 0u64;

    // Match release asset names from URL path (self-hosted names differ from install filenames).
    for (url, _install_name, _) in files {
        let asset_name = url.rsplit('/').next()?;
        let asset = assets
            .iter()
            .find(|asset| asset.get("name").and_then(|name| name.as_str()) == Some(asset_name))?;
        total_size += asset.get("size")?.as_u64()?;
    }

    Some(total_size)
}

fn sha256_hex_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 256];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_sha256(path: &Path, expected_hex: &str, label: &str) -> Result<(), String> {
    let got = sha256_hex_file(path)?;
    if !got.eq_ignore_ascii_case(expected_hex) {
        return Err(format!(
            "SHA-256 mismatch for {label}: expected {expected_hex}, got {got}"
        ));
    }
    Ok(())
}

async fn get_download_total_size(client: &reqwest::Client, files: &[(&str, &str, &str)]) -> u64 {
    if let Some(total_size) = get_release_asset_total_size(client, files).await {
        return total_size;
    }

    let mut total_size = 0u64;
    for (url, _, _) in files.iter() {
        match get_remote_file_size(client, url).await {
            Some(file_size) => total_size += file_size,
            None => return 0,
        }
    }
    total_size
}

fn is_current_multilingual_download(download_id: u64) -> bool {
    MULTILINGUAL_MODEL_DOWNLOAD_ID.load(Ordering::SeqCst) == download_id
}

fn ensure_current_multilingual_download(download_id: u64, temp_dir: &Path) -> Result<(), String> {
    if is_current_multilingual_download(download_id) {
        return Ok(());
    }

    let _ = std::fs::remove_dir_all(temp_dir);
    Err("Download canceled".to_string())
}

async fn clean_multilingual_download_temp_dirs(model_dir: &Path) {
    let Some(parent) = model_dir.parent() else {
        return;
    };
    let Some(model_name) = model_dir.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    let temp_prefix = format!("{}.download", model_name);
    let Ok(mut entries) = tokio::fs::read_dir(parent).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let should_remove = entry
            .file_name()
            .to_str()
            .map(|name| name == temp_prefix || name.starts_with(&format!("{}.", temp_prefix)))
            .unwrap_or(false);
        if should_remove {
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
}

pub async fn download_multilingual_text_model(app: AppHandle) -> Result<(), String> {
    let download_id = MULTILINGUAL_MODEL_DOWNLOAD_ID.fetch_add(1, Ordering::SeqCst) + 1;
    // Self-hosted pack is dynamic int8 — install under the -int8 directory.
    let base = AiEngine::multilingual_model_dir(&app)?;
    let model_dir = base.with_file_name(format!(
        "{}-int8",
        base.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("clip-vit-b32-multilingual-v1-text")
    ));
    clean_multilingual_download_temp_dirs(&model_dir).await;
    let temp_dir = model_dir.with_extension(format!("download.{}", download_id));
    match tokio::fs::remove_dir_all(&temp_dir).await {
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(format!("Failed to clean temporary download files: {}", e)),
    }
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| e.to_string())?;

    let files = [
        (
            MULTILINGUAL_TEXT_MODEL_URL,
            t_common::AI_TEXT_MODEL,
            "text_model",
        ),
        (
            MULTILINGUAL_TOKENIZER_URL,
            t_common::AI_TOKENIZER,
            "tokenizer",
        ),
    ];
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| format!("Failed to create download client: {}", e))?;
    let total_files = files.len() as f64;
    let mut downloaded_total = 0u64;
    let expected_total = get_download_total_size(&client, &files).await;
    ensure_current_multilingual_download(download_id, &temp_dir)?;

    let _ = app.emit(
        "image_search_model_download_progress",
        serde_json::json!({
            "progress": 0,
            "downloadedBytes": 0,
            "totalBytes": expected_total,
            "downloadId": download_id,
            "file": "start",
        }),
    );

    for (index, (url, filename, label)) in files.iter().enumerate() {
        ensure_current_multilingual_download(download_id, &temp_dir)?;
        let response = client
            .get(*url)
            .send()
            .await
            .map_err(|e| format!("Failed to download {}: {}", filename, e))?
            .error_for_status()
            .map_err(|e| format!("Failed to download {}: {}", filename, e))?;

        let path = temp_dir.join(filename);
        let mut file = tokio::fs::File::create(&path)
            .await
            .map_err(|e| e.to_string())?;
        let content_length = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;
        let mut response = response;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("Failed to read {}: {}", filename, e))?
        {
            if chunk.is_empty() {
                continue;
            }
            ensure_current_multilingual_download(download_id, &temp_dir)?;
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
            downloaded += chunk.len() as u64;
            downloaded_total += chunk.len() as u64;

            let file_progress = if content_length > 0 {
                (downloaded as f64 / content_length as f64).min(1.0)
            } else {
                0.0
            };
            let progress = if expected_total > 0 {
                ((downloaded_total as f64 / expected_total as f64).min(1.0) * 100.0).round() as i64
            } else {
                (((index as f64 + file_progress) / total_files) * 100.0).round() as i64
            };
            let _ = app.emit(
                "image_search_model_download_progress",
                serde_json::json!({
                    "progress": progress,
                    "downloadedBytes": downloaded_total,
                    "totalBytes": expected_total,
                    "downloadId": download_id,
                    "file": label,
                }),
            );
        }
        file.flush().await.map_err(|e| e.to_string())?;
        if downloaded == 0 {
            return Err(format!("Downloaded {} is empty", filename));
        }

        let progress = if expected_total > 0 {
            ((downloaded_total as f64 / expected_total as f64).min(1.0) * 100.0).round() as i64
        } else {
            ((((index + 1) as f64) / total_files) * 100.0).round() as i64
        };
        let _ = app.emit(
            "image_search_model_download_progress",
            serde_json::json!({
                "progress": progress,
                "downloadedBytes": downloaded_total,
                "totalBytes": expected_total,
                "downloadId": download_id,
                "file": label,
            }),
        );
    }

    ensure_current_multilingual_download(download_id, &temp_dir)?;
    let temp_text_model_path = temp_dir.join(t_common::AI_TEXT_MODEL);
    let temp_tokenizer_path = temp_dir.join(t_common::AI_TOKENIZER);
    if let Err(e) = verify_sha256(
        &temp_text_model_path,
        MULTILINGUAL_TEXT_MODEL_SHA256,
        "text_model.onnx",
    ) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(e);
    }
    if let Err(e) = verify_sha256(
        &temp_tokenizer_path,
        MULTILINGUAL_TOKENIZER_SHA256,
        "tokenizer.json",
    ) {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(e);
    }
    Tokenizer::from_file(&temp_tokenizer_path).map_err(|e| {
        let _ = std::fs::remove_dir_all(&temp_dir);
        format!("Downloaded tokenizer is invalid: {}", e)
    })?;
    if let Err(e) = AiEngine::load_session(&temp_text_model_path, "text") {
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(format!("Downloaded text model is invalid: {}", e));
    }

    tokio::fs::create_dir_all(&model_dir)
        .await
        .map_err(|e| e.to_string())?;
    ensure_current_multilingual_download(download_id, &temp_dir)?;
    for (_, filename, _) in files {
        let dest = model_dir.join(filename);
        let temp = temp_dir.join(filename);
        let _ = tokio::fs::remove_file(&dest).await;
        tokio::fs::rename(temp, dest)
            .await
            .map_err(|e| e.to_string())?;
    }
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    let _ = app.emit(
        "image_search_model_download_progress",
        serde_json::json!({
            "progress": 100,
            "downloadedBytes": downloaded_total,
            "totalBytes": expected_total,
            "downloadId": download_id,
            "file": "complete",
        }),
    );

    Ok(())
}

pub async fn cancel_multilingual_text_model_download(app: AppHandle) -> Result<(), String> {
    MULTILINGUAL_MODEL_DOWNLOAD_ID.fetch_add(1, Ordering::SeqCst);
    let model_dir = AiEngine::multilingual_model_dir(&app)?;
    clean_multilingual_download_temp_dirs(&model_dir).await;
    Ok(())
}

#[cfg(test)]
mod multilingual_gate_tests {
    use super::*;

    #[test]
    fn default_text_model_is_allowed() {
        assert!(assert_text_model_activatable(ImageSearchTextModel::Default).is_ok());
    }

    #[test]
    fn multilingual_aligned_is_allowed_at_gate() {
        // File/session smoke still required at set_text_model; gate only allows the variant.
        assert!(assert_text_model_activatable(ImageSearchTextModel::Multilingual).is_ok());
    }
}
