//! E2: bounded, source-revision-aware LSP session state machine.
//! All transport bytes must be carried by an independently authorized
//! restricted OS worker. This module NEVER launches or locates a process,
//! opens a URL, interprets a server filesystem path, or applies server edits.
use crate::lsp::{encode_frame, valid_envelope, DiagnosticSource, FrameError, FrameParser};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const MAX_TEXT_BYTES: usize = 512 * 1024;
const MAX_DIAGNOSTICS: usize = 200;
const MAX_SUGGESTIONS: usize = 100;
const MAX_PENDING: usize = 24;
const MAX_DETAIL: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspError {
    NotReady,
    StaleRevision,
    Protocol,
    Capacity,
    UntrustedUri,
    Unsupported,
}
impl From<FrameError> for LspError {
    fn from(_: FrameError) -> Self {
        Self::Protocol
    }
}
impl LspError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotReady => "LSP_NOT_READY",
            Self::StaleRevision => "LSP_STALE_REVISION",
            Self::Protocol => "LSP_PROTOCOL_REJECTED",
            Self::Capacity => "LSP_CAPACITY",
            Self::UntrustedUri => "LSP_UNTRUSTED_URI",
            Self::Unsupported => "LSP_UNSUPPORTED_OPERATION",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspPhase {
    Created,
    Initializing,
    Ready,
    Closing,
    Closed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub range: Span,
    pub severity: u8,
    pub message: String,
    pub code: Option<String>,
    pub from_utf16: usize,
    pub to_utf16: usize,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Completion {
    pub label: String,
    pub detail: String,
    pub kind: Option<u32>,
    pub insert_text: String,
    // A server-supplied textEdit is included only after native validation
    // that it replaces exactly the current identifier prefix. Never forward
    // arbitrary workspace edits or cross-document references.
    pub replace_from_utf16: Option<usize>,
    pub replace_to_utf16: Option<usize>,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Definition {
    pub range: Span,
    pub from_utf16: usize,
    pub to_utf16: usize,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum LspEvent {
    Diagnostics(Vec<Diagnostic>),
    Completions(Vec<Completion>),
    Hover(String),
    Definition(Option<Definition>),
    ServerRequestDenied,
    MalformedReply,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingKind {
    Completion,
    Hover,
    Definition,
}
struct Pending {
    kind: PendingKind,
    source: DiagnosticSource,
    uri: String,
    position: Position,
}
struct OpenDocument {
    source: DiagnosticSource,
    /// Must be minted from a *native-verified* project-owned file; server
    /// file:// links are never accepted as path authority.
    uri: String,
    text: String,
    version: u64,
}

/// This in-memory state machine is the protocol half of a supervised worker.
/// It cannot be constructed into an active transport without a separate
/// host-owned OS admission and a pinned, authenticated LSP subprocess.
pub struct LspSession {
    server_id: String,
    phase: LspPhase,
    parser: FrameParser,
    next_id: u64,
    initialize_id: Option<u64>,
    pending: BTreeMap<u64, Pending>,
    doc: Option<OpenDocument>,
}
impl LspSession {
    pub fn new(server_id: &str) -> Result<Self, LspError> {
        if !crate::lsp::available_servers()
            .iter()
            .any(|s| s.id == server_id)
        {
            return Err(LspError::Unsupported);
        }
        Ok(Self {
            server_id: server_id.to_owned(),
            phase: LspPhase::Created,
            parser: FrameParser::new(),
            next_id: 1,
            initialize_id: None,
            pending: BTreeMap::new(),
            doc: None,
        })
    }
    pub fn phase(&self) -> LspPhase {
        self.phase
    }
    pub fn server_id(&self) -> &str {
        &self.server_id
    }
    pub fn stop_uncertain(&mut self) {
        self.phase = LspPhase::Failed;
        self.pending.clear();
        self.doc = None;
        self.parser.reset();
    }
    fn next_request(&mut self) -> Result<u64, LspError> {
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or(LspError::Capacity)?;
        Ok(id)
    }
    pub fn initialize(&mut self) -> Result<Vec<u8>, LspError> {
        if self.phase != LspPhase::Created {
            return Err(LspError::NotReady);
        }
        let id = self.next_request()?;
        // No project path or workspace folder is disclosed until a future
        // native resource-grant decision explicitly authorizes its scope.
        let request = json!({
            "jsonrpc": "2.0", "id": id, "method": "initialize",
            "params": {
                "processId": null, "clientInfo": {"name":"Lu IDE","version":"0.1"},
                "rootUri": null, "workspaceFolders": null,
                "capabilities": {
                    "textDocument": {
                        "synchronization":{"didSave":false,"dynamicRegistration":false},
                        "completion":{"dynamicRegistration":false},
                        "hover":{"dynamicRegistration":false},
                        "definition":{"dynamicRegistration":false},
                        "publishDiagnostics":{"relatedInformation":false}
                    },
                    "workspace": {"applyEdit":false,"workspaceEdit":{"documentChanges":false}}
                },
                "trace":"off"
            }
        });
        self.initialize_id = Some(id);
        self.phase = LspPhase::Initializing;
        Ok(encode_frame(&request)?)
    }
    /// Native host only: 'uri' must correspond to the exact registered file
    /// and current disk/buffer revision. Do not accept a server-supplied URI.
    pub fn did_open(
        &mut self,
        source: DiagnosticSource,
        native_uri: &str,
        text: &str,
    ) -> Result<Vec<u8>, LspError> {
        if self.phase != LspPhase::Ready {
            return Err(LspError::NotReady);
        }
        if !source.valid()
            || !crate::lsp::available_servers().iter().any(|server| {
                server.id == self.server_id
                    && server.language_ids.contains(&source.language_id.as_str())
            })
            || !native_uri.starts_with("file:///")
            || native_uri.len() > 2048
            || native_uri.contains('\n')
            || text.len() > MAX_TEXT_BYTES
            || self.doc.is_some()
        {
            return Err(LspError::UntrustedUri);
        }
        let version = 1;
        let doc = OpenDocument {
            uri: native_uri.to_owned(),
            text: text.to_owned(),
            source: source.clone(),
            version,
        };
        let value = json!({"jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":native_uri,"languageId":source.language_id,
            "version":version,"text":text}}});
        let frame = encode_frame(&value)?;
        self.doc = Some(doc);
        Ok(frame)
    }
    pub fn did_change(
        &mut self,
        expected: &DiagnosticSource,
        next: DiagnosticSource,
        text: &str,
    ) -> Result<Vec<u8>, LspError> {
        if self.phase != LspPhase::Ready {
            return Err(LspError::NotReady);
        }
        let doc = self.doc.as_ref().ok_or(LspError::NotReady)?;
        if !doc.source.same_revision(expected)
            || !next.valid()
            || next.project_id != expected.project_id
            || next.file_id != expected.file_id
            || next.language_id != expected.language_id
            || next.buffer_revision <= expected.buffer_revision
        {
            return Err(LspError::StaleRevision);
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(LspError::Capacity);
        }
        let version = doc.version.checked_add(1).ok_or(LspError::Capacity)?;
        let value = json!({"jsonrpc":"2.0","method":"textDocument/didChange",
            "params":{"textDocument":{"uri":doc.uri,"version":version},
            "contentChanges":[{"text":text}]}});
        let frame = encode_frame(&value)?;
        let doc = self.doc.as_mut().ok_or(LspError::NotReady)?;
        doc.source = next;
        doc.text = text.to_owned();
        doc.version = version;
        self.pending.clear(); // All old LSP requests are now stale.
        Ok(frame)
    }
    pub fn request(
        &mut self,
        kind: &str,
        expected: &DiagnosticSource,
        pos: Position,
    ) -> Result<Vec<u8>, LspError> {
        if self.phase != LspPhase::Ready {
            return Err(LspError::NotReady);
        }
        let doc = self.doc.as_ref().ok_or(LspError::NotReady)?;
        if !doc.source.same_revision(expected) {
            return Err(LspError::StaleRevision);
        }
        if position_to_utf16(&doc.text, &pos).is_none() {
            return Err(LspError::StaleRevision);
        }
        if self.pending.len() >= MAX_PENDING {
            return Err(LspError::Capacity);
        }
        let (method, request_kind) = match kind {
            "completion" => ("textDocument/completion", PendingKind::Completion),
            "hover" => ("textDocument/hover", PendingKind::Hover),
            "definition" => ("textDocument/definition", PendingKind::Definition),
            _ => return Err(LspError::Unsupported),
        };
        let uri = doc.uri.clone();
        let id = self.next_request()?;
        let value = json!({"jsonrpc":"2.0","id":id,"method":method,
            "params":{"textDocument":{"uri":uri},"position":pos}});
        let frame = encode_frame(&value)?;
        self.pending.insert(
            id,
            Pending {
                kind: request_kind,
                source: expected.clone(),
                uri,
                position: pos,
            },
        );
        Ok(frame)
    }
    pub fn close_document(&mut self, expected: &DiagnosticSource) -> Result<Vec<u8>, LspError> {
        let doc = self.doc.as_ref().ok_or(LspError::NotReady)?;
        if !doc.source.same_revision(expected) {
            return Err(LspError::StaleRevision);
        }
        let frame = encode_frame(&json!({"jsonrpc":"2.0","method":"textDocument/didClose",
            "params":{"textDocument":{"uri":doc.uri}}}))?;
        self.pending.clear();
        self.doc = None;
        Ok(frame)
    }
    pub fn shutdown(&mut self) -> Result<Vec<u8>, LspError> {
        if self.phase != LspPhase::Ready {
            return Err(LspError::NotReady);
        }
        let id = self.next_request()?;
        self.phase = LspPhase::Closing;
        self.pending.clear();
        self.doc = None;
        Ok(encode_frame(
            &json!({"jsonrpc":"2.0","id":id,"method":"shutdown"}),
        )?)
    }
    /// Parses genuine server transport bytes from an external supervised
    /// channel. No server reply may install packages, execute commands or
    /// mutate source; unsupported server-initiated requests must be rejected
    /// by the worker-side sender with JSON-RPC method-not-found.
    pub fn receive(&mut self, bytes: &[u8]) -> Result<(Vec<LspEvent>, Vec<Vec<u8>>), LspError> {
        if matches!(
            self.phase,
            LspPhase::Created | LspPhase::Closed | LspPhase::Failed
        ) {
            return Err(LspError::NotReady);
        }
        let incoming = match self.parser.push(bytes) {
            Ok(incoming) => incoming,
            Err(_) => {
                self.stop_uncertain();
                return Err(LspError::Protocol);
            }
        };
        let mut events = Vec::new();
        let mut replies = Vec::new();
        for message in incoming {
            if !valid_envelope(&message) {
                self.stop_uncertain();
                return Err(LspError::Protocol);
            }
            if let Some(method) = message.get("method").and_then(Value::as_str) {
                if message.get("id").is_some() {
                    // Explicit deny for server-initiated method calls, including
                    // workspace/applyEdit and workspace/executeCommand.
                    let id = &message["id"];
                    if !id.is_null() {
                        // Some legitimate servers request workspace settings.
                        // Return an explicit array of nulls, never project
                        // configuration paths, credentials or executable edits.
                        if method == "workspace/configuration" {
                            if let Some(items) = message["params"]["items"].as_array() {
                                if items.len() <= 16 {
                                    replies.push(encode_frame(&json!({"jsonrpc":"2.0",
                                        "id":id, "result":vec![Value::Null;items.len()]}))?);
                                    continue;
                                }
                            }
                        }
                        replies.push(encode_frame(&json!({"jsonrpc":"2.0","id":id,
                            "error":{"code":-32601,"message":"Method not allowed"}}))?);
                    }
                    events.push(LspEvent::ServerRequestDenied);
                } else if method == "textDocument/publishDiagnostics"
                    && self.phase == LspPhase::Ready
                {
                    if let Some(value) = self.parse_diagnostics(&message["params"]) {
                        events.push(LspEvent::Diagnostics(value));
                    }
                }
                continue;
            }
            let Some(id) = message["id"].as_u64() else {
                continue;
            };
            if self.phase == LspPhase::Initializing && self.initialize_id == Some(id) {
                if message.get("error").is_some() {
                    self.stop_uncertain();
                    return Err(LspError::Protocol);
                }
                self.phase = LspPhase::Ready;
                self.initialize_id = None;
                replies.push(encode_frame(&json!({"jsonrpc":"2.0",
                    "method":"initialized","params":{}}))?);
                continue;
            }
            if self.phase != LspPhase::Ready {
                continue;
            }
            let Some(pending) = self.pending.remove(&id) else {
                continue;
            };
            let Some(doc) = self.doc.as_ref() else {
                continue;
            };
            if !doc.source.same_revision(&pending.source) || doc.uri != pending.uri {
                continue;
            }
            let event = match pending.kind {
                PendingKind::Completion => {
                    parse_completions(&message["result"], &pending.position, &doc.text)
                        .map(LspEvent::Completions)
                }
                PendingKind::Hover => parse_hover(&message["result"]).map(LspEvent::Hover),
                PendingKind::Definition => {
                    parse_definition(&message["result"], &doc.uri, &doc.text)
                        .map(LspEvent::Definition)
                }
            };
            events.push(event.unwrap_or(LspEvent::MalformedReply));
        }
        Ok((events, replies))
    }
    fn parse_diagnostics(&self, params: &Value) -> Option<Vec<Diagnostic>> {
        let doc = self.doc.as_ref()?;
        if params["uri"].as_str()? != doc.uri {
            return None;
        }
        // LSP allows a missing version, but an unversioned asynchronous
        // notification after didChange could refer to stale bytes. Accept it
        // only before the first edit; require an exact version thereafter.
        match params.get("version").and_then(Value::as_u64) {
            Some(version) if version == doc.version => {}
            None if doc.version == 1 => {}
            _ => return None,
        }
        let input = params["diagnostics"].as_array()?;
        let mut diagnostics = Vec::new();
        for diagnostic in input.iter().take(MAX_DIAGNOSTICS) {
            let span: Span = serde_json::from_value(diagnostic["range"].clone()).ok()?;
            let (start, end) = span_utf16(&doc.text, &span)?;
            if start > end {
                continue;
            }
            let message = diagnostic["message"].as_str()?;
            if message.len() > MAX_DETAIL {
                continue;
            }
            let severity = diagnostic["severity"].as_u64().unwrap_or(3).min(4) as u8;
            let code = diagnostic
                .get("code")
                .and_then(|v| {
                    v.as_str()
                        .map(ToOwned::to_owned)
                        .or_else(|| v.as_i64().map(|x| x.to_string()))
                })
                .filter(|v| v.len() <= 64);
            diagnostics.push(Diagnostic {
                range: span,
                severity,
                message: message.to_owned(),
                code,
                from_utf16: start,
                to_utf16: end,
            });
        }
        Some(diagnostics)
    }
}

pub fn position_to_utf16(text: &str, pos: &Position) -> Option<usize> {
    if pos.line > 250_000 || pos.character > 250_000 {
        return None;
    }
    let line = text.split('\n').nth(pos.line as usize)?;
    let line = line.strip_suffix('\r').unwrap_or(line);
    let units = line.encode_utf16().count();
    if pos.character as usize > units {
        return None;
    }
    // Reject positions splitting a UTF-16 surrogate pair. CodeMirror offsets
    // count UTF-16 units; Rust character indices are UTF-8.
    let mut offset = 0usize;
    for ch in line.chars() {
        if offset == pos.character as usize {
            break;
        }
        offset += ch.len_utf16();
        if offset > pos.character as usize {
            return None;
        }
    }
    let before = text
        .split_inclusive('\n')
        .take(pos.line as usize)
        .map(|s| s.encode_utf16().count())
        .sum::<usize>();
    Some(before + pos.character as usize)
}
fn span_utf16(text: &str, span: &Span) -> Option<(usize, usize)> {
    let from = position_to_utf16(text, &span.start)?;
    let to = position_to_utf16(text, &span.end)?;
    if from <= to {
        Some((from, to))
    } else {
        None
    }
}
fn plain_text(value: &Value) -> Option<String> {
    if let Some(v) = value.as_str() {
        return (v.len() <= MAX_DETAIL).then(|| v.to_owned());
    }
    if let Some(v) = value.get("value").and_then(Value::as_str) {
        return (v.len() <= MAX_DETAIL).then(|| v.to_owned());
    }
    None
}
fn parse_hover(value: &Value) -> Option<String> {
    if value.is_null() {
        return Some(String::new());
    }
    let contents = &value["contents"];
    if let Some(s) = plain_text(contents) {
        return Some(s);
    }
    if let Some(items) = contents.as_array() {
        let mut out = Vec::new();
        for item in items.iter().take(8) {
            out.push(plain_text(item)?);
        }
        let joined = out.join("\n");
        return (joined.len() <= MAX_DETAIL).then_some(joined);
    }
    None
}
fn parse_completions(value: &Value, pos: &Position, text: &str) -> Option<Vec<Completion>> {
    if value.is_null() {
        return Some(Vec::new());
    }
    let items = value
        .as_array()
        .or_else(|| value.get("items")?.as_array())?;
    let caret = position_to_utf16(text, pos)?;
    let mut result = Vec::new();
    for item in items.iter().take(MAX_SUGGESTIONS) {
        let Some(label) = item["label"].as_str() else {
            continue;
        };
        if label.is_empty() || label.len() > 256 || label.chars().any(char::is_control) {
            continue;
        }
        // This pure completion list never applies the server's action.
        // Snippet expansion and arbitrary workspace/text edits need a
        // separate host-reviewed operation, not a hidden autocomplete side
        // effect.
        if item["insertTextFormat"].as_u64() == Some(2)
            || item.get("command").is_some()
            || item.get("additionalTextEdits").is_some()
        {
            continue;
        }
        let edit = item.get("textEdit");
        let insert = edit
            .and_then(|v| v.get("newText"))
            .and_then(Value::as_str)
            .or_else(|| item.get("insertText").and_then(Value::as_str))
            .unwrap_or(label);
        if insert.len() > 512 || insert.chars().any(char::is_control) {
            continue;
        }
        let mut replace_range = None;
        if let Some(edit) = edit {
            let Some(text_insert) = edit.get("newText").and_then(Value::as_str) else {
                continue;
            };
            if text_insert != insert
                || edit.get("insert").is_some()
                || edit.get("replace").is_some()
            {
                continue;
            }
            let Ok(range) = serde_json::from_value::<Span>(edit["range"].clone()) else {
                continue;
            };
            if range.start.line != pos.line
                || range.end.line != pos.line
                || range.end.character != pos.character
                || range.start.character > range.end.character
                || range.end.character - range.start.character > 128
            {
                continue;
            }
            let Some((from, to)) = span_utf16(text, &range) else {
                continue;
            };
            if to != caret {
                continue;
            }
            // Safe character-prefix replacement only. A language server may
            // not rewrite punctuation, another line or a different symbol
            // through the completion UI.
            let prefix: String = text
                .encode_utf16()
                .skip(from)
                .take(to - from)
                .map(|v| char::from_u32(v as u32).unwrap_or('\u{fffd}'))
                .collect();
            if !prefix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
            {
                continue;
            }
            replace_range = Some((from, to));
        }
        let detail = item["detail"].as_str().unwrap_or("");
        result.push(Completion {
            label: label.to_owned(),
            detail: detail.chars().take(256).collect(),
            kind: item["kind"].as_u64().and_then(|n| u32::try_from(n).ok()),
            insert_text: insert.to_owned(),
            replace_from_utf16: replace_range.map(|(a, _)| a),
            replace_to_utf16: replace_range.map(|(_, b)| b),
        });
    }
    Some(result)
}
fn parse_definition(value: &Value, current_uri: &str, text: &str) -> Option<Option<Definition>> {
    if value.is_null() {
        return Some(None);
    }
    let item = if let Some(arr) = value.as_array() {
        arr.first()?
    } else {
        value
    };
    // Never follow a server-provided URI to a different file. Full
    // cross-file navigation needs native workspace root revalidation.
    let (uri, range) = if item.get("targetUri").is_some() {
        (item["targetUri"].as_str()?, &item["targetSelectionRange"])
    } else {
        (item["uri"].as_str()?, &item["range"])
    };
    if uri != current_uri {
        return Some(None);
    }
    let span: Span = serde_json::from_value(range.clone()).ok()?;
    let (from_utf16, to_utf16) = span_utf16(text, &span)?;
    Some(Some(Definition {
        range: span,
        from_utf16,
        to_utf16,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source(n: u64) -> DiagnosticSource {
        DiagnosticSource {
            project_id: format!("ide-{}", "a".repeat(32)),
            file_id: "file_1".into(),
            disk_sha256: "b".repeat(64),
            buffer_revision: n,
            language_id: "rust".into(),
        }
    }
    fn ready() -> LspSession {
        let mut sess = LspSession::new("rust-analyzer").unwrap();
        sess.initialize().unwrap();
        let (events, out) = sess
            .receive(
                &encode_frame(&json!({"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}))
                    .unwrap(),
            )
            .unwrap();
        assert!(events.is_empty());
        assert_eq!(out.len(), 1);
        assert_eq!(sess.phase(), LspPhase::Ready);
        sess
    }
    #[test]
    fn real_lsp_handshake_and_bounded_document_sync() {
        let mut s = ready();
        s.did_open(source(1), "file:///src/main.rs", "fn main() {}\n")
            .unwrap();
        let send = s
            .did_change(&source(1), source(2), "fn main() { println!(\"hey\"); }\n")
            .unwrap();
        assert!(String::from_utf8_lossy(&send).contains("didChange"));
        assert!(matches!(
            s.did_change(&source(1), source(3), "bad"),
            Err(LspError::StaleRevision)
        ));
        s.close_document(&source(2)).unwrap();
        assert_eq!(s.phase(), LspPhase::Ready);
    }
    #[test]
    fn completion_hover_definition_and_diagnostics_follow_exact_revision() {
        let mut s = ready();
        s.did_open(source(1), "file:///src/main.rs", "😀 test\nsymbol\n")
            .unwrap();
        let ask = s
            .request(
                "completion",
                &source(1),
                Position {
                    line: 1,
                    character: 2,
                },
            )
            .unwrap();
        assert!(String::from_utf8_lossy(&ask).contains("textDocument/completion"));
        let diag = json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics",
            "params":{"uri":"file:///src/main.rs","version":1,"diagnostics":[{
                "range":{"start":{"line":0,"character":0},"end":{"line":0,"character":2}},
                "message":"Example","severity":1}]}});
        let (events, _) = s.receive(&encode_frame(&diag).unwrap()).unwrap();
        assert!(matches!(&events[0],LspEvent::Diagnostics(d) if d[0].to_utf16==2));
        let (events, _) = s
            .receive(
                &encode_frame(&json!({"jsonrpc":"2.0","id":2,
            "result":[{"label":"symbol","insertText":"symbol"}]}))
                .unwrap(),
            )
            .unwrap();
        assert!(matches!(&events[0],LspEvent::Completions(d) if d.len()==1));
        assert!(s
            .request(
                "hover",
                &source(1),
                Position {
                    line: 3,
                    character: 0
                }
            )
            .is_err());
        s.did_change(&source(1), source(2), "other").unwrap();
        let (events, _) = s.receive(&encode_frame(&diag).unwrap()).unwrap();
        assert!(events.is_empty()); // stale version
    }
    #[test]
    fn server_edit_requests_are_denied_and_cross_file_definition_not_followed() {
        let mut s = ready();
        s.did_open(source(1), "file:///src/main.rs", "symbol")
            .unwrap();
        let req = json!({"jsonrpc":"2.0","id":17,"method":"workspace/applyEdit","params":{}});
        let (events, replies) = s.receive(&encode_frame(&req).unwrap()).unwrap();
        assert_eq!(events, vec![LspEvent::ServerRequestDenied]);
        assert_eq!(replies.len(), 1);
        let ask = s
            .request(
                "definition",
                &source(1),
                Position {
                    line: 0,
                    character: 1,
                },
            )
            .unwrap();
        assert!(String::from_utf8_lossy(&ask).contains("textDocument/definition"));
        let (events, _) = s
            .receive(
                &encode_frame(&json!({"jsonrpc":"2.0","id":2,
            "result":{"uri":"file:///outside.rs","range":{
                "start":{"line":0,"character":0},"end":{"line":0,"character":1}}}}))
                .unwrap(),
            )
            .unwrap();
        assert_eq!(events, vec![LspEvent::Definition(None)]);
    }
    #[test]
    fn unicode_surrogate_half_and_invalid_protocol_fail_closed() {
        assert_eq!(
            position_to_utf16(
                "😀 hi\nx",
                &Position {
                    line: 0,
                    character: 1
                }
            ),
            None
        );
        assert_eq!(
            position_to_utf16(
                "😀 hi\nx",
                &Position {
                    line: 1,
                    character: 1
                }
            ),
            Some(7)
        );
        let mut s = ready();
        assert!(s.receive(b"Content-Length: 0\r\n\r\n").is_err());
        assert_eq!(s.phase(), LspPhase::Failed);
    }
    #[test]
    fn real_clangd_17_wire_replays_diagnostics_hover_definition_and_safe_completion() {
        let fixture: Value =
            serde_json::from_str(include_str!("../fixtures/clangd17_real_wire.json")).unwrap();
        let uri = fixture["uri"].as_str().unwrap();
        let text = fixture["initialText"].as_str().unwrap();
        let changed = fixture["changedText"].as_str().unwrap();
        let messages = &fixture["messages"];
        let mut session = LspSession::new("clangd").unwrap();
        session.initialize().unwrap();
        session
            .receive(
                &encode_frame(&json!({"jsonrpc":"2.0","id":1,
            "result":{"capabilities":{}}}))
                .unwrap(),
            )
            .unwrap();
        let mut source = DiagnosticSource {
            project_id: format!("ide-{}", "c".repeat(32)),
            file_id: "file_clangd".into(),
            disk_sha256: "d".repeat(64),
            buffer_revision: 1,
            language_id: "c".into(),
        };
        session.did_open(source.clone(), uri, text).unwrap();
        let (diags, _) = session
            .receive(&encode_frame(&messages["diagnostics"]).unwrap())
            .unwrap();
        assert_eq!(diags, vec![LspEvent::Diagnostics(vec![])]);
        let at_symbol = Position {
            line: 1,
            character: 27,
        };
        session
            .request("hover", &source, at_symbol.clone())
            .unwrap();
        session.request("definition", &source, at_symbol).unwrap();
        let (hover, _) = session
            .receive(&encode_frame(&messages["hover"]).unwrap())
            .unwrap();
        assert!(matches!(&hover[0], LspEvent::Hover(text) if text.contains("addition")));
        let (definition, _) = session
            .receive(&encode_frame(&messages["definition"]).unwrap())
            .unwrap();
        assert!(matches!(&definition[0], LspEvent::Definition(Some(_))));
        let old = source.clone();
        source.buffer_revision = 2;
        session.did_change(&old, source.clone(), changed).unwrap();
        session
            .request(
                "completion",
                &source,
                Position {
                    line: 1,
                    character: 28,
                },
            )
            .unwrap();
        let (completions, _) = session
            .receive(&encode_frame(&messages["completion"]).unwrap())
            .unwrap();
        let LspEvent::Completions(items) = &completions[0] else {
            panic!("real clangd completion was not interpreted");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert_text, "addition");
        assert_eq!(
            items[0].replace_from_utf16,
            position_to_utf16(
                changed,
                &Position {
                    line: 1,
                    character: 24
                }
            )
        );
        assert_eq!(
            items[0].replace_to_utf16,
            position_to_utf16(
                changed,
                &Position {
                    line: 1,
                    character: 28
                }
            )
        );
    }
}
