#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET_DIR="$ROOT_DIR/src-tauri/resources/models"
mkdir -p "$TARGET_DIR"

# Product default text: CLIP-aligned bilingual int8. Legacy EN-only CLIP text URLs kept for observation:
#   https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model_quantized.onnx
#   https://huggingface.co/openai/clip-vit-base-patch32/resolve/main/tokenizer.json
MODELS=(
  "https://github.com/big2cater/picaipic-binaries/releases/download/models/clip-vit-b32-multilingual-v1-text-tokenizer.json|tokenizer.json"
  "https://github.com/big2cater/picaipic-binaries/releases/download/models/clip-vit-b32-multilingual-v1-text-int8.onnx|text_model.onnx"
  "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model_quantized.onnx|vision_model.onnx"
  "https://huggingface.co/deepghs/insightface/resolve/main/buffalo_s/det_500m.onnx?download=true|det_500m.onnx"
  "https://huggingface.co/deepghs/insightface/resolve/main/buffalo_s/w600k_mbf.onnx?download=true|w600k_mbf.onnx"
)

echo "Downloading ${#MODELS[@]} models into $TARGET_DIR..."

for entry in "${MODELS[@]}"; do
  URL="${entry%%|*}"
  FILENAME="${entry##*|}"
  FILEPATH="$TARGET_DIR/$FILENAME"

  if [ -f "$FILEPATH" ]; then
    echo "✓ $FILENAME already exists, skipping."
  else
    echo "⬇ Downloading $FILENAME..."
    CURL_ARGS=(-L)
    if [ -n "${HF_TOKEN:-}" ]; then
      CURL_ARGS+=(-H "Authorization: Bearer $HF_TOKEN")
    fi
    curl "${CURL_ARGS[@]}" -o "$FILEPATH" "$URL"
    echo "✓ $FILENAME done."
  fi
done

echo "All downloads complete!"
