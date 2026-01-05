use anyhow::{Result, anyhow};
use scraper::Html;

pub(crate) fn extract_text_from_html(bytes: &[u8]) -> Result<String> {
    let html = std::str::from_utf8(bytes).map_err(|error| anyhow!(error.to_string()))?;
    let fragment = Html::parse_fragment(html);
    let mut result = String::new();
    for text in fragment.root_element().text() {
        let cleaned_text = text.trim();
        if !cleaned_text.is_empty() {
            result.push_str(cleaned_text);
        }
    }
    Ok(result)
}
