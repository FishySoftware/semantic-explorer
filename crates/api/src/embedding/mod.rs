use anyhow::Result;
use once_cell::sync::Lazy;
use std::time::Duration;

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to build HTTP client")
});

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
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported embedding provider: {}",
                provider
            ))
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
        return Err(anyhow::anyhow!(
            "Embedding API error {}: {}",
            status,
            text
        ));
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
        _ => Err(anyhow::anyhow!("Unsupported provider: {}", provider)),
    }
}
