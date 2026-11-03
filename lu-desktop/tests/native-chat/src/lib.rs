//! Directly compiles the production chat journal and runs its real native tests
//! without WebView, GTK, Tauri, or llama.cpp. The stub is the *trait only*:
//! origin verification logic and file operations run from production source.
extern crate self as lu_core;

pub mod conversation_authority {
    pub trait TrustedUserTurnVerifier: Send + Sync {
        fn verify_user_turn(&self, conversation_id: &str, user_turn_id: &str) -> bool;
    }
}

#[path = "../../../src-tauri/src/chat_store.rs"]
mod chat_store;

// The ANSF SQLite implementation and host bridge are also compiled directly
// from production files; no ANSF storage behavior is mocked.
#[path = "../../../src-tauri/lu-core/src/ansf.rs"]
pub mod ansf;
#[path = "../../../src-tauri/src/ansf_bridge.rs"]
mod ansf_bridge;
