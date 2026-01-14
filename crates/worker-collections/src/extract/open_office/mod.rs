mod open_document;
mod open_presentation;
mod open_spreadsheet;

use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::json;
use std::io::{Cursor, Read};
use zip::ZipArchive;

pub(crate) fn extract_text_from_document(content: &[u8]) -> Result<String> {
    open_document::extract_text(content)
}

pub(crate) fn extract_text_from_spreadsheet(content: &[u8]) -> Result<String> {
    open_spreadsheet::extract_text(content)
}

pub(crate) fn extract_text_from_presentation(content: &[u8]) -> Result<String> {
    open_presentation::extract_text(content)
}

/// Extract metadata from OpenDocument format files (odt, ods, odp)
/// Metadata is stored in meta.xml
pub(crate) fn extract_document_metadata(content: &[u8]) -> Result<serde_json::Value> {
    let mut zip_archive = ZipArchive::new(Cursor::new(content))?;
    let mut metadata = serde_json::Map::new();

    let mut xml_content = String::new();
    if let Ok(mut file) = zip_archive.by_name("meta.xml") {
        file.read_to_string(&mut xml_content)?;
    } else {
        return Ok(serde_json::Value::Object(metadata));
    }

    let mut reader = Reader::from_str(&xml_content);
    reader.config_mut().trim_text(true);

    let mut current_tag = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
            }
            Ok(Event::Text(e)) => {
                if !current_tag.is_empty() {
                    let text = e.decode()?.to_string();
                    if !text.is_empty() {
                        let key = match current_tag.as_str() {
                            "title" => "title",
                            "description" => "description",
                            "subject" => "subject",
                            "keyword" => "keywords",
                            "initial-creator" => "author",
                            "creator" => "last_modified_by",
                            "creation-date" => "creation_date",
                            "date" => "modification_date",
                            "language" => "language",
                            "editing-cycles" => "revision_count",
                            "editing-duration" => "editing_duration",
                            "generator" => "application",
                            _ => continue,
                        };

                        // Try to parse numeric values
                        if let Ok(num) = text.parse::<i64>() {
                            metadata.insert(key.to_string(), json!(num));
                        } else {
                            metadata.insert(key.to_string(), json!(text));
                        }
                    }
                }
            }
            Ok(Event::End(_)) => {
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => (),
        }
    }

    // Try to get page/word count from document statistics
    if let Ok(stats) = extract_document_statistics(&mut zip_archive) {
        for (key, value) in stats {
            metadata.insert(key, value);
        }
    }

    Ok(serde_json::Value::Object(metadata))
}

fn extract_document_statistics(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut stats = serde_json::Map::new();

    let mut xml_content = String::new();
    if let Ok(mut file) = zip.by_name("meta.xml") {
        file.read_to_string(&mut xml_content)?;
    } else {
        return Ok(stats);
    }

    let mut reader = Reader::from_str(&xml_content);

    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let local_name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if local_name == "document-statistic" {
                    for attr in e.attributes().flatten() {
                        let key =
                            String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
                        let value = String::from_utf8_lossy(&attr.value).to_string();

                        let mapped_key = match key.as_str() {
                            "page-count" => "page_count",
                            "word-count" => "word_count",
                            "character-count" => "character_count",
                            "paragraph-count" => "paragraph_count",
                            "table-count" => "table_count",
                            "image-count" => "image_count",
                            _ => continue,
                        };

                        if let Ok(num) = value.parse::<i64>() {
                            stats.insert(mapped_key.to_string(), json!(num));
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => (),
        }
    }

    Ok(stats)
}
