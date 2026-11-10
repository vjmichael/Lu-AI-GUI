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
    def test_schema_preserves_two_choices_without_preselecting_tool(self):
        prompt='<|im_start|>assistant\n'
        out=m.model_payload(prompt,128,'json_schema')
        schema=out['json_schema']
        self.assertEqual(len(schema['oneOf']),2)
        self.assertEqual({arm['properties']['kind']['enum'][0] for arm in schema['oneOf']},{'tool','final'})
        self.assertEqual(schema['oneOf'][0]['properties']['name'],{'type':'string'})
        self.assertEqual(schema['oneOf'][0]['properties']['arguments'],{'type':'object'})
        self.assertNotIn('lu_reviewed_git_head',str(schema))
        self.assertNotIn('permission',str(schema))
        self.assertTrue(out['prompt'].endswith('<think>\n\n</think>\n\n'))

    def test_reviewed_first_step_requires_an_actual_model_chosen_tool_name(self):
        prompt='<|im_start|>assistant\n'
        out=m.model_payload(prompt,128,'json_schema','required')
        schema=out['json_schema']
        self.assertEqual(schema['properties']['kind']['enum'],['tool'])
        self.assertEqual(schema['properties']['name'],{'type':'string'})
        self.assertEqual(schema['properties']['arguments'],{'type':'object'})
        self.assertNotIn('lu_reviewed_git_head',str(schema))
        self.assertNotIn('lu_reviewed_media_facts',str(schema))
        self.assertNotIn('final',str(schema))
    def test_unreviewed_required_mode_is_not_admitted(self):
        with self.assertRaises(ValueError):
            m.model_payload('hello',128,'raw','required')
    def test_unknown_protocol_refused(self):
        with self.assertRaises(ValueError):
            m.model_payload('hello',128,'force_git_head')

if __name__=='__main__': unittest.main()
