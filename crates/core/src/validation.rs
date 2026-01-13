//! Input validation utilities for user-provided data.
//!
//! These functions validate and sanitize user input to prevent
//! injection attacks, path traversal, and other security issues.

use std::path::Path;

/// Maximum length for title/name fields
pub const MAX_TITLE_LENGTH: usize = 256;

/// Maximum length for description/details fields
pub const MAX_DESCRIPTION_LENGTH: usize = 4096;

/// Maximum length for tags
pub const MAX_TAG_LENGTH: usize = 64;

/// Maximum number of tags per resource
pub const MAX_TAGS_COUNT: usize = 20;

/// Validation error types
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    TooLong {
        field: &'static str,
        max: usize,
    },
    TooShort {
        field: &'static str,
        min: usize,
    },
    InvalidCharacters {
        field: &'static str,
        reason: &'static str,
    },
    TooMany {
        field: &'static str,
        max: usize,
    },
    PathTraversal {
        field: &'static str,
    },
    Empty {
        field: &'static str,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::TooLong { field, max } => {
                write!(f, "{} exceeds maximum length of {} characters", field, max)
            }
            ValidationError::TooShort { field, min } => {
                write!(f, "{} must be at least {} characters", field, min)
            }
            ValidationError::InvalidCharacters { field, reason } => {
                write!(f, "{}: {}", field, reason)
            }
            ValidationError::TooMany { field, max } => {
                write!(f, "too many {}: maximum is {}", field, max)
            }
            ValidationError::PathTraversal { field } => {
                write!(f, "{} contains invalid path characters", field)
            }
            ValidationError::Empty { field } => {
                write!(f, "{} cannot be empty", field)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate a title field (collection name, dataset title, etc.)
pub fn validate_title(title: &str) -> Result<(), ValidationError> {
    let trimmed = title.trim();

    if trimmed.is_empty() {
        return Err(ValidationError::Empty { field: "title" });
    }

    if trimmed.len() > MAX_TITLE_LENGTH {
        return Err(ValidationError::TooLong {
            field: "title",
            max: MAX_TITLE_LENGTH,
        });
    }

    // Allow alphanumeric, spaces, hyphens, underscores, and common punctuation
    if !trimmed.chars().all(|c| {
        c.is_alphanumeric()
            || c.is_whitespace()
            || matches!(
                c,
                '-' | '_' | '.' | ',' | '!' | '?' | '\'' | '"' | '(' | ')' | '[' | ']'
            )
    }) {
        return Err(ValidationError::InvalidCharacters {
            field: "title",
            reason: "contains invalid characters",
        });
    }

    Ok(())
}

/// Validate a description/details field
pub fn validate_description(description: &str) -> Result<(), ValidationError> {
    if description.len() > MAX_DESCRIPTION_LENGTH {
        return Err(ValidationError::TooLong {
            field: "description",
            max: MAX_DESCRIPTION_LENGTH,
        });
    }

    Ok(())
}

/// Validate a list of tags
pub fn validate_tags(tags: &[String]) -> Result<(), ValidationError> {
    if tags.len() > MAX_TAGS_COUNT {
        return Err(ValidationError::TooMany {
            field: "tags",
            max: MAX_TAGS_COUNT,
        });
    }

    for tag in tags {
        let trimmed = tag.trim();

        if trimmed.is_empty() {
            continue; // Skip empty tags
        }

        if trimmed.len() > MAX_TAG_LENGTH {
            return Err(ValidationError::TooLong {
                field: "tag",
                max: MAX_TAG_LENGTH,
            });
        }

        // Tags should be simple alphanumeric with hyphens/underscores
        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
        {
            return Err(ValidationError::InvalidCharacters {
                field: "tag",
                reason: "tags can only contain letters, numbers, hyphens, underscores, and spaces",
            });
        }
    }

    Ok(())
}

/// Validate a file path to prevent path traversal attacks
pub fn validate_file_path(path: &str) -> Result<(), ValidationError> {
    // Check for path traversal attempts
    if path.contains("..") {
        return Err(ValidationError::PathTraversal { field: "file path" });
    }

    // Check for absolute paths (shouldn't be allowed in user input)
    if Path::new(path).is_absolute() {
        return Err(ValidationError::PathTraversal { field: "file path" });
    }

    // Check for null bytes
    if path.contains('\0') {
        return Err(ValidationError::InvalidCharacters {
            field: "file path",
            reason: "contains null bytes",
        });
    }

    Ok(())
}

/// Validate a file name
pub fn validate_file_name(name: &str) -> Result<(), ValidationError> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err(ValidationError::Empty { field: "file name" });
    }

    if trimmed.len() > MAX_TITLE_LENGTH {
        return Err(ValidationError::TooLong {
            field: "file name",
            max: MAX_TITLE_LENGTH,
        });
    }

    // Check for path separators
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(ValidationError::InvalidCharacters {
            field: "file name",
            reason: "cannot contain path separators",
        });
    }

    validate_file_path(trimmed)?;

    Ok(())
}

