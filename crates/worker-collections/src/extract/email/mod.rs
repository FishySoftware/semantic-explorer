use anyhow::{Result, anyhow};
use mail_parser::{MessageParser, MimeHeaders, PartType};
use serde_json::{Value, json};

use crate::extract::config::ExtractionOptions;

/// Result of email extraction
#[derive(Debug)]
pub struct EmailExtractionResult {
    /// Combined text from email body and optionally attachments
    pub text: String,
    /// Email metadata
    pub metadata: Option<Value>,
}

/// Extracted email attachment
#[derive(Debug)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub content: Vec<u8>,
    pub size: usize,
}

/// Configuration for email extraction
#[derive(Debug, Clone)]
pub struct EmailOptions {
    /// Whether to recursively extract text from attachments
    pub extract_attachments: bool,
    /// Whether to flatten attachment text into main text or keep separate
    pub flatten_attachments: bool,
    /// Maximum attachment size to process (in bytes)
    pub max_attachment_size: usize,
    /// File extensions to skip for attachments
    pub skip_extensions: Vec<String>,
}

impl Default for EmailOptions {
    fn default() -> Self {
        Self {
            extract_attachments: true,
            flatten_attachments: true,
            max_attachment_size: 10 * 1024 * 1024, // 10MB
            skip_extensions: vec![
                "exe".into(),
                "dll".into(),
                "so".into(),
                "png".into(),
                "jpg".into(),
                "jpeg".into(),
                "gif".into(),
                "mp3".into(),
                "mp4".into(),
                "wav".into(),
            ],
        }
    }
}

