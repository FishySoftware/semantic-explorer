use std::collections::HashMap;

/// Model metadata service for provider-specific model information
pub struct ModelMetadataService {
    dimension_map: HashMap<String, usize>,
}

impl ModelMetadataService {
    pub fn new() -> Self {
        let mut dimension_map = HashMap::new();
        // OpenAI models
        dimension_map.insert("text-embedding-3-small".to_string(), 1536);
        dimension_map.insert("text-embedding-3-large".to_string(), 3072);
        dimension_map.insert("text-embedding-ada-002".to_string(), 1536);

        // Cohere models
        dimension_map.insert("embed-v4.0".to_string(), 1536);
        dimension_map.insert("embed-english-v3.0".to_string(), 1024);
        dimension_map.insert("embed-multilingual-v3.0".to_string(), 1024);
        dimension_map.insert("embed-english-light-v3.0".to_string(), 384);
        dimension_map.insert("embed-multilingual-light-v3.0".to_string(), 384);
        dimension_map.insert("embed-english-v2.0".to_string(), 4096);
        dimension_map.insert("embed-english-light-v2.0".to_string(), 1024);
        dimension_map.insert("embed-multilingual-v2.0".to_string(), 768);

        Self { dimension_map }
    }

    /// Get known dimensions for a model, returns None if unknown
    pub fn get_dimensions(&self, model: &str) -> Option<usize> {
        self.dimension_map.get(model).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_dimensions() {
        let service = ModelMetadataService::new();

        assert_eq!(service.get_dimensions("text-embedding-3-small"), Some(1536));
        assert_eq!(service.get_dimensions("embed-v4.0"), Some(1536));
        assert_eq!(service.get_dimensions("unknown-model"), None);
    }
}
