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
