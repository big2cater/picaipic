/**
 * Face Recognition module
 * Handles face detection (InsightFace SCRFD det_500m) and embedding (MobileFaceNet)
 * using ONNX Runtime.
 */
use crate::{t_cluster, t_common, t_sqlite};
use image::DynamicImage;
use ndarray::Array;
use ort::{
    inputs,
    session::{Session, builder::GraphOptimizationLevel},
    value::Value,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

// cancellation token for face indexing
#[derive(Clone)]
pub struct FaceIndexCancellation(pub Arc<Mutex<bool>>);

// detailed status for face indexing
#[derive(Clone)]
pub struct FaceIndexingStatus(pub Arc<Mutex<bool>>);

// face indexing progress
#[derive(Clone, serde::Serialize)]
pub struct FaceIndexProgress {
    pub current: usize,
    pub total: usize,
    pub faces_found: usize,
    pub phase: String,
}

#[derive(Clone)]
pub struct FaceIndexProgressState(pub Arc<Mutex<FaceIndexProgress>>);

// face stats
#[derive(Clone, serde::Serialize)]
pub struct FaceStats {
    pub total: usize,
    pub processed: usize,
    pub unprocessed: usize,
    pub faces: usize,
}

/// Detected face bounding box and landmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub confidence: f32,
    pub landmarks: Option<Vec<(f32, f32)>>, // 5 facial landmarks
}

/// Face with embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceData {
    pub bbox: FaceBox,
    pub embedding: Vec<f32>,
}

struct Anchor {
    cx: f32,
    cy: f32,
}

pub struct FaceEngine {
    detection_model: Option<Session>, // InsightFace SCRFD det_500m
    embedding_model: Option<Session>, // MobileFaceNet
}

impl FaceEngine {
    pub fn new() -> Self {
        Self {
            detection_model: None,
            embedding_model: None,
        }
    }

    pub fn load_models(&mut self, app: &AppHandle) -> Result<(), String> {
        self.load_models_with_threads(app, 4)
    }

