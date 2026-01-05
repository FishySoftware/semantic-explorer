# Text Extraction and Chunking Guide

## Overview

The semantic-explorer now features a state-of-the-art, configurable text extraction and chunking system with multiple strategies including semantic chunking based on embedding similarity.

## Architecture

```
Transform → Scanner → Worker-Collections → Chunks + Metadata
                         ↓
                   ExtractionService
                         ↓
                   ChunkingService
                         ↓
                    (5 Strategies)
```

## Chunking Strategies

### 1. Sentence-Based (Default)
**Strategy:** `sentence`

The original strategy - splits text at sentence boundaries using Unicode sentence segmentation.

**Best for:** General-purpose text, maintaining readability

**Configuration:**
```json
{
  "chunking": {
    "strategy": "sentence",
    "chunk_size": 200,
    "chunk_overlap": 0
  }
}
```

### 2. Recursive Character
**Strategy:** `recursive_character`

Hierarchically splits text using a sequence of separators, trying each in order until the text fits.

**Best for:** Technical documents, code, structured text

**Configuration:**
```json
{
  "chunking": {
    "strategy": "recursive_character",
    "chunk_size": 500,
    "chunk_overlap": 50,
    "options": {
      "recursive_character": {
        "separators": ["\n\n", "\n", ". ", " ", ""],
        "keep_separator": true
      }
    }
  }
}
```

**Options:**
- `separators`: List of separators to try in order (default: `["\n\n", "\n", ". ", " ", ""]`)
- `keep_separator`: Whether to keep the separator in the chunk (default: `true`)

### 3. Semantic Chunking ⭐
**Strategy:** `semantic`

**THE GAME CHANGER** - Uses embedding-based similarity to merge semantically related sentences into coherent chunks.

**Best for:** Creating semantically meaningful chunks, improving retrieval quality, maintaining topic coherence

**How it works:**
1. Splits text into sentences
2. Generates embeddings for each sentence using your configured embedder
3. Calculates cosine similarity between consecutive sentences
4. Merges sentences when similarity exceeds threshold
5. Respects min/max chunk size constraints

**Configuration:**
```json
{
  "chunking": {
    "strategy": "semantic",
    "chunk_size": 400,
    "embedder_id": 1,
    "options": {
      "semantic": {
        "similarity_threshold": 0.75,
        "min_chunk_size": 100,
        "max_chunk_size": 600,
        "buffer_size": 1
      }
    }
  }
}
```

**Options:**
- `embedder_id`: Required - ID of the embedder to use (from your embedders table)
- `similarity_threshold`: Minimum cosine similarity to merge sentences (default: 0.7, range: 0.0-1.0)
  - Higher = more conservative (only merge very similar sentences)
  - Lower = more aggressive (merge more loosely related sentences)
- `min_chunk_size`: Minimum characters per chunk (default: 50)
- `max_chunk_size`: Maximum characters per chunk (default: 500)
- `buffer_size`: Lookahead for similarity calculation (default: 1)

**Performance:**
- Batches 100 sentences at a time
- Typical processing time: 200-1000ms per document (network-dependent)
- Cost: ~$0.006 per 1000 files (OpenAI text-embedding-3-small)

### 4. Fixed-Size
**Strategy:** `fixed_size`

Hard character-limit chunking. Splits text into exact character-sized chunks.

**Best for:** When you need predictable chunk sizes, simple testing

**Configuration:**
```json
{
  "chunking": {
    "strategy": "fixed_size",
    "chunk_size": 512,
    "options": {
      "trim_whitespace": true
    }
  }
}
```

### 5. Markdown-Aware
**Strategy:** `markdown_aware`

Respects markdown structure - splits on headers and preserves code blocks.

**Best for:** Technical documentation, README files, code-heavy documents

**Configuration:**
```json
{
  "chunking": {
    "strategy": "markdown_aware",
    "chunk_size": 500,
    "options": {
      "markdown_aware": {
        "split_on_headers": true,
        "preserve_code_blocks": true
      }
    }
  }
}
```

