//! Wave E1: descriptive open-source language-server catalog and bounded
//! JSON-RPC/LSP stdio framing. NO executable resolution, spawn, download,
//! connection, remote workspace request, process grant or Tauri dispatch.
//!
//! LSP servers may execute arbitrary project/tooling code. Never equate
//! discovery with authorization; Wave D worker acceptance is a hard gate.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const LSP_CATALOG_VERSION: u32 = 1;
pub const MAX_HEADER: usize = 256;
pub const MAX_MESSAGE: usize = 512 * 1024;
const MAX_FRAMES_PER_CHUNK: usize = 32;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageServerDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub language_ids: &'static [&'static str],
    pub executable_basename: &'static str,
    pub fixed_arguments: &'static [&'static str],
    pub license: &'static str,
    pub upstream: &'static str,
    pub transport: &'static str,
    pub lifecycle: &'static str,
    pub executable_from_desktop: bool,
}

const SERVERS: &[LanguageServerDescriptor] = &[
    LanguageServerDescriptor {
        id: "rust-analyzer",
        label: "rust-analyzer",
        language_ids: &["rust"],
        executable_basename: "rust-analyzer",
        fixed_arguments: &[],
        license: "MIT OR Apache-2.0",
        upstream: "https://github.com/rust-lang/rust-analyzer",
        transport: "stdio",
        lifecycle: "catalog_only",
        executable_from_desktop: false,
    },
    LanguageServerDescriptor {
        id: "clangd",
        label: "clangd",
        language_ids: &["c", "cpp"],
        executable_basename: "clangd",
        fixed_arguments: &[],
        license: "Apache-2.0 WITH LLVM-exception",
        upstream: "https://clangd.llvm.org/",
        transport: "stdio",
        lifecycle: "catalog_only",
        executable_from_desktop: false,
    },
    LanguageServerDescriptor {
        id: "pyright",
        label: "Pyright",
        language_ids: &["python"],
        executable_basename: "pyright-langserver",
        fixed_arguments: &["--stdio"],
        license: "MIT",
        upstream: "https://github.com/microsoft/pyright",
        transport: "stdio",
        lifecycle: "catalog_only",
        executable_from_desktop: false,
    },
    LanguageServerDescriptor {
        id: "ruff",
        label: "Ruff",
        language_ids: &["python"],
        executable_basename: "ruff",
        fixed_arguments: &["server"],
        license: "MIT",
        upstream: "https://github.com/astral-sh/ruff",
        transport: "stdio",
        lifecycle: "catalog_only",
        executable_from_desktop: false,
    },
    LanguageServerDescriptor {
        id: "typescript-language-server",
        label: "TypeScript Language Server",
        language_ids: &["typescript", "typescriptreact", "javascript", "javascriptreact"],
        executable_basename: "typescript-language-server",
        fixed_arguments: &["--stdio"],
        license: "MIT AND Apache-2.0",
        upstream: "https://github.com/typescript-language-server/typescript-language-server",
        transport: "stdio",
        lifecycle: "catalog_only",
        executable_from_desktop: false,
    },
    LanguageServerDescriptor {
        id: "vscode-html-language-server",
        label: "VS Code HTML Language Server",
        language_ids: &["html"],
        executable_basename: "vscode-html-language-server",
        fixed_arguments: &["--stdio"],
        license: "MIT",
        upstream: "https://github.com/hrsh7th/vscode-langservers-extracted",
        transport: "stdio",
        lifecycle: "catalog_only",
        executable_from_desktop: false,
    },
    LanguageServerDescriptor {
        id: "vscode-css-language-server",
        label: "VS Code CSS Language Server",
        language_ids: &["css", "scss", "less"],
        executable_basename: "vscode-css-language-server",
        fixed_arguments: &["--stdio"],
        license: "MIT",
        upstream: "https://github.com/hrsh7th/vscode-langservers-extracted",
        transport: "stdio",
        lifecycle: "catalog_only",
        executable_from_desktop: false,
    },
    LanguageServerDescriptor {
        id: "vscode-json-language-server",
        label: "VS Code JSON Language Server",
        language_ids: &["json", "jsonc"],
        executable_basename: "vscode-json-language-server",
        fixed_arguments: &["--stdio"],
        license: "MIT",
        upstream: "https://github.com/hrsh7th/vscode-langservers-extracted",
        transport: "stdio",
        lifecycle: "catalog_only",
        executable_from_desktop: false,
    },
];

