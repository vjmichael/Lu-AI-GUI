//! Native acceptance for the AI-ready source-span read API.
use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use std::{fs, path::PathBuf};

struct Picker(Option<PathBuf>);
impl NativeDirectoryPicker for Picker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        Ok(self.0.take())
    }
}
fn fixture() -> (PathBuf, IdeWorkspaceRegistry, String) {
    let root = std::env::temp_dir().join(format!(
        "lu-ide-spans-{}-{:016x}", std::process::id(), rand::random::<u64>()
    ));
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("src.txt"), "α\r\n\nβ\n").unwrap();
    let ide = IdeWorkspaceRegistry::open(root.join("catalog.json")).unwrap();
    let id = ide.register_from_native_picker(&mut Picker(Some(project))).unwrap()
        .unwrap().project_id;
    (root, ide, id)
}
#[test]
fn revision_bound_read_has_exact_human_lines_and_utf8_byte_ranges() {
    let (root, ide, id) = fixture();
    let current = ide.preview_source_file(&id, "src.txt").unwrap();
    let span = ide.read_source_span(&id, "src.txt", &current.revision, 1, 4).unwrap();
    assert_eq!(span.schema_version, 1);
    assert_eq!(span.total_lines, 4);
    assert_eq!(span.lines[0].text, "α");
    assert_eq!(span.lines[0].newline, "crlf");
    assert_eq!((span.lines[2].start_byte, span.lines[2].end_byte), (5, 7));
    assert_eq!(span.lines[3].text, "");
    fs::write(root.join("project/src.txt"), "revised\n").unwrap();
    assert_eq!(
        ide.read_source_span(&id, "src.txt", &current.revision, 1, 1)
            .unwrap_err().code(), "FILE_CHANGED_ON_DISK"
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn rejects_bad_scope_and_out_of_bounds_lines() {
    let (root, ide, id) = fixture();
    let revision = ide.preview_source_file(&id, "src.txt").unwrap().revision;
    assert_eq!(
        ide.read_source_span(&id, "../secret", &revision, 1, 1)
            .unwrap_err().code(), "INVALID_RELATIVE_PATH"
    );
    assert_eq!(
        ide.read_source_span(&id, "src.txt", "invalid", 1, 1)
            .unwrap_err().code(), "INVALID_FILE_REVISION"
    );
    assert_eq!(
        ide.read_source_span(&id, "src.txt", &revision, 5, 5)
            .unwrap_err().code(), "INVALID_SOURCE_RANGE"
    );
    ide.close(&id).unwrap();
    assert_eq!(
        ide.read_source_span(&id, "src.txt", &revision, 1, 1)
            .unwrap_err().code(), "PROJECT_NOT_OPEN"
    );
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn denies_symlinked_source() {
    use std::os::unix::fs::symlink;
    let (root, ide, id) = fixture();
    let revision = ide.preview_source_file(&id, "src.txt").unwrap().revision;
    symlink(root.join("project/src.txt"), root.join("project/link.txt")).unwrap();
    assert_eq!(
        ide.read_source_span(&id, "link.txt", &revision, 1, 1)
            .unwrap_err().code(), "NOT_A_FILE"
    );
    fs::remove_dir_all(root).unwrap();
}
