import json
import unittest
from pathlib import Path
from verify_wo9_release import GATES, check

HEAD="a"*40

def complete():
    return {
        "schema":"lu.ide.wo9.native-release.v1",
        "sourceSha":HEAD,
        "observations":[{
            "gate":key,"workflow":workflow,"runnerOs":os_name,
            "sourceSha":HEAD,"conclusion":"success","skipped":False,
            "runId":1000+i,"jobId":2000+i,
            "installerSha256":"b"*64,
            "installedDesktopSha256":"c"*64,
            "nativeWitnessSha256":"d"*64,
            "authenticodeVerified":key.startswith("signed-"),
            "protectedSupervisorVerified":key.startswith("signed-"),
            "releaseApproved":False,
        } for i,(key,(workflow,os_name)) in enumerate(GATES.items())],
    }

class TestWo9FailClosedRelease(unittest.TestCase):
    def test_unverified_default_denied(self):
        data=json.loads(Path(__file__).with_name("wo9-release-evidence.json").read_text())
        with self.assertRaisesRegex(ValueError,"HEAD_CHANGED"):
            check(data,HEAD)

    def test_complete_claims_still_never_approve_or_verify(self):
        result=check(complete(),HEAD)
        self.assertFalse(result["externalNativeWitnessesIndependentlyVerified"])
        self.assertFalse(result["signedInstallerAccepted"])
        self.assertFalse(result["releaseApproved"])

    def test_reject_old_commit(self):
        data=complete()
        data["sourceSha"]="e"*40
        with self.assertRaisesRegex(ValueError,"HEAD_CHANGED"):
            check(data,HEAD)

    def test_reject_skipped_or_cross_os(self):
        data=complete()
        data["observations"][0]["skipped"]=True
        with self.assertRaisesRegex(ValueError,"SKIPPED"):
            check(data,HEAD)
        data=complete()
        data["observations"][0]["runnerOs"]="other-os"
        with self.assertRaisesRegex(ValueError,"CROSS_OS"):
            check(data,HEAD)

    def test_no_missing_signed_or_installed_gates(self):
        data=complete()
        data["observations"].pop()
        with self.assertRaisesRegex(ValueError,"MISSING_NATIVE_GATES"):
            check(data,HEAD)

    def test_signed_package_must_be_real(self):
        data=complete()
        for row in data["observations"]:
            if row["gate"].startswith("signed-"):
                row["authenticodeVerified"]=False
                break
        with self.assertRaisesRegex(ValueError,"SIGNED_NATIVE_PACKAGE"):
            check(data,HEAD)

if __name__=="__main__":
    unittest.main()
