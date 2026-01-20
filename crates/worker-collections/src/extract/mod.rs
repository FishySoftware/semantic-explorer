pub mod config;
pub mod error;
pub mod plain_text;
pub mod service;

mod archive;
mod email;
mod epub;
mod html;
mod json;
mod legacy_doc;
mod legacy_ppt;
mod legacy_xls;
mod log;
mod markdown;
mod office;
mod open_office;
mod pdf;
mod rtf;
mod xml;

pub use service::ExtractionService;
