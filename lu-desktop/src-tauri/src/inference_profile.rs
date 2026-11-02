//! R23 native per-model inference profiles: never loaded automatically.
//! The native host, not the WebView or model output, persists validated values.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

const MAX_PROFILE_FILE_BYTES: u64 = 96 * 1024;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InferenceProfile {
    pub context_tokens: u32,
    pub max_new_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub min_p: f32,
    pub threads: u32,
}
impl Default for InferenceProfile {
    fn default() -> Self {
        Self {
            context_tokens: 8192,
            max_new_tokens: 2048,
            temperature: 0.6,
            top_p: 0.95,
            top_k: 20,
            min_p: 0.0,
            threads: 4,
        }
    }
}
impl InferenceProfile {
    pub fn validate(self) -> Result<Self, String> {
        if !(4096..=262_144).contains(&self.context_tokens)
            || self.context_tokens % 1024 != 0
            || !(128..=8192).contains(&self.max_new_tokens)
            || self.max_new_tokens >= self.context_tokens
            || !self.temperature.is_finite()
            || !(0.0..=2.0).contains(&self.temperature)
            || !self.top_p.is_finite()
            || !(0.01..=1.0).contains(&self.top_p)
            || !(1..=100).contains(&self.top_k)
            || !self.min_p.is_finite()
            || !(0.0..=1.0).contains(&self.min_p)
            || !(1..=64).contains(&self.threads)
        {
            return Err(
                "invalid inference profile; check context, output limit and sampling range".into(),
            );
        }
        Ok(self)
    }
}

pub fn read(path: &Path) -> Result<HashMap<String, InferenceProfile>, String> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.file_type().is_file() || meta.len() > MAX_PROFILE_FILE_BYTES {
        return Err("profile storage is not a bounded regular file".into());
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_PROFILE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_PROFILE_FILE_BYTES {
        return Err("profile storage too large".into());
    }
    let profiles: HashMap<String, InferenceProfile> =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid profile storage: {e}"))?;
    if profiles.len() > 64 {
        return Err("too many profiles".into());
    }
    for (id, profile) in &profiles {
        if !id.starts_with("m-")
            || id.len() != 34
            || !id[2..]
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err("invalid model identity in profile storage".into());
        }
        profile.validate()?;
    }
    Ok(profiles)
}
pub fn write(path: &Path, profiles: &HashMap<String, InferenceProfile>) -> Result<(), String> {
    if profiles.len() > 64 {
        return Err("too many profiles".into());
    }
    for profile in profiles.values() {
        profile.validate()?;
    }
    let data = serde_json::to_vec_pretty(profiles).map_err(|e| e.to_string())?;
    if data.len() as u64 > MAX_PROFILE_FILE_BYTES {
        return Err("profile storage too large".into());
    }
    let unique = format!(
        ".lu-profile-{}-{}.tmp",
        std::process::id(),
        rand::random::<u64>()
    );
    let temp = path.with_file_name(unique);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(&data).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        // Replacing an existing profile file is supported on Windows as well.
        // Preserve the existing file until a complete temporary file exists.
        if path.exists() {
            let backup = path.with_extension("json.bak");
            if backup.exists() {
                fs::remove_file(&backup).map_err(|e| e.to_string())?;
            }
            fs::rename(path, &backup).map_err(|e| e.to_string())?;
            if let Err(err) = fs::rename(&temp, path) {
                let _ = fs::rename(&backup, path);
                return Err(err.to_string());
            }
            let _ = fs::remove_file(backup);
        } else {
            fs::rename(&temp, path).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_valid() {
        assert!(InferenceProfile::default().validate().is_ok());
    }
    #[test]
    fn nonfinite_sampling_is_rejected() {
        let mut p = InferenceProfile::default();
        p.temperature = f32::NAN;
        assert!(p.validate().is_err());
        p = InferenceProfile::default();
        p.max_new_tokens = 8192;
        assert!(p.validate().is_err());
    }
    #[test]
    fn profile_persists_and_reloads() {
        let folder = std::env::temp_dir().join(format!("lu-r23-{}", rand::random::<u64>()));
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("profiles.json");
        let key = format!("m-{:032x}", 42u128);
        let mut profiles = HashMap::new();
        profiles.insert(key.clone(), InferenceProfile::default());
        write(&file, &profiles).unwrap();
        assert_eq!(read(&file).unwrap().get(&key), profiles.get(&key));
        fs::remove_dir_all(folder).unwrap();
    }
}
