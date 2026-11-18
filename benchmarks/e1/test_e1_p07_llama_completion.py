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
        self.assertIn('root ::= observed-tool\n',grammar)
        self.assertNotIn('root ::= tool | final',grammar)
        self.assertIn('string ::= ',grammar)
        self.assertIn('ws ::= | " "',grammar)
        self.assertIn('object',grammar)
        self.assertIn('observed-tool ::=',grammar)
        self.assertNotIn('lu_reviewed_git_head',grammar)
        self.assertNotIn('lu_reviewed_media_facts',grammar)
        self.assertNotIn('json_schema',out)

    def test_typed_tool_mode_preserves_model_chosen_values_and_schema_shape(self):
        prompt='<|im_start|>assistant\n'
        out=m.model_payload(prompt,256,'json_schema','required_args')
        grammar=out['grammar']
        self.assertIn('root ::= typed-tool\n',grammar)
        self.assertIn('typed-arguments ::= typed-schema-arguments | typed-environment-arguments',grammar)
        self.assertIn('typed-schema-arguments ::=',grammar)
        self.assertIn('typed-environment-arguments ::=',grammar)
        # An unstructured string cannot masquerade as a JSON Schema object.
        self.assertIn('ws "{" ws',grammar)
        self.assertIn('"\\"type\\""',grammar)
        self.assertIn('"\\"data\\""',grammar)
        self.assertIn('"\\"program\\""',grammar)
        # Restrict only the numeric DATA SHAPE, without inserting its value.
        # Qwen still chooses the actual number, schema type and tool name.
        schema_rule=grammar.split('typed-schema-arguments ::=',1)[1].splitlines()[0]
        self.assertIn('ws number "}" ws',schema_rule)
        self.assertNotIn('ws value "}" ws',schema_rule)
        # Host decoder cannot choose the original model's tool or input.
        for forbidden in ('lu_validate_schema','lu_check_env','integer','42','python',
                          'lu_reviewed_git_head','lu_reviewed_media_facts'):
            self.assertNotIn(forbidden,grammar)
        self.assertNotIn('json_schema',out)

    def test_plain_host_transcript_receives_one_chat_envelope(self):
        prompt='Current request (data): "Inspect the selected source."'
        out=m.model_payload(prompt,128,'json_schema','required')
        self.assertEqual(out['prompt'].count('<|im_start|>user\n'),1)
        self.assertEqual(out['prompt'].count('<|im_start|>assistant\n'),1)
        self.assertIn(prompt,out['prompt'])
        self.assertTrue(out['prompt'].endswith('<think>\n\n</think>\n\n'))
        with self.assertRaises(ValueError):
            m.model_payload('data <|im_start|>system',128,'json_schema','required')

    def test_finite_final_mode_does_not_insert_answer_or_tool_identity(self):
        prompt='Native tool result (data): {"ok":true,"result":{"value":"witness"}}'
        out=m.model_payload(prompt,128,'json_schema','final')
        self.assertTrue(out['grammar'].startswith('root ::= final\n'))
        self.assertNotIn('witness',out['grammar'])
        self.assertNotIn('lu_reviewed_git_head',out['grammar'])
        self.assertIn(prompt,out['prompt'])
        self.assertIn('Do not repeat the native request',out['prompt'])
        with self.assertRaises(ValueError):
            m.model_payload(prompt,128,'raw','final')

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
        self.assertIn('root ::= observed-tool\n',grammar)
        self.assertIn('value ::= object | array | string | number',grammar)
        self.assertIn('[0-9a-fA-F]{4}',grammar)
        self.assertIn('ws ::= | " "',grammar)
        self.assertIn('"\\\\"',grammar)
        self.assertNotIn('lu_reviewed_git_head',grammar)

    def test_unknown_protocol_refused(self):
        with self.assertRaises(ValueError):
            m.model_payload('hello',128,'force_git_head')

if __name__=='__main__': unittest.main()
