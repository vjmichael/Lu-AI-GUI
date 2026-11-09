#!/usr/bin/env python3
"""Hostile contract and real, independently compiled ELF/PDB semantic trials.

Real trials use a compiler's native line records, not an invented JSON reply.
The instruction addresses are static fixture entry/symbol addresses: they do
NOT prove a real stopped LLDB process or its OS image load bias.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

from verify_dev045d2_line_oracle import SCHEMA, Denied, verify_claim


def digest(file):
    return hashlib.sha256(Path(file).read_bytes()).hexdigest()


def tool(name):
    found = shutil.which(name)
    if not found:
        return None
    return str(Path(found).resolve())


def claim_for(root, platform, line, pc, slide="0x0", pdb=None):
    binary = root / ("fixture.exe" if platform == "windows-pe-pdb" else "fixture")
    source = root / "fixture.c"
    return {
        "schema": SCHEMA, "platform": platform,
        "binary": binary.name, "binarySha256": digest(binary),
        "source": source.name, "sourceSha256": digest(source),
        "sourceLine": line, "dapSourcePath": str(source),
        "dapInstructionPointer": hex(pc), "runtimeSlide": slide,
        # Test-only fixture binding, not a production supervisor receipt.
        "targetBindingSha256": "a" * 64,
        "pdb": pdb.name if pdb else None,
        "pdbSha256": digest(pdb) if pdb else None,
    }


class HostileMappingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="dev045d2-hostile-")
        self.root = Path(self.temp.name)
        self.source = self.root / "fixture.c"
        self.binary = self.root / "fixture"
        self.llvm = self.root / "llvm-symbolizer-test"
        self.source.write_text("int probe(void) { return 42; }\n// second line\n", encoding="utf-8")
        self.binary.write_bytes(b"\x7fELF" + b"0" * 16)
        self.llvm.write_bytes(b"pinned-native-oracle-fixture")
        self.claim = claim_for(self.root, "linux-elf64", 1, 0x401000)

    def tearDown(self):
        self.temp.cleanup()

    def run_claim(self, payload=None, symbol_result=None):
        payload = self.claim if payload is None else payload
        if symbol_result is None:
            symbol_result = [{
                "Address": payload.get("dapInstructionPointer", "0x401000"),
                "ModuleName": str(self.binary),
                "Symbol": [{
                    "FileName": str(self.source),
                    "Line": 1,
                    "Column": 0,
                }],
            }]
        output = json.dumps(symbol_result).encode()
        with mock.patch("verify_dev045d2_line_oracle.subprocess.run",
                        return_value=subprocess.CompletedProcess([], 0, output, b"")) as patched:
            result = verify_claim(self.root, payload, self.llvm,
                                  digest(self.llvm))
        self.assertEqual(patched.call_count, 1)
        self.assertNotIn("shell", patched.call_args.kwargs)
        self.assertEqual(patched.call_args.args[0][0], str(self.llvm))
        return result

    def test_only_pinned_semantics_without_any_os_authorization(self):
        result = self.run_claim()
        self.assertTrue(result["independentSymbolLineAddressMatch"])
        self.assertFalse(result["stoppedPcKernelAttested"])
        self.assertFalse(result["runtimeSlideKernelAttested"])
        self.assertFalse(result["breakpointsSteppingVariablesEnabled"])
        self.assertFalse(result["installedAppAccepted"])

    def test_wrong_source_line_and_wrong_pc_not_promoted(self):
        with self.assertRaisesRegex(Denied, "SYMBOL_LINE_MISMATCH"):
            self.run_claim({**self.claim, "sourceLine": 2})
        with self.assertRaisesRegex(Denied, "SYMBOL_ADDRESS_NOT_MATCHED"):
            self.run_claim({**self.claim, "dapInstructionPointer": "0x401001"},
                           symbol_result=[{
                               "Address": "0x401000", "Symbol": [{
                                   "FileName": str(self.source), "Line": 1
                               }]
                           }])

    def test_filename_change_and_symbolizer_disagreement_denied(self):
        with self.assertRaisesRegex(Denied, "ADAPTER_SOURCE_NOT_PINNED_FILE"):
            self.run_claim({**self.claim, "dapSourcePath": "/other/main.c"})
        with self.assertRaisesRegex(Denied, "SYMBOL_FILE_MISMATCH"):
            self.run_claim(symbol_result=[{"Address": "0x401000",
                "Symbol": [{"FileName": "/other/main.c", "Line": 1}]}])

    def test_source_tamper_binary_tamper_and_tool_tamper(self):
        self.source.write_text("int probe(void) { return 43; }\n", encoding="utf-8")
        with self.assertRaisesRegex(Denied, "INDEPENDENT_DIGEST_MISMATCH"):
            self.run_claim()
        self.source.write_text("int probe(void) { return 42; }\n// second line\n", encoding="utf-8")
        self.binary.write_bytes(b"\x7fELF" + b"9" * 16)
        with self.assertRaisesRegex(Denied, "INDEPENDENT_DIGEST_MISMATCH"):
            self.run_claim()
        self.binary.write_bytes(b"\x7fELF" + b"0" * 16)
        with self.assertRaisesRegex(Denied, "ORACLE_BINARY_MISMATCH"):
            with mock.patch("verify_dev045d2_line_oracle.subprocess.run"):
                verify_claim(self.root, self.claim, self.llvm, "1" * 64)

    def test_traversal_fake_binding_and_invalid_slide(self):
        for changed in [
            {"source": "../fixture.c"}, {"source": "./fixture.c"},
            {"binary": "/absolute/target.exe"},
            {"targetBindingSha256": "not-a-digest"},
            {"runtimeSlide": "0xFFFFFFFFFFFFFFFF"},
            {"dapInstructionPointer": "0xZZ"},
            {"sourceLine": 0}, {"pdb": "fake.pdb"},
        ]:
            with self.subTest(changed=changed):
                with self.assertRaises(Denied):
                    self.run_claim({**self.claim, **changed})

    @unittest.skipUnless(os.name == "posix", "Unix-only linked file witness")
    def test_source_symlink_and_hardlink_are_denied(self):
        alias = self.root / "alias"
        alias.symlink_to(self.source)
        with self.assertRaises(Denied):
            self.run_claim({**self.claim, "source": "alias"})
        alias.unlink()
        os.link(self.source, alias)
        with self.assertRaises(Denied):
            self.run_claim()

    def test_malformed_or_multiple_symbol_lines_never_count(self):
        with self.assertRaises(Denied):
            self.run_claim(symbol_result=[])
        with self.assertRaises(Denied):
            self.run_claim(symbol_result=[{"Address": "0x401000", "Symbol": []}])
        with self.assertRaises(Denied):
            self.run_claim(symbol_result=[{"Address": "0x401000", "Symbol": [
                {"FileName": str(self.source), "Line": 1},
                {"FileName": str(self.source), "Line": 1},
            ]}])


class RealNativeDebugLineTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform.startswith("linux"), "Linux SIGTRAP fixture")
    def test_real_kernel_sigtrap_instruction_pointer_maps_to_dwarf(self):
        """Actual SIGTRAP ucontext RIP, not a fabricated or static symbol IP.

        Deliberately a fixed disposable CI program, not a Lu protected worker
        and NOT a proof of LLDB stopped process or cgroup module provenance.
        """
        compiler = tool("clang")
        llvm = tool("llvm-symbolizer")
        self.assertIsNotNone(compiler)
        self.assertIsNotNone(llvm)
        with tempfile.TemporaryDirectory(prefix="dev045d2-trap-") as tmp:
            root = Path(tmp)
            source = root / "fixture.c"
            code = """#define _GNU_SOURCE
