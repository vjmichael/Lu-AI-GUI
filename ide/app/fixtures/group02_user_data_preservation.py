#!/usr/bin/env python3
"""Group 2: read-only, OS-installed IDE project-data preservation witness.

Never grants permission, runs user code, follows user-supplied catalog paths
or accepts a successful uninstall solely because the executable disappeared.
Only for ephemeral test projects on disposable GitHub Actions machines.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import sys


def deny(reason):
    raise RuntimeError("IDE_G02_USER_DATA_DENIED:" + reason)


def file_hash(path):
    if path.is_symlink() or not path.is_file():
        deny("missing_or_symlinked_evidence")
    if path.stat().st_size > 131072:
        deny("evidence_exceeds_bound")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def catalog_candidates():
    if os.name == "nt":
        bases = [Path(os.environ[key]) for key in ("APPDATA", "LOCALAPPDATA")
                 if os.environ.get(key)]
    else:
        bases = [Path.home() / ".local/share", Path.home() / ".config"]
    return [base / "org.lu.ide" / "projects.json" for base in bases]


def take_snapshot(fixture):
    if os.environ.get("GITHUB_ACTIONS") != "true" or not os.environ.get("RUNNER_TEMP"):
        deny("requires_disposable_ci")
    runner = Path(os.environ["RUNNER_TEMP"]).resolve(strict=True)
    root = Path(fixture).resolve(strict=True)
    try:
        root.relative_to(runner)
    except ValueError:
        deny("fixture_outside_runner")
    source = root / "main.rs"
    if source.is_symlink() or not source.is_file():
        deny("source_missing_or_symlinked")
    catalogs = [path for path in catalog_candidates() if path.exists() or path.is_symlink()]
    if len(catalogs) != 1:
        deny("missing_or_ambiguous_catalog")
    catalog = catalogs[0]
    catalog_sha = file_hash(catalog)
    try:
        data = json.loads(catalog.read_bytes())
    except (UnicodeError, ValueError):
        deny("catalog_invalid_json")
    if not isinstance(data, dict) or data.get("version") != 1:
        deny("catalog_version_mismatch")
    rows = data.get("projects")
    if not isinstance(rows, list) or len(rows) > 128:
        deny("catalog_records_invalid")
    matches = []
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("root"), str):
            deny("catalog_project_invalid")
        try:
            resolved = Path(row["root"]).resolve(strict=True)
        except (OSError, RuntimeError, ValueError):
            deny("catalog_root_unavailable")
        if os.path.normcase(str(resolved)) == os.path.normcase(str(root)):
            matches.append(row)
    if len(matches) != 1 or not re.fullmatch(r"ide-[a-f0-9]{32}", str(matches[0].get("id", ""))):
        deny("exact_registered_project_not_proven")
    return {"catalogSha256": catalog_sha,
            "sourceSha256": file_hash(source),
            "projectIdentitySha256": hashlib.sha256(str(root).encode("utf-8")).hexdigest()}


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ("before", "after"):
        deny("usage_before_or_after_with_fixture")
    fixture = sys.argv[2]
    runner = Path(os.environ.get("RUNNER_TEMP", "/nonexistent")).resolve()
    receipt = runner / "ide-group02-user-data-receipt.json"
    current = take_snapshot(fixture)
    if sys.argv[1] == "before":
        if receipt.exists() or receipt.is_symlink():
            deny("receipt_preexists")
        receipt.write_text(json.dumps(current, sort_keys=True), encoding="utf-8")
        print("IDE_G02_NATIVE_USER_DATA_BEFORE_UNINSTALL=PASS", flush=True)
    else:
        if not receipt.is_file() or receipt.is_symlink():
            deny("original_receipt_missing")
        try:
            original = json.loads(receipt.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            deny("original_receipt_invalid")
        if original != current:
            deny("user_catalog_or_source_changed_by_uninstall")
        print("IDE_G02_NATIVE_UNINSTALL_PRESERVED_CATALOG_AND_SOURCE=PASS", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(2)
