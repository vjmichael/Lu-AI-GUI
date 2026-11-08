"""P03 Windows startup: source regression only; real Job/AppContainer OS CI is mandatory."""
from pathlib import Path
import unittest

SOURCE = (Path(__file__).resolve().parents[2] / "src" / "windows_lsp_worker.rs")


class WindowsProtectedDapStartupTests(unittest.TestCase):
    def test_debug_adapter_cwd_is_inside_measured_bundle_not_workspace(self):
        source = SOURCE.read_text(encoding="utf-8")
        self.assertIn("pub fn start_windows_dap(", source)
        lsp, dap = source.split("pub fn start_windows_dap(", 1)
        # Native clangd continues with a project-local workspace cwd.
        self.assertIn(
            "spawn_contained_stdio(&profile, &job, &executable, &mut command, &workspace, &env)",
            lsp,
        )
        # LLDB is not executing user project commands. Make its startup CWD
        # its sealed, private, package-SID-owned adapter bin directory.
        self.assertIn('let executable = bundle.join("bin").join(executable_basename);', dap)
        self.assertIn('let adapter_cwd = executable.parent().ok_or_else(denied)?;', dap)
        self.assertIn('if !clean_meta(adapter_cwd)?.is_dir()', dap)
        self.assertIn('native_lsp_bundle_digest(trusted_release_bundle, &plan.recipe_id)?', dap)
        self.assertIn('stage_bundle(', dap)
        self.assertIn(
            "spawn_contained_stdio(&profile, &job, &executable, &mut command, adapter_cwd, &env)",
            dap,
        )
        self.assertNotIn(
            "spawn_contained_stdio(&profile, &job, &executable, &mut command, &workspace, &env)",
            dap,
        )

    def test_no_debugger_startup_permission_escape(self):
        source = SOURCE.read_text(encoding="utf-8")
        dap = source.split("pub fn start_windows_dap(", 1)[1]
        for expected in (
            'plan.recipe_id != "dap_lldb_fixed"',
            'plan.effect_class != "debug_inspection_isolated"',
            'let profile = AppProfile::create(job_id)?;',
            'let job = Job::create(job_id, &plan_hash, plan)?;',
            'let mut lease = BrokerLease::begin(',
            'lease.grant_null()?;',
            'supervisor.consume_grant_after_attestation(job_id, plan, &proof())?;',
            'native_lsp_trace("suspended_child_attested")',
            'verify_input(&staged_input)',
            'supervisor.native_mark_running(job_id)',
            'ResumeThread(child_thread.0)',
        ):
            self.assertIn(expected, dap)
        self.assertIn('capabilities.Capabilities = ptr::null_mut();', source)
        self.assertIn('capabilities.CapabilityCount = 0;', source)
        for prohibited in ("CreateProcessAsUser", "ShellExecute", "runInTerminal",
                           "DEBUG_PROCESS", "SeDebugPrivilege", "ALL APPLICATION PACKAGES",
                           "Everyone", "icacls /grant", "takeown /f"):
            self.assertNotIn(prohibited, dap)
        # A source-only pass never substitutes for real protected OS debugger.
        self.assertIn('native_lsp_trace("child_resumed")', dap)


if __name__ == "__main__":
    unittest.main()
