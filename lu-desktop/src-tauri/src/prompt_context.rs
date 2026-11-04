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

/// Unprivileged model-facing proposal hints until the native typed-tool catalog
/// is carried in inference IPC. The real saved user prompt remains unchanged.
/// A proposal is text data, NEVER a filesystem action or an authority grant.
pub fn model_file_creation_hint(prompt: &str) -> String {
    let lower=prompt.to_ascii_lowercase();
    let subject=["file","document","readme","config","source code","script",
        ".json",".jsonl",".md",".txt",".py",".rs",".toml",".yaml",".yml",
        ".csv",".ts",".js",".html",".css"]
        .iter().any(|word|lower.contains(word));
    let explicit_edit=["edit","modify","revise","update","replace","change",
        "patch","correct","fix"].iter().any(|verb|lower.contains(verb));
    let explicit_create=["create","write","save","export","generate"]
        .iter().any(|verb|lower.contains(verb));
    let negated=["do not create","don't create","do not save","don't save",
        "do not write","don't write","do not edit","don't edit",
        "do not modify","don't modify","do not change","don't change",
        "do not replace","don't replace"]
        .iter().any(|phrase|lower.contains(phrase));
    const CREATE_HINT:&str="\n\n[Lu new-file proposal contract, unprivileged]\nIf and only if the user requested creating a NEW local UTF-8 text file and provided enough content, output exactly one JSON object with only these fields: {\"kind\":\"propose_create_file\",\"file_name\":\"appropriate.ext\",\"purpose\":\"brief description\",\"content\":\"complete UTF-8 text\"}. Choose the extension from the intended format (.json must contain valid JSON, .md Markdown, .py Python, etc.). Do not provide a directory, absolute path, file handle, user approval, or tool grant. This can only create a file that does NOT already exist. Lu will ask the user to choose a folder and confirm before performing a write. Never claim it was saved merely by proposing JSON. If insufficient content, ask rather than fabricate. [/Lu new-file proposal contract]";
    const EDIT_HINT:&str="\n\n[Lu existing-file edit proposal contract, unprivileged]\nIf and only if the user explicitly requests editing an EXISTING local UTF-8 text file and you can supply the COMPLETE proposed replacement text, output exactly one JSON object with only these fields: {\"kind\":\"propose_edit_file\",\"file_name\":\"existing-name.ext\",\"purpose\":\"brief reason\",\"content\":\"complete replacement UTF-8 text\"}. This is NOT a patch fragment. Select a meaningful filename/extension and syntactically valid content (.json must be valid JSON). Do not invent original-file bytes, a directory, host path, file handle, approval or grant; you cannot modify files yourself. Lu will require the human to select the matching existing file using a native picker, show ALL old/new bytes and changes, obtain a separate native confirmation, and perform a journaled conflict-checked write. If you lack enough information for a complete replacement, ask for it instead of returning a proposal. Never claim an edit occurred because you proposed one. [/Lu existing-file edit proposal contract]";
    if !subject||negated {return prompt.to_owned();}
    let hint=if explicit_edit {EDIT_HINT}
        else if explicit_create {CREATE_HINT} else {return prompt.to_owned()};
    if prompt.len()+hint.len()>16*1024 {return prompt.to_owned();}
    format!("{prompt}{hint}")
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
        assert!(prompt.contains("choose a folder and confirm"));
    }
    #[test]
    fn explicit_edit_prompts_receive_only_an_inert_replacement_proposal() {
        let p=model_file_creation_hint("Edit notes.md to correct the grammar");
        assert!(p.contains("propose_edit_file"));
        assert!(p.contains("COMPLETE proposed replacement"));
        assert!(!p.contains("propose_create_file"));
        assert_eq!(model_file_creation_hint("Don't edit notes.md"),"Don't edit notes.md");
        assert_eq!(model_file_creation_hint("How do I edit notes.md?"),
            // Queries about *how* to edit may receive an unprivileged hint,
            // but no edit occurs without an explicit native file selection.
            model_file_creation_hint("How do I edit notes.md?"));
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
