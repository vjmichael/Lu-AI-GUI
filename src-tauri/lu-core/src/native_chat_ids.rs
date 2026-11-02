//! Canonical saved conversation IDs at the Lu-owned native tool authority gate.
//!
//! Existing Lu desktop chat routes use `ulidx::ulid()` (26-char Crockford
//! ULID); historical/native fixtures can use canonical lowercase UUIDs.
//! A format check is NEVER a permission grant: read_only_turn_context must
//! still attest exact saved USER turn + canonical profile-private paths.
//!
//! This file is Tauri-independent and has no Jan/Cowork dependency.

/// Accept only one canonical, filesystem-segment-safe representation.
/// UUID: 8-4-4-4-12 lowercase hexadecimal; ULID: 26 uppercase Crockford
/// base-32 characters with the first character restricted to 0..7.
/// Reject lookalike/un-normalized variants instead of rewriting identities.
pub fn canonical_conversation_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.len() == 36 {
        return bytes.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_digit() || matches!(*byte, b'a'..=b'f')
            }
        });
    }
    if bytes.len() == 26 {
        return matches!(bytes[0], b'0'..=b'7')
            && bytes.iter().all(|byte| {
                matches!(
                    *byte, b'0'..=b'9' | b'A'..=b'H' | b'J'..=b'K'
                    | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z'
                )
            });
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_real_lu_ulid_and_canonical_legacy_uuid() {
        assert!(canonical_conversation_id("01ARZ3NDEKTSV4RRFFQ69G5FAV"));
        assert!(canonical_conversation_id("7ZZZZZZZZZZZZZZZZZZZZZZZZZ"));
        assert!(canonical_conversation_id(
            "b28e03c1-13e8-4e38-b6fa-96610f353e9d"
        ));
        assert!(canonical_conversation_id(
            "00000000-0000-0000-0000-000000000000"
        ));
    }

    #[test]
    fn rejects_noncanonical_and_unsafe_variants() {
        for id in [
            "",
            " ",
            "01ARZ3NDEKTSV4RRFFQ69G5FAV ",
            "01arz3ndektsv4rrffq69g5fav",
            "8ZZZZZZZZZZZZZZZZZZZZZZZZZ",
            "01ARZ3NDEKTSV4RRFFQ69G5FAI",
            "01ARZ3NDEKTSV4RRFFQ69G5FAO",
            "01ARZ3NDEKTSV4RRFFQ69G5FAU",
            "01ARZ3NDEKTSV4RRFFQ69G5FAL",
            "01ARZ3NDEKTSV4RRFFQ69G5FA/",
            "../01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "B28E03C1-13E8-4E38-B6FA-96610F353E9D",
            "b28e03c113e84e38b6fa96610f353e9d",
            "{b28e03c1-13e8-4e38-b6fa-96610f353e9d}",
            "b28e03c1-13e8-4e38-b6fa-96610f353e9d/..",
            "b28e03c1-13e8-4e38-b6fa-96610f353e9d\0",
        ] {
            assert!(
                !canonical_conversation_id(id),
                "unexpected Lu identity {id:?}"
            );
        }
    }

    #[test]
    fn rejects_wrong_length_and_ambiguous_ulid_aliases() {
        assert!(!canonical_conversation_id("01H8XGJWBWBAQ"));
        assert!(!canonical_conversation_id(&"0".repeat(27)));
        assert!(!canonical_conversation_id(&"0".repeat(25)));
        assert!(!canonical_conversation_id(&"0".repeat(37)));
    }
}
