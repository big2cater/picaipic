//! Track C Phase 0: load bundled CLIP vision + candidate bilingual text with product `ort`.
//!
//! Vision is ALWAYS `src-tauri/resources/models/vision_model.onnx` (not overwritten).
//! Text is the candidate pack under `--text-dir` (AltCLIP-class).
//!
//! Usage (repo root, after placing text ONNX + tokenizer):
//!   cargo run --manifest-path scripts/probe_altclip_ort/Cargo.toml --release -- \
//!     --text-dir scripts/.probe-models/altclip-m9-text
//!
//! Exit 0 = load + encode + dim==512 OK; non-zero = failure.

use image::{imageops::FilterType, ImageBuffer, Rgb};
use ndarray::{Array, Array4};
use ort::{
    inputs,
    session::{builder::GraphOptimizationLevel, Session},
    value::Value,
};
use std::env;
use std::path::{Path, PathBuf};
use std::process::exit;
use tokenizers::Tokenizer;

const IMAGE_SIZE: u32 = 224;
// CLIP mean/std (product / offline compare)
const CLIP_MEAN: [f32; 3] = [0.48145466, 0.4578275, 0.40821073];
const CLIP_STD: [f32; 3] = [0.26862954, 0.26130258, 0.27577711];
const CLIP_MAX_LEN: usize = 77;
const EXPECTED_DIM: usize = 512;

fn die(msg: impl AsRef<str>) -> ! {
    eprintln!("ERROR: {}", msg.as_ref());
    exit(1);
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn default_clip_dir() -> PathBuf {
    repo_root().join("src-tauri").join("resources").join("models")
}

fn default_text_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(".probe-models")
        .join("altclip-m9-text")
}

struct Args {
    clip_dir: PathBuf,
    text_dir: PathBuf,
    text_name: String,
    max_text_len: usize,
}

fn parse_args() -> Args {
    let mut clip_dir = default_clip_dir();
    let mut text_dir = default_text_dir();
    let mut text_name = "text_model.onnx".to_string();
    let mut max_text_len = CLIP_MAX_LEN;
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--clip-dir" => {
                clip_dir = PathBuf::from(args.next().unwrap_or_else(|| die("--clip-dir needs path")));
            }
            "--text-dir" => {
                text_dir = PathBuf::from(args.next().unwrap_or_else(|| die("--text-dir needs path")));
            }
            "--text" => {
                text_name = args
                    .next()
                    .unwrap_or_else(|| die("--text needs filename"));
            }
            "--max-len" => {
                let v = args.next().unwrap_or_else(|| die("--max-len needs int"));
                max_text_len = v.parse().unwrap_or_else(|_| die("bad --max-len"));
            }
            "-h" | "--help" => {
                println!(
                    "probe_altclip_ort — CLIP vision + candidate text (Track C Phase 0)\n\n\
                     --clip-dir <path>   default: src-tauri/resources/models\n\
                     --text-dir <path>   candidate text pack dir\n\
                     --text <file>       text onnx filename (default text_model.onnx)\n\
                     --max-len <n>       text max tokens (default 77)"
                );
                exit(0);
            }
            other => die(format!("unknown arg: {other}")),
        }
    }
    Args {
        clip_dir,
        text_dir,
        text_name,
        max_text_len,
    }
}

fn load_session(path: &Path, label: &str) -> Session {
    if !path.is_file() {
        die(format!("missing {label}: {}", path.display()));
    }
    println!(
        "  loading {label}: {} ({} bytes)",
        path.display(),
        path.metadata().map(|m| m.len()).unwrap_or(0)
    );
    Session::builder()
        .unwrap_or_else(|e| die(format!("Session::builder: {e}")))
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .unwrap_or_else(|e| die(format!("optimization: {e}")))
        .with_intra_threads(2)
        .unwrap_or_else(|e| die(format!("intra_threads: {e}")))
        .commit_from_file(path)
        .unwrap_or_else(|e| die(format!("commit_from_file {label}: {e}")))
}

