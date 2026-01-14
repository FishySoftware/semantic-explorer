use std::fmt;

/// Structured error types for the extraction module
#[derive(Debug)]
pub enum ExtractionError {
    /// Unsupported MIME type for extraction
    UnsupportedMimeType {
        mime_type: String,
        context: Option<String>,
    },
    /// Failed to parse document content
    ParseError { format: String, message: String },
    /// Archive extraction error
    ArchiveError { format: String, message: String },
}

impl fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtractionError::UnsupportedMimeType { mime_type, context } => {
                write!(f, "Unsupported MIME type: {}", mime_type)?;
                if let Some(ctx) = context {
                    write!(f, " ({})", ctx)?;
                }
                Ok(())
            }
            ExtractionError::ParseError { format, message } => {
                write!(f, "Failed to parse {} document: {}", format, message)
            }
            ExtractionError::ArchiveError { format, message } => {
                write!(f, "Archive extraction error ({}): {}", format, message)
            }
        }
    }
}

impl std::error::Error for ExtractionError {}

impl ExtractionError {
    /// Create an unsupported MIME type error
    pub fn unsupported_mime(mime_type: impl Into<String>) -> Self {
        ExtractionError::UnsupportedMimeType {
            mime_type: mime_type.into(),
            context: None,
        }
    }

    /// Create an unsupported MIME type error with context
    pub fn unsupported_mime_with_context(
        mime_type: impl Into<String>,
        context: impl Into<String>,
    ) -> Self {
        ExtractionError::UnsupportedMimeType {
            mime_type: mime_type.into(),
            context: Some(context.into()),
        }
    }

    /// Create a parse error
    pub fn parse_error(format: impl Into<String>, message: impl Into<String>) -> Self {
        ExtractionError::ParseError {
            format: format.into(),
            message: message.into(),
        }
    }

    /// Create an archive error
    pub fn archive_error(format: impl Into<String>, message: impl Into<String>) -> Self {
        ExtractionError::ArchiveError {
            format: format.into(),
            message: message.into(),
        }
    }
}

pub type ExtractionResult<T> = Result<T, ExtractionError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_formatting_unsupported_mime() {
        let err = ExtractionError::unsupported_mime("application/unknown");
        assert_eq!(
            err.to_string(),
            "Unsupported MIME type: application/unknown"
        );
    }

    #[test]
    fn test_error_formatting_unsupported_mime_with_context() {
        let err = ExtractionError::unsupported_mime_with_context(
            "application/unknown",
            "detected during scan",
        );
        assert_eq!(
            err.to_string(),
            "Unsupported MIME type: application/unknown (detected during scan)"
        );
    }

    #[test]
    fn test_error_formatting_parse_error() {
        let err = ExtractionError::parse_error("JSON", "unexpected token");
        assert_eq!(
            err.to_string(),
            "Failed to parse JSON document: unexpected token"
        );
    }

    #[test]
    fn test_error_formatting_archive_error() {
        let err = ExtractionError::archive_error("ZIP", "corrupt header");
        assert_eq!(
            err.to_string(),
            "Archive extraction error (ZIP): corrupt header"
        );
    }
}