    /// Resolve ONNX model paths under the app resource `models/` directory.
    pub fn resolve_model_paths(
        app: &AppHandle,
    ) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
        let resource_dir = app
            .path()
            .resolve("models", tauri::path::BaseDirectory::Resource)
            .map_err(|e| format!("Failed to resolve resource path: {}", e))?;
        let detection_model_path = resource_dir.join(t_common::DETECTION_MODEL);
        let embedding_model_path = resource_dir.join(t_common::EMBEDDING_MODEL);
        if !detection_model_path.exists() {
            return Err(format!(
                "Detection model not found at {:?}",
                detection_model_path
            ));
        }
        if !embedding_model_path.exists() {
            return Err(format!(
                "Embedding model not found at {:?}",
                embedding_model_path
            ));
        }
        Ok((detection_model_path, embedding_model_path))
    }

    pub fn load_models_with_threads(
        &mut self,
        app: &AppHandle,
        intra_threads: usize,
    ) -> Result<(), String> {
        let (detection_model_path, embedding_model_path) = Self::resolve_model_paths(app)?;
        self.load_models_from_paths(&detection_model_path, &embedding_model_path, intra_threads)
    }

    /// Load sessions from explicit paths (used by parallel index workers).
    pub fn load_models_from_paths(
        &mut self,
        detection_model_path: &std::path::Path,
        embedding_model_path: &std::path::Path,
        intra_threads: usize,
    ) -> Result<(), String> {
        if self.detection_model.is_some() && self.embedding_model.is_some() {
            return Ok(());
        }

        let threads = intra_threads.max(1);

        // Load Detection Model (InsightFace SCRFD det_500m)
        let detection_model = Session::builder()
            .map_err(|e| e.to_string())?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| e.to_string())?
            .with_intra_threads(threads)
            .map_err(|e| e.to_string())?
            .commit_from_file(detection_model_path)
            .map_err(|e| format!("Failed to load detection model: {}", e))?;

        self.detection_model = Some(detection_model);

        // Load Embedding Model (MobileFaceNet)
        let embedding_model = Session::builder()
            .map_err(|e| e.to_string())?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| e.to_string())?
            .with_intra_threads(threads)
            .map_err(|e| e.to_string())?
            .commit_from_file(embedding_model_path)
            .map_err(|e| format!("Failed to load embedding model: {}", e))?;

        self.embedding_model = Some(embedding_model);

        Ok(())
    }

    pub fn is_loaded(&self) -> bool {
        self.detection_model.is_some() && self.embedding_model.is_some()
    }

    /// Detect faces implementation (from DynamicImage)
    fn detect_faces(&mut self, img: &DynamicImage) -> Result<Vec<FaceBox>, String> {
        let original_width = img.width() as f32;
        let original_height = img.height() as f32;

        // SCRFD (det_500m) expects a square letterboxed input; 640 is the training size.
        // Optimization: For small images (like thumbnails ~512px), use their native size slightly rounded up.
        // For large images, downscale to 640px max dimension.
        let max_dim = original_width.max(original_height);
        let target_size = if max_dim < 640.0 {
            // Round up to nearest multiple of 32
            ((max_dim as u32 + 31) / 32) * 32
        } else {
            640
        };
        // Resize preserving aspect ratio (Letterbox)
        // Use max dimension to fit within target
        let scale = (target_size as f32) / original_width.max(original_height);
        // Use round() to minimize truncation error
        let new_w = (original_width * scale).round() as u32;
        let new_h = (original_height * scale).round() as u32;

        let rgb_buf; // Owned buffer if needed
        let rgb_img = if new_w == img.width() && new_h == img.height() {
            // Optimization: Skip resize if unnecessary
            if let Some(buf) = img.as_rgb8() {
                buf
            } else {
                rgb_buf = img.to_rgb8();
                &rgb_buf
            }
        } else {
            rgb_buf = img
                .resize_exact(new_w, new_h, image::imageops::FilterType::Triangle)
                .into_rgb8();
            &rgb_buf
        };

        // InsightFace SCRFD preprocessing aligns to Top-Left (0,0)

        // Normalize: (pixel - 127.5) / 128.0
        // Initialize with zeros (padding)
        let mut array = Array::zeros((1, 3, target_size as usize, target_size as usize));

        if let Some(slice) = array.as_slice_mut() {
            let area = (target_size as usize) * (target_size as usize);
            let offset_b = 0;
            let offset_g = area;
            let offset_r = area * 2;
            let target_w = target_size as usize;

            for (x, y, pixel) in rgb_img.enumerate_pixels() {
                let r = (pixel[0] as f32 - 127.5) / 128.0;
                let g = (pixel[1] as f32 - 127.5) / 128.0;
                let b = (pixel[2] as f32 - 127.5) / 128.0;

                let idx = (y as usize) * target_w + (x as usize);

                slice[offset_b + idx] = b;
                slice[offset_g + idx] = g;
                slice[offset_r + idx] = r;
            }
        } else {
            // Fallback if array is not contiguous (should not happen with default init)
            for (x, y, pixel) in rgb_img.enumerate_pixels() {
                let r = (pixel[0] as f32 - 127.5) / 128.0;
                let g = (pixel[1] as f32 - 127.5) / 128.0;
                let b = (pixel[2] as f32 - 127.5) / 128.0;

                array[[0, 0, y as usize, x as usize]] = b; // Blue
                array[[0, 1, y as usize, x as usize]] = g; // Green
                array[[0, 2, y as usize, x as usize]] = r; // Red
            }
        }

        let input_value = Value::from_array(array).map_err(|e| e.to_string())?;

        // Use block scope to ensure outputs is dropped before calling nms
        let mut faces = {
            let outputs = self
                .detection_model
                .as_mut()
                .ok_or_else(|| "Face detection model is not loaded".to_string())?
                .run(inputs!["input.1" => input_value])
                .map_err(|e| format!("Detection inference error: {}", e))?;

            let mut all_detections = Vec::new();
            // SCRFD multi-level FPN: strides 8/16/32, two scales per cell.
            // Bundled det_500m.onnx outputs (verified): scores [N,1], boxes [N,4], landmarks [N,10].
            let strides = [8, 16, 32];
            let min_sizes = [[16, 32], [64, 128], [256, 512]];

            // Map output indices based on model export order
            // Scores, Boxes, Landmarks indices per stride
            let indices = [
                (0, 3, 6), // Stride 8
                (1, 4, 7), // Stride 16
                (2, 5, 8), // Stride 32
            ];

            if outputs.len() < 9 {
                return Err(format!(
                    "Unexpected detection model outputs: expected >= 9 tensors (SCRFD), got {}",
                    outputs.len()
                ));
            }

            let confidence_threshold = 0.6;

            for (i, &stride) in strides.iter().enumerate() {
                let (score_idx, box_idx, _) = indices[i];

                let scores_tensor = &outputs[score_idx];
                let boxes_tensor = &outputs[box_idx];

                let (scores_shape, scores_data) = scores_tensor
                    .try_extract_tensor::<f32>()
                    .map_err(|e| format!("Failed stride {} scores: {}", stride, e))?;
                let (boxes_shape, boxes_data) = boxes_tensor
                    .try_extract_tensor::<f32>()
                    .map_err(|e| format!("Failed stride {} boxes: {}", stride, e))?;

                let feature_map_w = target_size / stride;
                let feature_map_h = target_size / stride;
                let anchors =
                    Self::generate_anchors(stride, &min_sizes[i], feature_map_w, feature_map_h);

                // Guard against wrong architecture (e.g. classic RetinaFace class scores).
                let score_count = scores_data.len();
                let box_count = boxes_data.len() / 4;
                if score_count != anchors.len() || box_count != anchors.len() {
                    return Err(format!(
                        "Detection tensor/anchor mismatch at stride {}: scores_shape={:?} ({}), boxes_shape={:?} ({}), anchors={}",
                        stride,
                        scores_shape.to_vec(),
                        score_count,
                        boxes_shape.to_vec(),
                        box_count,
                        anchors.len()
                    ));
                }

                for (j, anchor) in anchors.iter().enumerate() {
                    // SCRFD face score is a single channel per anchor (shape [N,1]).
                    let score = scores_data[j];
                    if score < confidence_threshold {
                        continue;
                    }

                    // Decode SCRFD distances [l, t, r, b] from anchor center, scaled by stride.
                    let l = boxes_data[j * 4];
                    let t = boxes_data[j * 4 + 1];
                    let r = boxes_data[j * 4 + 2];
                    let b = boxes_data[j * 4 + 3];

                    // x1 = cx - l * stride
                    // y1 = cy - t * stride
                    // x2 = cx + r * stride
                    // y2 = cy + b * stride

                    let x1 = anchor.cx - l * stride as f32;
                    let y1 = anchor.cy - t * stride as f32;
                    let x2 = anchor.cx + r * stride as f32;
                    let y2 = anchor.cy + b * stride as f32;

                    // Scale back to original image
                    // Use effective scale factors derived from actual resized dimensions
                    let inv_scale_x = original_width / new_w as f32;
                    let inv_scale_y = original_height / new_h as f32;

                    // Scale directly (no padding offset)
                    let original_x1 = x1 * inv_scale_x;
                    let original_y1 = y1 * inv_scale_y;
                    let original_x2 = x2 * inv_scale_x;
                    let original_y2 = y2 * inv_scale_y;

                    all_detections.push(FaceBox {
                        x: original_x1,
                        y: original_y1,
                        width: original_x2 - original_x1,
                        height: original_y2 - original_y1,
                        confidence: score,
                        landmarks: None,
                    });
                }
            }

            all_detections
        };

        // Non-maximum suppression
        faces = self.nms(faces, 0.4);

        if faces.is_empty() {
            // No faces found after NMS
        }

        Ok(faces)
    }

    /// Generate anchors for a specific stride
    fn generate_anchors(
        stride: u32,
        min_sizes: &[u32],
        feature_w: u32,
        feature_h: u32,
    ) -> Vec<Anchor> {
        let mut anchors =
            Vec::with_capacity((feature_w * feature_h * min_sizes.len() as u32) as usize);

        for y in 0..feature_h {
            for x in 0..feature_w {
                for &_min_size in min_sizes {
                    // SCRFD dense anchors: cell origin (x*stride, y*stride), not (x+0.5)*stride.
                    // (Classic RetinaFace centers differ — do not "fix" this without re-checking det_500m.)
                    let cx = (x as f32) * stride as f32;
                    let cy = (y as f32) * stride as f32;

                    anchors.push(Anchor { cx, cy });
                }
            }
        }
        anchors
    }

    /// Get face embedding implementation (from DynamicImage)
    fn get_face_embedding(
        &mut self,
        img: &DynamicImage,
        bbox: &FaceBox,
    ) -> Result<Vec<f32>, String> {
        // Crop face region with some padding
        let padding = 0.2;
        let x = (bbox.x - bbox.width * padding).max(0.0) as u32;
        let y = (bbox.y - bbox.height * padding).max(0.0) as u32;
        let w = (bbox.width * (1.0 + 2.0 * padding)) as u32;
        let h = (bbox.height * (1.0 + 2.0 * padding)) as u32;

        let max_x = (x + w).min(img.width());
        let max_y = (y + h).min(img.height());

        // Optimize: check if we can reuse the crop or if we need to resize
        // MobileFaceNet expects 112x112
        let target_size = 112;
        let face_crop = img.crop_imm(x, y, max_x - x, max_y - y);
        let rgb_buf;
        let rgb_face = if face_crop.width() == target_size && face_crop.height() == target_size {
            if let Some(buf) = face_crop.as_rgb8() {
                buf
            } else {
                rgb_buf = face_crop.to_rgb8();
                &rgb_buf
            }
        } else {
            rgb_buf = face_crop
                .resize_exact(
                    target_size,
                    target_size,
                    image::imageops::FilterType::Triangle,
                )
                .into_rgb8();
            &rgb_buf
        };

        // Normalize: (pixel - 127.5) / 128.0
        let mut array = Array::zeros((1, 3, 112, 112));

        // Optimize: use slice access
        if let Some(slice) = array.as_slice_mut() {
            let area = 112 * 112;
            let offset_g = area;
            let offset_b = area * 2;
            let width = 112;

            for (x, y, pixel) in rgb_face.enumerate_pixels() {
                let r = (pixel[0] as f32 - 127.5) / 128.0;
                let g = (pixel[1] as f32 - 127.5) / 128.0;
                let b = (pixel[2] as f32 - 127.5) / 128.0;

                let idx = (y as usize) * width + (x as usize);

                slice[idx] = r;
                slice[offset_g + idx] = g;
                slice[offset_b + idx] = b;
            }
        } else {
            for (fx, fy, pixel) in rgb_face.enumerate_pixels() {
                let r = (pixel[0] as f32 - 127.5) / 128.0;
                let g = (pixel[1] as f32 - 127.5) / 128.0;
                let b = (pixel[2] as f32 - 127.5) / 128.0;

                array[[0, 0, fy as usize, fx as usize]] = r;
                array[[0, 1, fy as usize, fx as usize]] = g;
                array[[0, 2, fy as usize, fx as usize]] = b;
            }
        }

        let input_value = Value::from_array(array).map_err(|e| e.to_string())?;

        let outputs = self
            .embedding_model
            .as_mut()
            .ok_or_else(|| "Face embedding model is not loaded".to_string())?
            .run(inputs!["input.1" => input_value])
            .map_err(|e| format!("Embedding inference error: {}", e))?;

        let embedding = &outputs[0];
        let (_, embedding_data) = embedding
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract embedding: {}", e))?;

        // Normalize embedding to unit vector
        let emb_vec = embedding_data.to_vec();
        let norm: f32 = emb_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if !norm.is_finite() || norm <= f32::EPSILON {
            return Err("Invalid face embedding norm".to_string());
        }
        let normalized: Vec<f32> = emb_vec.iter().map(|x| x / norm).collect();

        Ok(normalized)
    }

    /// Compute cosine similarity between two embeddings
    #[allow(dead_code)]
    pub fn compare_faces(emb1: &[f32], emb2: &[f32]) -> f32 {
        if emb1.len() != emb2.len() {
            return 0.0;
        }
        // Embeddings are already normalized, so dot product = cosine similarity
        emb1.iter().zip(emb2.iter()).map(|(a, b)| a * b).sum()
    }

    /// Process image: detect all faces and get embeddings
    /// Filters out low-quality faces (low confidence, small size, blurry)
    pub fn process_image(
        &mut self,
        image_path: &str,
    ) -> Result<(Vec<FaceData>, (u32, u32)), String> {
        let img = image::open(image_path).map_err(|e| format!("Failed to open image: {}", e))?;
        self.process_dynamic_image(&img)
    }

    pub fn process_image_from_bytes(
        &mut self,
        image_bytes: &[u8],
    ) -> Result<(Vec<FaceData>, (u32, u32)), String> {
        let img = image::load_from_memory(image_bytes)
            .map_err(|e| format!("Failed to load image from memory: {}", e))?;
        self.process_dynamic_image(&img)
    }

    fn process_dynamic_image(
        &mut self,
        img: &DynamicImage,
    ) -> Result<(Vec<FaceData>, (u32, u32)), String> {
        let faces = self.detect_faces(img)?;

        let mut results = Vec::new();
        for face in faces {
            // Filter 1: Skip low confidence faces
            if face.confidence < t_common::MIN_CONFIDENCE {
                continue;
            }

            // Filter 2: Skip very small faces (likely background people)
            // let face_area = face.width * face.height;
            // let img_width = img.width() as f32;
            // let img_height = img.height() as f32;
            // let img_area = img_width * img_height;
            // if face_area / img_area < t_common::MIN_FACE_RATIO {
            //     continue;
            // }

            // Filter 3: Skip faces smaller than minimum pixel size
            // if face.width < t_common::MIN_FACE_SIZE || face.height < t_common::MIN_FACE_SIZE {
            //     continue;
            // }

            // Filter 4: Skip blurry faces
            let blur_score = self.calculate_blur_score(img, &face);
            if blur_score < t_common::MIN_BLUR_SCORE {
                continue;
            }

            // Get embedding for quality face
            let embedding = self.get_face_embedding(img, &face)?;
            results.push(FaceData {
                bbox: face,
                embedding,
            });
        }

        Ok((results, (img.width(), img.height())))
    }

    /// Calculate blur score using Variance of Laplacian
    /// Optimized: Uses Welford's online algorithm to avoid allocating a large vector
    fn calculate_blur_score(&self, img: &DynamicImage, bbox: &FaceBox) -> f32 {
        let x = bbox.x.max(0.0) as u32;
        let y = bbox.y.max(0.0) as u32;
        // Check bounds to ensure we don't crash on cropping
        let w = bbox.width.min(img.width() as f32 - bbox.x) as u32;
        let h = bbox.height.min(img.height() as f32 - bbox.y) as u32;

        if w < 3 || h < 3 {
            return 0.0;
        }

        let crop = img.crop_imm(x, y, w, h).to_luma8();
        let (width, height) = crop.dimensions();

        // Online variance calculation (Welford's algorithm)
        let mut count = 0usize;
        let mut m2 = 0.0;
        let mut mean = 0.0;

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let p = crop.get_pixel(x, y).0[0] as i16;
                let top = crop.get_pixel(x, y - 1).0[0] as i16;
                let bottom = crop.get_pixel(x, y + 1).0[0] as i16;
                let left = crop.get_pixel(x - 1, y).0[0] as i16;
                let right = crop.get_pixel(x + 1, y).0[0] as i16;

                let sum = top + bottom + left + right - 4 * p;
                let val = sum as f32;

                count += 1;
                let delta = val - mean;
                mean += delta / count as f32;
                let delta2 = val - mean;
                m2 += delta * delta2;
            }
        }

        if count < 2 {
            return 0.0;
        }

        // Variance
        m2 / (count as f32)
    }

    /// Non-maximum suppression
    fn nms(&self, mut boxes: Vec<FaceBox>, iou_threshold: f32) -> Vec<FaceBox> {
        boxes.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));

        let mut keep = Vec::new();
        let mut suppressed = vec![false; boxes.len()];

        for i in 0..boxes.len() {
            if suppressed[i] {
                continue;
            }
            keep.push(boxes[i].clone());

            for j in (i + 1)..boxes.len() {
                if suppressed[j] {
                    continue;
                }
                if self.iou(&boxes[i], &boxes[j]) > iou_threshold {
                    suppressed[j] = true;
                }
            }
        }

        keep
    }

    /// Intersection over Union
    /// Optimized: Simplified redundant max(0.0) for valid boxes
    fn iou(&self, a: &FaceBox, b: &FaceBox) -> f32 {
        let x1 = a.x.max(b.x);
        let y1 = a.y.max(b.y);
        let x2 = (a.x + a.width).min(b.x + b.width);
        let y2 = (a.y + a.height).min(b.y + b.height);

        if x2 <= x1 || y2 <= y1 {
            return 0.0;
        }

        let inter_area = (x2 - x1) * (y2 - y1);
        let a_area = a.width * a.height;
        let b_area = b.width * b.height;

        inter_area / (a_area + b_area - inter_area)
    }
}

