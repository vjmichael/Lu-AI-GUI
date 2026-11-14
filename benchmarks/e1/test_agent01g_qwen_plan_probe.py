"""Parser integrity tests only; the genuine Qwen3 model run is a separate CI gate."""
import json
import unittest

from agent01g_qwen_plan_probe import validate

GOOD = {
    "luPlanV1": {"steps": [
        {"kind": "INDEPENDENT_CHECK", "dependsOn": None, "expected": "INDEPENDENT_WITNESS"},
        {"kind": "PRESENT", "dependsOn": 0, "expected": "EXPLANATION"}
    ]}
}


class PlanModelBoundaryTests(unittest.TestCase):
    def test_bounded_review_plan(self):
        self.assertEqual(validate(json.dumps(GOOD)), (True, "strict_review_only_plan", 2))

    def test_duplicate_json_key_rejected(self):
        self.assertEqual(validate(
            '{"luPlanV1":{"steps":[{"kind":"INSPECT","kind":"PRESENT",'
            '"expected":"EXPLANATION"}]}}')[1], "not_strict_json")

    def test_no_model_supplied_tool_arguments_or_authority(self):
        corrupt = json.loads(json.dumps(GOOD))
        corrupt["luPlanV1"]["steps"][0]["tool"] = "shell"
        self.assertEqual(validate(json.dumps(corrupt))[1], "unreviewed_extra_step_fields")

    def test_cyclic_dependency_rejected(self):
        corrupt = json.loads(json.dumps(GOOD))
        corrupt["luPlanV1"]["steps"][0]["dependsOn"] = 0
        self.assertEqual(validate(json.dumps(corrupt))[1], "untrusted_step_dependency")

    def test_bool_is_not_a_step_index(self):
        corrupt = json.loads(json.dumps(GOOD))
        corrupt["luPlanV1"]["steps"][1]["dependsOn"] = True
        self.assertEqual(validate(json.dumps(corrupt))[1], "untrusted_step_dependency")

    def test_unhashable_step_kind_is_cleanly_rejected_not_a_crash(self):
        corrupt = json.loads(json.dumps(GOOD))
        corrupt["luPlanV1"]["steps"][0]["kind"] = ["INDEPENDENT_CHECK"]
        self.assertEqual(validate(json.dumps(corrupt))[1], "unsupported_step_type")

    def test_model_prose_does_not_become_a_plan(self):
        self.assertEqual(validate("I will run Git now.")[1], "not_strict_json")

    def test_empty_steps_and_extra_root_denied(self):
        self.assertEqual(validate('{"luPlanV1":{"steps":[]}}')[1], "invalid_step_count")
        self.assertEqual(validate('{"luPlanV1":{"steps":[{"kind":"PRESENT",'
                          '"expected":"EXPLANATION"}]},"permissions":true}')[1],
                          "unknown_envelope_fields")


if __name__ == "__main__":
    unittest.main()
