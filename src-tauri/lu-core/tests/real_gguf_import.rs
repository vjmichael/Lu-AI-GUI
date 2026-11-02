//! Explicit real-model import acceptance. The CI host supplies a verified GGUF
//! file path through LU_REAL_GGUF_PATH, never via model-generated instructions.
//! The test proves that the model file can be atomically imported, admitted,
//! reopened after simulated restart, and cannot overwrite a prior import.
//! It does not claim to test the GUI picker or model generation.
use lu_core::{
    model_admission::ModelAdmissionManager,
    model_import::{import_selected_gguf, ImportError},
};
use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

struct TemporaryStore(PathBuf);
impl Drop for TemporaryStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn exact_copy(a: &Path, b: &Path) -> bool {
    let (Ok(mut original), Ok(mut imported)) = (fs::File::open(a), fs::File::open(b)) else {
        return false;
    };
    let mut left = [0u8; 128 * 1024];
    let mut right = [0u8; 128 * 1024];
    loop {
        let Ok(left_size) = original.read(&mut left) else {
            return false;
        };
        if left_size == 0 {
            return matches!(imported.read(&mut right[..1]), Ok(0));
        }
        if imported.read_exact(&mut right[..left_size]).is_err()
            || left[..left_size] != right[..left_size]
        {
            return false;
        }
    }
}

#[test]
#[ignore = "requires an independently checksum-verified real GGUF provided by the test host"]
fn real_gguf_imports_atomically_and_is_admitted_again_after_restart() {
    let source = PathBuf::from(env::var("LU_REAL_GGUF_PATH").expect("host GGUF path required"));
    let size = fs::metadata(&source)
        .expect("verified source GGUF missing")
        .len();
    assert!(
        size > 1_000_000,
        "synthetic 24-byte GGUF is not an acceptance model"
    );
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = TemporaryStore(
        env::temp_dir().join(format!("lu-real-import-{}-{tick}", std::process::id())),
    );
    let model_store = root.0.join("lu-models");
    fs::create_dir_all(&model_store).expect("create private model directory");

    let imported = import_selected_gguf(&model_store, &source)
        .expect("real GGUF copy into Lu private store failed");
    assert_eq!(imported.bytes, size, "import copied a partial file");
    let destination = model_store.join(&imported.filename);
    assert!(destination.is_file());
    assert!(
        exact_copy(&source, &destination),
        "imported GGUF bytes differ from source"
    );
    let mut admission = ModelAdmissionManager::new();
    let first = admission
        .admit(&model_store, &imported.filename)
        .expect("real GGUF header not admitted");
    assert_eq!(first.header().file_bytes, size);
    first
        .recheck()
        .expect("imported GGUF changed after admission");
    drop(admission);

    // Simulate a new desktop session with an empty in-memory model catalogue.
    let mut restored = ModelAdmissionManager::new();
    restored
        .admit(&model_store, &imported.filename)
        .expect("imported model was not discoverable after restart");
    assert!(matches!(
        import_selected_gguf(&model_store, &source),
        Err(ImportError::AlreadyPresent)
    ));
    let present: Vec<_> = fs::read_dir(&model_store)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        present.len(),
        1,
        "unpublished partial import or duplicate remained"
    );
    println!("LU_REAL_GGUF_IMPORT_PASS bytes={size} reopened=true unchanged=true partials=0");
}
