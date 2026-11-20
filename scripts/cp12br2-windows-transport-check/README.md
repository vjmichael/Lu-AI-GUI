# CP-12B-R2 native transport witness

The standalone crate includes the actual tracked desktop diagnostic module, not a copied protocol or fake transport. Run on a native Windows x64 evaluation host with the locked Rust/MSVC/SDK toolchain:

```powershell
cargo test --locked --offline --features windows_dx_eval -- --test-threads=1
cargo test --locked --offline -- --test-threads=1
```

The first command exercises the real Windows kernel pipe; the second verifies the default feature set leaves Windows sharing unavailable. The ignored subprocess receiver/sender entries are called by the parent tests. A successful cross-build is not native execution evidence. Neither test executable is a desktop installer.

For a **disposable, isolated Windows lab only**, the independent account witness creates a single randomly named standard account, checks its authenticated SID and effective kernel access, and removes it. Run from an administrator test console, never on a personal machine or production environment:

```powershell
$env:LU_DX_DISPOSABLE_LAB = '1'
cargo test --locked --offline --features windows_dx_eval -- --exact dx_ipc::windows_pipe::tests::disposable_native_account_lifecycle --ignored --nocapture
Remove-Item Env:LU_DX_DISPOSABLE_LAB
```

The separately provisioned-account test can instead use a disposable standard-account credential supplied securely in the test environment. Neither path should be run without the isolated test owner's authorization. If account provisioning, logon, effective access, or cleanup is blocked, retain the failure and discard the test guest overlay; do not substitute an ACL prediction for an executed cross-account witness.

The evaluation feature is not a production authorization. Full Windows desktop graphical consent, supported physical Windows ordinary-user and separate-account process tests, remote-client rejection and final release review remain independent gates.