pub fn available_servers() -> &'static [LanguageServerDescriptor] {
    SERVERS
}

pub fn servers_for_language(language_id: &str) -> Vec<LanguageServerDescriptor> {
    if language_id.len() > 32
        || language_id.is_empty()
        || !language_id.bytes().all(|b| b.is_ascii_lowercase())
            && !language_id.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Vec::new();
    }
    SERVERS
        .iter()
        .copied()
        .filter(|server| server.language_ids.contains(&language_id))
        .collect()
}

/// An LSP document reference is *not* a filesystem path or executable right.
/// The native IDE must resolve project/file IDs to its own current source
/// object and reject stale disk/buffer revisions before publishing diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticSource {
    pub project_id: String,
    pub file_id: String,
    pub disk_sha256: String,
    pub buffer_revision: u64,
    pub language_id: String,
}

impl DiagnosticSource {
    pub fn valid(&self) -> bool {
        self.project_id.starts_with("ide-")
            && self.project_id.len() == 36
            && self.project_id[4..].bytes().all(|b| b.is_ascii_hexdigit())
            && !self.file_id.is_empty()
            && self.file_id.len() <= 128
            && self.file_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            && self.disk_sha256.len() == 64
            && self.disk_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            && !servers_for_language(&self.language_id).is_empty()
    }

    pub fn same_revision(&self, other: &Self) -> bool {
        self.valid() && other.valid() && self == other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    OversizedHeader,
    InvalidHeader,
    InvalidLength,
    OversizedMessage,
    InvalidJson,
    TooManyMessages,
}

/// Validate only the JSON-RPC envelope. Server-provided edits, URI targets,
/// diagnostics and configuration must undergo SEPARATE host-native checks.
/// Valid framing does not confer semantic correctness or trust.
pub fn valid_envelope(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return false;
    }
    if let Some(method) = object.get("method") {
        let Some(method) = method.as_str() else {
            return false;
        };
        if method.is_empty()
            || method.len() > 128
            || method.chars().any(char::is_control)
        {
            return false;
        }
    } else if !object.contains_key("result") && !object.contains_key("error") {
        return false;
    }
    match object.get("id") {
        None => object.contains_key("method"),
        Some(Value::Null | Value::Number(_)) => true,
        Some(Value::String(id)) => id.len() <= 128 && !id.chars().any(char::is_control),
        _ => false,
    }
}

pub fn encode_frame(value: &Value) -> Result<Vec<u8>, FrameError> {
    if !valid_envelope(value) {
        return Err(FrameError::InvalidJson);
    }
    let bytes = serde_json::to_vec(value).map_err(|_| FrameError::InvalidJson)?;
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE {
        return Err(FrameError::OversizedMessage);
    }
    let header = format!("Content-Length: {}\r\n\r\n", bytes.len());
    let mut out = Vec::with_capacity(header.len() + bytes.len());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(&bytes);
    Ok(out)
}

#[derive(Default)]
pub struct FrameParser {
    pending: Vec<u8>,
}

impl FrameParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.pending.clear();
    }

    pub fn pending_bytes(&self) -> usize {
        self.pending.len()
    }

    /// Consume one bounded transport chunk; no network or subprocess I/O.
    /// On a protocol violation, discard the buffered untrusted bytes.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<Value>, FrameError> {
        let result = self.push_inner(chunk);
        if result.is_err() {
            self.reset();
        }
        result
    }

    fn push_inner(&mut self, chunk: &[u8]) -> Result<Vec<Value>, FrameError> {
        if chunk.len() > MAX_MESSAGE + MAX_HEADER
            || self.pending.len().saturating_add(chunk.len()) > MAX_MESSAGE + MAX_HEADER
        {
            return Err(FrameError::OversizedMessage);
        }
        self.pending.extend_from_slice(chunk);
        let mut decoded = Vec::new();
        loop {
            let Some(end) = self
                .pending
                .windows(4)
                .position(|slice| slice == b"\r\n\r\n")
            else {
                if self.pending.len() > MAX_HEADER {
                    return Err(FrameError::OversizedHeader);
                }
                break;
            };
            if end > MAX_HEADER {
                return Err(FrameError::OversizedHeader);
            }
            let header = std::str::from_utf8(&self.pending[..end])
                .map_err(|_| FrameError::InvalidHeader)?;
            let length = parse_length(header)?;
            let frame_len = end + 4 + length;
            if self.pending.len() < frame_len {
                break;
            }
            if decoded.len() == MAX_FRAMES_PER_CHUNK {
                return Err(FrameError::TooManyMessages);
            }
            let value: Value = serde_json::from_slice(&self.pending[end + 4..frame_len])
                .map_err(|_| FrameError::InvalidJson)?;
            if !valid_envelope(&value) {
                return Err(FrameError::InvalidJson);
            }
            decoded.push(value);
            self.pending.drain(..frame_len);
        }
        Ok(decoded)
    }
}