**Features:**
- Splits on markdown headers (# ## ### etc.)
- Never splits code blocks (```...```)
- Handles both backtick and tilde fences
- Merges small sections with previous chunks

## Chunk Overlap

All strategies support overlapping chunks to maintain context across boundaries.

**Configuration:**
```json
{
  "chunking": {
    "strategy": "recursive_character",
    "chunk_size": 500,
    "chunk_overlap": 50  // Last 50 chars of previous chunk prepended to next
  }
}
```

**Use cases:**
- Prevents losing context at chunk boundaries
- Improves retrieval for queries spanning chunk boundaries
- Typical overlap: 10-20% of chunk_size

## Extraction Strategies

### Plain Text (Default)
**Strategy:** `plain_text`

Extracts text and applies normalization:
- Unicode NFC normalization
- Removes control characters (except \n, \t, \r)
- Collapses whitespace
- Removes empty lines

**Configuration:**
```json
{
  "extraction": {
    "strategy": "plain_text"
  }
}
```

### Future: Structure Preserving & Markdown
Coming in future updates - preserves document structure (headings, lists, tables) and converts documents to markdown.

## Metadata

Each chunk includes metadata:

```json
{
  "content": "The actual chunk text...",
  "metadata": {
    "chunk_index": 0,
    "total_chunks": 10,
    "chunk_size": 234,
    "extraction_metadata": null,
    "structure_info": null
  }
}
```

## Complete Configuration Example

```json
{
  "extraction": {
    "strategy": "plain_text"
  },
  "chunking": {
    "strategy": "semantic",
    "chunk_size": 400,
    "chunk_overlap": 50,
    "embedder_id": 1,
    "options": {
      "semantic": {
        "similarity_threshold": 0.75,
        "min_chunk_size": 100,
        "max_chunk_size": 600
      },
      "preserve_sentence_boundaries": true,
      "trim_whitespace": true,
      "min_chunk_size": 50
    }
  },
  "metadata": {
    "include_structure_info": false,
    "include_extraction_stats": false,
    "include_chunk_position": true
  }
}
```

## API Usage

### Creating a Transform

```bash
POST /api/transforms
{
  "title": "My Semantic Transform",
  "job_type": "collection_to_dataset",
  "collection_id": 1,
  "dataset_id": 2,
  "job_config": {
    "chunking": {
      "strategy": "semantic",
      "chunk_size": 400,
      "embedder_id": 1,
      "options": {
        "semantic": {
          "similarity_threshold": 0.75
        }
      }
    }
  }
}
```

### Default Configuration

If you don't specify extraction or chunking in `job_config`, the system uses defaults:

```bash
POST /api/transforms
{
  "title": "Transform with Defaults",
  "job_type": "collection_to_dataset",
  "collection_id": 1,
  "dataset_id": 2,
  "chunk_size": 200  // Used as default chunk_size if job_config.chunking not specified
}
```

Default behavior:
- **Extraction**: `plain_text` strategy
- **Chunking**: `sentence` strategy with `chunk_size` from transform column

## Choosing a Strategy

### Decision Tree

```
Technical docs with code?
  → markdown_aware

Need semantically coherent chunks?
  → semantic (requires embedder)

Processing transcripts or conversations?
  → semantic or sentence

Need predictable chunk sizes?
  → fixed_size

Complex hierarchical text?
  → recursive_character

Default/General purpose?
  → sentence
```

### Strategy Comparison

| Strategy | Speed | Quality | Cost | Use Case |
|----------|-------|---------|------|----------|
| sentence | ⚡⚡⚡ | ⭐⭐⭐ | Free | General purpose |
| recursive_character | ⚡⚡⚡ | ⭐⭐⭐⭐ | Free | Structured text |
| semantic | ⚡⚡ | ⭐⭐⭐⭐⭐ | ~$0.02/1M tokens | Best retrieval quality |
| fixed_size | ⚡⚡⚡ | ⭐⭐ | Free | Simple/testing |
| markdown_aware | ⚡⚡⚡ | ⭐⭐⭐⭐ | Free | Technical docs |

## Performance Tips

1. **Semantic Chunking:**
   - Batches 100 sentences at a time automatically
   - Consider caching for reprocessing
   - Monitor API costs with high-volume processing

2. **Chunk Size:**
   - Smaller chunks (200-300): Better precision, more chunks
   - Larger chunks (500-1000): Better context, fewer chunks
   - Balance based on your retrieval needs

3. **Overlap:**
   - 10-20% overlap recommended for most use cases
   - Higher overlap increases context but also storage

4. **Embedder Selection:**
   - Fast/cheap: text-embedding-3-small
   - Quality: text-embedding-3-large
   - Domain-specific: Cohere embed-english-v3.0 with input_type

## Troubleshooting

### Semantic Chunking Not Working?

1. **Check embedder_id is specified** in chunking config
2. **Verify embedder exists** and user has access
3. **Check embedder API key** is valid
4. **Review logs** for API errors
5. **Test embedder separately** with dataset_to_vector_storage transform

### Chunks Too Large/Small?

1. Adjust `chunk_size` parameter
2. For semantic: tune `min_chunk_size` and `max_chunk_size`
3. Consider different strategy (e.g., fixed_size for consistent sizing)

### Poor Retrieval Quality?

1. **Try semantic chunking** - often dramatically improves results
2. Add `chunk_overlap` (50-100 chars)
3. Experiment with `similarity_threshold` (0.6-0.8 range)
4. Use markdown_aware for technical content

## Implementation Details

### File Structure
```
crates/worker-collections/src/
├── chunk/
│   ├── config.rs              # Configuration structures
│   ├── metadata.rs            # Chunk metadata
│   ├── service.rs             # Main chunking service
│   └── strategies/
│       ├── sentence.rs        # Sentence-based
│       ├── recursive_character.rs
│       ├── semantic.rs        # Async, embedding-based
│       ├── fixed_size.rs
│       ├── markdown_aware.rs
│       └── overlap.rs         # Overlap wrapper
└── extract/
    ├── config.rs              # Extraction configuration
    ├── service.rs             # Extraction service
    └── strategies/
        └── plain_text.rs      # Text normalization
```

### Async Architecture

- **CPU-bound strategies** (sentence, recursive, fixed, markdown): Run directly
- **I/O-bound strategies** (semantic): True async with API calls
- **No spawn_blocking** - clean async/await pattern throughout

### Embedder Integration

Scanner fetches embedder config → Passes in TransformFileJob → Worker uses for semantic chunking

## Future Enhancements

- Structure-preserving extraction
- Markdown conversion for all document types
- LLM-guided chunking
- Context-aware chunking with summaries
- Multi-modal support (images, tables)
- Adaptive chunk sizing based on content complexity
- Chunking quality metrics

## Support

For issues or questions:
- GitHub: https://github.com/anthropics/claude-code/issues
- Check logs in worker-collections for detailed error messages
- Monitor transform status via API

---

**Version:** 1.0
**Last Updated:** 2026-01-05
**Status:** Production Ready ✓
