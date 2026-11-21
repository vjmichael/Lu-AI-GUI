"""Source-only successor to the historical Linux/Windows DAP startup tests."""
from pathlib import Path
import unittest

from probe_lldb_dap_launch import CASES, SERVER_NAMESPACE, classify

ROOT = Path(__file__).resolve().parents[2] / "src"


def compact(source):
    return " ".join(source.split())


class NativeSourceContractTests(unittest.TestCase):
    def test_diagnostic_namespace_and_fixed_commands(self):
        self.assertEqual(len(CASES), 4)
        self.assertEqual(SERVER_NAMESPACE[SERVER_NAMESPACE.index("--chdir") + 1], "/tmp")
        for flag in ("--unshare-all", "--die-with-parent", "--ro-bind", "--proc", "--clearenv"):
            self.assertIn(flag, SERVER_NAMESPACE)
        for flag in ("--share-net", "--cap-add"):
            self.assertNotIn(flag, SERVER_NAMESPACE)
        self.assertEqual({item[2] for item in CASES}, {
            "/opt/lu-ide-wo6-dap-test/debuggee/wo6-selftest",
            "/ls/debuggee/wo6-selftest",
        })

    def test_classification_is_bounded_and_redacted(self):
        for detail, expected in (
            ("ptrace denied /secret", "ptrace_denied"),
            ("ASLR /secret", "aslr_personality_denied"),
            ("permission denied /secret", "kernel_permission_denied"),
            ("failed to launch /secret", "fixed_launch_rejected"),
            ("no such file /secret", "fixed_dependency_missing"),
            ("", "no_adapter_detail"),
        ):
            self.assertEqual(classify(detail), expected)
            self.assertNotIn("/secret", classify(detail))

    def test_linux_fixed_and_project_launch_stays_in_sealed_namespace(self):
        source = (ROOT / "linux_lsp_worker.rs").read_text(encoding="utf-8")
        dap = compact(source.split("fn start_linux_dap_inner(", 1)[1].split("pub const WO6_CI_SOURCE_TRAP_RS", 1)[0])
        for check in (
            '"dap_lldb_fixed", None', '"dap_lldb_project", Some(target)',
            'verify_input(input)', 'target.verify()?',
            'native_lsp_bundle_digest(release_root, &plan.recipe_id)?',
            'let args: &[&str] = &[];',
            '.arg("--ro-bind") .arg(&input.root) .arg("/workspace")',
            '.arg("--ro-bind") .arg(release_root) .arg("/ls")',
            '.arg("--tmpfs") .arg("/tmp")',
            '.arg("--ro-bind") .arg(&target.root) .arg("/debug-target")',
            '.arg("--chdir") .arg("/workspace")',
            '.arg("--unshare-all") .arg("--clearenv")',
            'let native_executable = format!("/ls/bin/{executable}");',
            'require_kernel_child_membership(&group, child.id())',
        ):
            self.assertIn(check, dap)
        self.assertIn('launch_fixed_fixture("/ls/debuggee/wo6-selftest")', source)
        for flag in ('"--share-net"', '"--cap-add"', '".exe"'):
            self.assertNotIn(flag, dap)
        self.assertLess(dap.index("consume_grant_after_attestation"), dap.index("command.spawn()"))
        self.assertLess(dap.index("require_kernel_child_membership"), dap.index("native_mark_running"))

    def test_windows_fixed_and_project_launch_stages_local_workspace(self):
        source = (ROOT / "windows_lsp_worker.rs").read_text(encoding="utf-8")
        dap = compact(source.split("fn start_windows_dap_inner(", 1)[1].split("/// Native CI receipt", 1)[0])
        for check in (
            '"dap_lldb_fixed", None', '"dap_lldb_project", Some(target)',
            'let arguments: &[&str] = &[];',
            'let local = profile.local_state()?', 'let workspace = local.join("workspace");',
            'stage_workspace(input, &workspace)?;', 'verify_input(&staged_input)',
            'native_lsp_bundle_digest(trusted_release_bundle, &plan.recipe_id)?',
            'let executable = bundle.join("bin").join(executable_basename);',
            'target.verify()?', 'lease.grant_null()?;',
            'spawn_contained_stdio( &profile, &job, &executable, &mut command, &workspace, &mut env,',
            'supervisor.native_mark_running(job_id)', 'ResumeThread(child_thread.0)',
        ):
            self.assertIn(check, dap)
        for prohibited in ("CreateProcessAsUser", "ShellExecute", "runInTerminal", "SeDebugPrivilege", "icacls /grant"):
            self.assertNotIn(prohibited, dap)
        child = source.split("fn spawn_contained_stdio(", 1)[1].split("capabilities.CapabilityCount = 0;", 1)[0]
        self.assertIn("capabilities.Capabilities = ptr::null_mut();", child)
        self.assertIn("capabilities.CapabilityCount = 0;", source)
        self.assertLess(dap.index("consume_grant_after_attestation"), dap.index("spawn_contained_stdio"))
        self.assertLess(dap.index("native_mark_running"), dap.index("ResumeThread"))


if __name__ == "__main__":
    unittest.main()