fn describe_session(label: &str, sess: &Session) {
    println!("== {label} ==");
    for item in &sess.inputs {
        println!("  in  {} {:?}", item.name, item.input_type);
    }
    for item in &sess.outputs {
        println!("  out {} {:?}", item.name, item.output_type);
    }
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn cosine_raw(a: &[f32], b: &[f32]) -> f32 {
    let na = l2_norm(a);
    let nb = l2_norm(b);
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    dot / (na * nb)
}

fn dummy_clip_pixels() -> Array4<f32> {
    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(IMAGE_SIZE, IMAGE_SIZE, Rgb([30, 120, 40]));
    for y in 70..140 {
        for x in 70..150 {
            img.put_pixel(x, y, Rgb([200, 60, 40]));
        }
    }
    let img = image::DynamicImage::ImageRgb8(img).resize_exact(
        IMAGE_SIZE,
        IMAGE_SIZE,
        FilterType::Triangle,
    );
    let rgb = img.to_rgb8();
    let mut array = Array::zeros((1, 3, IMAGE_SIZE as usize, IMAGE_SIZE as usize));
    for (x, y, p) in rgb.enumerate_pixels() {
        let r = (p[0] as f32 / 255.0 - CLIP_MEAN[0]) / CLIP_STD[0];
        let g = (p[1] as f32 / 255.0 - CLIP_MEAN[1]) / CLIP_STD[1];
        let b = (p[2] as f32 / 255.0 - CLIP_MEAN[2]) / CLIP_STD[2];
        array[[0, 0, y as usize, x as usize]] = r;
        array[[0, 1, y as usize, x as usize]] = g;
        array[[0, 2, y as usize, x as usize]] = b;
    }
    array
}

fn extract_embedding(outputs: &ort::session::SessionOutputs<'_>) -> Vec<f32> {
    // Prefer projected sentence/text embeds (512 for CLIP-aligned packs) over
    // token_embeddings / last_hidden (often 768 DistilBERT width).
    let embedding = if let Some(vals) = outputs.get("sentence_embedding") {
        vals
    } else if let Some(vals) = outputs.get("sentence_embeddings") {
        vals
    } else if let Some(vals) = outputs.get("text_embeds") {
        vals
    } else if let Some(vals) = outputs.get("pooler_output") {
        vals
    } else if let Some(vals) = outputs.get("image_embeds") {
        vals
    } else {
        // Prefer any 2D [batch, dim] output over 3D token sequences.
        let mut picked = None;
        for (i, o) in outputs.iter().enumerate() {
            if let Ok((shape, _)) = o.1.try_extract_tensor::<f32>() {
                if shape.len() == 2 {
                    picked = Some(i);
                    break;
                }
            }
        }
        if let Some(i) = picked {
            &outputs[i]
        } else {
            &outputs[0]
        }
    };
    let (shape, data) = embedding
        .try_extract_tensor::<f32>()
        .unwrap_or_else(|e| die(format!("extract: {e}")));
    if shape.len() >= 3 {
        let hidden = *shape.last().unwrap_or(&0) as usize;
        if data.len() < hidden {
            die("embedding shorter than hidden");
        }
        return data[..hidden].to_vec();
    }
    data.to_vec()
}

fn encode_image(sess: &mut Session, pixels: Array4<f32>) -> Vec<f32> {
    let value = Value::from_array(pixels).unwrap_or_else(|e| die(format!("from_array: {e}")));
    let outputs = sess
        .run(inputs!["pixel_values" => value])
        .unwrap_or_else(|e| die(format!("vision run: {e}")));
    extract_embedding(&outputs)
}

fn encode_text(
    sess: &mut Session,
    tokenizer: &Tokenizer,
    text: &str,
    max_len: usize,
) -> (Vec<f32>, Vec<u32>) {
    let encoding = tokenizer
        .encode(text, true)
        .unwrap_or_else(|e| die(format!("tokenize: {e}")));
    let mut ids: Vec<u32> = encoding.get_ids().to_vec();
    if ids.len() > max_len {
        ids.truncate(max_len);
    }
    let mut mask: Vec<i64> = ids.iter().map(|_| 1i64).collect();
    while ids.len() < max_len {
        ids.push(0);
        mask.push(0);
    }
    let ids_i64: Vec<i64> = ids.iter().map(|&x| x as i64).collect();
    let arr = Array::from_shape_vec((1, ids_i64.len()), ids_i64)
        .unwrap_or_else(|e| die(format!("shape: {e}")));
    let value = Value::from_array(arr).unwrap_or_else(|e| die(format!("from_array: {e}")));

    let outputs = if sess.inputs.iter().any(|i| i.name == "attention_mask") {
        let mask_arr = Array::from_shape_vec((1, mask.len()), mask).unwrap();
        let mask_val = Value::from_array(mask_arr).unwrap();
        sess.run(inputs![
            "input_ids" => value,
            "attention_mask" => mask_val,
        ])
        .unwrap_or_else(|e| die(format!("text run: {e}")))
    } else {
        sess.run(inputs!["input_ids" => value])
            .unwrap_or_else(|e| die(format!("text run: {e}")))
    };
    (extract_embedding(&outputs), ids)
}

fn main() {
    let args = parse_args();
    println!("PicAiPic Track C Phase 0 — CLIP vision + candidate text (ort)");
    println!("  clip_dir: {}", args.clip_dir.display());
    println!("  text_dir: {}", args.text_dir.display());
    println!("  text onnx: {}", args.text_name);
    println!("  note: does NOT modify bundled models");

    let vision_path = args.clip_dir.join("vision_model.onnx");
    let clip_text_path = args.clip_dir.join("text_model.onnx");
    let clip_tok_path = args.clip_dir.join("tokenizer.json");
    let cand_text_path = args.text_dir.join(&args.text_name);
    let cand_tok_path = args.text_dir.join("tokenizer.json");

    for p in [
        &vision_path,
        &clip_text_path,
        &clip_tok_path,
        &cand_text_path,
        &cand_tok_path,
    ] {
        if !p.is_file() {
            die(format!("missing {}", p.display()));
        }
    }

    println!("\n-- load --");
    let mut vision = load_session(&vision_path, "clip-vision");
    let mut clip_text = load_session(&clip_text_path, "clip-text");
    let mut cand_text = load_session(&cand_text_path, "candidate-text");
    describe_session("clip-vision", &vision);
    describe_session("clip-text", &clip_text);
    describe_session("candidate-text", &cand_text);

    let clip_tok = Tokenizer::from_file(&clip_tok_path)
        .unwrap_or_else(|e| die(format!("clip tokenizer: {e}")));
    let cand_tok = Tokenizer::from_file(&cand_tok_path)
        .unwrap_or_else(|e| die(format!("candidate tokenizer: {e}")));

    println!("\n-- image (CLIP vision) --");
    let img_emb = encode_image(&mut vision, dummy_clip_pixels());
    println!(
        "  dim={} l2={:.4} (expect dim={EXPECTED_DIM})",
        img_emb.len(),
        l2_norm(&img_emb)
    );
    if img_emb.len() != EXPECTED_DIM {
        die(format!("CLIP vision dim {} != {EXPECTED_DIM}", img_emb.len()));
    }

    let probes = [
        ("en_bird_clip", "a photo of a bird", true),
        ("en_plant_clip", "a photo of a plant", true),
        ("zh_bird_cand", "一只鸟", false),
        ("zh_plant_cand", "一株植物", false),
        ("en_bird_cand", "a photo of a bird", false),
    ];

    println!("\n-- texts --");
    let mut soft_ok = true;
    for (key, text, use_clip) in probes {
        let (emb, ids) = if use_clip {
            encode_text(&mut clip_text, &clip_tok, text, CLIP_MAX_LEN)
        } else {
            encode_text(&mut cand_text, &cand_tok, text, args.max_text_len)
        };
        let cos = cosine_raw(&img_emb, &emb);
        println!(
            "  {key}: dim={} cos_to_synth_image={:.4} ids_head={:?}",
            emb.len(),
            cos,
            &ids[..ids.len().min(4)]
        );
        if emb.len() != EXPECTED_DIM {
            eprintln!("  FAIL: {key} dim {} != {EXPECTED_DIM}", emb.len());
            soft_ok = false;
        }
    }

    if !soft_ok {
        die("candidate or CLIP text dim mismatch — Track C cannot skip reindex with this pack");
    }

    println!("\nPASS load+dim smoke (owner still needs album compare for quality gate)");
    println!("  next: python scripts/compare_clip_en_vs_altclip_cn.py --images <album>");
}
