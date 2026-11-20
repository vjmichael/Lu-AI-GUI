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
test('FIN05 manifest selection cannot shadow separately gated offline vcpkg binary-only execution',()=>{
 const manifestStart=linux.indexOf('let manifest = match plan.recipe_id.as_str() {');
 const manifestEnd=linux.indexOf('let linker = trusted_gnu_linker()?',manifestStart);
 const commandStart=linux.indexOf('cmd.arg("--");',manifestEnd);
 const commandEnd=linux.indexOf('cmd.stdin(Stdio::null())',commandStart);
 assert.ok(manifestStart>=0 && manifestEnd>manifestStart && commandStart>manifestEnd && commandEnd>commandStart);
 const manifest=linux.slice(manifestStart,manifestEnd);
 const dispatch=linux.slice(commandStart,commandEnd);
 assert.equal((manifest.match(/"vcpkg_install_offline"/g)||[]).length,1);
 assert.equal((dispatch.match(/"vcpkg_install_offline"/g)||[]).length,1);
 assert.doesNotMatch(manifest,/\bcmd\./);
 assert.match(manifest,/RuntimeProfile::VcpkgInstallOffline/);
 assert.match(dispatch,/cmd\.arg\("\/runtime\/bin\/vcpkg"\)/);
 for(const required of ['--no-downloads','--only-binarycaching','--binarysource=clear','--binarysource=files,/runtime/binary-cache,read']) {
   assert.ok(dispatch.includes(required),required);
 }
});
test('FIN05 native worker code never accepts a user-supplied shell command',()=>{
 assert.doesNotMatch(linux,/std::process::Command::new\(plan\.recipe_id\)/);
 assert.match(linux,/Command::new\(BWRAP\)/);
 assert.match(linux,/--unshare-all/);
 assert.match(linux,/--die-with-parent/);
});

test('FIN05 port builds are denied and only reviewed offline binary cache can serve packages',()=>{
 assert.match(linux,/--only-binarycaching/);
 assert.match(linux,/--binarysource=files,\/runtime\/binary-cache,read/);
 assert.match(linux,/--vcpkg-root=\/runtime/);
 assert.match(runtimes,/binary-cache\/manifest\.txt/);
 assert.match(linux,/--no-downloads/);
});

test('FIN05 cache restore writes transient state only to private output',()=>{
 for(const flag of ['--x-buildtrees-root=/output/buildtrees','--x-packages-root=/output/packages','--downloads-root=/output/downloads']) assert.ok(linux.includes(flag),flag);
 assert.match(linux,/\.arg\("VCPKG_FORCE_SYSTEM_BINARIES"\)/);
 assert.match(linux,/--binarysource=files,\/runtime\/binary-cache,read/);
});
