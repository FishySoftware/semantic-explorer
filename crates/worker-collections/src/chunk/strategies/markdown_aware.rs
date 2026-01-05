use anyhow::Result;

use crate::chunk::config::ChunkingConfig;
use crate::chunk::metadata::{ChunkWithStructure, StructureInfo};

pub fn chunk(text: String, config: &ChunkingConfig) -> Result<Vec<ChunkWithStructure>> {
    let opts = config
        .options
        .markdown_aware
        .as_ref()
        .cloned()
        .unwrap_or_default();

    if opts.split_on_headers {
        chunk_by_headers(text, config, opts.preserve_code_blocks)
    } else {
        // chunk_preserving_blocks doesn't extract structure
        let chunks = chunk_preserving_blocks(text, config, opts.preserve_code_blocks)?;
        Ok(chunks
            .into_iter()
            .map(|content| ChunkWithStructure {
                content,
                structure_info: None,
            })
            .collect())
    }
}

fn chunk_by_headers(
    text: String,
    config: &ChunkingConfig,
    _preserve_code_blocks: bool,
) -> Result<Vec<ChunkWithStructure>> {
    let mut chunks = Vec::new();
    let mut heading_hierarchy: Vec<String> = Vec::new();
    let mut current_chunk = String::new();
    let mut in_code_block = false;
    let mut code_block_fence = String::new();

    for line in text.lines() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            if !in_code_block {
                in_code_block = true;
                code_block_fence = line
                    .trim_start()
                    .chars()
                    .take_while(|&c| c == '`' || c == '~')
                    .collect();
            } else if line.trim_start().starts_with(&code_block_fence) {
                in_code_block = false;
                code_block_fence.clear();
            }
        }

        let is_header = !in_code_block && line.trim_start().starts_with('#');

        if is_header {
            let header_text = line.trim_start();
            let level = header_text.chars().take_while(|&c| c == '#').count();
            let title = header_text[level..].trim().to_string();
            if level > 0 && level <= 6 {
                heading_hierarchy.truncate(level - 1);
                if level <= heading_hierarchy.len() + 1 {
                    if heading_hierarchy.len() >= level {
                        heading_hierarchy[level - 1] = title.clone();
                    } else {
                        heading_hierarchy.push(title.clone());
                    }
                }
            }

            if !current_chunk.is_empty() {
                if current_chunk.trim().len() >= config.options.min_chunk_size {
                    let structure_info = if !heading_hierarchy.is_empty() {
                        Some(StructureInfo {
                            heading_hierarchy: Some(heading_hierarchy.clone()),
                            section_title: heading_hierarchy.last().cloned(),
                            page_number: None,
                        })
                    } else {
                        None
                    };

                    chunks.push(ChunkWithStructure {
                        content: current_chunk.trim().to_string(),
                        structure_info,
                    });
                    current_chunk.clear();
                } else if !chunks.is_empty() && !current_chunk.trim().is_empty() {
                    let last = chunks.last_mut().unwrap();
                    last.content.push('\n');
                    last.content.push_str(current_chunk.trim());
                    current_chunk.clear();
                }
            }
        }

        if !current_chunk.is_empty() {
            current_chunk.push('\n');
        }
        current_chunk.push_str(line);

        if !in_code_block
            && current_chunk.len() > config.chunk_size
            && current_chunk.trim().len() >= config.options.min_chunk_size
        {
            let structure_info = if !heading_hierarchy.is_empty() {
                Some(StructureInfo {
                    heading_hierarchy: Some(heading_hierarchy.clone()),
                    section_title: heading_hierarchy.last().cloned(),
                    page_number: None,
                })
            } else {
                None
            };

            chunks.push(ChunkWithStructure {
                content: current_chunk.trim().to_string(),
                structure_info,
            });
            current_chunk.clear();
        }
    }

    if !current_chunk.trim().is_empty() {
        if current_chunk.trim().len() >= config.options.min_chunk_size || chunks.is_empty() {
            let structure_info = if !heading_hierarchy.is_empty() {
                Some(StructureInfo {
                    heading_hierarchy: Some(heading_hierarchy.clone()),
                    section_title: heading_hierarchy.last().cloned(),
                    page_number: None,
                })
            } else {
                None
            };

            chunks.push(ChunkWithStructure {
                content: current_chunk.trim().to_string(),
                structure_info,
            });
        } else if !chunks.is_empty() {
            let last = chunks.last_mut().unwrap();
            last.content.push('\n');
            last.content.push_str(current_chunk.trim());
        }
    }

    Ok(chunks)
}

