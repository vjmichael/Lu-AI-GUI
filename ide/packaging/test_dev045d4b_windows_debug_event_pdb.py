"""DEV-04.5D4B: actual Windows DEBUG_EVENT RIP -> independent PDB line.

Runs only a disposable, fixed first-party CI executable observed by a
separate first-party Win32 debugger. Does NOT run under Lu's SCM/Job
custody, mint a Lu debugging permission, or enable a production debugger.
"""
import hashlib
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).parent))
from verify_dev045d2_line_oracle import SCHEMA, Denied, verify_claim


def measured(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def located(executable):
    found = shutil.which(executable)
    if found:
        return str(Path(found).resolve())
    install = Path(r"C:\Program Files\LLVM\bin") / (executable + ".exe")
    return str(install) if install.is_file() else None


def run(command):
    result = subprocess.run(command, capture_output=True, text=True, timeout=50)
    if result.returncode != 0:
        # Never echo unrestricted compiler logs or arbitrary paths to CI
        raise AssertionError("D4B_FIXED_NATIVE_COMPILER_OR_OBSERVER_DENIED")
    return result.stdout


@unittest.skipUnless(os.name == "nt", "actual Windows kernel debug-event only")
class ActualWindowsEventAndPdbTests(unittest.TestCase):
    def test_win32_event_context_pe_relocation_and_codeview_line(self):
        clang = located("clang-cl")
        linker = located("lld-link")
        llvm = located("llvm-symbolizer")
        self.assertIsNotNone(clang, "DENIED: genuine native Windows clang-cl missing")
        self.assertIsNotNone(linker, "DENIED: genuine native Windows lld-link missing")
        self.assertIsNotNone(llvm, "DENIED: separate native LLVM reader missing")
        observer_source = (
            Path(__file__).resolve().parents[1]
            / "supervisor/fixtures/debug/dev045d4b_win_event_observer.c"
        )
        self.assertTrue(observer_source.is_file())
        with tempfile.TemporaryDirectory(prefix="dev045d4b-windows-kernel-") as tmp:
            root = Path(tmp)
            source = root / "fixture.c"
            source_text = (
                "__declspec(noinline) void entry(void) {\n"
                "    __debugbreak(); volatile int marker = 41; /* D4B_STOP */\n"
                "    for (;;) { marker++; }\n"
                "}\n"
            )
            source.write_text(source_text, encoding="utf-8")
            obj, exe, pdb = root/"fixture.obj", root/"fixture.exe", root/"fixture.pdb"
            observer = root/"observer.exe"
            run([clang, "/nologo", "/Zi", "/Od", "/GS-", "/c",
                 str(source), "/Fo" + str(obj)])
            run([linker, "/nologo", "/nodefaultlib", "/entry:entry",
                 "/subsystem:console", "/debug", "/pdb:" + str(pdb),
                 "/out:" + str(exe), str(obj)])
            # This standalone observer uses the normal test runner's
            # compiler CRT, not the Lu app/service distribution.
            run([clang, "/nologo", "/MD", "/O2", str(observer_source),
                 "/Fe:" + str(observer)])
            output = run([str(observer), str(exe)])
            match = re.fullmatch(
                r"D4B_OS_DEBUG_EVENT=PASS\n"
                r"D4B_OS_IMAGE_BASE=(0x[0-9a-fA-F]+)\n"
                r"D4B_OS_EVENT_ADDRESS=(0x[0-9a-fA-F]+)\n"
                r"D4B_OS_THREAD_RIP=(0x[0-9a-fA-F]+)\n"
                r"D4B_OS_EXE_IDENTITY=PASS\n",
                output,
            )
            self.assertIsNotNone(match, "DENIED: no native Windows debug event evidence")
            base = int(match[1], 16)
            exception_address = int(match[2], 16)
            rip = int(match[3], 16)
            self.assertTrue(base <= exception_address <= rip < base + (16 << 20))
            data = exe.read_bytes()
            self.assertEqual(data[:2], b"MZ")
            header = struct.unpack_from("<I", data, 0x3c)[0]
            self.assertEqual(data[header:header+4], b"PE\x00\x00")
            optional = header + 24
            self.assertEqual(struct.unpack_from("<H", data, optional)[0], 0x20b)
            preferred = struct.unpack_from("<Q", data, optional + 24)[0]
            size = struct.unpack_from("<I", data, optional + 56)[0]
            self.assertTrue(size > 0 and base <= rip < base + size)
            self.assertGreaterEqual(base, preferred, "DENIED: unexpected relocation")
            slide = base - preferred
            expected_line = next(
                i for i, line in enumerate(source_text.splitlines(), 1)
                if "D4B_STOP" in line
            )
            claim = {
                "schema": SCHEMA,
                "platform": "windows-pe-pdb",
                "binary": exe.name,
                "binarySha256": measured(exe),
                "source": source.name,
                "sourceSha256": measured(source),
                "sourceLine": expected_line,
                "dapSourcePath": str(source),
                "dapInstructionPointer": hex(rip),
                "runtimeSlide": hex(slide),
                "targetBindingSha256": "a" * 64,  # disposable fixture only
                "pdb": pdb.name,
                "pdbSha256": measured(pdb),
            }
            witness = verify_claim(root, claim, llvm, measured(llvm))
            self.assertTrue(witness["independentSymbolLineAddressMatch"])
            # This independent OS event belongs to a fixture only, NOT
            # Lu's actual SCM/Job-protected LLDB process or approved grant.
            self.assertFalse(witness["stoppedPcKernelAttested"])
            self.assertFalse(witness["breakpointsSteppingVariablesEnabled"])
            self.assertFalse(witness["installedAppAccepted"])

            wrong = dict(claim, sourceLine=expected_line+1)
            with self.assertRaises(Denied):
                verify_claim(root, wrong, llvm, measured(llvm))
            wrong = dict(claim, dapInstructionPointer=hex(rip+0x10000))
            with self.assertRaises(Denied):
                verify_claim(root, wrong, llvm, measured(llvm))
            original = pdb.read_bytes()
            changed = bytearray(original)
            # A modified but freshly rehashed PDB is no longer the PE's PDB.
            # The standalone D2 oracle enforces MSF7 Info GUID+age vs RSDS.
            block = struct.unpack_from("<I", changed, 32)[0]
            directory_size = struct.unpack_from("<I", changed, 44)[0]
            map_page = struct.unpack_from("<I", changed, 52)[0]
            pages = (directory_size + block-1)//block
            indices = struct.unpack_from("<" + "I"*pages, changed, map_page*block)
            directory = b"".join(
                changed[k*block:(k+1)*block] for k in indices
            )[:directory_size]
            n = struct.unpack_from("<I", directory, 0)[0]
            sizes = struct.unpack_from("<" + "I"*n, directory, 4)
            offset = 4 + n*4 + 4*((sizes[0]+block-1)//block)
            info_page = struct.unpack_from("<I", directory, offset)[0]
            age_at = info_page*block+8
            age = struct.unpack_from("<I", changed, age_at)[0]
            struct.pack_into("<I", changed, age_at, age+1)
            pdb.write_bytes(changed)
            with self.assertRaisesRegex(Denied, "PE_PDB_GUID_AGE_MISMATCH"):
                verify_claim(root, dict(claim, pdbSha256=measured(pdb)),
                             llvm, measured(llvm))
            pdb.write_bytes(original)
            source.write_text(source_text + "/* modified */\n", encoding="utf-8")
            with self.assertRaisesRegex(Denied, "INDEPENDENT_DIGEST_MISMATCH"):
                verify_claim(root, claim, llvm, measured(llvm))


if __name__ == "__main__":
    unittest.main()
