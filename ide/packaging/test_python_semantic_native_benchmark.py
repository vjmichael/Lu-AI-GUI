import io
import json
from pathlib import Path
import tempfile
import unittest

from python_semantic_native_benchmark import (
    FIXTURE_COUNT, fixtures, framing, read_packet, completions,
    hover_is_real, definition_is_inside_source, SCHEMA,
)
from stage_ty_semantic_candidate_ci import PINS, VERSION


class PythonSemanticBenchmarkSourceTests(unittest.TestCase):
    def test_twenty_real_python_fixtures_have_unambiguous_semantic_offsets(self):
        with tempfile.TemporaryDirectory() as tmp:
            cases=fixtures(Path(tmp)/"project")
            self.assertEqual(len(cases),20)
            self.assertEqual(len(set(x[0] for x in cases)),20)
            for symbol,path,text in cases:
                self.assertTrue(path.is_file())
                lines=text.splitlines()
                self.assertEqual(len(lines),6)
                self.assertEqual(lines[3][9:9+len(symbol)],symbol)
                self.assertEqual(lines[4][13:20],"calcula")
                self.assertIn("completely_unknown_",lines[5])

    def test_length_framing_roundtrip_and_remote_results_must_be_real(self):
        obj={"jsonrpc":"2.0","id":42,"result":{"contents":{"kind":"plaintext","value":"int"}}}
        self.assertEqual(read_packet(io.BytesIO(framing(obj))),obj)
        with self.assertRaises((ValueError,EOFError)):
            read_packet(io.BytesIO(b"Content-Length: 999999999\r\n\r\n"))
        self.assertTrue(hover_is_real({"contents":{"kind":"plaintext","value":"int"}}))
        self.assertFalse(hover_is_real({"contents":""}))
        self.assertTrue(completions([{"label":"calculate_01"}],"calculate_01"))
        self.assertFalse(completions([{"label":"calculate_01"}],"calculate_02"))
        uri="file:///temp/case_00.py"
        self.assertTrue(definition_is_inside_source(
            {"targetUri":uri,"targetRange":{}},uri))
        self.assertFalse(definition_is_inside_source(
            [{"uri":"file:///outside.py"}],uri))
        self.assertEqual(SCHEMA,"lu.ide.dev02.python-real-lsp-comparison.v1")

    def test_ty_beta_releases_are_pinned_for_both_native_platforms(self):
        self.assertEqual(VERSION,"0.0.99")
        self.assertEqual(set(PINS),{"windows-x86_64","linux-x86_64"})
        for platform,(archive,sha,binary) in PINS.items():
            self.assertEqual(len(sha),64)
            self.assertTrue(all(ch in "0123456789abcdef" for ch in sha))
            self.assertTrue(archive.startswith("ty-x86_64-"))
            self.assertEqual(binary,"ty.exe" if platform.startswith("windows") else "ty")


if __name__=="__main__":
    unittest.main()
