//! DEV-03.4: bounded, read-only conventional test discovery.
//! No user-selected traversal paths, script evaluation, test commands,
//! package manager, discovered executable, or execution authorizations.
use crate::{browser::BrowserError, IdeWorkspaceRegistry};
use serde::Serialize;
use std::io::ErrorKind;

const MAX_TEST_FILES: usize = 96;
const MAX_FIXED_DIRS: usize = 7;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredTestFile {
    pub relative_path: String,
    pub language: &'static str,
    pub recipe_id: Option<&'static str>,
    pub file_bytes: u64,
    pub runnable: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestDiscovery {
    pub schema_version: u32,
    pub project_id: String,
    pub source: &'static str,
    pub directories_checked: usize,
    pub files: Vec<DiscoveredTestFile>,
    pub truncated: bool,
    pub incomplete: bool,
    pub scan_notes: Vec<String>,
    pub process_started: bool,
    pub worker_grant_issued: bool,
    pub source_contents_exported: bool,
    pub discovery_is_execution_authority: bool,
}

const DIRS: [&str; MAX_FIXED_DIRS] = [
    "tests",
    "test",
    "src",
    "src/tests",
    "src/test",
    "__tests__",
    "ide/host/tests",
];

fn classify(dir: &str, name: &str) -> Option<(&'static str, Option<&'static str>)> {
    if name.len() > 160 || name.starts_with('.') {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    // File-name conventions are *hints*; no inspection of test functions,
    // imports, scripts or package declarations occurs.
    if lower.ends_with(".rs")
        && (dir.ends_with("tests") || lower.starts_with("test_") || lower.ends_with("_test.rs"))
    {
        return Some((
            "rust",
            Some(if dir == "ide/host/tests" {
                "lu_ide_host_tests"
            } else {
                "rust_tests"
            }),
        ));
    }
    if lower.ends_with(".py") && (lower.starts_with("test_") || lower.ends_with("_test.py")) {
        return Some(("python", None));
    }
    if [
        ".test.js",
        ".spec.js",
        ".test.mjs",
        ".spec.mjs",
        ".test.cjs",
        ".spec.cjs",
        ".test.ts",
        ".spec.ts",
        ".test.jsx",
        ".spec.jsx",
        ".test.tsx",
        ".spec.tsx",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
    {
        return Some(("javascript", None));
    }
    if [".cpp", ".cc", ".cxx", ".c"]
        .iter()
        .any(|ext| lower.ends_with(ext))
        && (lower.starts_with("test_") || lower.contains("test") || lower.ends_with("_spec.cpp"))
    {
        return Some(("cpp", None));
    }
    None
}

impl IdeWorkspaceRegistry {
    /// Fixed-directory metadata only. A project cannot ask to scan any path.
    /// A rust_tests hint cannot authorize running Cargo on a dirty tree.
    pub fn discover_conventional_test_files(
        &self,
        project_id: &str,
    ) -> Result<TestDiscovery, &'static str> {
        self.source_capability(project_id)
            .map_err(|_| "DEV034_PROJECT_NOT_OPEN")?;
        let mut files = Vec::new();
        let mut notes = Vec::new();
        let mut incomplete = true; // Never claim exhaustive discovery.
        let mut truncated = false;
        for dir in DIRS {
            match self.list_source_directory(project_id, dir) {
                Ok(list) if list.project_id == project_id && list.relative_dir == dir => {
                    if list.truncated {
                        truncated = true;
                        notes.push(format!("{dir}:directory_bounded"));
                    }
                    for entry in list.entries {
                        if entry.kind != "file" {
                            continue;
                        }
                        let Some((language, recipe)) = classify(dir, &entry.name) else {
                            continue;
                        };
                        if entry.relative_path != format!("{dir}/{}", entry.name) {
                            notes.push(format!("{dir}:entry_identity_denied"));
                            continue;
                        }
                        if files.len() >= MAX_TEST_FILES {
                            truncated = true;
                            break;
                        }
                        files.push(DiscoveredTestFile {
                            relative_path: entry.relative_path,
                            language,
                            recipe_id: recipe,
                            file_bytes: entry.size,
                            runnable: false,
                        });
                    }
                }
                Ok(_) => notes.push(format!("{dir}:native_identity_mismatch")),
                Err(BrowserError::Io(e)) if e.kind() == ErrorKind::NotFound => {
                    notes.push(format!("{dir}:absent"))
                }
                Err(_) => notes.push(format!("{dir}:unverifiable")),
            }
            if files.len() >= MAX_TEST_FILES {
                truncated = true;
                break;
            }
        }
        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        files.dedup_by(|a, b| a.relative_path == b.relative_path);
        // The complete suite may include generated tests, workspaces, doctests,
        // pytest parametrization, test discovery plugins, nested source trees.
        incomplete |= truncated;
        Ok(TestDiscovery {
            schema_version: 1,
            project_id: project_id.to_owned(),
            source: "fixed_native_conventional_file_metadata",
            directories_checked: MAX_FIXED_DIRS,
            files,
            truncated,
            incomplete,
            scan_notes: notes,
            process_started: false,
            worker_grant_issued: false,
            source_contents_exported: false,
            discovery_is_execution_authority: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conventional_names_only() {
        assert_eq!(classify("tests", "test_math.py"), Some(("python", None)));
        assert_eq!(
            classify("src", "player.test.mjs"),
            Some(("javascript", None))
        );
        assert_eq!(
            classify("tests", "engine_test.rs"),
            Some(("rust", Some("rust_tests")))
        );
        assert_eq!(classify("src", "engine.rs"), None);
        assert_eq!(classify("tests", "setup.sh"), None);
        assert_eq!(classify("tests", "payload.test.txt"), None);
    }
}

#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TestSymbolHint {
    pub name:String,
    pub line:u32,
    pub candidate_only:bool,
    pub executable_test_verified:bool,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct TestSymbolInspection {
    pub schema_version:u32,
    pub project_id:String,
    pub relative_path:String,
    pub source_revision:String,
    pub language:&'static str,
    pub symbols:Vec<TestSymbolHint>,
    pub truncated:bool,
    pub candidate_only:bool,
    pub independent_assertions_verified:bool,
    pub execution_authorized:bool,
    pub source_contents_exported:bool,
}
fn safe_symbol(s:&str)->Option<String>{
    let value=s.trim();
    if value.len()==0 || value.len()>100 ||
        !value.bytes().all(|b| b.is_ascii_alphanumeric() ||
            matches!(b,b'_'|b'-'|b'.'|b' ')) {return None;}
    Some(value.to_owned())
}
fn test_symbol_candidates(language:&str, text:&str)
    ->(Vec<TestSymbolHint>,bool){
    let mut symbols=Vec::new();
    let mut pending_rust_test=false;
    let mut truncated=false;
    for (i,line) in text.lines().take(4000).enumerate(){
        let s=line.trim();
        let candidate=match language {
            "python"=>{
                let function=s.strip_prefix("def test_")
                    .or_else(||s.strip_prefix("async def test_"));
                function.and_then(|f|f.split_once('(').map(|(name,_)|
                    format!("test_{name}")))
            },
            "rust"=>{
                if s.starts_with("#[test]") ||
                    s.starts_with("#[tokio::test]") {pending_rust_test=true;continue;}
                let found=if pending_rust_test {
                    s.strip_prefix("fn ").or_else(||s.strip_prefix("pub fn "))
                        .and_then(|f|f.split_once('(').map(|(name,_)|name.to_owned()))
                }else{None};
                if !s.starts_with("#[") && !s.is_empty(){pending_rust_test=false;}
                found
            },
            "javascript"=>{
                ["test(","it(","describe("].iter().find_map(|prefix|
                    s.strip_prefix(prefix)
                        .and_then(|rest|rest.strip_prefix('"')
                            .or_else(||rest.strip_prefix('\'')))
                        .and_then(|rest|rest.split(['"', '\'']).next())
                        .map(str::to_owned))
            },
            "cpp"=>{
                s.strip_prefix("TEST(").or_else(||s.strip_prefix("TEST_F("))
                    .and_then(|f|f.split_once(')').map(|(n,_)|
                        n.replace(',', ".")))
            },
            _=>None,
        };
        if let Some(name)=candidate.and_then(|s|safe_symbol(&s)){
            if symbols.len()>=128{truncated=true;break;}
            symbols.push(TestSymbolHint{name,line:(i+1) as u32,
                candidate_only:true,executable_test_verified:false});
        }
    }
    if text.lines().count()>4000{truncated=true;}
    (symbols,truncated)
}
impl IdeWorkspaceRegistry {
    /// Source-bound metadata read only. No imports, test enumeration protocol,
    /// package manager or executable is launched from discovered source.
    pub fn inspect_test_symbol_candidates(
        &self,project_id:&str,path:&str
    )->Result<TestSymbolInspection,&'static str>{
        let inventory=self.discover_conventional_test_files(project_id)?;
        let file=inventory.files.iter().find(|item|item.relative_path==path)
            .ok_or("FIN04_TEST_FILE_NOT_DISCOVERED")?;
        let language=file.language;
        let original=self.preview_source_file(project_id,path)
            .map_err(|_|"FIN04_SAVED_SOURCE_UNAVAILABLE")?;
        if original.project_id!=project_id || original.relative_path!=path ||
            original.byte_length>512*1024 {
            return Err("FIN04_SAVED_SOURCE_IDENTITY_DENIED");
        }
        let (symbols,truncated)=test_symbol_candidates(language,&original.text);
        Ok(TestSymbolInspection{
            schema_version:1,project_id:project_id.to_owned(),
            relative_path:path.to_owned(),source_revision:original.revision,
            language,symbols,truncated,candidate_only:true,
            independent_assertions_verified:false,execution_authorized:false,
            source_contents_exported:false,
        })
    }
}
#[cfg(test)]
mod fin04_tests {
    use super::*;
    #[test]fn parse_static_names_only_as_hints(){
        let (py,_) = test_symbol_candidates("python",
            "def test_a():\n assert True\nasync def test_b():\n pass");
        assert_eq!(py.len(),2);assert!(!py[0].executable_test_verified);
        let (rust,_) = test_symbol_candidates("rust",
            "#[test]\nfn works() {}\nfn not_a_test() {}");
        assert_eq!(rust.len(),1);
        let (js,_) = test_symbol_candidates("javascript",
            "test(\"works\", () => {})");
        assert_eq!(js.len(),1);
    }
}
