//! Phase 0 Rust ort probe for SigLIP2 int8 dual-tower.
//!
//! Uses the same `ort` / `tokenizers` / `ndarray` family as PicAiPic (`t_ai.rs`).
//! Does NOT touch bundled CLIP under `src-tauri/resources/models`.
//!
//! Usage (from repo root, after Python probe downloaded the pack):
//!   cargo run --manifest-path scripts/probe_siglip2_ort/Cargo.toml --release -- \
//!     --dir scripts/.probe-models/siglip2-base-patch16-224
//!
//! Exit 0 = load + encode OK; non-zero = failure.

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
const MEAN: [f32; 3] = [0.5, 0.5, 0.5];
const STD: [f32; 3] = [0.5, 0.5, 0.5];
const MAX_TEXT_LEN: usize = 64;
const PAD_ID: u32 = 0;

fn die(msg: impl AsRef<str>) -> ! {
    eprintln!("ERROR: {}", msg.as_ref());
    exit(1);
}

fn default_pack_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(".probe-models")
        .join("siglip2-base-patch16-224")
}

struct Args {
    dir: PathBuf,
    vision_name: String,
    text_name: String,
}

fn parse_args() -> Args {
    let mut dir = default_pack_dir();
    let mut vision_name = "vision_model_int8.onnx".to_string();
    let mut text_name = "text_model_int8.onnx".to_string();
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dir" => {
                dir = PathBuf::from(args.next().unwrap_or_else(|| die("--dir needs a path")));
            }
            "--vision" => {
                vision_name = args
                    .next()
                    .unwrap_or_else(|| die("--vision needs a filename"));
            }
            "--text" => {
                text_name = args
                    .next()
                    .unwrap_or_else(|| die("--text needs a filename"));
            }
            "--variant" => {
                let v = args.next().unwrap_or_else(|| die("--variant needs a value"));
                match v.as_str() {
                    "int8" => {
                        vision_name = "vision_model_int8.onnx".into();
                        text_name = "text_model_int8.onnx".into();
                    }
                    "quantized" => {
                        vision_name = "vision_model_quantized.onnx".into();
                        text_name = "text_model_quantized.onnx".into();
                    }
                    "fp16" => {
                        vision_name = "vision_model_fp16.onnx".into();
                        text_name = "text_model_fp16.onnx".into();
                    }
                    "fp32" => {
                        vision_name = "vision_model.onnx".into();
                        text_name = "text_model.onnx".into();
                    }
                    other => die(format!("unknown variant: {other}")),
                }
            }
            "-h" | "--help" => {
                println!(
                    "probe_siglip2_ort — Rust ort Phase 0 for SigLIP2\n\n\
                     --dir <path>        pack directory\n\
                     --variant int8|quantized|fp16|fp32\n\
                     --vision <file>     override vision onnx filename\n\
                     --text <file>       override text onnx filename"
                );
                exit(0);
            }
            other => die(format!("unknown arg: {other}")),
        }
    }
    Args {
        dir,
        vision_name,
        text_name,
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
        .unwrap_or_else(|e| die(format!("optimization level: {e}")))
        .with_intra_threads(2)
        .unwrap_or_else(|e| die(format!("intra_threads: {e}")))
        .commit_from_file(path)
        .unwrap_or_else(|e| die(format!("commit_from_file {label}: {e}")))
}

