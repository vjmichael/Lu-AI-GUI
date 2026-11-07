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

// Real M2 ANSF goal events and chat journal compile together without Tauri.
#[path = "../../../../src-tauri/lu-core/src/ansf_goals.rs"]
pub mod ansf_goals;
// The production ANSF SQLite module is compiled directly without a mock.
#[path = "../../../../src-tauri/lu-core/src/ansf.rs"]
pub mod ansf;