#[derive(Clone)]
pub struct FaceState(pub std::sync::Arc<Mutex<FaceEngine>>);

/// Parallel face-index worker count: ~half of logical cores, clamped 2–4.
/// Keeps per-session intra-threads low so total ONNX threads stay bounded.
fn face_index_worker_count() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    (cores / 2).clamp(2, 4)
}

type FaceIndexJob = (i64, String, i64, i64);

/// Multi-consumer job queue (std mpsc Receiver is not Clone / multi-consumer).
struct FaceJobQueue {
    inner: Mutex<FaceJobQueueInner>,
    cvar: std::sync::Condvar,
}

struct FaceJobQueueInner {
    jobs: std::collections::VecDeque<FaceIndexJob>,
    closed: bool,
}

impl FaceJobQueue {
    fn new() -> Self {
        Self {
            inner: Mutex::new(FaceJobQueueInner {
                jobs: std::collections::VecDeque::new(),
                closed: false,
            }),
            cvar: std::sync::Condvar::new(),
        }
    }

    fn push(&self, job: FaceIndexJob) {
        let mut g = t_common::lock_mutex(&self.inner);
        if g.closed {
            return;
        }
        g.jobs.push_back(job);
        self.cvar.notify_one();
    }

    fn close(&self) {
        let mut g = t_common::lock_mutex(&self.inner);
        g.closed = true;
        self.cvar.notify_all();
    }