/// Validate a search query
pub fn validate_search_query(query: &str) -> Result<(), ValidationError> {
    let trimmed = query.trim();

    if trimmed.is_empty() {
        return Err(ValidationError::Empty {
            field: "search query",
        });
    }

    if trimmed.len() > 1024 {
        return Err(ValidationError::TooLong {
            field: "search query",
            max: 1024,
        });
    }

    Ok(())
}

/// Validate an S3 object key to prevent path traversal and injection attacks
pub fn validate_s3_key(key: &str) -> Result<(), ValidationError> {
    if key.is_empty() {
        return Err(ValidationError::Empty { field: "S3 key" });
    }

    // Check for path traversal
    if key.contains("..") {
        return Err(ValidationError::PathTraversal { field: "S3 key" });
    }

    // Check for null bytes
    if key.contains('\0') {
        return Err(ValidationError::InvalidCharacters {
            field: "S3 key",
            reason: "contains null bytes",
        });
    }

    // S3 keys shouldn't start with /
    if key.starts_with('/') {
        return Err(ValidationError::InvalidCharacters {
            field: "S3 key",
            reason: "cannot start with /",
        });
    }

    // Check for control characters
    if key.chars().any(|c| c.is_control()) {
        return Err(ValidationError::InvalidCharacters {
            field: "S3 key",
            reason: "contains control characters",
        });
    }

    // Max S3 key length is 1024 bytes
    if key.len() > 1024 {
        return Err(ValidationError::TooLong {
            field: "S3 key",
            max: 1024,
        });
    }

    Ok(())
}

/// Validate an S3 bucket name
pub fn validate_bucket_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::Empty {
            field: "bucket name",
        });
    }

    // Bucket names must be 3-63 characters
    if name.len() < 3 {
        return Err(ValidationError::TooShort {
            field: "bucket name",
            min: 3,
        });
    }

    if name.len() > 63 {
        return Err(ValidationError::TooLong {
            field: "bucket name",
            max: 63,
        });
    }

    // Must start and end with alphanumeric
    if !name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return Err(ValidationError::InvalidCharacters {
            field: "bucket name",
            reason: "must start with lowercase letter or number",
        });
    }

    if !name
        .chars()
        .last()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return Err(ValidationError::InvalidCharacters {
            field: "bucket name",
            reason: "must end with lowercase letter or number",
        });
    }

    // Only lowercase letters, numbers, and hyphens
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(ValidationError::InvalidCharacters {
            field: "bucket name",
            reason: "can only contain lowercase letters, numbers, and hyphens",
        });
    }

    Ok(())
}

/// Sanitize a string by removing potentially dangerous characters
/// while preserving readability
pub fn sanitize_string(input: &str) -> String {
    input
        .chars()
        .filter(|c| !matches!(c, '\0' | '\x1b'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_title_valid() {
        assert!(validate_title("My Collection").is_ok());
        assert!(validate_title("test-dataset_v2").is_ok());
        assert!(validate_title("Hello, World!").is_ok());
    }

    #[test]
    fn test_validate_title_empty() {
        assert!(matches!(
            validate_title(""),
            Err(ValidationError::Empty { .. })
        ));
        assert!(matches!(
            validate_title("   "),
            Err(ValidationError::Empty { .. })
        ));
    }

    #[test]
    fn test_validate_title_too_long() {
        let long_title = "a".repeat(MAX_TITLE_LENGTH + 1);
        assert!(matches!(
            validate_title(&long_title),
            Err(ValidationError::TooLong { .. })
        ));
    }

    #[test]
    fn test_validate_file_path_traversal() {
        assert!(matches!(
            validate_file_path("../etc/passwd"),
            Err(ValidationError::PathTraversal { .. })
        ));
        assert!(matches!(
            validate_file_path("foo/../bar"),
            Err(ValidationError::PathTraversal { .. })
        ));
    }

    #[test]
    fn test_validate_file_path_valid() {
        assert!(validate_file_path("documents/report.pdf").is_ok());
        assert!(validate_file_path("file.txt").is_ok());
    }

    #[test]
    fn test_validate_tags() {
        assert!(validate_tags(&["tag1".to_string(), "tag-2".to_string()]).is_ok());

        let too_many_tags: Vec<String> = (0..MAX_TAGS_COUNT + 1)
            .map(|i| format!("tag{}", i))
            .collect();
        assert!(matches!(
            validate_tags(&too_many_tags),
            Err(ValidationError::TooMany { .. })
        ));
    }
}
