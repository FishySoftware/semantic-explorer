use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashMap;
use stop_words::{get, LANGUAGE};
use unicode_segmentation::UnicodeSegmentation;

/// TF-IDF topic namer that generates descriptive labels for document clusters
pub struct TfidfTopicNamer {
    top_n_keywords: usize,
    stemmer: Stemmer,
    stop_words: Vec<String>,
}

impl TfidfTopicNamer {
    /// Create a new TF-IDF topic namer with default settings
    pub fn new() -> Self {
        Self::with_keywords(5)
    }

    /// Create a new TF-IDF topic namer with custom number of keywords
    pub fn with_keywords(top_n_keywords: usize) -> Self {
        let stemmer = Stemmer::create(Algorithm::English);
        let stop_words = get(LANGUAGE::English);

        Self {
            top_n_keywords,
            stemmer,
            stop_words,
        }
    }

    /// Generate a topic label from a list of document texts
    pub fn generate_topic_label(&self, documents: &[String]) -> String {
        if documents.is_empty() {
            return "Empty Topic".to_string();
        }

        // Tokenize all documents
        let tokens: Vec<String> = documents
            .iter()
            .flat_map(|doc| {
                doc.unicode_words()
                    .map(|word| word.to_lowercase())
                    .filter(|word| word.len() > 2 && !self.stop_words.contains(word))
                    .map(|word| self.stemmer.stem(&word).to_string())
            })
            .collect();

        if tokens.is_empty() {
            return "No Keywords".to_string();
        }

        // Calculate term frequency
        let tf = self.calculate_tf(&tokens);

        // For a single cluster, we use TF directly (no IDF across clusters)
        // Sort by frequency descending
        let mut tf_scores: Vec<(String, f64)> = tf.into_iter().collect();
        tf_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top keywords
        let keywords: Vec<String> = tf_scores
            .into_iter()
            .take(self.top_n_keywords)
            .map(|(term, _)| term)
            .collect();

        if keywords.is_empty() {
            "No Keywords".to_string()
        } else {
            keywords.join(", ")
        }
    }

    fn calculate_tf(&self, tokens: &[String]) -> HashMap<String, f64> {
        let mut tf = HashMap::new();
        let total_terms = tokens.len() as f64;

        if total_terms == 0.0 {
            return tf;
        }

        for token in tokens {
            *tf.entry(token.clone()).or_insert(0.0) += 1.0;
        }

        for count in tf.values_mut() {
            *count /= total_terms;
        }

        tf
    }
}

impl Default for TfidfTopicNamer {
    fn default() -> Self {
        Self::new()
    }
}
