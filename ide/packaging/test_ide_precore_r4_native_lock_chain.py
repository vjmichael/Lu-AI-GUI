"""R4 lock-chain integration guard: old evidence must not become current approval."""
import tempfile
from pathlib import Path
from unittest import mock
import unittest

import verify_ide_precore_r4_native_lock_chain as bridge


class R4NativeLockChainTests(unittest.TestCase):
    def test_historical_ref_is_fixed_and_successor_is_separate(self):
        self.assertEqual(bridge.HISTORICAL_SHA, "f1f0ed4bc1e740d2a9f85615b988b779a46651db")
        self.assertEqual(bridge.PLATFORMS, ("windows-2022", "ubuntu-24.04"))
        self.assertEqual(bridge.PINS[bridge.SUPERVISOR][3], 171)

    def test_wrong_platform_fails_without_touching_source(self):
        with self.assertRaisesRegex(bridge.Denied, "unreviewed_platform"):
            bridge.check(Path.cwd(), "macos-latest")

    def test_original_historical_verifiers_both_run_in_detached_worktree(self):
        calls = []
        def fake_git(root, *args):
            calls.append(("git", args))
            if args == ("rev-parse", "HEAD"):
                return bridge.HISTORICAL_SHA
            return ""
        def fake_invoke(argv, timeout=180):
            calls.append(("invoke", [str(x) for x in argv]))
            return "OK"
        with mock.patch.object(bridge, "git", side_effect=fake_git), \
             mock.patch.object(bridge, "invoke", side_effect=fake_invoke):
            bridge.verify_historical(Path.cwd(), "windows-2022")
        g=[x[1] for x in calls if x[0]=="git"]
        run=[x[1] for x in calls if x[0]=="invoke"]
        self.assertTrue(any(x[:2]==("worktree", "add") and bridge.HISTORICAL_SHA in x for x in g))
        self.assertTrue(any(x[:2]==("worktree", "remove") for x in g))
        self.assertEqual(len(run),2)
        self.assertIn("verify_ide_merge02c_native_locks.py", run[0][1])
        self.assertIn("verify_p06_candidate_lock.py", run[1][1])
        self.assertIn("--ide-merge02-successor", run[1])
        self.assertIn("windows-2022", run[1])

    def test_historical_failure_fails_closed_and_removes_worktree(self):
        events=[]
        def fake_git(root,*args):
            events.append(args)
            return bridge.HISTORICAL_SHA if args==("rev-parse","HEAD") else ""
        with mock.patch.object(bridge,"git",side_effect=fake_git), \
             mock.patch.object(bridge,"invoke",side_effect=bridge.Denied("original_gate_failed")):
            with self.assertRaisesRegex(bridge.Denied,"original_gate_failed"):
                bridge.verify_historical(Path.cwd(),None)
        self.assertTrue(any(x[:2]==("worktree","remove") for x in events))

    def test_current_source_must_contain_pinned_supervisor_lock(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            (root/".gitattributes").write_text(
                "ide/host/Cargo.lock text eol=lf\n"
                "ide/supervisor/Cargo.lock text eol=lf\n"
                "ide/app/src-tauri/Cargo.lock text eol=lf\n")
            for name in ("ide/host/Cargo.lock","ide/app/src-tauri/Cargo.lock"):
                p=root/name; p.parent.mkdir(parents=True,exist_ok=True); p.write_bytes(b"fake\n")
            def fake_git(root,*args):
                if args==("rev-parse","HEAD"): return "a"*40
                if args[:1]==("merge-base",): return bridge.HISTORICAL_SHA
                return "b"*40
            with mock.patch.object(bridge,"git",side_effect=fake_git), \
                 mock.patch.object(bridge,"verify_ipc_successor",return_value=True):
                with self.assertRaisesRegex(bridge.Denied,"reviewed_lock_missing_or_linked"):
                    bridge.verify_current(root)

    def test_original_supervisor_lock_cannot_be_self_relabelled(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            (root/".gitattributes").write_text(
                "ide/host/Cargo.lock text eol=lf\n"
                "ide/supervisor/Cargo.lock text eol=lf\n"
                "ide/app/src-tauri/Cargo.lock text eol=lf\n")
            for name in ("ide/host/Cargo.lock","ide/supervisor/Cargo.lock",
                         "ide/app/src-tauri/Cargo.lock"):
                p=root/name; p.parent.mkdir(parents=True,exist_ok=True); p.write_bytes(b"fake\n")
            def fake_git(root,*args):
                if args==("rev-parse","HEAD"): return "a"*40
                if args[:1]==("merge-base",): return bridge.HISTORICAL_SHA
                if args==("rev-parse","HEAD:ide/supervisor/Cargo.lock") or \
                   args==("hash-object","--","ide/supervisor/Cargo.lock"):
                    return "0"*40
                return "b"*40
            with mock.patch.object(bridge,"git",side_effect=fake_git), \
                 mock.patch.object(bridge,"verify_ipc_successor",return_value=True), \
                 mock.patch.object(bridge,"verify_supervisor_successor",side_effect=bridge.Denied("supervisor_lock_successor_bytes_unreviewed")):
                with self.assertRaisesRegex(bridge.Denied,"supervisor_lock_successor_bytes_unreviewed"):
                    bridge.verify_current(root)

    def test_exact_reproducible_supervisor_local_successor_and_hostile_mutations(self):
        import subprocess
        root = Path(__file__).resolve().parents[2]
        original = subprocess.check_output([
            "git", "-C", str(root), "show", bridge.HISTORICAL_SHA + ":" + bridge.SUPERVISOR
        ])
        reconstructed = bridge.reproduce_supervisor_lock(original)
        current = (root / bridge.SUPERVISOR).read_bytes()
        self.assertEqual(reconstructed, current)
        self.assertEqual(len(reconstructed), len(original) + 23)
        self.assertEqual(bridge.REVIEWED_SUPERVISOR_BLOB,
                         "d3f1af3f66f29d8c7d08cae826131de27b9dfe7c")
        self.assertTrue(bridge.verify_supervisor_successor(root, current))
        for corrupted in (
            original,
            current + b" ",
            current.replace(b' "rustix",\n', b' "rustiy",\n', 1),
            current.replace(b' "zeroize",\n', b' "zer0ize",\n', 1),
            current.replace(b' "rustix",\n', b'', 1),
        ):
            with self.subTest(length=len(corrupted)):
                with self.assertRaisesRegex(bridge.Denied, "supervisor_lock_successor_bytes_unreviewed"):
                    bridge.verify_supervisor_successor(root, corrupted)

    def test_source_tests_do_not_grant_signed_release(self):
        self.assertIn("IDE_PRECORE_R4_SIGNED_INSTALLED_AND_RELEASE=NOT_APPROVED",
                      Path(bridge.__file__).read_text(encoding="utf-8"))
        
    def test_single_existing_six_job_workflow_uses_both_historical_and_r2_gates(self):
        source = (Path(__file__).resolve().parents[2] /
                  ".github/workflows/ide-group6f-integrated-native.yml").read_text(encoding="utf-8")
        self.assertEqual(source.count("verify_ide_precore_r4_native_lock_chain.py"), 6)
        self.assertEqual(source.count("fetch-depth: 0"), 4)
        self.assertEqual(source.count("test_ide_merge02c_native_locks.py"), 4)
        self.assertEqual(source.count("test_ide_precore_lock_successor.py"), 4)
        self.assertEqual(source.count("test_ide_precore_r4_native_lock_chain.py"), 4)
        self.assertNotIn("python ide/packaging/verify_ide_merge02c_native_locks.py --repo-root .", source)
        self.assertNotIn("python3 ide/packaging/verify_ide_merge02c_native_locks.py --repo-root .", source)
        self.assertNotIn("python ide/packaging/verify_p06_candidate_lock.py", source)
        self.assertIn("workflow_dispatch:", source)



if __name__=="__main__":
    unittest.main()
