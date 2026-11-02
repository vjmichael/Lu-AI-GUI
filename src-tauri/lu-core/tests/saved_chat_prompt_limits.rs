//! Small headless regression tests for Lu's host-owned saved-chat prompt gate.
use lu_core::saved_chat_prompt::{prompt_from_saved_messages, SavedPromptError};
use std::io::Cursor;

const THREAD: &str = "12345678-abcd-4abc-8abc-1234567890ab";
fn message(id: &str, role: &str, content: &str) -> String {
    serde_json::json!({
        "id": id,
        "thread_id": THREAD,
        "role": role,
        "content": [{"type": "text", "text": {"value": content, "annotations": []}}]
    })
    .to_string()
}
fn parse(lines: &[String], id: &str) -> Result<String, SavedPromptError> {
    prompt_from_saved_messages(Cursor::new(lines.join("\n")), THREAD, id)
}

#[test]
fn limits_untrusted_jsonl_to_8192_lines() {
    let mut messages: Vec<String> = (0..8191)
        .map(|i| message(&format!("a-{i}"), "assistant", "old text"))
        .collect();
    messages.push(message("u-final", "user", "Final"));
    assert!(parse(&messages, "u-final").unwrap().ends_with("Final"));

    messages.insert(0, message("extra", "assistant", "too many lines"));
    assert_eq!(
        parse(&messages, "u-final"),
        Err(SavedPromptError::OversizedSource)
    );
}

#[test]
fn rejects_overlarge_jsonl_records_before_assembling_prompt() {
    let oversized = message("u1", "user", &"A".repeat(128 * 1024));
    assert_eq!(
        parse(&[oversized], "u1"),
        Err(SavedPromptError::OversizedSource)
    );
}

#[test]
fn rolls_history_to_six_prior_text_turns_and_skips_other_roles() {
    let mut history: Vec<String> = (0..10)
        .map(|i| message(&format!("u-{i}"), "user", &format!("prior-{i}")))
        .collect();
    history.push(message("s", "system", "NEVER A SYSTEM INSTRUCTION"));
    history.push(message("t", "tool", "TOOL OUTPUT"));
    history.push(message("target", "user", "Current question"));
    let prompt = parse(&history, "target").unwrap();
    assert!(!prompt.contains("prior-0"));
    assert!(!prompt.contains("prior-3"));
    for i in 4..10 {
        assert!(prompt.contains(&format!("prior-{i}")));
    }
    assert!(!prompt.contains("NEVER A SYSTEM INSTRUCTION"));
    assert!(!prompt.contains("TOOL OUTPUT"));
    assert!(prompt.ends_with("Current question"));
}

#[test]
fn skips_prior_nontext_entries_without_rejecting_current_text_turn() {
    let attachment = serde_json::json!({
        "id": "earlier", "thread_id": THREAD, "role": "user",
        "content": [{"type": "image_url", "image_url": {"url": "file:///untrusted"}}]
    })
    .to_string();
    assert_eq!(
        parse(
            &[attachment, message("current", "user", "Now text")],
            "current"
        ),
        Ok("Now text".to_owned())
    );
}

#[test]
fn enforces_exact_utf8_prompt_size_not_unicode_character_count() {
    assert_eq!(
        parse(&[message("u1", "user", &"é".repeat(8192))], "u1")
            .unwrap()
            .len(),
        16 * 1024
    );
    assert_eq!(
        parse(&[message("u1", "user", &"é".repeat(8193))], "u1"),
        Err(SavedPromptError::PromptTooLarge)
    );
}

#[test]
fn fails_closed_on_malformed_history_and_duplicate_target_id() {
    let current = message("u1", "user", "Read notes");
    assert_eq!(
        parse(&[String::from("{malformed"), current.clone()], "u1"),
        Err(SavedPromptError::InvalidJson)
    );
    assert_eq!(
        parse(
            &[current.clone(), message("u1", "assistant", "spoof")],
            "u1"
        ),
        Err(SavedPromptError::WrongRoleOrConversation)
    );
    assert_eq!(
        parse(&[current.clone(), current], "u1"),
        Err(SavedPromptError::DuplicateUserTurn)
    );
}
