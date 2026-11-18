//! Durable, user-facing browser visibility preference.
//! Preferences configure rendering only; they never grant browser/network
//! authorization. The default preserves one headed Chromium session hidden
//! through native window management.
use crate::browser_visibility_settings::BrowserVisibilityPreference;
use rusqlite::{params,Connection};
use std::{path::Path,sync::Mutex};
#[derive(Debug)]
pub enum BrowserPreferenceError {
    Database(String),
    Poisoned,
    InvalidSavedMode,
}
pub struct BrowserPreferences{db:Mutex<Connection>}
impl BrowserPreferences{
    pub fn open(path:&Path)->Result<Self,BrowserPreferenceError>{
        let db=Connection::open(path)
            .map_err(|e|BrowserPreferenceError::Database(e.to_string()))?;
        db.pragma_update(None,"journal_mode","WAL")
            .map_err(|e|BrowserPreferenceError::Database(e.to_string()))?;
        db.pragma_update(None,"synchronous","FULL")
            .map_err(|e|BrowserPreferenceError::Database(e.to_string()))?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS lu_browser_preferences(
               id INTEGER PRIMARY KEY CHECK(id=1),
               visibility TEXT NOT NULL CHECK(visibility IN ('automatic_hidden','always_visible'))
             );
             INSERT OR IGNORE INTO lu_browser_preferences(id,visibility)
             VALUES(1,'automatic_hidden');"
        ).map_err(|e|BrowserPreferenceError::Database(e.to_string()))?;
        Ok(Self{db:Mutex::new(db)})
    }
    pub fn visibility(&self)->Result<BrowserVisibilityPreference,BrowserPreferenceError>{
        let db=self.db.lock().map_err(|_|BrowserPreferenceError::Poisoned)?;
        let mode:String=db.query_row(
            "SELECT visibility FROM lu_browser_preferences WHERE id=1",
            [],|row|row.get(0)
        ).map_err(|e|BrowserPreferenceError::Database(e.to_string()))?;
        match mode.as_str(){
            "automatic_hidden"=>Ok(BrowserVisibilityPreference::AutomaticHidden),
            "always_visible"=>Ok(BrowserVisibilityPreference::AlwaysVisible),
            _=>Err(BrowserPreferenceError::InvalidSavedMode),
        }
    }
    /// Only invoke through a user-initiated settings action; this does not
    /// authorize browser navigation or any other external side effect.
    pub fn set_visibility(&self,mode:BrowserVisibilityPreference)
        ->Result<(),BrowserPreferenceError>{
        let text=match mode {
            BrowserVisibilityPreference::AutomaticHidden=>"automatic_hidden",
            BrowserVisibilityPreference::AlwaysVisible=>"always_visible",
        };
        let db=self.db.lock().map_err(|_|BrowserPreferenceError::Poisoned)?;
        db.execute(
            "UPDATE lu_browser_preferences SET visibility=?1 WHERE id=1",
            params![text]
        ).map_err(|e|BrowserPreferenceError::Database(e.to_string()))?;
        Ok(())
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn default_persists_and_mode_changes_survive_reopen(){
        let path=std::env::temp_dir().join(format!(
            "lu-browser-preferences-{}-{}.sqlite",std::process::id(),
            std::thread::current().name().unwrap_or("default").replace(':',"_")));
        let _=std::fs::remove_file(&path);
        {
            let prefs=BrowserPreferences::open(&path).unwrap();
            assert_eq!(prefs.visibility().unwrap(),BrowserVisibilityPreference::AutomaticHidden);
            prefs.set_visibility(BrowserVisibilityPreference::AlwaysVisible).unwrap();
        }
        {
            let prefs=BrowserPreferences::open(&path).unwrap();
            assert_eq!(prefs.visibility().unwrap(),BrowserVisibilityPreference::AlwaysVisible);
        }
        let _=std::fs::remove_file(&path);
        let _=std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _=std::fs::remove_file(path.with_extension("sqlite-shm"));
    }
}
