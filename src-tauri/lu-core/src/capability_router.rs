//! Lu-owned *generic* host-review and native route admission.
//!
//! Lu Core MUST NOT have built-in add-on IDs, schemas, model prompts, NLP
//! heuristics, or worker implementation details. A DLC can advertise typed
//! capabilities via the existing signed-package, per-turn grant and registry
//! surfaces. A host-reviewed candidate is NOT a grant or an execution event.
//!
//! Host code (not a model, WebView or untrusted worker) may supply a candidate
//! derived from the *current verified USER turn*. This module checks that it
//! matches an exact host-owned rule, installed tool, effect, Protobuf codec,
//! and the per-turn advertised model schema before the existing authorized,
//! durable dispatcher sees it. Default Lu has NO built-in DLC routes.
use crate::{contracts::ToolEffect, model::ModelToolCatalog, registry::NativeToolRegistry};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Deliberately empty: third-party modules are separate add-ons, not Lu
/// Core imports, compiled route tables or implicit installed packages.
pub const BUILTIN_DLC_ROUTES: &[ReviewedHostRouteRule] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewedHostRouteRule {
    pub rule_id: &'static str,
    pub public_name: &'static str,
    pub native_tool_id: &'static str,
    pub effect: ToolEffect,
    pub input_type_url: &'static str,
    pub argument_name: &'static str,
}

/// A non-authoritative host suggestion, bound to the same persisted current
/// USER turn used for generation. Created by trusted host-side route planning
/// after package verification, never deserialized from a model tool request.
/// This is independent of any specific DLC or domain (math, IDE, film, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewedRouteCandidate {
    pub rule_id: String,
    pub public_name: String,
    pub arguments: Value,
    pub current_user_digest: [u8; 32],
}

impl ReviewedRouteCandidate {
    pub fn from_host_current_user(
        verified_current_user: &str,
        rule_id: &str,
        public_name: &str,
        arguments: Value,
    ) -> Self {
        let digest = Sha256::digest(verified_current_user.as_bytes());
        let mut current_user_digest = [0u8; 32];
        current_user_digest.copy_from_slice(&digest);
        Self {
            rule_id: rule_id.to_owned(),
            public_name: public_name.to_owned(),
            arguments,
            current_user_digest,
        }
    }
}

/// Host-side natural-language preference is a veto; not all user requests
/// mentioning an add-on authorize an add-on or a tool execution.
pub fn user_declines_tools(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    [
        "no tools",
        "no tool",
        "without tools",
        "without any tools",
        "without using tools",
        "do not use tools",
        "don't use tools",
        "do not call tools",
        "don't call tools",
        "without specialists",
        "without any specialists",
        "do not use specialists",
        "never call a tool",
    ]
    .iter()
    .any(|refusal| lower.contains(refusal))
}

/// A positive match only makes the candidate eligible to reach the normal
/// conversation-bound, policy-checked durable dispatcher. In particular:
/// - a DLC manifest/tool discovery is NEVER authority;
/// - no source from model text/previous chat is treated as a route;
/// - duplicates/ambiguous host rules are denied;
/// - a candidate cannot request extra fields or a different effect/schema;
/// - current USER provenance must be exact and nonempty.
pub fn admit_reviewed_host_candidate(
    verified_current_user: &str,
    candidate: &ReviewedRouteCandidate,
    trusted_rules: &[ReviewedHostRouteRule],
    catalog: &ModelToolCatalog,
    registry: &NativeToolRegistry,
    admitted_schemas: &[Value],
) -> Option<ReviewedRouteCandidate> {
    if verified_current_user.trim().is_empty()
        || verified_current_user.len() > 16 * 1024
        || user_declines_tools(verified_current_user)
        || candidate.public_name.is_empty()
        || candidate.rule_id.is_empty()
    {
        return None;
    }
    let digest = Sha256::digest(verified_current_user.as_bytes());
    if digest[..] != candidate.current_user_digest[..] {
        return None;
    }
    let mut matching_rule = None;
    for rule in trusted_rules {
        if rule.rule_id != candidate.rule_id || rule.public_name != candidate.public_name {
            continue;
        }
        if matching_rule.is_some() {
            return None;
        }
        matching_rule = Some(rule);
    }
    let rule = matching_rule?;
    let args = candidate.arguments.as_object()?;
    if args.len() != 1
        || args
            .get(rule.argument_name)
            .and_then(Value::as_str)
            .is_none_or(|argument| argument.is_empty() || argument.len() > 4096)
    {
        return None;
    }
    if !catalog.matches_reviewed_route(
        registry,
        admitted_schemas,
        rule.public_name,
        rule.native_tool_id,
        rule.effect,
        rule.input_type_url,
        rule.argument_name,
    ) {
        return None;
    }
    Some(candidate.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_add_on_is_built_in_or_activated_from_discovery() {
        assert!(BUILTIN_DLC_ROUTES.is_empty());
        let catalog = ModelToolCatalog::new();
        let registry = NativeToolRegistry::new();
        let candidate = ReviewedRouteCandidate::from_host_current_user(
            "Read a document",
            "host.reviewed.files.v1",
            "dlc_read",
            serde_json::json!({"path":"document.txt"}),
        );
        assert!(admit_reviewed_host_candidate(
            "Read a document",
            &candidate,
            BUILTIN_DLC_ROUTES,
            &catalog,
            &registry,
            &[],
        )
        .is_none());
        assert!(admit_reviewed_host_candidate(
            "Changed current request",
            &candidate,
            BUILTIN_DLC_ROUTES,
            &catalog,
            &registry,
            &[],
        )
        .is_none());
    }

    #[test]
    fn current_user_digest_and_refusal_are_hard_guards() {
        let catalog = ModelToolCatalog::new();
        let registry = NativeToolRegistry::new();
        let rule = ReviewedHostRouteRule {
            rule_id: "host.reviewed.operation.v1",
            public_name: "dlc_execute",
            native_tool_id: "acme.extension.execute",
            effect: ToolEffect::ExecuteCode,
            input_type_url: "type.googleapis.com/acme.v1.ExecuteRequest",
            argument_name: "expression",
        };
        let candidate = ReviewedRouteCandidate::from_host_current_user(
            "Run a reviewed operation",
            rule.rule_id,
            rule.public_name,
            serde_json::json!({"expression":"safe"}),
        );
        let mut tampered = candidate.clone();
        tampered.current_user_digest[0] ^= 1;
        for c in [candidate, tampered] {
            assert!(admit_reviewed_host_candidate(
                "Do not use tools",
                &c,
                &[rule],
                &catalog,
                &registry,
                &[],
            )
            .is_none());
        }
        assert!(!user_declines_tools("Read a document"));
        assert!(user_declines_tools("Read a document without tools"));
    }
}
