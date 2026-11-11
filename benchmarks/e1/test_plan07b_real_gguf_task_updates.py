#!/usr/bin/env python3
"""Syntax/source binding tests only: never stand in for genuine Qwen acceptance."""
import unittest

from plan07b_real_gguf_task_updates import grammar_for, original_native_context


def native(*, exact=False, status="MISSING"):
    return {
        "previewSha256": "a"*64,
        "registerId": "lu-register-original",
        "tasks": [{
            "taskId": "lu-task-original",
            "originalRequirementId": "requirement-original",
            "status":status,
            "allowedEvidenceIds":["requirement-original","evidence-original"],
            "exactCriterionCandidate":exact,
        }],
    }


class GenuinePlan07bSourceBinding(unittest.TestCase):
    def test_original_source_ids_and_status_are_grammar_bound(self):
        result=grammar_for(native())
        self.assertIn("lu-project-task-update-model-v1",result)
        self.assertIn("lu-task-original",result)
        self.assertIn("requirement-original",result)
        self.assertIn("MISSING",result)
        self.assertIn("collect_evidence",result)
        self.assertIn("review_source_change",result)
        self.assertNotIn("review_exact_criterion",result)
        self.assertIn("{10,180}",result)
        self.assertNotIn("evidence-original",result)

    def test_only_native_host_exact_candidate_may_offer_exact_criterion(self):
        result=grammar_for(native(exact=True,status="ACCEPTED"))
        self.assertIn("review_exact_criterion",result)
        self.assertIn("ACCEPTED",result)
        forged=native()
        forged["tasks"][0]["allowedEvidenceIds"]=["wrong-id"]
        with self.assertRaisesRegex(ValueError,"citation"):
            grammar_for(forged)

    def test_original_prompt_envelope_rejects_replayed_source(self):
        n=native()
        prompt="instructions\nBEGIN_UNTRUSTED_NATIVE_CONTEXT\n"
        import json
        prompt+=json.dumps(n)+"\nEND_UNTRUSTED_NATIVE_CONTEXT\ninstructions"
        f={"prompt":prompt,"previewSha256":n["previewSha256"],
           "registerId":n["registerId"],"originalTaskIds":["lu-task-original"]}
        self.assertEqual(original_native_context(f),n)
        f["previewSha256"]="b"*64
        with self.assertRaisesRegex(ValueError,"mismatch"):
            original_native_context(f)


if __name__=="__main__":
    unittest.main()
