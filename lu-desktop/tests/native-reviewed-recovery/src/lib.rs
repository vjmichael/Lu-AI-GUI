//! Portable compile/test of the *production* desktop E1-P08 recovery module.
//! There are no Tauri/WebView/GTK mocks; lu-core TaskStore and EffectJournal
//! are the same SQLite implementations used by the installed native host.
#[path = "../../../src-tauri/src/reviewed_app_recovery.rs"]
mod reviewed_app_recovery;

#[path = "../../../src-tauri/src/reviewed_app_audit.rs"]
mod reviewed_app_audit;
