use anyhow::Result;
use semantic_explorer_core::http_client::HTTP_CLIENT;

pub async fn generate_embedding(
    provider: &str,
    base_url: &str,
    api_key: Option<&str>,
    config: &serde_json::Value,
    query: &str,
) -> Result<Vec<f32>> {
    let client = &*HTTP_CLIENT;

    let (url, body, needs_auth) = match provider {
        "openai" => {
            if base_url.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "Invalid embedder configuration: base_url cannot be empty for OpenAI provider"
                ));
            }
            let model = config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("text-embedding-3-small");
            let endpoint = format!("{}/embeddings", base_url.trim_end_matches('/'));
            let body = serde_json::json!({
                "input": query,
                "model": model,
            });
            (endpoint, body, true)
        }
        "cohere" => {
            if base_url.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "Invalid embedder configuration: base_url cannot be empty for Cohere provider"
                ));
            }
            let model = config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("embed-english-v3.0");
            let endpoint = base_url.to_string();
            let body = serde_json::json!({
                "texts": [query],
                "model": model,
                "input_type": "search_query",
            });
            (endpoint, body, true)
        }
        "internal" => {
            // Internal inference via embedding-inference-api service uses EMBEDDING_INFERENCE_API_URL environment variable
            let model = config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("BAAI/bge-small-en-v1.5");
            let inference_url = std::env::var("EMBEDDING_INFERENCE_API_URL")
                .unwrap_or_else(|_| "http://localhost:8090".to_string());
            let endpoint = format!("{}/api/embed", inference_url.trim_end_matches('/'));
            let body = serde_json::json!({
                "text": query,
                "model": model,
            });
            (endpoint, body, false)
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported embedding provider: {}",
                provider
            ));
        }
    };

    let mut req = client.post(&url).json(&body);
    if needs_auth {
        if let Some(key) = api_key {
            req = req.bearer_auth(key);
        } else {
            return Err(anyhow::anyhow!(
                "API key required for provider: {}",
                provider
            ));
        }
    }

    let resp: reqwest::Response = req.send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await?;
        return Err(anyhow::anyhow!("Embedding API error {}: {}", status, text));
    }

    let response: serde_json::Value = resp.json().await?;

    match provider {
        "openai" => {
            let embedding = response
                .get("data")
                .and_then(|d| d.get(0))
                .and_then(|e| e.get("embedding"))
                .and_then(|e| serde_json::from_value(e.clone()).ok())
                .ok_or_else(|| anyhow::anyhow!("Invalid OpenAI response format"))?;
            Ok(embedding)
        }
        "cohere" => {
            let embeddings = response
                .get("embeddings")
                .ok_or_else(|| anyhow::anyhow!("Missing embeddings in Cohere response"))?;

            if let Some(float_emb) = embeddings.get("float") {
                let arr: Vec<Vec<f32>> = serde_json::from_value(float_emb.clone())?;
                Ok(arr.into_iter().next().unwrap_or_default())
            } else if let Some(arr) = embeddings.as_array() {
                serde_json::from_value(arr[0].clone())
                    .map_err(|e| anyhow::anyhow!("Failed to parse Cohere embedding: {}", e))
            } else {
                Err(anyhow::anyhow!("Invalid Cohere response format"))
            }
        }
        "internal" => {
            // Internal embedding-inference-api returns embeddings array
            let embeddings = response
                .get("embeddings")
                .and_then(|e| e.as_array())
                .and_then(|arr| arr.first())
                .and_then(|e| serde_json::from_value(e.clone()).ok())
                .ok_or_else(|| anyhow::anyhow!("Invalid local inference response format"))?;
            Ok(embeddings)
        }
        _ => Err(anyhow::anyhow!("Unsupported provider: {}", provider)),
    }
}
