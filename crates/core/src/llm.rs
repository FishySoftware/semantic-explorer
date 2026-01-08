use async_trait::async_trait;
use rust_stemmers::{Algorithm, Stemmer};
use std::{
    collections::HashMap,
    error::Error,
    fmt::{Display, Formatter},
};
use stop_words::{LANGUAGE, get};
use unicode_segmentation::UnicodeSegmentation;

#[async_trait]
pub trait TopicNamer: Send + Sync {
    async fn generate_topic_label(&self, documents: &[String]) -> Result<String, TopicNamingError>;
}

#[derive(Debug, Clone)]
pub enum TopicNamingError {
    ApiError(String),
    NetworkError(String),
    ValidationError(String),
    InternalError(String),
}

impl Display for TopicNamingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TopicNamingError::ApiError(msg) => write!(f, "API Error: {}", msg),
            TopicNamingError::NetworkError(msg) => write!(f, "Network Error: {}", msg),
            TopicNamingError::ValidationError(msg) => write!(f, "Validation Error: {}", msg),
            TopicNamingError::InternalError(msg) => write!(f, "Internal Error: {}", msg),
        }
    }
}

impl Error for TopicNamingError {}

pub struct TfidfTopicNamer {
    top_n_keywords: usize,
    stemmer: Stemmer,
    stop_words: Vec<String>,
}

impl TfidfTopicNamer {
    pub fn new() -> Self {
        Self::with_keywords(5)
    }

    pub fn with_keywords(top_n_keywords: usize) -> Self {
        let stemmer = Stemmer::create(Algorithm::English);
        let stop_words = get(LANGUAGE::English)
            .iter()
            .map(|s| s.to_string())
            .collect();

        Self {
            top_n_keywords,
            stemmer,
            stop_words,
        }
    }

    pub fn generate_topic_label_sync(&self, documents: &[String]) -> String {
        if documents.is_empty() {
            return "Empty Topic".to_string();
        }

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

        let tf = self.calculate_tf(&tokens);

        let mut tf_scores: Vec<(String, f64)> = tf.into_iter().collect();
        tf_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

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

#[async_trait]
impl TopicNamer for TfidfTopicNamer {
    async fn generate_topic_label(&self, documents: &[String]) -> Result<String, TopicNamingError> {
        Ok(self.generate_topic_label_sync(documents))
    }
}

pub struct LLMTopicNamer {
    provider: String,
    api_url: String,
    api_key: String,
    model: String,
    http_client: reqwest::Client,
    temperature: f32,
}

impl LLMTopicNamer {
    pub fn new(provider: String, api_url: String, api_key: String, model: String) -> Self {
        Self {
            provider,
            api_url,
            api_key,
            model,
            http_client: reqwest::Client::new(),
            temperature: 0.3,
        }
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature.clamp(0.0, 2.0);
        self
    }

    fn create_prompt(&self, documents: &[String]) -> String {
        let sample_docs = documents
            .iter()
            .take(10) // Use first 10 docs as samples
            .map(|doc| {
                if doc.len() > 200 {
                    format!("{}...", &doc[..200])
                } else {
                    doc.clone()
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        format!(
            "Given these document excerpts representing a topic cluster, generate a concise and descriptive topic label (3-5 words maximum) that captures the main theme.\n\nDocuments:\n{}\n\nGenerate only the topic label, nothing else.",
            sample_docs
        )
    }
}

#[async_trait]
impl TopicNamer for LLMTopicNamer {
    async fn generate_topic_label(&self, documents: &[String]) -> Result<String, TopicNamingError> {
        if documents.is_empty() {
            return Ok("Empty Topic".to_string());
        }

        let prompt = self.create_prompt(documents);

        match self.provider.to_lowercase().as_str() {
            "openai" => self.call_openai(&prompt).await,
            "cohere" => self.call_cohere(&prompt).await,
            _ => Err(TopicNamingError::ValidationError(format!(
                "Unknown provider: {}",
                self.provider
            ))),
        }
    }
}

impl LLMTopicNamer {
    async fn call_openai(&self, prompt: &str) -> Result<String, TopicNamingError> {
        #[derive(serde::Serialize)]
        struct OpenAIRequest {
            model: String,
            messages: Vec<OpenAIMessage>,
            temperature: f32,
            max_tokens: u32,
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        struct OpenAIMessage {
            role: String,
            content: String,
        }

        #[derive(serde::Deserialize)]
        struct OpenAIResponse {
            choices: Vec<OpenAIChoice>,
        }

        #[derive(serde::Deserialize)]
        struct OpenAIChoice {
            message: OpenAIMessage,
        }

        let request_body = OpenAIRequest {
            model: self.model.clone(),
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            temperature: self.temperature,
            max_tokens: 20,
        };

        let response = self
            .http_client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| TopicNamingError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(TopicNamingError::ApiError(format!(
                "OpenAI API error: {}",
                error_text
            )));
        }

        let response_data: OpenAIResponse = response.json().await.map_err(|e| {
            TopicNamingError::ApiError(format!("Failed to parse OpenAI response: {}", e))
        })?;

        response_data
            .choices
            .first()
            .map(|choice| choice.message.content.trim().to_string())
            .ok_or_else(|| TopicNamingError::ApiError("No choices in OpenAI response".to_string()))
    }

    async fn call_cohere(&self, prompt: &str) -> Result<String, TopicNamingError> {
        #[derive(serde::Serialize)]
        struct CohereRequest {
            prompt: String,
            model: String,
            max_tokens: u32,
            temperature: f32,
        }

        #[derive(serde::Deserialize)]
        struct CohereResponse {
            generations: Vec<CohereGeneration>,
        }

        #[derive(serde::Deserialize)]
        struct CohereGeneration {
            text: String,
        }

        let request_body = CohereRequest {
            prompt: prompt.to_string(),
            model: self.model.clone(),
            max_tokens: 20,
            temperature: self.temperature,
        };

        let response = self
            .http_client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| TopicNamingError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(TopicNamingError::ApiError(format!(
                "Cohere API error: {}",
                error_text
            )));
        }

        let response_data: CohereResponse = response.json().await.map_err(|e| {
            TopicNamingError::ApiError(format!("Failed to parse Cohere response: {}", e))
        })?;

        response_data
            .generations
            .first()
            .map(|generation| generation.text.trim().to_string())
            .ok_or_else(|| {
                TopicNamingError::ApiError("No generations in Cohere response".to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tfidf_namer_basic() {
        let namer = TfidfTopicNamer::new();
        let docs = vec![
            "machine learning algorithms and neural networks".to_string(),
            "deep learning and artificial intelligence".to_string(),
            "training neural networks with backpropagation".to_string(),
        ];

        let label = namer.generate_topic_label_sync(&docs);
        assert!(!label.is_empty());
        assert!(!label.contains("machine")); // Stop word should be filtered
    }

    #[test]
    fn test_tfidf_namer_empty() {
        let namer = TfidfTopicNamer::new();
        let label = namer.generate_topic_label_sync(&[]);
        assert_eq!(label, "Empty Topic");
    }

    #[test]
    fn test_tfidf_namer_single_word() {
        let namer = TfidfTopicNamer::new();
        let docs = vec!["a".to_string(), "the".to_string()]; // All stop words
        let label = namer.generate_topic_label_sync(&docs);
        assert_eq!(label, "No Keywords");
    }
}
