#!/usr/bin/env bash
# SEC-05 manual destructive native acceptance, NOT a release/CI job.
# Run only as an ordinary user in a genuinely delegated cgroup-v2 service.
# A false kernel-empty witness MUST make the whole gate RED.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
if printenv SEC05_EVIDENCE_DIR >/dev/null 2>&1; then
  EVIDENCE="$(printenv SEC05_EVIDENCE_DIR)"
else
  EVIDENCE="$(mktemp -d /tmp/lu-sec05-native-XXXXXXXX)"
fi
mkdir -p "$EVIDENCE"
echo "SEC05_ORIGINAL_EVIDENCE=$EVIDENCE"
printf 'HEAD=%s\nOS=%s\n' "$(git rev-parse HEAD)" "$(uname -s)" > "$EVIDENCE/source.txt"
if [[ "$(uname -s)" != "Linux" ]] || [[ "$(id -u)" -eq 0 ]]; then
  echo "SEC05_NOT_AVAILABLE=1 Linux unprivileged delegated kernel required" | tee "$EVIDENCE/kernel-result.txt"
  exit 77
fi
for cmd in cargo python3 node; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "SEC05_NOT_AVAILABLE=1 missing $cmd" | tee "$EVIDENCE/kernel-result.txt"
    exit 77
  fi
done
node scripts/verify-source-tree.mjs 2>&1 | tee "$EVIDENCE/exact-source-lock.log"
node --test \
  lu-desktop/tests/p0-sec-guardian-hardening.test.mjs \
  lu-desktop/tests/p0-sec05-adversarial.test.mjs \
  2>&1 | tee "$EVIDENCE/source-regression.log"
cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml \
  --lib p0_sec05_task_adversarial_tests -- --nocapture \
  2>&1 | tee "$EVIDENCE/core-native-attack.log"
cargo test --locked --manifest-path lu-desktop/src-tauri/Cargo.toml \
  p0_sec05_adversarial_tests -- --nocapture \
  2>&1 | tee "$EVIDENCE/desktop-ansf-native-attack.log"
cargo test --locked --manifest-path src-tauri/lu-core/Cargo.toml \
  --lib native_background_process::tests::sec03_cancel_workspace_sweeps_all_owned_workers_and_spares_other_chat \
  -- --nocapture \
  2>&1 | tee "$EVIDENCE/host-scoped-process-stop.log"
python3 -m py_compile benchmarks/e1/p0_sec02_linux_guardian_negative.py
if [[ ! -f /sys/fs/cgroup/cgroup.controllers ]]; then
  echo "SEC05_NOT_AVAILABLE=1 unified cgroup-v2 kernel unavailable" | tee "$EVIDENCE/kernel-result.txt"
  exit 77
fi
cat /proc/self/cgroup > "$EVIDENCE/native-original-cgroup.txt"
cargo build --locked --manifest-path src-tauri/lu-win-worker-job/Cargo.toml \
  --features e1-p08-crash-fixture \
  --bin lu-linux-cgroup-guard --bin lu-p0-sec02-linux-negative \
  2>&1 | tee "$EVIDENCE/first-party-fixture-build.log"
WORKER="$ROOT/src-tauri/lu-win-worker-job/target/debug/lu-p0-sec02-linux-negative"
export LU_AGENT01L_TEST_CGROUP_GUARD="$ROOT/src-tauri/lu-win-worker-job/target/debug/lu-linux-cgroup-guard"
test -x "$WORKER"
test -x "$LU_AGENT01L_TEST_CGROUP_GUARD"
for mode in guardian-death guardian-stall worker-stall ancestor-migration; do
  # Missing delegation, an escaped survivor with a false receipt, or failed
  # original PIDFD cleanup is a failing gate, NEVER PASS.
  echo "SEC05_CASE=$mode" | tee "$EVIDENCE/$mode.log"
  python3 benchmarks/e1/p0_sec02_linux_guardian_negative.py \
    --fixture "$WORKER" --mode "$mode" \
    2>&1 | tee -a "$EVIDENCE/$mode.log"
done
echo "SEC05_KERNEL_ADVERSARIAL_PASS=1 no_positive_receipt_for_escape=1" |
  tee "$EVIDENCE/kernel-result.txt"
