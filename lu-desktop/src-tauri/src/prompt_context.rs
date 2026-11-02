//! Chat-scoped prompt assembly for Lu's local completion backend.
//! Only the current conversation is considered. Interrupted assistant text is
//! display-only, never silently promoted to completed context.
use crate::chat_store::ChatMessage;

const MAX_CONTEXT_BYTES: usize = 15 * 1024;
const MAX_PRIOR_TURNS: usize = 16;

pub fn assemble(previous: &[ChatMessage], current: &str) -> String {
    if previous.is_empty() {
        return current.to_owned();
    }
    let mut turns = Vec::new();
    let mut used = current.len() + 128;
    for entry in previous.iter().rev() {
        if entry.role != "user" && (entry.role != "assistant" || entry.interrupted) {
            continue;
        }
        if entry.text.trim().is_empty() {
            continue;
        }
        let role = if entry.role == "user" {
            "User"
        } else {
            "Assistant"
        };
        let chunk = format!("{role}: {}\n", entry.text.trim());
        if used + chunk.len() > MAX_CONTEXT_BYTES {
            break;
        }
        turns.push(chunk);
        used += turns.last().unwrap().len();
        if turns.len() == MAX_PRIOR_TURNS {
            break;
        }
    }
    if turns.is_empty() {
        return current.to_owned();
    }
    turns.reverse();
    format!(
        "Continue this same conversation. Answer the current user message in context.\n\n{}User: {}\nAssistant:",
        turns.concat(), current.trim()
    )
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
    fn empty_history_leaves_single_turn_unchanged() {
        assert_eq!(assemble(&[], "Hi"), "Hi");
    }
    #[test]
    fn cancelled_arkansas_question_remains_available_to_followup() {
        let history = [
            msg("user", "What is the cost of living in Arkansas?", false),
            msg("assistant", "Partial answer that was canceled", true),
        ];
        let prompt = assemble(&history, "How much is the cost of living again?");
        assert!(prompt.contains("cost of living in Arkansas"));
        assert!(!prompt.contains("Partial answer"));
        assert!(prompt.ends_with("User: How much is the cost of living again?\nAssistant:"));
    }
    #[test]
    fn completed_assistant_turn_in_context() {
        let history = [
            msg("user", "What is a GGUF?", false),
            msg("assistant", "A model file format.", false),
        ];
        let prompt = assemble(&history, "Does it have weights?");
        assert!(prompt.contains("Assistant: A model file format."));
    }
    #[test]
    fn bounded_history_does_not_exceed_transport_limit() {
        let history = vec![msg("user", &"x".repeat(5000), false); 40];
        let prompt = assemble(&history, "hello");
        assert!(prompt.len() <= MAX_CONTEXT_BYTES);
    }
}
