import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8');
const supervisor=read('../../supervisor/src/lib.rs');
const linux=read('../../supervisor/src/linux_worker.rs');
const runtimes=read('../../supervisor/src/runtime_bundles.rs');
const app=read('../src-tauri/Cargo.toml');
test('FIN05 vcpkg is separate bounded and feature-gated package authority',()=>{
 assert.match(supervisor,/feature = "package-vcpkg-worker"/);
 assert.match(supervisor,/package_vcpkg_install_offline:/);
 assert.match(supervisor,/value\.runtime_limit_ms <= 10 \* 60_000/);
 assert.match(supervisor,/value\.process_limit <= 12/);
 assert.match(runtimes,/VcpkgInstallOffline/);
 assert.match(runtimes,/LICENSES\/vcpkg\.txt/);
 assert.match(linux,/LU_IDE05_LINUX_VCPKG_SHA256/);
 assert.match(linux,/vcpkg_install_offline/);
 assert.match(linux,/--no-downloads/);
 assert.match(linux,/--x-manifest-root=\/workspace/);
 assert.match(linux,/--x-install-root=\/output\/installed/);
 assert.match(linux,/--binarysource=clear/);
 assert.doesNotMatch(app,/package-vcpkg-worker/);
});
test('FIN05 native worker code never accepts a user-supplied shell command',()=>{
 assert.doesNotMatch(linux,/std::process::Command::new\(plan\.recipe_id\)/);
 assert.match(linux,/Command::new\(BWRAP\)/);
 assert.match(linux,/--unshare-all/);
 assert.match(linux,/--die-with-parent/);
});