    /// Block until a job is available, or return None when closed and drained.
    fn pop(&self) -> Option<FaceIndexJob> {
        let mut g = t_common::lock_mutex(&self.inner);
        loop {
            if let Some(job) = g.jobs.pop_front() {
                return Some(job);
            }
            if g.closed {
                return None;
            }
            g = self.cvar.wait(g).unwrap_or_else(|e| e.into_inner());
        }
    }
}

struct FaceIndexWorkerResult {
    #[allow(dead_code)]
    file_id: i64,
    #[allow(dead_code)]
    file_path: String,
    /// None = inference failed (leave unscanned for retry).
    write: Option<(i64, i32, Vec<(String, Vec<f32>)>)>,
}

pub fn run_face_indexing(
    app_handle: AppHandle,
    face_state: FaceState,
    cancel_token_struct: FaceIndexCancellation,
    status_token_struct: FaceIndexingStatus,
    progress_token_struct: FaceIndexProgressState,
    cluster_epsilon: Option<f32>,
    cluster_mode: Option<String>,
) -> Result<(), String> {
    let cancel_token = cancel_token_struct.0.clone();
    let status_token = status_token_struct.0.clone();
    let progress_token = progress_token_struct.0.clone();
    // Use provided epsilon or default to 0.42
    let epsilon = cluster_epsilon.unwrap_or(0.42);
    let mode = t_cluster::ClusterMode::parse(cluster_mode.as_deref());

    // Check if already running
    {
        let mut running = t_common::lock_mutex(&status_token);
        if *running {
            return Err("Face indexing is already running".to_string());
        }
        *running = true;
    }

    // Reset cancellation flag
    *t_common::lock_mutex(&cancel_token) = false;

    // Reset progress
    {
        let mut progress = t_common::lock_mutex(&progress_token);
        progress.current = 0;
        progress.total = 0;
        progress.faces_found = 0;
        progress.phase = "indexing".to_string();
    }

    tauri::async_runtime::spawn(async move {
        // 1. Initialization
        let reset_status = || {
            *t_common::lock_mutex(&status_token) = false;
        };

        // Load models if not already loaded
        {
            let mut engine = t_common::lock_mutex(&face_state.0);
            if !engine.is_loaded() {
                if let Err(e) = engine.load_models(&app_handle) {
                    eprintln!("Failed to load face models: {}", e);
                    let _ = app_handle.emit(
                        "face_index_finished",
                        serde_json::json!({
                            "total_faces": 0,
                            "total_persons": 0,
                            "cancelled": false,
                            "error": e.to_string()
                        }),
                    );
                    reset_status();
                    return;
                }
            }
        }

        // 2. Preparation (Get files and stats)
        let (processed_count, existing_faces_count) = match t_sqlite::Face::get_stats() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to get stats: {}", e);
                (0, 0)
            }
        };

        let files = match t_sqlite::Face::get_unprocessed_image_files() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to get unprocessed files: {}", e);
                let _ = app_handle.emit(
                    "face_index_finished",
                    serde_json::json!({
                        "total_faces": 0,
                        "total_persons": 0,
                        "cancelled": false,
                        "error": e
                    }),
                );
                reset_status();
                return;
            }
        };

        let total_files = processed_count + files.len();
        let mut total_faces = existing_faces_count;
        let mut current = processed_count;

        // Init progress
        {
            let mut progress = t_common::lock_mutex(&progress_token);
            progress.total = total_files;
            progress.current = current;
            progress.faces_found = total_faces;
            progress.phase = "indexing".to_string();
        }

        let _ = app_handle.emit(
            "face_index_progress",
            serde_json::json!({
                "current": current,
                "total": total_files,
                "faces_found": total_faces,
                "phase": "indexing"
            }),
        );

        // 3. Image Processing Loop — bounded worker pool + batched SQLite writes.
        // Each worker owns its own ONNX sessions (ort Session is not Sync).
        // FaceState engine is only used for the initial shared load probe.
        let mut cancelled = false;
        let db_conn = match t_sqlite::open_conn() {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("Failed to open DB connection for face indexing: {}", e);
                let _ = app_handle.emit(
                    "face_index_finished",
                    serde_json::json!({
                        "total_faces": 0,
                        "total_persons": 0,
                        "cancelled": false,
                        "error": e
                    }),
                );
                reset_status();
                return;
            }
        };

        // Keep FaceState loaded for other callers; workers clone paths + own sessions.
        let model_paths = match FaceEngine::resolve_model_paths(&app_handle) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to resolve face models: {}", e);
                let _ = app_handle.emit(
                    "face_index_finished",
                    serde_json::json!({
                        "total_faces": 0,
                        "total_persons": 0,
                        "cancelled": false,
                        "error": e
                    }),
                );
                reset_status();
                return;
            }
        };
        let _ = face_state; // managed state retained for process-lifetime; workers use local engines

        let worker_count = face_index_worker_count();
        let write_batch_size = 32usize;
        let job_queue = Arc::new(FaceJobQueue::new());
        let (result_tx, result_rx) = std::sync::mpsc::channel::<FaceIndexWorkerResult>();

        let mut worker_handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let job_queue = Arc::clone(&job_queue);
            let result_tx = result_tx.clone();
            let det_path = model_paths.0.clone();
            let emb_path = model_paths.1.clone();
            let cancel = cancel_token.clone();
            worker_handles.push(std::thread::spawn(move || {
                let mut engine = FaceEngine::new();
                // Fewer intra-threads per session when multiple workers share the CPU.
                let per_session_threads = if worker_count >= 4 { 1 } else { 2 };
                if let Err(e) =
                    engine.load_models_from_paths(&det_path, &emb_path, per_session_threads)
                {
                    eprintln!("Face worker failed to load models: {}", e);
                    return;
                }

                while let Some((file_id, file_path, width, height)) = job_queue.pop() {
                    if *t_common::lock_mutex(&cancel) {
                        // Still report the job so the main loop advances progress to 100%.
                        // Work is retryable (no write); only progress accounting is closed out.
                        let _ = result_tx.send(FaceIndexWorkerResult {
                            file_id,
                            file_path,
                            write: None,
                        });
                        continue;
                    }

                    // Thumbnail first (same optimization as serial path).
                    let (process_result, used_thumb) = match t_sqlite::AThumb::fetch(file_id) {
                        Ok(Some(thumb)) if thumb.thumb_data.is_some() => {
                            let thumb_bytes = thumb.thumb_data.as_ref().unwrap();
                            match engine.process_image_from_bytes(thumb_bytes) {
                                Ok(res) => (Ok(res), true),
                                Err(_) => (engine.process_image(&file_path), false),
                            }
                        }
                        _ => (engine.process_image(&file_path), false),
                    };

                    let write = match process_result {
                        Ok((mut faces, (proc_w, proc_h))) => {
                            if used_thumb {
                                let scale_x = width as f32 / proc_w as f32;
                                let scale_y = height as f32 / proc_h as f32;
                                for face in &mut faces {
                                    face.bbox.x *= scale_x;
                                    face.bbox.y *= scale_y;
                                    face.bbox.width *= scale_x;
                                    face.bbox.height *= scale_y;
                                }
                            }
                            let has_faces = !faces.is_empty();
                            let status = if has_faces { 1 } else { 2 };
                            let face_rows: Vec<(String, Vec<f32>)> = faces
                                .into_iter()
                                .map(|face_data| {
                                    let bbox_json = serde_json::json!({
                                        "x": face_data.bbox.x,
                                        "y": face_data.bbox.y,
                                        "width": face_data.bbox.width,
                                        "height": face_data.bbox.height,
                                        "confidence": face_data.bbox.confidence,
                                    })
                                    .to_string();
                                    (bbox_json, face_data.embedding)
                                })
                                .collect();
                            Some((file_id, status, face_rows))
                        }
                        Err(e) => {
                            eprintln!("Failed to process image {}: {}", file_path, e);
                            // Leave has_faces untouched so a later run can retry.
                            None
                        }
                    };

                    if result_tx
                        .send(FaceIndexWorkerResult {
                            file_id,
                            file_path,
                            write,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        drop(result_tx);

        // Feed jobs from a dedicated thread so this async task can flush DB writes.
        let feed_cancel = cancel_token.clone();
        let feed_queue = Arc::clone(&job_queue);
        let feed_handle = std::thread::spawn(move || {
            for item in files {
                if *t_common::lock_mutex(&feed_cancel) {
                    break;
                }
                feed_queue.push(item);
            }
            feed_queue.close();
        });

        let mut write_batch: Vec<(i64, i32, Vec<(String, Vec<f32>)>)> =
            Vec::with_capacity(write_batch_size);
        let flush_writes = |batch: &mut Vec<(i64, i32, Vec<(String, Vec<f32>)>)>,
                            total_faces: &mut usize| {
            if batch.is_empty() {
                return;
            }
            match t_sqlite::Face::apply_scan_batch_with_conn(&db_conn, batch) {
                Ok(n) => *total_faces += n,
                Err(e) => eprintln!("Failed to apply face scan batch: {}", e),
            }
            batch.clear();
        };

        loop {
            let result = match result_rx.recv() {
                Ok(r) => r,
                Err(_) => break, // all workers finished and dropped senders
            };

            if *t_common::lock_mutex(&cancel_token) {
                cancelled = true;
            }

            current += 1;
            if let Some(write) = result.write {
                write_batch.push(write);
                if write_batch.len() >= write_batch_size {
                    flush_writes(&mut write_batch, &mut total_faces);
                }
            }

            if current % 10 == 0 || current == total_files || cancelled {
                {
                    let mut progress = t_common::lock_mutex(&progress_token);
                    progress.current = current;
                    progress.faces_found = total_faces;
                }
                let _ = app_handle.emit(
                    "face_index_progress",
                    serde_json::json!({
                        "current": current,
                        "total": total_files,
                        "faces_found": total_faces,
                        "phase": "indexing"
                    }),
                );
            }

            if cancelled {
                // Keep draining until workers exit so completed work is not lost.
                // Do not break: feeder stops; workers see cancel and exit; channel closes.
            }
        }

        let _ = feed_handle.join();
        for h in worker_handles {
            let _ = h.join();
        }
        while let Ok(result) = result_rx.try_recv() {
            current += 1;
            if let Some(write) = result.write {
                write_batch.push(write);
            }
        }
        flush_writes(&mut write_batch, &mut total_faces);

        {
            let mut progress = t_common::lock_mutex(&progress_token);
            progress.current = current.min(total_files);
            progress.faces_found = total_faces;
        }

        if *t_common::lock_mutex(&cancel_token) {
            cancelled = true;
        }

        if cancelled {
            // Feeder may have stopped early; force progress bar to 100% on cancel.
            {
                let mut progress = t_common::lock_mutex(&progress_token);
                progress.current = total_files;
                progress.faces_found = total_faces;
            }
            let _ = app_handle.emit(
                "face_index_progress",
                serde_json::json!({
                    "current": total_files,
                    "total": total_files,
                    "faces_found": total_faces,
                    "phase": "indexing"
                }),
            );
            let _ = app_handle.emit(
                "face_index_finished",
                serde_json::json!({
                    "total_faces": total_faces,
                    "total_persons": 0,
                    "cancelled": true
                }),
            );
            reset_status();
            return;
        }

        // 4. Clustering
        {
            let mut progress = t_common::lock_mutex(&progress_token);
            progress.phase = "clustering".to_string();
        }

        let _ = app_handle.emit(
            "face_index_progress",
            serde_json::json!({
                "current": total_files,
                "total": total_files,
                "faces_found": total_faces,
                "phase": "clustering"
            }),
        );

        let cancel_token_cluster = cancel_token.clone();
        let cluster_result = t_cluster::cluster_faces(
            epsilon,
            mode,
            |progress| {
                let _ = app_handle.emit(
                    "cluster_progress",
                    serde_json::json!({
                        "phase": progress.phase,
                        "current": progress.current,
                        "total": progress.total,
                    }),
                );
            },
            || {
                // Check if user has cancelled
                *t_common::lock_mutex(&cancel_token_cluster)
            },
        );
        let cancelled_during_cluster = *t_common::lock_mutex(&cancel_token)
            || cluster_result
                .as_ref()
                .err()
                .map(|e| e.starts_with("cancelled"))
                .unwrap_or(false);
        let total_persons = match cluster_result {
            Ok(count) => count,
            Err(e) if e.starts_with("cancelled") => {
                eprintln!("Clustering cancelled: {}", e);
                0
            }
            Err(e) => {
                eprintln!("Clustering failed: {}", e);
                0
            }
        };

        // 5. Finished
        let _ = app_handle.emit(
            "face_index_finished",
            serde_json::json!({
                "total_faces": total_faces,
                "total_persons": total_persons,
                "cancelled": cancelled_during_cluster
            }),
        );
        reset_status();
    });

    Ok(())
}
