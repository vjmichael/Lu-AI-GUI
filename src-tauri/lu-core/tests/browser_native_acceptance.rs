//! TOOLS-10C real headed Chromium/native X11/Playwright acceptance.
//! Set LU_BROWSER_NATIVE_ACCEPTANCE=1 and supply trusted host-selected
//! executable paths through the CI harness. No model/tool selects them.
#[cfg(target_os="linux")]
#[test]
fn real_x11_owned_browser_hide_reveal_rehide_and_cleanup() {
    use lu_core::{
        browser_chromium_launcher::launch_owned_headed_chromium,
        browser_worker_supervisor::{BrowserLaunchConfig,BrowserOperation,OwnedBrowserWorker},
        browser_visibility_settings::BrowserVisibilityPreference,
    };
    use serde_json::json;
    use std::{path::PathBuf,time::Duration};
    if std::env::var("LU_BROWSER_NATIVE_ACCEPTANCE").as_deref()!=Ok("1"){
        eprintln!("SKIP: X11 real acceptance requires explicit CI launch");
        return;
    }
    let path=|name:&str|PathBuf::from(std::env::var(name)
        .unwrap_or_else(|_|panic!("missing real acceptance executable: {name}")));
    let chromium=path("LU_BROWSER_CHROMIUM");
    let node=path("LU_BROWSER_NODE");
    let xdotool=path("LU_BROWSER_XDOTOOL");
    let script=PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("workers/lu_browser_worker.mjs");
    let profile=std::env::temp_dir().join(format!(
        "lu-native-browser-{}-{:?}",std::process::id(),std::thread::current().id()));
    std::fs::create_dir(&profile).expect("dedicated profile");
    let result=(||{
        let browser=launch_owned_headed_chromium(
            &chromium,&profile,Duration::from_secs(25)).expect("real headed Chromium launch");
        let config=BrowserLaunchConfig{
            trusted_node:node,trusted_script:script,trusted_profile:profile.clone(),
            approved_origins:vec!["https://example.com".into()],
            cdp_endpoint:None,
            visibility:BrowserVisibilityPreference::AutomaticHidden,
            command_timeout:Duration::from_secs(35)
        };
        let mut worker=OwnedBrowserWorker::launch_attached(config,browser,xdotool)
            .expect("Lu-owned Chromium window and Node worker");
        let initial=worker.execute(BrowserOperation::Status,json!({}))
            .expect("native first-hide visibility handshake");
        assert_eq!(initial["ok"],true,"{initial:?}");
        assert_eq!(initial["result"]["windowVisibility"],"HIDDEN");
        let watched=worker.execute(BrowserOperation::Show,json!({}))
            .expect("same-session native visible transition");
        assert_eq!(watched["result"]["windowVisibility"],"VISIBLE");
        let hidden=worker.execute(BrowserOperation::Hide,json!({}))
            .expect("same-session native re-hide transition");
        assert_eq!(hidden["result"]["windowVisibility"],"HIDDEN");
        let final_status=worker.execute(BrowserOperation::Status,json!({}))
            .expect("same browser remains live after visibility changes");
        assert_eq!(final_status["result"]["status"],"READY");
        assert_eq!(final_status["result"]["windowVisibility"],"HIDDEN");
        // RAII terminates and reaps the owned worker+Chromium process.
    })();
    let _=std::fs::remove_dir_all(&profile);
    result
}