fn describe_session(label: &str, sess: &Session) {
    println!("== {label} ==");
    println!("  inputs:");
    for item in &sess.inputs {
        println!("    - {} {:?}", item.name, item.input_type);
    }
    println!("  outputs:");
    for item in &sess.outputs {
        println!("    - {} {:?}", item.name, item.output_type);
    }
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn l2_normalize(v: &[f32]) -> Vec<f32> {
    let n = l2_norm(v);
    if n <= 0.0 {
        return v.to_vec();
    }
    v.iter().map(|x| x / n).collect()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let a = l2_normalize(a);
    let b = l2_normalize(b);
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn dummy_pixel_values() -> Array4<f32> {
    // Synthetic RGB then SigLIP 0.5 mean/std — not a real photo.
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
        let r = (p[0] as f32 / 255.0 - MEAN[0]) / STD[0];
        let g = (p[1] as f32 / 255.0 - MEAN[1]) / STD[1];
        let b = (p[2] as f32 / 255.0 - MEAN[2]) / STD[2];
        array[[0, 0, y as usize, x as usize]] = r;
        array[[0, 1, y as usize, x as usize]] = g;
        array[[0, 2, y as usize, x as usize]] = b;
    }
    array
}

fn extract_pooler_or_first(outputs: &ort::session::SessionOutputs<'_>) -> Vec<f32> {
    let embedding = if let Some(vals) = outputs.get("pooler_output") {
        vals
    } else if let Some(vals) = outputs.get("image_embeds") {
        vals
    } else if let Some(vals) = outputs.get("text_embeds") {
        vals
    } else {
        &outputs[0]
    };
    let (shape, data) = embedding
        .try_extract_tensor::<f32>()
        .unwrap_or_else(|e| die(format!("extract tensor: {e}")));
    // [1, dim] or [1, seq, dim]
    if shape.len() >= 3 {
        let hidden = *shape.last().unwrap_or(&0) as usize;
        if data.len() < hidden {
            die("embedding shorter than hidden size");
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
    extract_pooler_or_first(&outputs)
}

fn pad_or_trunc(ids: &mut Vec<u32>, max_len: usize, pad: u32) {
    if ids.len() > max_len {
        ids.truncate(max_len);
    } else {
        while ids.len() < max_len {
            ids.push(pad);
        }
    }
}

fn encode_text(sess: &mut Session, tokenizer: &Tokenizer, text: &str) -> (Vec<f32>, Vec<u32>) {
    let encoding = tokenizer
        .encode(text, true)
        .unwrap_or_else(|e| die(format!("tokenize: {e}")));
    let mut ids: Vec<u32> = encoding.get_ids().to_vec();
    pad_or_trunc(&mut ids, MAX_TEXT_LEN, PAD_ID);
    let ids_i64: Vec<i64> = ids.iter().map(|&x| x as i64).collect();
    let arr = Array::from_shape_vec((1, ids_i64.len()), ids_i64)
        .unwrap_or_else(|e| die(format!("shape: {e}")));
    let value = Value::from_array(arr).unwrap_or_else(|e| die(format!("from_array: {e}")));

    // This export only needs input_ids (Python probe confirmed).
    let outputs = if sess.inputs.iter().any(|i| i.name == "attention_mask") {
        let mask: Vec<i64> = ids
            .iter()
            .map(|&id| if id != PAD_ID { 1 } else { 0 })
            .collect();
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

    (extract_pooler_or_first(&outputs), ids)
}

fn main() {
    let args = parse_args();
    let dir = args.dir;
    println!("PicAiPic Phase 0 — SigLIP2 Rust ort probe");
    println!("  pack dir: {}", dir.display());
    println!("  vision:   {}", args.vision_name);
    println!("  text:     {}", args.text_name);
    println!("  note: does NOT touch bundled CLIP resources");

    let vision_path = dir.join(&args.vision_name);
    let text_path = dir.join(&args.text_name);
    let tok_path = dir.join("tokenizer.json");

    for p in [&vision_path, &text_path, &tok_path] {
        if !p.is_file() {
            die(format!(
                "missing {}\n  run: python scripts/probe_siglip2_onnx.py  first",
                p.display()
            ));
        }
    }

    println!("\n-- load sessions (ort crate same line as PicAiPic) --");
    let mut vision = load_session(&vision_path, "vision");
    let mut text = load_session(&text_path, "text");
    describe_session("vision", &vision);
    describe_session("text", &text);

    let tokenizer = Tokenizer::from_file(&tok_path)
        .unwrap_or_else(|e| die(format!("Tokenizer::from_file: {e}")));

    println!("\n-- encode image --");
    let pixels = dummy_pixel_values();
    let img_emb = encode_image(&mut vision, pixels);
    println!(
        "  dim={} raw_l2={:.4} unit_l2={:.4}",
        img_emb.len(),
        l2_norm(&img_emb),
        l2_norm(&l2_normalize(&img_emb))
    );

    let probes = [
        ("en_bird", "a photo of a bird"),
        ("en_plant", "a photo of a plant"),
        ("zh_bird", "一只鸟"),
        ("zh_plant", "一株植物"),
    ];

    println!("\n-- encode texts --");
    let mut en_bird = Vec::new();
    let mut zh_bird = Vec::new();
    for (key, t) in probes {
        let (emb, ids) = encode_text(&mut text, &tokenizer, t);
        let cos = cosine(&img_emb, &emb);
        println!(
            "  {key}: ids_len={} first3={:?} last3={:?} dim={} raw_l2={:.4} cos_to_image={:.4}",
            ids.len(),
            &ids[..ids.len().min(3)],
            &ids[ids.len().saturating_sub(3)..],
            emb.len(),
            l2_norm(&emb),
            cos
        );
        if key == "en_bird" {
            en_bird = emb.clone();
        }
        if key == "zh_bird" {
            zh_bird = emb;
        }
    }

    if !en_bird.is_empty() && !zh_bird.is_empty() {
        let c = cosine(&en_bird, &zh_bird);
        println!("\n  en_bird vs zh_bird text cosine: {c:.4}");
        if c < 0.05 {
            eprintln!("WARN: en/zh text cosine very low — check tokenizer/template");
        }
    }

    if img_emb.len() != 768 {
        eprintln!(
            "WARN: expected dim 768 (Python probe), got {}",
            img_emb.len()
        );
    }

    println!("\n== SUMMARY ==");
    println!("  Rust ort load + encode: PASS");
    println!("  dim={}", img_emb.len());
    println!("  Next: real-album EN/CN smoke vs CLIP; then Track B engine if quality wins.");
    exit(0);
}