/// Extract text from email with full options
pub(crate) fn extract_with_options(
    bytes: &[u8],
    options: &ExtractionOptions,
    email_opts: &EmailOptions,
) -> Result<EmailExtractionResult> {
    let message = MessageParser::default()
        .parse(bytes)
        .ok_or_else(|| anyhow!("Failed to parse email message"))?;

    let mut text_parts = Vec::new();
    let mut attachments = Vec::new();

    // Extract headers
    let subject = message.subject().unwrap_or("");
    let from = message
        .from()
        .map(|f| f.first().map(|a| a.address().unwrap_or("")).unwrap_or(""))
        .unwrap_or("");
    let to = message
        .to()
        .map(|t| {
            t.iter()
                .filter_map(|a| a.address())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let date = message.date().map(|d| d.to_rfc3339()).unwrap_or_default();

    // Add header information
    if !subject.is_empty() {
        text_parts.push(format!("Subject: {}", subject));
    }
    if !from.is_empty() {
        text_parts.push(format!("From: {}", from));
    }
    if !to.is_empty() {
        text_parts.push(format!("To: {}", to));
    }
    if !date.is_empty() {
        text_parts.push(format!("Date: {}", date));
    }

    text_parts.push(String::new()); // Separator

    // Extract body parts
    for part in message.parts.iter() {
        match &part.body {
            PartType::Text(text) => {
                text_parts.push(text.to_string());
            }
            PartType::Html(html) => {
                // Extract text from HTML
                let html_text = extract_text_from_html(html.as_bytes());
                text_parts.push(html_text);
            }
            PartType::Binary(binary) | PartType::InlineBinary(binary) => {
                if email_opts.extract_attachments {
                    let filename = part
                        .attachment_name()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "attachment".to_string());

                    let content_type = part
                        .content_type()
                        .map(|ct| ct.ctype())
                        .unwrap_or("application/octet-stream")
                        .to_string();

                    // Check size and extension
                    if binary.len() <= email_opts.max_attachment_size
                        && !should_skip_attachment(&filename, email_opts)
                    {
                        attachments.push(EmailAttachment {
                            filename,
                            content_type,
                            size: binary.len(),
                            content: binary.to_vec(),
                        });
                    }
                }
            }
            PartType::Message(nested_msg) => {
                // Handle forwarded/attached emails
                if let Some(nested) = MessageParser::default().parse(nested_msg.raw_message()) {
                    if let Some(nested_subject) = nested.subject() {
                        text_parts.push(format!(
                            "\n--- Forwarded Message ---\nSubject: {}",
                            nested_subject
                        ));
                    }
                    for nested_part in nested.parts.iter() {
                        if let PartType::Text(text) = &nested_part.body {
                            text_parts.push(text.to_string());
                        }
                    }
                }
            }
            PartType::Multipart(_) => {
                // Multipart containers are handled by iterating parts
            }
        }
    }

    // Process attachments for text extraction
    let attachment_texts = if email_opts.flatten_attachments && !attachments.is_empty() {
        process_attachments(&attachments)
    } else {
        Vec::new()
    };

    // Combine text
    let mut full_text = text_parts.join("\n");
    if !attachment_texts.is_empty() {
        full_text.push_str("\n\n--- Attachments ---\n");
        full_text.push_str(&attachment_texts.join("\n\n"));
    }

    // Build metadata
    let metadata = if options.include_metadata {
        Some(json!({
            "format": "email",
            "subject": subject,
            "from": from,
            "to": to,
            "date": date,
            "attachment_count": attachments.len(),
            "attachments": attachments.iter().map(|a| json!({
                "filename": a.filename,
                "content_type": a.content_type,
                "size": a.size,
            })).collect::<Vec<_>>(),
            "message_id": message.message_id().unwrap_or(""),
        }))
    } else {
        None
    };

    Ok(EmailExtractionResult {
        text: full_text,
        metadata,
    })
}

/// Check if attachment should be skipped
fn should_skip_attachment(filename: &str, options: &EmailOptions) -> bool {
    if let Some(ext) = filename.rsplit('.').next() {
        options
            .skip_extensions
            .iter()
            .any(|skip| skip.eq_ignore_ascii_case(ext))
    } else {
        false
    }
}

/// Simple HTML text extraction for email bodies
fn extract_text_from_html(html: &[u8]) -> String {
    let html_str = String::from_utf8_lossy(html);

    // Remove script and style tags with content
    let no_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>")
        .map(|re| re.replace_all(&html_str, "").to_string())
        .unwrap_or_else(|_| html_str.to_string());
    let no_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>")
        .map(|re| re.replace_all(&no_script, "").to_string())
        .unwrap_or(no_script);

    // Replace common HTML elements with appropriate spacing
    let with_breaks = no_style
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n")
        .replace("</div>", "\n")
        .replace("</li>", "\n")
        .replace("</tr>", "\n");

    // Remove remaining HTML tags
    let no_tags = regex::Regex::new(r"<[^>]+>")
        .map(|re| re.replace_all(&with_breaks, "").to_string())
        .unwrap_or(with_breaks);

    // Decode common HTML entities
    no_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Process attachments to extract text where possible
fn process_attachments(attachments: &[EmailAttachment]) -> Vec<String> {
    let mut texts = Vec::new();

    for attachment in attachments {
        // Try to extract text based on content type
        let extracted = match attachment.content_type.as_str() {
            "text/plain" => Some(String::from_utf8_lossy(&attachment.content).to_string()),
            "text/html" => Some(extract_text_from_html(&attachment.content)),
            "text/csv" => Some(String::from_utf8_lossy(&attachment.content).to_string()),
            _ => None,
        };

        if let Some(text) = extracted
            && !text.trim().is_empty()
        {
            texts.push(format!("[{}]\n{}", attachment.filename, text));
        }
    }

    texts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_simple_email() -> Vec<u8> {
        b"From: sender@example.com\r\n\
          To: recipient@example.com\r\n\
          Subject: Test Email\r\n\
          Date: Mon, 15 Jan 2024 10:30:00 +0000\r\n\
          Content-Type: text/plain\r\n\
          \r\n\
          Hello, this is the email body.\r\n\
          Best regards,\r\n\
          Sender"
            .to_vec()
    }

    fn create_html_email() -> Vec<u8> {
        b"From: sender@example.com\r\n\
          To: recipient@example.com\r\n\
          Subject: HTML Email\r\n\
          Content-Type: text/html\r\n\
          \r\n\
          <html><body><h1>Hello</h1><p>This is <b>HTML</b> content.</p></body></html>"
            .to_vec()
    }

    #[test]
    fn test_extract_simple_email() {
        let email = create_simple_email();
        let result = extract_with_options(
            &email,
            &ExtractionOptions::default(),
            &EmailOptions::default(),
        );
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("Subject: Test Email"));
        assert!(text.contains("Hello, this is the email body"));
    }

    #[test]
    fn test_extract_email_headers() {
        let email = create_simple_email();
        let mut options = ExtractionOptions::default();
        options.include_metadata = true;
        let email_opts = EmailOptions::default();

        let result = extract_with_options(&email, &options, &email_opts);
        assert!(result.is_ok());
        let extraction = result.unwrap();

        let meta = extraction.metadata.unwrap();
        assert_eq!(meta["subject"], "Test Email");
        assert_eq!(meta["from"], "sender@example.com");
    }

    #[test]
    fn test_extract_html_email() {
        let email = create_html_email();
        let result = extract_with_options(
            &email,
            &ExtractionOptions::default(),
            &EmailOptions::default(),
        );
        assert!(result.is_ok());
        let text = result.unwrap().text;
        assert!(text.contains("Hello"));
        assert!(text.contains("HTML content"));
        // HTML tags should be stripped
        assert!(!text.contains("<html>"));
    }

    #[test]
    fn test_extract_text_from_html() {
        let html = b"<html><head><style>body{}</style></head><body><p>Hello</p><br/><p>World</p></body></html>";
        let text = extract_text_from_html(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("<p>"));
        assert!(!text.contains("body{}"));
    }

    #[test]
    fn test_skip_binary_attachments() {
        let options = EmailOptions::default();
        assert!(should_skip_attachment("file.exe", &options));
        assert!(should_skip_attachment("image.png", &options));
        assert!(!should_skip_attachment("document.txt", &options));
        assert!(!should_skip_attachment("data.pdf", &options));
    }

    #[test]
    fn test_html_entity_decoding() {
        let html = b"Hello &amp; World &lt;test&gt;";
        let text = extract_text_from_html(html);
        assert_eq!(text, "Hello & World <test>");
    }

    #[test]
    fn test_minimal_email() {
        // The mail-parser is lenient and will parse most content
        // Test with truly minimal/empty content
        let minimal = b"";
        let result = extract_with_options(
            minimal,
            &ExtractionOptions::default(),
            &EmailOptions::default(),
        );
        // Empty content should fail to parse
        assert!(result.is_err());
    }

    #[test]
    fn test_email_options_no_attachments() {
        let email = create_simple_email();
        let options = ExtractionOptions::default();
        let email_opts = EmailOptions {
            extract_attachments: false,
            ..Default::default()
        };

        let result = extract_with_options(&email, &options, &email_opts);
        assert!(result.is_ok());
        // Verify extraction still works with attachments disabled
        let extraction = result.unwrap();
        assert!(extraction.text.contains("Hello, this is the email body"));
    }
}