fn parse_length(header: &str) -> Result<usize, FrameError> {
    let mut length = None;
    for line in header.split("\r\n") {
        let (name, value) = line.split_once(':').ok_or(FrameError::InvalidHeader)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("Content-Length") {
            if length.is_some()
                || value.is_empty()
                || !value.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(FrameError::InvalidLength);
            }
            let size: usize = value.parse().map_err(|_| FrameError::InvalidLength)?;
            if size == 0 {
                return Err(FrameError::InvalidLength);
            }
            if size > MAX_MESSAGE {
                return Err(FrameError::OversizedMessage);
            }
            length = Some(size);
        } else if name.eq_ignore_ascii_case("Content-Type") {
            if value.len() > 80 || !value.is_ascii() {
                return Err(FrameError::InvalidHeader);
            }
        } else {
            return Err(FrameError::InvalidHeader);
        }
    }
    length.ok_or(FrameError::InvalidLength)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catalog_is_fixed_non_executable_and_supported_language_ids_are_known() {
        assert_eq!(LSP_CATALOG_VERSION, 1);
        assert_eq!(available_servers().len(), 8);
        for entry in available_servers() {
            assert_eq!(entry.transport, "stdio");
            assert_eq!(entry.lifecycle, "catalog_only");
            assert!(!entry.executable_from_desktop);
            assert!(entry.upstream.starts_with("https://"));
            assert!(!entry.executable_basename.contains('/'));
            assert!(!entry.executable_basename.contains('\\'));
            assert!(!entry.fixed_arguments.iter().any(|s| s.contains(';')));
        }
        assert_eq!(servers_for_language("python").len(), 2);
        assert_eq!(servers_for_language("rust").len(), 1);
        assert!(servers_for_language("../../rust").is_empty());
        assert!(servers_for_language("").is_empty());
    }

    #[test]
    fn split_headers_and_concatenated_responses_are_bounded() {
        let first = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
        let second = json!({"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}});
        let a = encode_frame(&first).unwrap();
        let b = encode_frame(&second).unwrap();
        let mut parser = FrameParser::new();
        assert!(parser.push(&a[..8]).unwrap().is_empty());
        assert_eq!(parser.push(&a[8..]).unwrap(), vec![first.clone()]);
        let mut both = a.clone();
        both.extend_from_slice(&b);
        assert_eq!(parser.push(&both).unwrap(), vec![first, second]);
        assert_eq!(parser.pending_bytes(), 0);
    }

    #[test]
    fn rejects_duplicate_or_nondecimal_length_and_bad_json() {
        let mut p = FrameParser::new();
        for malformed in [
            b"Content-Length: 1\r\nContent-Length: 1\r\n\r\nx".as_slice(),
            b"Content-Length: 1e5\r\n\r\nx".as_slice(),
            b"Content-Length: 0\r\n\r\n".as_slice(),
            b"Content-Length: 2\r\n\r\n{}".as_slice(),
            b"Content-Length: 3\r\n\r\n[] ".as_slice(),
        ] {
            assert!(p.push(malformed).is_err());
            assert_eq!(p.pending_bytes(), 0);
        }
        assert_eq!(
            p.push(b"Content-Length: 524289\r\n\r\n").unwrap_err(),
            FrameError::OversizedMessage
        );
    }

    #[test]
    fn stale_diagnostics_cannot_be_mistaken_for_same_buffer_revision() {
        let source = DiagnosticSource {
            project_id: format!("ide-{}", "a".repeat(32)),
            file_id: "file_123".into(),
            disk_sha256: "b".repeat(64),
            buffer_revision: 5,
            language_id: "rust".into(),
        };
        assert!(source.valid());
        let mut edited = source.clone();
        edited.buffer_revision = 6;
        assert!(!source.same_revision(&edited));
        let mut swapped = source.clone();
        swapped.disk_sha256 = "c".repeat(64);
        assert!(!source.same_revision(&swapped));
    }
}
