//! Shared provider-facing Goose data types.
//!
//! This crate is intentionally runtime-independent: it should not read Goose
//! configuration, sessions, filesystem paths, or other application state.

use base64::Engine;
pub use rmcp::model::ErrorData;
use rmcp::model::ResourceContents;
use unicode_normalization::UnicodeNormalization;

pub mod conversation;
pub mod model;
pub mod usage;

pub use conversation::Conversation;
pub use model::{Config, ConfigError, ConfigParamError, EnvConfig, ModelConfig, ThinkingEffort};
pub use usage::{ProviderUsage, Usage};

pub type ToolResult<T> = Result<T, ErrorData>;

pub fn extract_text_from_resource(resource: &ResourceContents) -> String {
    match resource {
        ResourceContents::TextResourceContents { text, .. } => text.clone(),
        ResourceContents::BlobResourceContents {
            blob, mime_type, ..
        } => match base64::engine::general_purpose::STANDARD.decode(blob) {
            Ok(bytes) => {
                let byte_len = bytes.len();
                match String::from_utf8(bytes) {
                    Ok(text) => text,
                    Err(_) => {
                        let mime = mime_type
                            .as_ref()
                            .map(|m| m.as_str())
                            .unwrap_or("application/octet-stream");
                        format!("[Binary content ({}) - {} bytes]", mime, byte_len)
                    }
                }
            }
            Err(_) => blob.clone(),
        },
    }
}

fn is_in_unicode_tag_range(c: char) -> bool {
    matches!(c, '\u{E0000}'..='\u{E007F}')
}

pub fn contains_unicode_tags(text: &str) -> bool {
    text.chars().any(is_in_unicode_tag_range)
}

pub fn sanitize_unicode_tags(text: &str) -> String {
    let normalized: String = text.nfc().collect();

    normalized
        .chars()
        .filter(|&c| !is_in_unicode_tag_range(c))
        .collect()
}
