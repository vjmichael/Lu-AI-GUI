#!/usr/bin/env python3
"""Source-only decoder contract, not proof a GGUF uses tools."""
import unittest
import e1_p07_llama_completion as m

class RealQwenJsonInterface(unittest.TestCase):
    def test_raw_remains_raw(self):
        prompt='<|im_start|>assistant\n'
        out=m.model_payload(prompt,128,'raw')
        self.assertEqual(out['prompt'],prompt)
        self.assertNotIn('json_schema',out)
        self.assertNotIn('grammar',out)
    def test_complete_grammar_preserves_two_choices_without_preselecting_tool(self):
        prompt='<|im_start|>assistant\n'
        out=m.model_payload(prompt,128,'json_schema')
        grammar=out['grammar']
        self.assertIn('root ::= tool | final',grammar)
        self.assertIn('name',grammar)
        self.assertIn('arguments',grammar)
        self.assertIn('object',grammar)
        self.assertIn('number',grammar)
        self.assertNotIn('lu_reviewed_git_head',grammar)
        self.assertNotIn('lu_reviewed_media_facts',grammar)
        self.assertNotIn('permission',grammar)
        self.assertNotIn('json_schema',out)
        self.assertTrue(out['prompt'].endswith('<think>\n\n</think>\n\n'))

    def test_reviewed_first_step_requires_tool_envelope_not_tool_identity(self):
        prompt='<|im_start|>assistant\n'
        out=m.model_payload(prompt,128,'json_schema','required')
        grammar=out['grammar']
        self.assertIn('root ::= observed_tool\n',grammar)
        self.assertNotIn('root ::= tool | final',grammar)
        self.assertIn('string ::= ',grammar)
        self.assertIn('ws ::= | " "',grammar)
        self.assertIn('object',grammar)
        self.assertIn('observed_tool ::=',grammar)
        self.assertNotIn('lu_reviewed_git_head',grammar)
        self.assertNotIn('lu_reviewed_media_facts',grammar)
        self.assertNotIn('json_schema',out)

    def test_unreviewed_required_mode_is_not_admitted(self):
        with self.assertRaises(ValueError):
            m.model_payload('hello',128,'raw','required')
    def test_literal_gbnf_preflight_has_no_tool_or_authority_name(self):
        payload=m.grammar_preflight_payload()
        self.assertEqual(payload['grammar'], 'root ::= "LU_GRAMMAR_PROBE_OK"')
        self.assertNotIn('lu_reviewed_git_head',str(payload))
        self.assertNotIn('permission',str(payload))
        self.assertNotIn('schema',payload)
    def test_full_grammar_uses_pinned_llama_json_primitive_rules(self):
        grammar=m.GENERIC_REPLY_GBNF['required']
        self.assertIn('root ::= observed_tool\n',grammar)
        self.assertIn('value ::= object | array | string | number',grammar)
        self.assertIn('[0-9a-fA-F]{4}',grammar)
        self.assertIn('ws ::= | " "',grammar)
        self.assertIn('"\\\\"',grammar)
        self.assertNotIn('lu_reviewed_git_head',grammar)

    def test_unknown_protocol_refused(self):
        with self.assertRaises(ValueError):
            m.model_payload('hello',128,'force_git_head')

if __name__=='__main__': unittest.main()
