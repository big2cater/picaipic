/**
 * Common constants and shared types
 */
use std::sync::{Mutex, MutexGuard, PoisonError};

// Image support
pub const NORMAL_IMGS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "bmp", "tif", "tiff", "webp", "avif", "heic", "heif", "hif", "jxl",
];

// Image formats decoded through the bundled FFmpeg sidecar.
pub const FFMPEG_BACKED_IMGS: &[&str] = &[
    "psd", // Photoshop native format; preview only (merged composite), no edit support
    "exr", // HDR industry standard (VFX/rendering/Resolve output)
    "hdr", "rgbe", // Radiance RGBE format; HDR panoramas and tonemapped output
    "tga",  // Legacy format; some cameras/scanners/game asset pipelines
    "dds",  // DirectDraw Surface; game texture format; relevant only for game-dev users
    "qoi",  // Fast lossless format (2021); ecosystem still immature
    "jp2", "j2k", "j2c", "jpc", "jpf",
    "jpx", // JPEG 2000 family; medical/satellite use only; no mature pure-Rust decoder
    "dpx", // Digital cinema intermediate format; niche film/grading pipeline only
];

// RAW support
pub const RAW_IMGS: &[&str] = &[
    "cr2", "cr3", "crw", // Canon
    "nef", "nrw", // Nikon
    "arw", "srf", "sr2", // Sony
    "raf", // Fujifilm
    "rw2", // Panasonic
    "orf", // Olympus / OM System
    "pef", // Pentax
    "dng", // Adobe / generic RAW
    "srw", // Samsung
    "rwl", // Leica
    "mrw", // Minolta / Konica Minolta
    "3fr", // Hasselblad
    "mos", // Leaf / Phase One
    "iiq", // Phase One
    "x3f", // Sigma / Foveon - LibRaw may report FileUnsupported for sampled files.
    // Safe to list: `AFile::new_profiled` degrades to unknown dimensions now
    // instead of failing the whole index.
    "dcr", "kdc", // Kodak
    "erf", // Epson
    "mef", // Mamiya
    "raw", // Generic vendor RAW extension
    "mdc", // Legacy RAW variant in sample set
];

// Video support
pub const VIDEOS: &[&str] = &[
    "mpg", "mpeg", "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "3gp", "m4v", "hevc", "asf",
    "mts", "m2ts", "mod", "tod", "ts",
];
// pub const AUDIOS: &[&str] = &[
//     "mp3", "wav", "flac", "aac", "m4a", "ogg", "wma", "mp2", "mp1", "ape", "alac", "wavpack",
// ];

// AI search
pub const AI_TEXT_MODEL: &str = "text_model.onnx";
pub const AI_VISION_MODEL: &str = "vision_model.onnx";
pub const AI_TOKENIZER: &str = "tokenizer.json";
/// Longest edge for CLIP embed source (before final 224 square). Shared by JPEG
/// scaled decode, generic open+thumbnail, and RAW LibRaw preview sizing.
pub const EMBED_SOURCE_MAX_EDGE: u32 = 1024;

// Face Recognition Constants

// models
// InsightFace SCRFD-500m (det_500m.onnx): 9 outputs (scores/boxes/landmarks × 3 strides)
pub const DETECTION_MODEL: &str = "det_500m.onnx";
pub const EMBEDDING_MODEL: &str = "w600k_mbf.onnx"; // MobileFaceNet

// Quality thresholds - Recommended Values
pub const MIN_CONFIDENCE: f32 = 0.65; // 0.6-0.7 is standard. 0.65 balances precision/recall.
// pub const MIN_FACE_RATIO: f32 = 0.0; // Disabled. Rely on absolute pixel size instead (better for high-res photos).
// pub const MIN_FACE_SIZE: f32 = 90.0; // 80-112px is minimum for good recognition. 90.0 is a safe high-quality baseline.
pub const MIN_BLUR_SCORE: f32 = 200.0; // Standard Laplacian variance threshold. Below 100 is usually blurry.

// Clustering Constants
pub const K_NEIGHBORS: usize = 80; // Prune edges to Top-K (K-NN)
pub const MIN_SAMPLES: usize = 1; // Minimum samples per cluster
/// Face count below this (in `auto` mode) uses classic row-wise all-pairs graph build.
/// At/above: HNSW ANN Top-K (`instant-distance`), with blocked exact fallback on ANN failure.
/// See `docs/guide/face-cluster-ann-plan.md`.
pub const CLUSTER_N_EXACT: usize = 8000;
/// Tile size for blocked exact KNN (fallback / forced exact-distance path).
pub const CLUSTER_BLOCK_SIZE: usize = 2048;
/// HNSW `efSearch` floor (also raised to at least `3 * K_NEIGHBORS` at query time).
pub const CLUSTER_ANN_EF_SEARCH: usize = 120;
/// HNSW `efConstruction` — higher = better graph quality / slower build.
pub const CLUSTER_ANN_EF_CONSTRUCTION: usize = 200;

// Image-search ANN (exact matrix for smaller libraries)
/// Below this embed count, AI search scores the full in-memory matrix (rayon).
/// At/above: HNSW candidate retrieval + exact cosine rerank (`instant-distance`).
pub const IMAGE_SEARCH_ANN_MIN_N: usize = 8000;
pub const IMAGE_SEARCH_ANN_EF_SEARCH: usize = 120;
pub const IMAGE_SEARCH_ANN_EF_CONSTRUCTION: usize = 200;
/// How many HNSW neighbors to pull before exact rerank (covers thr_cap 200 + headroom).
pub const IMAGE_SEARCH_ANN_CANDIDATES: usize = 500;

/// Recover from mutex poisoning instead of panicking subsequent work.
#[inline]
pub fn lock_mutex<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
