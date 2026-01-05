use anyhow::{Result, anyhow};
use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::error;

pub(crate) fn extract_text_from_xml(bytes: &[u8]) -> Result<String> {
    let xml_data = std::str::from_utf8(bytes).map_err(|error| anyhow!(error.to_string()))?;

    let mut reader = Reader::from_str(xml_data);
    reader.config_mut().trim_text(true);

    let mut result = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(e)) => {
                result.push_str(&e.decode()?);
            }
            Ok(Event::Eof) => break,
            Err(error) => error!("error at position {}: {error:?}", reader.buffer_position()),
            _ => (),
        }
    }
    Ok(result)
}
