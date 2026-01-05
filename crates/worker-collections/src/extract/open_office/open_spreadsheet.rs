use anyhow::{Result, anyhow};
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::reader::Reader;
use std::io::{Cursor, Read};
use tracing::error;
use zip::ZipArchive;

pub(crate) fn extract_text(content: &[u8]) -> Result<String> {
    let mut zip_archive =
        ZipArchive::new(Cursor::new(content)).map_err(|error| anyhow!(error.to_string()))?;

    let mut xml_content = String::new();

    for i in 0..zip_archive.len() {
        let mut file = zip_archive
            .by_index(i)
            .map_err(|error| anyhow!(error.to_string()))?;

        if file.name() == "content.xml" {
            let mut buf = String::new();
            file.read_to_string(&mut buf)
                .map_err(|error| anyhow!(error.to_string()))?;
            xml_content += buf.as_str();
        }
    }

    let mut xml_reader = Reader::from_str(&xml_content);
    xml_reader.config_mut().trim_text(true);

    let mut texts = Vec::new();

    if !xml_content.is_empty() {
        let mut to_read = false;
        loop {
            match xml_reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    if let QName(b"text:p") = e.name() {
                        to_read = true;
                        texts.push("\n".to_string());
                    }
                }
                Ok(Event::Text(e)) => {
                    if to_read {
                        let text = e.decode()?.to_string();
                        texts.push(text);
                        to_read = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(error) => error!(
                    "error at position {}: {error:?}",
                    xml_reader.buffer_position(),
                ),
                _ => (),
            }
        }
    }

    Ok(texts.join(""))
}
