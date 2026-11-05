//! Lu-owned persistent conversation index and per-conversation append journal.
//! Stored conversation IDs are host-generated. No UI-controlled path is opened.
//! The legacy flat R14 chat log is imported once and left untouched for rollback.
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

const MAX_INDEX: u64 = 512 * 1024;
const MAX_LOG: u64 = 32 * 1024 * 1024;
const MAX_ENTRY: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatSummary {
    pub chat_id: String,
    pub title: String,
    pub folder_id: Option<String>,
    pub favorite: bool,
    pub updated_seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChatFolder {
    pub folder_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChatIndex {
    version: u32,
    sequence: u64,
    folders: Vec<ChatFolder>,
    chats: Vec<ChatSummary>,
}
impl Default for ChatIndex {
    fn default() -> Self {
        Self {
            version: 1,
            sequence: 0,
            folders: vec![],
            chats: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Event {
    User { text: String, #[serde(default, skip_serializing_if = "Option::is_none")] user_turn_id: Option<String> },
    // Host-minted generation ID binds the actual persisted human request
    // to the ephemeral Reader grant; old chat journals remain compatible.
    ReaderUser { text: String, generation_id: String },
    AssistantFinal { text: String }, // R14 migration; a complete answer.
    AssistantProgress { generation_id: String, text: String }, // cumulative checkpoint.
    AssistantCompleted { generation_id: String },
    AssistantCancelled { generation_id: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub text: String,
    pub interrupted: bool,
    pub cancelled: bool,
}

pub struct ChatStore {
    dir: PathBuf,
    index_path: PathBuf,
    inner: Mutex<ChatIndex>,
}

fn id(prefix: &str) -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    format!(
        "{}{}",
        prefix,
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
fn valid_id(value: &str, prefix: &str) -> bool {
    value.len() == prefix.len() + 32
        && value.starts_with(prefix)
        && value[prefix.len()..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn valid_name(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 100 && !s.contains('\0')
}
fn validate(index: &ChatIndex) -> Result<(), String> {
    if index.version != 1 || index.chats.len() > 1024 || index.folders.len() > 128 {
        return Err("invalid conversation index capacity/version".into());
    }
    let folders: HashSet<_> = index.folders.iter().map(|x| &x.folder_id).collect();
    let mut unique = HashSet::new();
    for folder in &index.folders {
        if !valid_id(&folder.folder_id, "f-")
            || !valid_name(&folder.name)
            || !unique.insert(folder.folder_id.as_str())
        {
            return Err("invalid conversation folder metadata".into());
        }
    }
    unique.clear();
    for chat in &index.chats {
        if !valid_id(&chat.chat_id, "c-")
            || !valid_name(&chat.title)
            || !unique.insert(chat.chat_id.as_str())
            || chat
                .folder_id
                .as_ref()
                .is_some_and(|x| !folders.contains(x))
            || chat.updated_seq > index.sequence
        {
            return Err("invalid conversation metadata".into());
        }
    }
    Ok(())
}
fn atomic_index(path: &Path, index: &ChatIndex) -> Result<(), String> {
    validate(index)?;
    let bytes = serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_INDEX {
        return Err("conversation index size cap".into());
    }
    let temp = path.with_file_name(format!(".conversation-index-{}.tmp", id("")));
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
fn append_event(path: &Path, event: &Event) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(event).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_ENTRY {
        return Err("conversation event too large".into());
    }
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() + bytes.len() as u64 > MAX_LOG {
        return Err("conversation log size cap".into());
    }
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.sync_data().map_err(|e| e.to_string())
}
impl ChatStore {
    pub fn open(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let dir = root.join("conversations");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let index_path = root.join("conversation-index.json");
        let index = if index_path.exists() {
            let meta = fs::metadata(&index_path).map_err(|e| e.to_string())?;
            if meta.len() > MAX_INDEX {
                return Err("conversation index too large".into());
            }
            let bytes = fs::read(&index_path).map_err(|e| e.to_string())?;
            let value: ChatIndex = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            validate(&value)?;
            value
        } else {
            let mut index = ChatIndex::default();
            let legacy = root.join("chats.jsonl");
            if legacy.exists() {
                let content = fs::read_to_string(&legacy).map_err(|e| e.to_string())?;
                if content.len() as u64 > MAX_LOG {
                    return Err("legacy history too large".into());
                }
                // Stable migration identity makes a failed index publication
                // retryable. The source is never modified or deleted.
                let migrated_id = format!("c-{}", "0".repeat(31) + "1");
                let journal = dir.join(format!("{migrated_id}.jsonl"));
                let mut events = Vec::new();
                for line in content.lines().filter(|line| !line.trim().is_empty()) {
                    let value: serde_json::Value =
                        serde_json::from_str(line).map_err(|e| format!("legacy JSONL: {e}"))?;
                    let role = value
                        .get("role")
                        .and_then(|x| x.as_str())
                        .ok_or("legacy role missing")?;
                    let text = value
                        .get("text")
                        .and_then(|x| x.as_str())
                        .ok_or("legacy text missing")?;
                    match role {
                        "user" => events.push(Event::User {
                            text: text.to_string(),
                            user_turn_id: None, // Legacy migration is context, not new authority.
                        }),
                        "assistant" => events.push(Event::AssistantFinal {
                            text: text.to_string(),
                        }),
                        _ => return Err("unrecognized legacy chat role".into()),
                    }
                }
                if !events.is_empty() {
                    // On retry, replace only the deterministic migration journal.
                    let tmp = dir.join(".legacy-migration.tmp");
                    let mut file = File::create(&tmp).map_err(|e| e.to_string())?;
                    for event in &events {
                        serde_json::to_writer(&mut file, event).map_err(|e| e.to_string())?;
                        file.write_all(b"\n").map_err(|e| e.to_string())?;
                    }
                    file.sync_all().map_err(|e| e.to_string())?;
                    drop(file);
                    fs::rename(&tmp, &journal).map_err(|e| e.to_string())?;
                    index.sequence = 1;
                    index.chats.push(ChatSummary {
                        chat_id: migrated_id,
                        title: "Previous Lu conversation".into(),
                        folder_id: None,
                        favorite: false,
                        updated_seq: 1,
                    });
                }
            }
            atomic_index(&index_path, &index)?;
            index
        };
        Ok(Self {
            dir,
            index_path,
            inner: Mutex::new(index),
        })
    }
    fn chat_path(&self, chat_id: &str) -> PathBuf {
        self.dir.join(format!("{chat_id}.jsonl"))
    }
    pub fn list(&self) -> Result<(Vec<ChatSummary>, Vec<ChatFolder>), String> {
        let catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        let mut chats = catalog.chats.clone();
        chats.sort_by(|a, b| b.updated_seq.cmp(&a.updated_seq));
        Ok((chats, catalog.folders.clone()))
    }
    pub fn create_chat(&self) -> Result<ChatSummary, String> {
        let mut catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        if catalog.chats.len() >= 1024 {
            return Err("conversation limit".into());
        }
        let id = id("c-");
        let journal = self.chat_path(&id);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&journal)
            .map_err(|e| e.to_string())?;
        let mut next = catalog.clone();
        next.sequence += 1;
        let chat = ChatSummary {
            chat_id: id,
            title: "New chat".into(),
            folder_id: None,
            favorite: false,
            updated_seq: next.sequence,
        };
        next.chats.push(chat.clone());
        if let Err(err) = atomic_index(&self.index_path, &next) {
            let _ = fs::remove_file(&journal);
            return Err(err);
        }
        *catalog = next;
        Ok(chat)
    }
    pub fn create_folder(&self, name: &str) -> Result<ChatFolder, String> {
        if !valid_name(name) {
            return Err("invalid folder name".into());
        }
        let mut catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        let mut next = catalog.clone();
        let folder = ChatFolder {
            folder_id: id("f-"),
            name: name.trim().into(),
        };
        next.folders.push(folder.clone());
        atomic_index(&self.index_path, &next)?;
        *catalog = next;
        Ok(folder)
    }
    pub fn favorite(&self, chat_id: &str, favorite: bool) -> Result<(), String> {
        let mut catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        let mut next = catalog.clone();
        let chat = next
            .chats
            .iter_mut()
            .find(|c| c.chat_id == chat_id)
            .ok_or("unknown chat")?;
        chat.favorite = favorite;
        atomic_index(&self.index_path, &next)?;
        *catalog = next;
        Ok(())
    }
    pub fn move_chat(&self, chat_id: &str, folder_id: Option<String>) -> Result<(), String> {
        let mut catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        let mut next = catalog.clone();
        if folder_id
            .as_ref()
            .is_some_and(|f| !next.folders.iter().any(|x| &x.folder_id == f))
        {
            return Err("unknown folder".into());
        }
        let chat = next
            .chats
            .iter_mut()
            .find(|c| c.chat_id == chat_id)
            .ok_or("unknown chat")?;
        chat.folder_id = folder_id;
        atomic_index(&self.index_path, &next)?;
        *catalog = next;
        Ok(())
    }
    pub fn append_user(&self, chat_id: &str, text: &str) -> Result<(), String> {
        self.append_user_event(chat_id, text, Event::User { text: text.into(), user_turn_id: None })
    }
    /// A Reader grant must refer to this exact durable user event. Ordinary
    /// chat messages do not acquire Reader authority, even if text matches.
    pub fn append_reader_user(
        &self, chat_id: &str, generation_id: &str, text: &str,
    ) -> Result<(), String> {
        if !valid_id(generation_id, "g-") {
            return Err("invalid Reader generation identity".into());
        }
        self.append_user_event(chat_id, text, Event::ReaderUser {
            text: text.into(), generation_id: generation_id.into(),
        })
    }
    fn append_user_event(
        &self, chat_id: &str, text: &str, event: Event,
    ) -> Result<(), String> {
        if text.is_empty() || text.len() > 16 * 1024 {
            return Err("invalid chat input".into());
        }
        let mut catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        let mut next = catalog.clone();
        let chat = next
            .chats
            .iter_mut()
            .find(|c| c.chat_id == chat_id)
            .ok_or("unknown chat")?;
        // Journal is durable first. Failure to write index cannot erase the turn.
        append_event(&self.chat_path(chat_id), &event)?;
        if chat.title == "New chat" {
            chat.title = text.chars().take(65).take_while(|_| true).collect();
            while chat.title.len() > 96 {
                chat.title.pop();
            }
        }
        next.sequence += 1;
        chat.updated_seq = next.sequence;
        atomic_index(&self.index_path, &next)?;
        *catalog = next;
        Ok(())
    }
    /// Host-generated opaque turn identity, committed with the exact User event.
    /// A caller-supplied model ID or old imported User event never authenticates
    /// a new tool task. The host owns issuance and chat ownership verification.
    pub fn append_user_with_turn_id(&self, chat_id: &str, user_turn_id: &str, text: &str) -> Result<(), String> {
        if !valid_id(user_turn_id, "g-") || text.is_empty() || text.len() > 16 * 1024 {
            return Err("invalid authenticated user turn".into());
        }
        let mut catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        let mut next = catalog.clone();
        let chat = next.chats.iter_mut().find(|c| c.chat_id == chat_id).ok_or("unknown chat")?;
        // Fail closed if any historical event is corrupt, or if the identifier
        // is already present in this journal. Do not skip malformed records.
        let path = self.chat_path(chat_id);
        let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
        if metadata.len() > MAX_LOG { return Err("conversation too large".into()); }
        let input = File::open(&path).map_err(|e| e.to_string())?;
        for line in BufReader::new(input).lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line.is_empty() { return Err("empty conversation event".into()); }
            let event: Event = serde_json::from_str(&line).map_err(|_| "corrupt user-turn journal")?;
            if let Event::User { user_turn_id: Some(existing), .. } = event {
                if existing == user_turn_id { return Err("duplicate authenticated user turn".into()); }
            }
        }
        append_event(&path, &Event::User { text: text.into(), user_turn_id: Some(user_turn_id.into()) })?;
        if chat.title == "New chat" {
            chat.title = text.chars().take(65).collect();
            while chat.title.len() > 96 { chat.title.pop(); }
        }
        next.sequence += 1;
        chat.updated_seq = next.sequence;
        atomic_index(&self.index_path, &next)?;
        *catalog = next;
        Ok(())
    }

    /// Verify only an exact persisted User turn from an existing host-owned
    /// conversation. Unknown, old imported, duplicate or corrupted turns fail.
    pub fn trusted_user_turn_text(&self, chat_id: &str, user_turn_id: &str) -> Option<String> {
        if !valid_id(chat_id, "c-") || !valid_id(user_turn_id, "g-") { return None; }
        let catalog = self.inner.lock().ok()?;
        if !catalog.chats.iter().any(|c| c.chat_id == chat_id) { return None; }
        let path = self.chat_path(chat_id);
        if fs::metadata(&path).ok()?.len() > MAX_LOG { return None; }
        let file = File::open(&path).ok()?;
        let mut found = None;
        for line in BufReader::new(file).lines() {
            let line = line.ok()?;
            if line.is_empty() { return None; }
            let event: Event = serde_json::from_str(&line).ok()?;
            if let Event::User { text, user_turn_id: Some(id) } = event {
                if id == user_turn_id {
                    if found.is_some() { return None; }
                    found = Some(text);
                }
            }
        }
        found
    }
    /// Verify a host-saved Reader user event by both immutable identity and
    /// content, not by matching the last chat message's text. This allows a
    /// queued Reader request to run after subsequent prompts without granting
    /// those prompts its document permission. Any journal error fails closed.
    pub fn reader_user_turn_matches(
        &self, chat_id: &str, generation_id: &str, expected_text: &str,
    ) -> Result<bool, String> {
        if !valid_id(generation_id, "g-") {
            return Ok(false);
        }
        let catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        if !catalog.chats.iter().any(|c| c.chat_id == chat_id) {
            return Ok(false);
        }
        let file = File::open(self.chat_path(chat_id)).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_LOG {
            return Err("conversation too large".into());
        }
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line.is_empty() { continue; }
            let event: Event = serde_json::from_str(&line)
                .map_err(|e| format!("corrupt chat journal: {e}"))?;
            if let Event::ReaderUser { text, generation_id: saved_id } = event {
                if saved_id == generation_id {
                    return Ok(text == expected_text);
                }
            }
        }
        Ok(false)
    }
    pub fn append_progress(
        &self,
        chat_id: &str,
        generation_id: &str,
        text: &str,
    ) -> Result<(), String> {
        self.append_assistant(
            chat_id,
            Event::AssistantProgress {
                generation_id: generation_id.into(),
                text: text.into(),
            },
        )
    }
    pub fn complete(&self, chat_id: &str, generation_id: &str) -> Result<(), String> {
        self.append_assistant(
            chat_id,
            Event::AssistantCompleted {
                generation_id: generation_id.into(),
            },
        )
    }
    pub fn cancel(&self, chat_id: &str, generation_id: &str) -> Result<(), String> {
        self.append_assistant(
            chat_id,
            Event::AssistantCancelled {
                generation_id: generation_id.into(),
            },
        )
    }
    fn append_assistant(&self, chat_id: &str, event: Event) -> Result<(), String> {
        let catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        if !catalog.chats.iter().any(|c| c.chat_id == chat_id) {
            return Err("unknown chat".into());
        }
        append_event(&self.chat_path(chat_id), &event)
    }
    pub fn history(&self, chat_id: &str) -> Result<Vec<ChatMessage>, String> {
        let catalog = self.inner.lock().map_err(|_| "chat lock poisoned")?;
        if !catalog.chats.iter().any(|c| c.chat_id == chat_id) {
            return Err("unknown chat".into());
        }
        let path = self.chat_path(chat_id);
        let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
        if meta.len() > MAX_LOG {
            return Err("conversation too large".into());
        }
        let file = File::open(&path).map_err(|e| e.to_string())?;
        // Place a journaled partial assistant message at the exact position
        // where it was first generated. Completion finalizes it in place;
        // cancellation retains the partial output, visibly marked interrupted.
        let mut result: Vec<Option<ChatMessage>> = Vec::new();
        let mut pending = std::collections::HashMap::<String, usize>::new();
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line.is_empty() {
                continue;
            }
            let event: Event =
                serde_json::from_str(&line).map_err(|e| format!("corrupt chat journal: {e}"))?;
            match event {
                Event::User { text, .. } | Event::ReaderUser { text, .. } => result.push(Some(ChatMessage {
                    role: "user".into(),
                    text,
                    interrupted: false,
                    cancelled: false,
                })),
                Event::AssistantFinal { text } => result.push(Some(ChatMessage {
                    role: "assistant".into(),
                    text,
                    interrupted: false,
                    cancelled: false,
                })),
                Event::AssistantProgress {
                    generation_id,
                    text,
                } => {
                    if let Some(i) = pending.get(&generation_id) {
                        if let Some(message) = &mut result[*i] {
                            message.text = text;
                        }
                    } else {
                        pending.insert(generation_id, result.len());
                        result.push(Some(ChatMessage {
                            role: "assistant".into(),
                            text,
                            interrupted: true,
                            cancelled: false,
                        }));
                    }
                }
                Event::AssistantCompleted { generation_id } => {
                    if let Some(i) = pending.remove(&generation_id) {
                        if let Some(message) = &mut result[i] {
                            message.interrupted = false;
                        }
                    }
                }
                Event::AssistantCancelled { generation_id } => {
                    if let Some(i) = pending.remove(&generation_id) {
                        if let Some(message) = &mut result[i] {
                            message.interrupted = true;
                            message.cancelled = true;
                        }
                    }
                }
            }
        }
        let result = result.into_iter().flatten().collect();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        std::env::temp_dir().join(id("lu-conversation-test-"))
    }
    #[test]
    fn survives_restart_and_keeps_chats_separate() {
        let root = root();
        let s = ChatStore::open(&root).unwrap();
        let a = s.create_chat().unwrap();
        let b = s.create_chat().unwrap();
        s.append_user(&a.chat_id, "Hello world").unwrap();
        s.append_progress(&a.chat_id, "g-1", "partial").unwrap();
        s.append_progress(&a.chat_id, "g-1", "finished").unwrap();
        s.complete(&a.chat_id, "g-1").unwrap();
        s.append_user(&b.chat_id, "Another question").unwrap();
        drop(s);
        let restored = ChatStore::open(&root).unwrap();
        assert_eq!(restored.history(&a.chat_id).unwrap().len(), 2);
        assert_eq!(restored.history(&a.chat_id).unwrap()[1].text, "finished");
        assert_eq!(restored.history(&b.chat_id).unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn favoriting_moving_and_interrupted_recovery() {
        let root = root();
        let s = ChatStore::open(&root).unwrap();
        let a = s.create_chat().unwrap();
        let f = s.create_folder("Projects").unwrap();
        s.favorite(&a.chat_id, true).unwrap();
        s.move_chat(&a.chat_id, Some(f.folder_id.clone())).unwrap();
        s.append_progress(&a.chat_id, "g-1", "unsaved final")
            .unwrap();
        assert!(s.history(&a.chat_id).unwrap()[0].interrupted);
        s.cancel(&a.chat_id, "g-1").unwrap();
        let cancelled = s.history(&a.chat_id).unwrap();
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].text, "unsaved final");
        assert!(cancelled[0].interrupted && cancelled[0].cancelled);
        assert!(s.list().unwrap().0[0].favorite);
        assert_eq!(s.list().unwrap().0[0].folder_id, Some(f.folder_id));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn migrates_r14_without_dropping_messages() {
        let root = root();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("chats.jsonl"), "{\"role\":\"user\",\"text\":\"old prompt\"}\n{\"role\":\"assistant\",\"text\":\"old reply\"}\n").unwrap();
        let store = ChatStore::open(&root).unwrap();
        let (chats, _) = store.list().unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(store.history(&chats[0].chat_id).unwrap().len(), 2);
        assert!(root.join("chats.jsonl").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_message_keeps_original_turn_order() {
        let root = root();
        let store = ChatStore::open(&root).unwrap();
        let chat = store.create_chat().unwrap();
        store.append_user(&chat.chat_id, "first").unwrap();
        store
            .append_progress(&chat.chat_id, "g-1", "partial first")
            .unwrap();
        store.append_user(&chat.chat_id, "second").unwrap();
        store
            .append_progress(&chat.chat_id, "g-2", "second reply")
            .unwrap();
        store.complete(&chat.chat_id, "g-2").unwrap();
        let history = store.history(&chat.chat_id).unwrap();
        assert_eq!(history.len(), 4);
        assert_eq!(history[1].text, "partial first");
        assert!(history[1].interrupted);
        assert_eq!(history[3].text, "second reply");
        assert!(!history[3].interrupted);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn authenticated_user_turn_is_durable_exact_and_fail_closed() {
        use lu_core::conversation_authority::TrustedUserTurnVerifier;
        let root = root();
        let store = ChatStore::open(&root).unwrap();
        let a = store.create_chat().unwrap();
        let b = store.create_chat().unwrap();
        let turn = format!("g-{}", "a".repeat(32));
        store.append_user_with_turn_id(&a.chat_id, &turn, "trusted prompt").unwrap();
        assert!(store.verify_user_turn(&a.chat_id, &turn));
        assert!(!store.verify_user_turn(&b.chat_id, &turn));
        assert!(!store.verify_user_turn(&a.chat_id, "g-invalid"));
        assert!(store.append_user_with_turn_id(&a.chat_id, &turn, "duplicate").is_err());
        store.append_user(&a.chat_id, "untrusted legacy context").unwrap();
        drop(store);
        let restored = ChatStore::open(&root).unwrap();
        assert_eq!(restored.trusted_user_turn_text(&a.chat_id, &turn), Some("trusted prompt".into()));
        let journal = root.join("conversations").join(format!("{}.jsonl", a.chat_id));
        use std::io::Write as _;
        std::fs::OpenOptions::new().append(true).open(&journal).unwrap()
            .write_all(b"{malformed journal row}\n").unwrap();
        assert!(!restored.verify_user_turn(&a.chat_id, &turn));
        assert!(restored.append_user_with_turn_id(&a.chat_id, &format!("g-{}", "b".repeat(32)), "text").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reader_authorization_is_bound_to_exact_durable_human_turn() {
        let root = root();
        let chat_store = ChatStore::open(&root).unwrap();
        let chat = chat_store.create_chat().unwrap();
        let second = chat_store.create_chat().unwrap();
        let a = id("g-");
        let b = id("g-");
        // Matching text alone is not an authority record.
        chat_store.append_user(&chat.chat_id, "Read this").unwrap();
        assert!(!chat_store.reader_user_turn_matches(&chat.chat_id, &a, "Read this").unwrap());
        chat_store.append_reader_user(&chat.chat_id, &a, "Read this").unwrap();
        // An interleaved or identical later prompt does not invalidate the
        // original queued Reader turn or grant a different generation.
        chat_store.append_user(&chat.chat_id, "Later prompt").unwrap();
        chat_store.append_reader_user(&chat.chat_id, &b, "Read this").unwrap();
        assert!(chat_store.reader_user_turn_matches(&chat.chat_id, &a, "Read this").unwrap());
        assert!(chat_store.reader_user_turn_matches(&chat.chat_id, &b, "Read this").unwrap());
        assert!(!chat_store.reader_user_turn_matches(&chat.chat_id, &a, "Other text").unwrap());
        assert!(!chat_store.reader_user_turn_matches(&second.chat_id, &a, "Read this").unwrap());
        assert!(!chat_store.reader_user_turn_matches(&chat.chat_id, "g-forged", "Read this").unwrap());
        drop(chat_store);
        let reopened = ChatStore::open(&root).unwrap();
        assert!(reopened.reader_user_turn_matches(&chat.chat_id, &a, "Read this").unwrap());
        assert_eq!(reopened.history(&chat.chat_id).unwrap().len(), 4);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn refuses_path_ids_and_unknown_folders() {
        let root = root();
        let s = ChatStore::open(&root).unwrap();
        let a = s.create_chat().unwrap();
        assert!(s.history("../../oops").is_err());
        assert!(s.move_chat(&a.chat_id, Some("../../".into())).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

impl lu_core::conversation_authority::TrustedUserTurnVerifier for ChatStore {
    fn verify_user_turn(&self, conversation_id: &str, user_turn_id: &str) -> bool {
        self.trusted_user_turn_text(conversation_id, user_turn_id).is_some()
    }
}
