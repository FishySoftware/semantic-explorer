# Quantization Guide for LLM Inference API

This guide explains how to use quantized models for reduced memory usage.

## Overview

**Quantization** reduces model size and memory usage by using lower precision numbers (e.g., 8-bit or 4-bit integers instead of 16-bit floats).

**✅ Fully Supported: Pre-Quantized Models (Recommended)**  
- **GGUF**: Works on CPU, CUDA (NVIDIA), and Metal (Apple Silicon)
- **GPTQ**: Works on CUDA (NVIDIA GPUs only)

**⚠️ Alternative: Runtime ISQ Quantization (Slow, Not Cached)**

---

## Pre-Quantized GGUF Models (Recommended - All Devices)

### What are GGUF Models?

GGUF models are pre-quantized by experts and cached forever. They load in seconds instead of minutes.

**Best for**: Any device (CPU, NVIDIA GPU, Apple Silicon)

### How to Use GGUF

#### Step 1: Choose a GGUF Model

Popular sources:
- **TheBloke**: [huggingface.co/TheBloke](https://huggingface.co/TheBloke) - Largest collection
- **bartowski**: [huggingface.co/bartowski](https://huggingface.co/bartowski) - Quality quantizations

Example models:
- `TheBloke/Mistral-7B-Instruct-v0.2-GGUF`
- `TheBloke/Llama-2-7B-Chat-GGUF`
- `bartowski/Meta-Llama-3.1-8B-Instruct-GGUF`

#### Step 2: Configure .env

```bash
# Use GGUF model (auto-selects Q4_K_M by default)
LLM_ALLOWED_MODELS="TheBloke/Mistral-7B-Instruct-v0.2-GGUF"

# Or specify exact GGUF file:
# LLM_ALLOWED_MODELS="TheBloke/Mistral-7B-Instruct-v0.2-GGUF:mistral-7b-instruct-v0.2.Q8_0.gguf"

# Disable ISQ (not needed for GGUF)
LLM_ENABLE_ISQ="false"
```

#### Step 3: Start the Service

```bash
cargo run
# Loads in seconds! ⚡
```

---

## Pre-Quantized GPTQ Models (CUDA Only)

### What are GPTQ Models?

GPTQ models are pre-quantized using the GPTQ algorithm and automatically detected by mistral.rs. They use Marlin kernels for fast inference on NVIDIA GPUs.

**Best for**: NVIDIA GPUs with CUDA support  
**Supports**: 2-bit, 3-bit, 4-bit, 8-bit quantization  
**Performance**: Uses optimized Marlin kernels for 4-bit and 8-bit

### How to Use GPTQ

#### Step 1: Choose a GPTQ Model

Popular sources:
- **TheBloke**: [huggingface.co/TheBloke](https://huggingface.co/TheBloke) - Many GPTQ models
- **kaitchup**: Specialized GPTQ quantizations

Example models:
- `TheBloke/Mistral-7B-Instruct-v0.2-GPTQ`
- `kaitchup/Phi-3-mini-4k-instruct-gptq-4bit`
- `TheBloke/Llama-2-7B-Chat-GPTQ`

#### Step 2: Configure .env

```bash
# Use GPTQ model (auto-detected by mistral.rs)
LLM_ALLOWED_MODELS=\"TheBloke/Mistral-7B-Instruct-v0.2-GPTQ\"

# Disable ISQ (not needed for GPTQ)
LLM_ENABLE_ISQ=\"false\"

# IMPORTANT: GPTQ requires CUDA
# Ensure you have CUDA enabled and CUDA_VISIBLE_DEVICES set
```

#### Step 3: Start the Service

```bash
# GPTQ requires CUDA features enabled
cargo run --features cuda
# Loads quickly and uses optimized Marlin kernels! 🚀
```

### GPTQ Limitations

⚠️ **CUDA Only**: GPTQ models only work with NVIDIA GPUs. They will not work on:
- CPU
- Apple Silicon (Metal)
- AMD GPUs

For non-CUDA devices, use GGUF models instead.

---

## Runtime ISQ Quantization (Alternative)

### What is ISQ?

ISQ (In-Situ Quantization) quantizes models during loading. Use this only for standard HuggingFace models that don't have GGUF versions available.

### Pros & Cons

✅ **Advantages**:
- Works with any HuggingFace model
- No need to find pre-quantized versions

⚠️ **Disadvantages**:
- **Slow first load (5-10 minutes for 7B models)**
- **Not cached between restarts** - quantizes every time
- Service unavailable during quantization

### How to Use ISQ

#### Step 1: Enable ISQ in .env

```bash
# Enable runtime quantization
LLM_ENABLE_ISQ="true"

# Choose quantization type (Q4_K recommended)
LLM_ISQ_TYPE="Q4_K"

# Use standard HuggingFace model
LLM_ALLOWED_MODELS="mistralai/Mistral-7B-Instruct-v0.2"
```

#### Step 2: Restart and Wait

```bash
# Start the service
cargo run

# Wait 5-10 minutes for quantization to complete...
# Check logs for progress
```

### ISQ Quantization Types

Available ISQ types (set in `LLM_ISQ_TYPE`):

- `Q4_0`, `Q4_1` - 4-bit quantization
- `Q5_0`, `Q5_1` - 5-bit quantization
- `Q8_0`, `Q8_1` - 8-bit quantization (high quality)
- `Q2_K`, `Q3_K`, `Q4_K`, `Q5_K`, `Q6_K`, `Q8_K` - K-means based quantization

---

## Comparison

| Feature | GGUF | GPTQ | ISQ (Runtime) |
|---------|------|------|---------------|
| Load Time | **Seconds** ⚡ | **Seconds** ⚡ | 5-10 minutes 🐌 |
| Cached? | **Yes** ✅ | **Yes** ✅ | No ❌ |
| Startup Impact | None | None | Blocks service |
| Device Support | **CPU, CUDA, Metal** | CUDA only | All devices |
| Memory Usage | Low | Low | Low |
| Quality | Excellent | Excellent | Same as GGUF/GPTQ |
| **Best For** | **Any device** ⭐ | NVIDIA GPUs | Avoid if possible |
| **Recommendation** | **✅ Use This** | ✅ CUDA users | ⚠️ Avoid |

---

## Examples

### Example 1: Production Setup (GGUF - Universal)

```bash
# .env
LLM_ALLOWED_MODELS="TheBloke/Mistral-7B-Instruct-v0.2-GGUF"
LLM_ENABLE_ISQ="false"
LLM_MAX_CONCURRENT_REQUESTS="10"
HF_HOME="/models"
```

**Result**: Fast startup, works on any device (CPU/CUDA/Metal), efficient memory usage, production-ready.

### Example 2: CUDA Production Setup (GPTQ)

```bash
# .env
LLM_ALLOWED_MODELS="TheBloke/Mistral-7B-Instruct-v0.2-GPTQ"
LLM_ENABLE_ISQ="false"
LLM_MAX_CONCURRENT_REQUESTS="10"
HF_HOME="/models"
CUDA_VISIBLE_DEVICES="0"
```

**Result**: Fast startup, optimized Marlin kernels for CUDA, excellent performance on NVIDIA GPUs.

### Example 3: Testing ISQ (Not Recommended)

```bash
# .env
LLM_ALLOWED_MODELS="mistralai/Mistral-7B-Instruct-v0.2"
LLM_ENABLE_ISQ="true"
LLM_ISQ_TYPE="Q4_K"
```

**Result**: Slow startup (~10 min), not cached, same quality as GGUF/GPTQ Q4_K.

---

## FAQ

### Q: How do I know if my model is a GGUF or GPTQ?

A: Check the model repo name:
- **GGUF**: Contains "GGUF" (e.g., `TheBloke/Model-Name-GGUF`)
- **GPTQ**: Contains "GPTQ" or "gptq" (e.g., `TheBloke/Model-Name-GPTQ`)

### Q: Which should I use - GGUF or GPTQ?

A: 
- **GGUF**: Works everywhere (CPU, CUDA, Metal) - use this if unsure
- **GPTQ**: CUDA-only but may have slightly better performance on NVIDIA GPUs with Marlin kernels

### Q: Can I use GPTQ on my Mac (Apple Silicon)?

A: **No.** GPTQ requires CUDA (NVIDIA GPU). For Mac, use GGUF models which work great on Metal.

### Q: Can I use GPTQ on CPU?

A: **No.** GPTQ requires CUDA. For CPU, use GGUF models.

### Q: What's the best quantization level?

A: **Q4_K_M** or **Q5_K_M** for most use cases. Use **Q8_0** if you have plenty of VRAM and want maximum quality.

### Q: Does ISQ cache the quantized model?

A: **No.** ISQ quantizes the model every time you start the service. Use GGUF or GPTQ instead.

### Q: Where can I find GGUF models?

A: [TheBloke on HuggingFace](https://huggingface.co/TheBloke) has the largest collection of GGUF models.

### Q: Where can I find GPTQ models?

A: [TheBloke](https://huggingface.co/TheBloke) and [kaitchup](https://huggingface.co/kaitchup) on HuggingFace.

---

## Summary

**Use pre-quantized models** for:
- ✅ Fast startup
- ✅ Production deployments
- ✅ Efficient memory usage
- ✅ Cached downloads

**GGUF**: Best choice for any device (CPU, CUDA, Metal)  
**GPTQ**: Best choice for NVIDIA GPUs with CUDA

**Avoid ISQ** unless you have a specific reason, as it's slow and not cached.

For questions or issues, refer to the main [README](README.md).
