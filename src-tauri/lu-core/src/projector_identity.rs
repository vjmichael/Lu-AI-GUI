//! Canonical host-only GGUF/model projector identity mapping.
//!
//! Stable references must agree across importer, supervisor, Protobuf and
//! persisted sessions. A model is `m-` plus 32 lowercase hexadecimal digits;
//! its companion is `p-` plus the *same* 32 digits. Never use `p-m-...`, infer
//! a projector from nearby files, or accept a model-/webview-issued file path.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectorIdentityError {
    InvalidModelReference,
}

pub fn valid_model_ref(model_ref: &str) -> bool {
    model_ref.len() == 34
        && model_ref.starts_with("m-")
        && model_ref.as_bytes()[2..]
            .iter()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
}

pub fn projector_ref_for_model(model_ref: &str) -> Result<String, ProjectorIdentityError> {
    if !valid_model_ref(model_ref) {
        return Err(ProjectorIdentityError::InvalidModelReference);
    }
    Ok(format!("p-{}", &model_ref[2..]))
}

pub fn projector_filename_for_model(model_ref: &str) -> Result<String, ProjectorIdentityError> {
    if !valid_model_ref(model_ref) {
        return Err(ProjectorIdentityError::InvalidModelReference);
    }
    Ok(format!("mmproj-for-{model_ref}.gguf"))
}

#[cfg(test)]
mod tests {
    use super::*;
    const MODEL: &str = "m-00112233445566778899aabbccddeeff";
    #[test]
    fn canonical_pair_remains_stable_after_rehydration() {
        let from_first_session = projector_ref_for_model(MODEL).unwrap();
        let after_restart = projector_ref_for_model(&MODEL.to_owned()).unwrap();
        assert_eq!(from_first_session, "p-00112233445566778899aabbccddeeff");
        assert_eq!(from_first_session, after_restart);
        assert_eq!(
            projector_filename_for_model(MODEL).unwrap(),
            "mmproj-for-m-00112233445566778899aabbccddeeff.gguf"
        );
    }
    #[test]
    fn rejects_ambiguous_or_unsafe_model_refs() {
        for model in [
            "",
            "m-",
            "m-00112233445566778899aabbccddeef",
            "m-00112233445566778899aabbccddeeffff",
            "m-00112233445566778899AABBCCDDEEFF",
            "m-00112233445566778899aabbccddeef/",
            "p-00112233445566778899aabbccddeeff",
            "../00112233445566778899aabbccddeef",
            "m-00112233445566778899aabbccddefé",
        ] {
            assert!(!valid_model_ref(model), "unexpectedly accepted {model:?}");
            assert!(projector_ref_for_model(model).is_err());
            assert!(projector_filename_for_model(model).is_err());
        }
    }
    #[test]
    fn distinct_model_refs_do_not_alias() {
        let other = "m-00112233445566778899aabbccddeefe";
        assert_ne!(
            projector_ref_for_model(MODEL),
            projector_ref_for_model(other)
        );
    }
}
