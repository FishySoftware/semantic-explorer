mod document;
mod presentation;
mod spreadsheet;

use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::json;
use std::io::{Cursor, Read};
use zip::ZipArchive;

pub(crate) fn extract_text_from_document(content: &[u8]) -> Result<String> {
    document::extract_text(content)
}

pub(crate) fn extract_text_from_spreadsheet(content: &[u8]) -> Result<String> {
    spreadsheet::extract_text(content)
}

pub(crate) fn extract_text_from_presentation(content: &[u8]) -> Result<String> {
    presentation::extract_text(content)
}

/// Extract metadata from Office Open XML documents (docx, xlsx, pptx)
/// Metadata is stored in docProps/core.xml and docProps/app.xml
pub(crate) fn extract_document_metadata(content: &[u8]) -> Result<serde_json::Value> {
    let mut zip_archive = ZipArchive::new(Cursor::new(content))?;
    let mut metadata = serde_json::Map::new();

    // Try to read core.xml for Dublin Core metadata
    if let Ok(core_meta) = extract_core_metadata(&mut zip_archive) {
        for (key, value) in core_meta {
            metadata.insert(key, value);
        }
    }

    // Try to read app.xml for application-specific metadata
    if let Ok(app_meta) = extract_app_metadata(&mut zip_archive) {
        for (key, value) in app_meta {
            metadata.insert(key, value);
        }
    }

    Ok(serde_json::Value::Object(metadata))
}

fn extract_core_metadata(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut metadata = serde_json::Map::new();

    let mut xml_content = String::new();
    if let Ok(mut file) = zip.by_name("docProps/core.xml") {
        file.read_to_string(&mut xml_content)?;
    } else {
        return Ok(metadata);
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
                            "subject" => "subject",
                            "creator" => "author",
                            "keywords" => "keywords",
                            "description" => "description",
                            "lastModifiedBy" => "last_modified_by",
                            "created" => "creation_date",
                            "modified" => "modification_date",
                            "category" => "category",
                            _ => &current_tag,
                        };
                        metadata.insert(key.to_string(), json!(text));
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

    Ok(metadata)
}

fn extract_app_metadata(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut metadata = serde_json::Map::new();

    let mut xml_content = String::new();
    if let Ok(mut file) = zip.by_name("docProps/app.xml") {
        file.read_to_string(&mut xml_content)?;
    } else {
        return Ok(metadata);
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
                            "Application" => "application",
                            "AppVersion" => "app_version",
                            "Company" => "company",
                            "Pages" => "page_count",
                            "Words" => "word_count",
                            "Characters" => "character_count",
                            "Template" => "template",
                            "TotalTime" => "total_editing_time",
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

    Ok(metadata)
}