fn chunk_preserving_blocks(
    text: String,
    config: &ChunkingConfig,
    preserve_code_blocks: bool,
) -> Result<Vec<String>> {
    if !preserve_code_blocks {
        return crate::chunk::strategies::fixed_size::chunk(text, config);
    }

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut code_block_fence = String::new();

    for line in text.lines() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            if !in_code_block {
                in_code_block = true;
                code_block_fence = line
                    .trim_start()
                    .chars()
                    .take_while(|&c| c == '`' || c == '~')
                    .collect();
                code_block_content = line.to_string();
            } else if line.trim_start().starts_with(&code_block_fence) {
                in_code_block = false;
                code_block_content.push('\n');
                code_block_content.push_str(line);

                if current_chunk.len() + code_block_content.len() > config.chunk_size
                    && !current_chunk.is_empty()
                {
                    chunks.push(current_chunk.trim().to_string());
                    current_chunk = code_block_content.clone();
                } else {
                    if !current_chunk.is_empty() {
                        current_chunk.push('\n');
                    }
                    current_chunk.push_str(&code_block_content);
                }

                code_block_content.clear();
                code_block_fence.clear();
                continue;
            } else {
                code_block_content.push('\n');
                code_block_content.push_str(line);
                continue;
            }
        }

        if in_code_block {
            code_block_content.push('\n');
            code_block_content.push_str(line);
        } else {
            if current_chunk.len() + line.len() > config.chunk_size && !current_chunk.is_empty() {
                chunks.push(current_chunk.trim().to_string());
                current_chunk.clear();
            }

            if !current_chunk.is_empty() {
                current_chunk.push('\n');
            }
            current_chunk.push_str(line);
        }
    }

    if !code_block_content.is_empty() {
        if !current_chunk.is_empty() {
            current_chunk.push('\n');
        }
        current_chunk.push_str(&code_block_content);
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::config::{ChunkingConfig, ChunkingOptions, MarkdownAwareOptions};

    fn create_config(chunk_size: usize, split_on_headers: bool) -> ChunkingConfig {
        ChunkingConfig {
            chunk_size,
            chunk_overlap: 0,
            options: ChunkingOptions {
                markdown_aware: Some(MarkdownAwareOptions {
                    split_on_headers,
                    preserve_code_blocks: true,
                }),
                min_chunk_size: 10,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_markdown_split_on_headers() {
        let text = "# Header 1\nContent 1\n\n## Header 2\nContent 2\n\n### Header 3\nContent 3"
            .to_string();
        let config = create_config(100, true);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Should split on headers
        assert!(chunks.len() >= 3);
    }

    #[test]
    fn test_markdown_no_split_on_headers() {
        let text = "# Header 1\nContent\n## Header 2\nMore content".to_string();
        let config = create_config(50, false);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_markdown_preserve_code_blocks() {
        let text =
            "Text before\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\nText after"
                .to_string();
        let config = create_config(100, false);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
        // Code block should be kept together
        let has_code_block = chunks.iter().any(|c| c.content.contains("fn main()"));
        assert!(has_code_block);
    }

    #[test]
    fn test_markdown_nested_code_blocks() {
        let text = "# Code Example\n```python\ndef hello():\n    print('world')\n```\n## Another Example\n```javascript\nconsole.log('test');\n```".to_string();
        let config = create_config(200, true);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_markdown_empty_text() {
        let text = String::new();
        let config = create_config(100, true);

        let result = chunk(text, &config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_markdown_no_headers() {
        let text = "Just plain text without any markdown headers.".to_string();
        let config = create_config(100, true);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_markdown_multiple_levels() {
        let text = "# H1\nContent\n## H2\nMore\n### H3\nEven more\n#### H4\nAnd more".to_string();
        let config = create_config(50, true);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Should split on each header
        assert!(chunks.len() >= 4);
    }

    #[test]
    fn test_markdown_lists() {
        let text = "# List Example\n- Item 1\n- Item 2\n- Item 3\n\n## Another List\n1. First\n2. Second\n3. Third".to_string();
        let config = create_config(100, true);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_markdown_links_and_images() {
        let text =
            "# Section\n[Link text](https://example.com)\n![Alt text](image.png)".to_string();
        let config = create_config(100, true);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_markdown_blockquotes() {
        let text =
            "# Quote\n> This is a quote\n> Spanning multiple lines\n\nRegular text".to_string();
        let config = create_config(100, true);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_markdown_horizontal_rules() {
        let text = "Section 1\n\n---\n\nSection 2\n\n***\n\nSection 3".to_string();
        let config = create_config(100, false);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_markdown_inline_code() {
        let text = "Use `code` in text. And `more code` here.".to_string();
        let config = create_config(100, false);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("`code`"));
    }

    #[test]
    fn test_markdown_tilde_code_blocks() {
        let text = "~~~python\nprint('hello')\n~~~".to_string();
        let config = create_config(100, false);

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_markdown_min_chunk_size() {
        let text = "# H1\nA\n## H2\nB\n### H3\nC".to_string();
        let mut config = create_config(50, true);
        config.options.min_chunk_size = 10;

        let result = chunk(text, &config);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        // Small chunks should be merged
        for chunk in &chunks {
            assert!(chunk.content.len() >= 10 || chunks.len() == 1);
        }
    }
}
