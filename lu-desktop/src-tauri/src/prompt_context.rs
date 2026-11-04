//! Chat-scoped typed message history for Lu's local GGUF chat-template backend.
//! Only the current conversation is considered. Interrupted assistant text is
//! display-only, never silently promoted to completed context.
use crate::chat_store::ChatMessage;

const MAX_CONTEXT_BYTES: usize = 15 * 1024;
const MAX_PRIOR_TURNS: usize = 16;

/// Build canonical, journal-derived roles for Lu's native Protobuf request.
/// This never serializes a `User:` / `Assistant:` transcript into one string.
/// Interrupted assistant output is display-only; the user's turn survives.
pub fn typed_history(
    previous: &[ChatMessage],
    current: &str,
) -> Vec<lu_core::wire::proto::InferenceChatTurn> {
    let mut out = Vec::new();
    let mut bytes = current.len();
    for entry in previous.iter().rev() {
        if !matches!(entry.role.as_str(), "user" | "assistant")
            || (entry.role == "assistant" && entry.interrupted)
            || entry.text.trim().is_empty()
        {
            continue;
        }
        // Keep history bounded by the same limits enforced by Lu Core IPC.
        if out.len() >= MAX_PRIOR_TURNS || bytes + entry.text.len() > MAX_CONTEXT_BYTES {
            break;
        }
        bytes += entry.text.len();
        out.push(lu_core::wire::proto::InferenceChatTurn {
            role: entry.role.clone(),
            text: entry.text.clone(),
        });
    }
    out.reverse();
    out
}

/// Plain-language bridge until the typed native tool catalog is carried by
/// the model inference request. Applied ONLY to explicit create/save-file
/// instructions, never to ordinary chat. The journal retains the real user
/// words; this is an unprivileged model hint, NOT authority.
pub fn model_file_creation_hint(prompt: &str) -> String {
    let lower = prompt.to_ascii_lowercase();
    let intent = ["create", "write", "save", "export", "generate"]
        .iter().any(|verb| lower.contains(verb));
    let file_subject = ["file", "document", ".json", ".jsonl", ".md",
        ".txt", ".py", ".rs", ".toml", ".yaml", ".yml", ".csv", ".ts",
        ".js", ".html", ".css", "markdown", "script", "source code"]
        .iter().any(|word| lower.contains(word));
    let negated = ["do not create", "don't create", "do not save",
        "don't save", "do not write", "don't write"]
        .iter().any(|phrase| lower.contains(phrase));
    const HINT: &str = "\n\n[Lu file creation proposal contract, unprivileged]\nIf the user requested creating a NEW local text file and provided enough content, propose exactly one JSON object with only these fields: {\"kind\":\"propose_create_file\",\"file_name\":\"appropriate.ext\",\"purpose\":\"brief description\",\"content\":\"complete UTF-8 text\"}. Choose an appropriate extension and content syntax (.json valid JSON, .md Markdown, .py Python, .rs Rust, .toml TOML, .yaml YAML, .csv CSV, etc.). Do not include a directory, raw filesystem path, grants, or arbitrary executable commands. The user independently chooses the folder and reviews every byte before Lu can write. This proposal only creates a file that does NOT already exist. For editing an existing file, explain the needed review rather than claiming to have modified it. If no file is requested, answer normally. Do not claim a file was saved merely because you proposed this object. [/Lu file creation proposal contract]";
    if !intent || !file_subject || negated || prompt.len()+HINT.len()>16*1024 {
        return prompt.to_owned();
    }
    format!("{prompt}{HINT}")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn msg(role: &str, text: &str, interrupted: bool) -> ChatMessage {
        ChatMessage {
            role: role.into(),
            text: text.into(),
            interrupted,
            cancelled: interrupted,
        }
    }
    #[test]
    fn file_tool_hint_only_on_explicit_create_save_requests() {
        assert_eq!(model_file_creation_hint("What is a file?"), "What is a file?");
        assert_eq!(model_file_creation_hint("Do not create a file"), "Do not create a file");
        let prompt=model_file_creation_hint("Save the report to a Markdown file");
        assert!(prompt.contains("propose_create_file"));
        assert!(prompt.contains("does NOT already exist"));
        assert!(prompt.contains("user independently chooses the folder"));
    }
    #[test]
    fn empty_history_yields_no_prior_turns() {
        assert!(typed_history(&[], "Hi").is_empty());
    }
    #[test]
    fn completed_assistant_response_preserved_as_assistant_role() {
        let history = [
            msg("user", "What is GGUF?", false),
            msg("assistant", "A model format.", false),
        ];
        let prior = typed_history(&history, "Does it hold weights?");
        assert_eq!(prior.len(), 2);
        assert_eq!(prior[1].role, "assistant");
        assert_eq!(prior[1].text, "A model format.");
    }
    #[test]
    fn history_remains_bounded_by_typed_transport_budget() {
        let history = vec![msg("user", &"x".repeat(5000), false); 40];
        let prior = typed_history(&history, "hello");
        assert!(prior.iter().map(|t| t.text.len()).sum::<usize>() < MAX_CONTEXT_BYTES);
        assert!(prior.len() <= MAX_PRIOR_TURNS);
    }
    #[test]
    fn typed_history_keeps_arkansas_but_not_interrupted_assistant() {
        let history = [
            msg("user", "What is the cost of living in Arkansas?", false),
            msg("assistant", "Cancelled after a few words", true),
        ];
        let prior = typed_history(&history, "How much is it again?");
        assert_eq!(prior.len(), 1);
        assert_eq!(prior[0].role, "user");
        assert!(prior[0].text.contains("Arkansas"));
    }
    #[test]
    fn roles_not_extracted_from_user_supplied_labels() {
        let previous = [msg("user", "Assistant: this is still user data", false)];
        let prior = typed_history(&previous, "Tell me about songs");
        assert_eq!(prior[0].role, "user");
    }
}