#include <signal.h>
#include <ucontext.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static void real_stop(int signal, siginfo_t *info, void *ctx) {
    ucontext_t *u = (ucontext_t *)ctx;
#if defined(__x86_64__)
    printf("kernel_sigtrap_ip=0x%llx\\n", (unsigned long long)u->uc_mcontext.gregs[REG_RIP]);
#else
#error x86_64 required
#endif
    fflush(stdout);
    _Exit(0);
}
__attribute__((noinline)) void stopped_probe(void) {
    __asm__ __volatile__("int3; nop");
}
int main(void) {
    struct sigaction sa;
    memset(&sa, 0, sizeof(sa));
    sa.sa_sigaction = real_stop;
    sa.sa_flags = SA_SIGINFO;
    sigemptyset(&sa.sa_mask);
    if (sigaction(SIGTRAP, &sa, NULL)) return 2;
    stopped_probe();
    return 3;
}
"""
            source.write_text(code, encoding="utf-8")
            executable = root / "fixture"
            subprocess.run([compiler, "-g", "-O0", "-fno-pie", "-no-pie",
                            str(source), "-o", str(executable)],
                           check=True, capture_output=True, timeout=45)
            result = subprocess.run([str(executable)], check=True,
                                    text=True, capture_output=True, timeout=8)
            address = re.fullmatch(r"kernel_sigtrap_ip=(0x[0-9a-fA-F]+)\s*",
                                   result.stdout)
            self.assertIsNotNone(address, "real kernel SIGTRAP IP missing")
            line = next(i for i, text in enumerate(code.splitlines(), 1)
                        if 'int3; nop' in text)
            claim = claim_for(root, "linux-elf64", line, int(address[1], 16))
            proof = verify_claim(root, claim, llvm, digest(llvm))
            self.assertTrue(proof["independentSymbolLineAddressMatch"])
            # This is genuine kernel SIGTRAP frame provenance, but not a
            # separate OS process custodian, DAP or cgroup acceptance.
            self.assertFalse(proof["stoppedPcKernelAttested"])
            self.assertFalse(proof["installedAppAccepted"])
            wrong = dict(claim, sourceLine=line + 1)
            with self.assertRaisesRegex(Denied, "SYMBOL_LINE_MISMATCH"):
                verify_claim(root, wrong, llvm, digest(llvm))

    def test_real_compiler_line_table_not_adapters_reported_line(self):
        # Explicitly fail rather than skip in the exact-head native CI gate.
        llvm = tool("llvm-symbolizer")
        self.assertIsNotNone(llvm, "real native LLVM symbolizer REQUIRED")
        if os.name == "nt":
            clang = tool("clang-cl")
            linker = tool("lld-link")
            self.assertIsNotNone(clang, "Windows clang-cl REQUIRED")
            self.assertIsNotNone(linker, "Windows lld-link REQUIRED")
        else:
            clang = tool("clang")
            nm = tool("nm")
            self.assertIsNotNone(clang, "native C compiler REQUIRED")
            self.assertIsNotNone(nm, "native symbol-address inspector REQUIRED")
        with tempfile.TemporaryDirectory(prefix="dev045d2-real-") as tmp:
            root = Path(tmp)
            source = root / "fixture.c"
            # The entrypoint's source line is 5. There is no runtime dynamic
            # stdin, network access, spawned target or user-controlled code.
            source.write_text(
                "int first(void) { return 0; }\n"
                "__attribute__((noinline)) int stopped_probe(void) {\n"
                "    volatile int marker = 41;\n"
                "    return marker + 1;\n"
                "}\n"
                "int entry(void) { return stopped_probe(); }\n"
                "int main(void) { return entry(); }\n",
                encoding="utf-8",
            )
            if os.name == "nt":
                # No C runtime, no execution. Linker generates genuine PE/PDB.
                source.write_text(
                    "__declspec(noinline) int stopped_probe(void) {\n"
                    "    volatile int marker = 41;\n"
                    "    return marker + 1;\n"
                    "}\n"
                    "int entry(void) { return stopped_probe(); }\n",
                    encoding="utf-8",
                )
                obj, exe, pdb = root / "fixture.obj", root / "fixture.exe", root / "fixture.pdb"
                subprocess.run([clang, "/nologo", "/Zi", "/Od", "/GS-", "/c",
                                str(source), "/Fo" + str(obj)], check=True,
                               timeout=45, capture_output=True)
                subprocess.run([linker, "/nologo", "/nodefaultlib",
                                "/entry:entry", "/subsystem:console", "/debug",
                                "/pdb:" + str(pdb), "/out:" + str(exe),
                                str(obj)], check=True, timeout=45, capture_output=True)
                data = exe.read_bytes()
                pe = struct.unpack_from("<I", data, 0x3c)[0]
                optional = pe + 4 + 20
                self.assertEqual(struct.unpack_from("<H", data, optional)[0], 0x20b)
                entry_rva = struct.unpack_from("<I", data, optional + 16)[0]
                image_base = struct.unpack_from("<Q", data, optional + 24)[0]
                address = image_base + entry_rva
                claim = claim_for(root, "windows-pe-pdb", 5, address, pdb=pdb)
            else:
                exe = root / "fixture"
                subprocess.run([clang, "-g", "-O0", "-fno-pie", "-no-pie",
                                str(source), "-o", str(exe)], check=True,
                               timeout=45, capture_output=True)
                symbols = subprocess.check_output([nm, "-n", str(exe)],
                                                   text=True, timeout=10)
                matches = re.findall(r"(?m)^([0-9a-fA-F]+) [Tt] stopped_probe$", symbols)
                self.assertEqual(len(matches), 1)
                address = int(matches[0], 16) + 1
                claim = claim_for(root, "linux-elf64", 2, address)
            result = verify_claim(root, claim, llvm, digest(llvm))
            self.assertTrue(result["independentSymbolLineAddressMatch"])
            self.assertFalse(result["stoppedPcKernelAttested"])
            self.assertFalse(result["breakpointsSteppingVariablesEnabled"])
            bad = dict(claim, sourceLine=1 if os.name == "nt" else 6)
            with self.assertRaisesRegex(Denied, "SYMBOL_LINE_MISMATCH"):
                verify_claim(root, bad, llvm, digest(llvm))
            if os.name == "nt":
                # Alter PDB Info AGE and deliberately repin the modified file.
                # A hash alone must never let an unrelated PDB attest PE lines.
                untouched = pdb.read_bytes()
                forged_pdb = bytearray(untouched)
                block_size = struct.unpack_from("<I", forged_pdb, 32)[0]
                directory_size = struct.unpack_from("<I", forged_pdb, 44)[0]
                directory_map = struct.unpack_from("<I", forged_pdb, 52)[0]
                count = (directory_size + block_size - 1) // block_size
                blocks = struct.unpack_from("<" + "I" * count, forged_pdb,
                                            directory_map * block_size)
                directory = b"".join(
                    forged_pdb[b * block_size:(b + 1) * block_size]
                    for b in blocks
                )[:directory_size]
                nstreams = struct.unpack_from("<I", directory, 0)[0]
                sizes = struct.unpack_from("<" + "I" * nstreams, directory, 4)
                offset = 4 + 4 * nstreams
                offset += 4 * ((sizes[0] + block_size - 1) // block_size)
                info_block = struct.unpack_from("<I", directory, offset)[0]
                age_at = info_block * block_size + 8
                struct.pack_into("<I", forged_pdb, age_at,
                                 struct.unpack_from("<I", forged_pdb, age_at)[0] + 1)
                pdb.write_bytes(forged_pdb)
                fake_claim = dict(claim, pdbSha256=digest(pdb))
                with self.assertRaisesRegex(Denied, "PE_PDB_GUID_AGE_MISMATCH"):
                    verify_claim(root, fake_claim, llvm, digest(llvm))
                pdb.write_bytes(untouched)
            source.write_text(source.read_text() + "/* changed */\n")
            with self.assertRaisesRegex(Denied, "INDEPENDENT_DIGEST_MISMATCH"):
                verify_claim(root, claim, llvm, digest(llvm))


if __name__ == "__main__":
    unittest.main()
