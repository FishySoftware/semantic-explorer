use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashMap;
use stop_words::{LANGUAGE, get};
use unicode_segmentation::UnicodeSegmentation;

/// TF-IDF topic namer that calculates true TF-IDF across all clusters
pub struct TfidfTopicNamer {
    pub top_n_keywords: usize,
    #[allow(dead_code)]
    stemmer: Stemmer,
    #[allow(dead_code)]
    stop_words: Vec<String>,
    all_cluster_documents: Vec<Vec<String>>,
}

impl TfidfTopicNamer {
    /// Create a new TF-IDF topic namer
    ///
    /// # Arguments
    /// * `top_n_keywords` - Number of top keywords to include in the name
    /// * `all_clusters` - All documents grouped by cluster for IDF calculation
    pub fn new(top_n_keywords: usize, all_clusters: &[Vec<&str>]) -> Self {
        let stemmer = Stemmer::create(Algorithm::English);
        let stop_words = get(LANGUAGE::English);

        // Pre-tokenize all clusters for IDF calculation
        let all_cluster_documents: Vec<Vec<String>> = all_clusters
            .iter()
            .map(|cluster_docs| {
                cluster_docs
                    .into_iter()
                    .flat_map(|doc| {
                        doc.unicode_words()
                            .map(|word| word.to_lowercase())
                            .filter(|word| word.len() > 2 && !stop_words.contains(word))
                            .map(|word| stemmer.stem(&word).to_string())
                    })
                    .collect()
            })
            .collect();

        Self {
            top_n_keywords,
            stemmer,
            stop_words,
            all_cluster_documents,
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

    fn calculate_idf(&self) -> HashMap<String, f64> {
        let n_clusters = self.all_cluster_documents.len() as f64;
        let mut document_frequency: HashMap<String, usize> = HashMap::new();

        for cluster_tokens in &self.all_cluster_documents {
            let unique_terms: std::collections::HashSet<_> = cluster_tokens.iter().collect();
            for term in unique_terms {
                *document_frequency.entry(term.clone()).or_insert(0) += 1;
            }
        }

        let mut idf = HashMap::new();
        for (term, df) in document_frequency {
            idf.insert(term, (n_clusters / (1.0 + df as f64)).ln());
        }

        idf
    }

    pub fn generate_name_for_cluster(&self, cluster_index: usize) -> String {
        if cluster_index >= self.all_cluster_documents.len() {
            return "Invalid Cluster".to_string();
        }

        let tokens = &self.all_cluster_documents[cluster_index];

        if tokens.is_empty() {
            return "Empty Topic".to_string();
        }

        let tf = self.calculate_tf(tokens);
        let idf = self.calculate_idf();

        let mut tfidf_scores: Vec<(String, f64)> = tf
            .iter()
            .map(|(term, &tf_val)| {
                let idf_val = idf.get(term).copied().unwrap_or(0.0);
                (term.clone(), tf_val * idf_val)
            })
            .collect();

        tfidf_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let keywords: Vec<String> = tfidf_scores
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
}
