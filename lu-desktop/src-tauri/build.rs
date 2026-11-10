// tauri-build itself emits the resource containing the Windows app manifest.
// Keep exactly one authoritative Win32 RT_MANIFEST (including Common Controls
// v6 required by native file dialogs). A second MSVC link-arg .res causes
// CVT1100/LNK1123. Use this exact manifest for both test and production.
fn main() {
    let attrs = tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new()
            .app_manifest(include_str!("../scripts/lu-desktop.manifest")),
    );
    tauri_build::try_build(attrs).expect("Lu Tauri native resource/manifest failed");
}
