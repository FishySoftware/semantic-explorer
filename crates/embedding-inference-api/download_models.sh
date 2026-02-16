#!/usr/bin/env bash
# Download all HuggingFace models supported by the embedding-inference-api.
#
#  sudo apt install pipx
#  pipx install huggingface_hub[cli]
#  export HF_HOME="$HOME/.cache/huggingface"  # where models are stored (optional)
#  export HF_ENDPOINT="custom-mirror-url" # where to download models from (optional)

# Usage:
#   ./download_models.sh            # download all models
#   ./download_models.sh --dry-run  # show what would be downloaded
#
# Requires: huggingface-cli  (pip install huggingface_hub[cli])
set -euo pipefail

DRY_RUN=""
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN="--dry-run"
    echo "=== DRY RUN — nothing will be downloaded ==="
    echo
fi

download() {
    local repo="$1"
    echo "--- $repo ---"
    hf download $DRY_RUN "$repo"
    echo
}

# ─────────────────────────────────────────────────────────────
# Embedding models — ONNX (fastembed TextEmbedding, model.onnx)
# ─────────────────────────────────────────────────────────────
echo "=========================================="
echo " ONNX Embedding Models (22)"
echo "=========================================="

download snowflake/snowflake-arctic-embed-xs
download snowflake/snowflake-arctic-embed-s
download "Snowflake/snowflake-arctic-embed-m"
download "snowflake/snowflake-arctic-embed-m-long"
download snowflake/snowflake-arctic-embed-l

download Xenova/all-MiniLM-L12-v2
download Xenova/all-mpnet-base-v2

download Xenova/bge-base-en-v1.5
download Xenova/bge-large-en-v1.5
download Xenova/bge-small-en-v1.5

download nomic-ai/nomic-embed-text-v1
download nomic-ai/nomic-embed-text-v1.5

download Xenova/paraphrase-multilingual-MiniLM-L12-v2
download Xenova/paraphrase-multilingual-mpnet-base-v2

download BAAI/bge-m3
download lightonai/modernbert-embed-large

download intfloat/multilingual-e5-small
download intfloat/multilingual-e5-base

download mixedbread-ai/mxbai-embed-large-v1

download Alibaba-NLP/gte-base-en-v1.5
download Alibaba-NLP/gte-large-en-v1.5

download jinaai/jina-embeddings-v2-base-code

# ─────────────────────────────────────────────────────────────
# Qwen3 Embedding models (Candle backend — full safetensors)
# ─────────────────────────────────────────────────────────────
echo "=========================================="
echo " Qwen3 Embedding Models (3)"
echo "=========================================="

download Qwen/Qwen3-Embedding-0.6B
download Qwen/Qwen3-Embedding-4B
download Qwen/Qwen3-Embedding-8B

# ─────────────────────────────────────────────────────────────
# Reranker models (fastembed TextRerank)
# ─────────────────────────────────────────────────────────────
echo "=========================================="
echo " Reranker Models (4)"
echo "=========================================="

download BAAI/bge-reranker-base
download rozgo/bge-reranker-v2-m3
download jinaai/jina-reranker-v1-turbo-en
download jinaai/jina-reranker-v2-base-multilingual

echo "=========================================="
echo " Done "
echo "=========================================="
