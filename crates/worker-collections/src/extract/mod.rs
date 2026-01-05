mod html;
mod office;
mod open_office;
mod pdf;
mod xml;

use anyhow::{Result, anyhow};
use unicode_normalization::UnicodeNormalization;

pub(crate) fn extract_text(content_type: &mime::Mime, buffer: &[u8]) -> Result<String> {
    let raw_text = match content_type.type_() {
        mime::APPLICATION => process_application_type(content_type.subtype().as_str(), buffer),
        mime::TEXT => process_text_type(content_type, buffer),
        _ => Err(anyhow!("unsupported content type: {}", content_type)),
    }?;
    Ok(clean_text(&raw_text))
}

fn process_application_type(sub_type: &str, buffer: &[u8]) -> Result<String> {
    match sub_type {
        "pdf" => Ok(pdf::extract_text_from_pdf(buffer)?),
        "msword"
        | "vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "vnd.openxmlformats-officedocument.wordprocessingml.template"
        | "vnd.ms-word.document.macroEnabled.12"
        | "vnd.ms-word.template.macroEnabled.12" => Ok(office::extract_text_from_document(buffer)?),
        "vnd.ms-excel"
        | "vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "vnd.openxmlformats-officedocument.spreadsheetml.template"
        | "vnd.ms-excel.sheet.macroEnabled.12"
        | "vnd.ms-excel.template.macroEnabled.12"
        | "vnd.ms-excel.addin.macroEnabled.12"
        | "vnd.ms-excel.sheet.binary.macroEnabled.12" => {
            Ok(office::extract_text_from_spreadsheet(buffer)?)
        }
        "mspowerpoint"
        | "powerpoint"
        | "vnd.ms-powerpoint"
        | "x-mspowerpoint"
        | "vnd.openxmlformats-officedocument.presentationml.presentation" => {
            Ok(office::extract_text_from_presentation(buffer)?)
        }
        "vnd.oasis.opendocument.text" => Ok(open_office::extract_text_from_document(buffer)?),
        "vnd.oasis.opendocument.spreadsheet" => {
            Ok(open_office::extract_text_from_spreadsheet(buffer)?)
        }
        "vnd.oasis.opendocument.presentation" => {
            Ok(open_office::extract_text_from_presentation(buffer)?)
        }
        "xml" => Ok(xml::extract_text_from_xml(buffer)?),
        "html" => Ok(html::extract_text_from_html(buffer)?),
        _ => Err(anyhow!("unsupported application subtype: {}", sub_type)),
    }
}

fn process_text_type(content_type: &mime::Mime, buffer: &[u8]) -> Result<String> {
    match content_type.subtype() {
        mime::PLAIN | mime::CSV => Ok(String::from_utf8_lossy(buffer).to_string()),
        mime::XML => Ok(xml::extract_text_from_xml(buffer)?),
        mime::HTML => Ok(html::extract_text_from_html(buffer)?),
        _ => Err(anyhow!(
            "unsupported text subtype: {}",
            content_type.subtype()
        )),
    }
}

fn clean_text(raw_text: &str) -> String {
    let normalized = raw_text.nfc().collect::<String>();
    let without_controls: String = normalized
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t' | '\r'))
        .collect();
    let lines: Vec<String> = without_controls
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect();
    lines.join("\n").trim().to_string()
}
