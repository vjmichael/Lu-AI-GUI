# E1-P12 — Independent Git status, packed history and MP4 semantics

Date: November 8, 2026

## Scope and authority

This is a third, separate **host-only semantic verification** phase
stacked on E1-P11. It does not reauthorize anything. Lu Core remains
a native desktop host, not sandboxed. Lu's reviewed Git/FFprobe workers
still require the original human turn's read/execute authority and the
existing bounded supervisor and durable journal. The model cannot
choose a path, command, verification mode, or proof digest.

The independent E1-P10 oracle is invoked *after* the supervised worker
has produced a distinct APPLIED effect receipt, *before* the result is
returned to the model. Verified evidence is bound to that exact effect
receipt in `lu_independent_outcomes`. A mismatch records CONFLICT and
blocks tool feedback. Unrecognized-but-valid structure returns
UNSUPPORTED instead of a false verified success.

## 1. Git tracked-status observer

`src-tauri/lu-core/src/independent_git_status.rs` verifies a bounded
ordinary Git index file (v2/v3, SHA-1 checksum) against the **actual
committed HEAD tree** resolved through independent Git object reading,
then compares each permitted tracked regular file with its Git blob
identity.

Supported scope: ordinary SHA-1 repos with at most 4,096 tracked
regular, mode-100644 files, a fully merged index exactly matching
HEAD, no staged changes, and ordinary unstaged working-file edits or
deletions. This avoids double counting a tracked path. It does not run
Git a second time or use Git's status stdout as a source of truth.

Explicitly unsupported: index with staged additions/changes/renames,
sparse/split indexes, unfamiliar extensions, submodules, symlink or
executable-mode worktree entries, nontrivial filters or attributes,
autocrlf/EOL normalization, oversize files, linked worktrees, and
cases where worktree state cannot be independently compared with the
committed tree. It is **not** a comprehensive Git status replacement.

If a scoped direct-source count differs from the supervised Git
`trackedChangedEntries`, record CONFLICT; otherwise record
`GIT_STATUS_INDEX_TREE_V1 / VERIFIED`.

## 2. Packed Git history and object reader

`src-tauri/lu-core/src/independent_git_pack.rs` adds a read-only,
bounded Git PACK and IDX v2 decoder. It validates fanout tables and
object offsets, full pack/index cryptographic checksums and materialized
object identities, and reconstructs ordinary objects and offset/ref
deltas using bounded copy/insert instructions. Git pack delta entries
store the inflated **delta-instruction length**, distinct from the
reconstructed object's target size; the decoder verifies both declared
lengths separately. A 14-byte synthetic delta reconstructing 100,003
bytes and actual native OFS_DELTA blob packs guard against confusing
those sizes.

The Git history observer now reuses a single object store for
loose and packed commits, walking all reachable parent edges and
deduplicating merge ancestors. A complete matching count is reported
as `GIT_HISTORY_OBJECT_DAG_V2 / VERIFIED`. Any missing, corrupt,
malformed or unsupported object representation is UNSUPPORTED,
**not** a guessed count. A false supervised count is CONFLICT.

Conservative limits currently include eight packs, 64 MiB each,
100,000 indexed objects per pack, 2 MiB materialized objects,
16 recursive delta levels, 4,096 commits and 32 parents per commit.
Shallow histories, grafts, replacement refs, external alternates,
untrusted symlinks/reparse paths and boundary violations remain
unsupported. Large production repositories will commonly exceed
these initial limits and must not be advertised as verified.

## 3. Independent MP4/ISO-BMFF metadata

`src-tauri/lu-core/src/independent_mp4.rs` reads the exact
host-approved MP4 bytes and validates a bounded nonfragmented
ISO-BMFF box structure. The observer checks distinct `ftyp`,
`moov/mvhd`, `trak/mdia/mdhd/hdlr`, `stsd`, and `mdat` boxes,
parsing independent movie duration and ordinary video/audio sample
entries.

The first supported sample entries include AVC/H.264, HEVC,
AV1, VP9 and MPEG-4 Visual video, with bounded dimensions and
their respective codec configuration, and ordinary AAC `mp4a`
with an explicitly parsed MPEG-4 Audio decoder configuration,
channel count and sample rate. It verifies stream count, order,
types, codecs, video dimensions, audio channels and rates, and
container duration to a small numerical rounding tolerance.

There is no FFprobe rerun, no dependence on FFprobe's own JSON for
the second parsing stage, and no media path or raw metadata forwarded
to the model. Any SHA-256 content change or contradiction with the
already applied FFprobe receipt is CONFLICT. Fragmented movies,
edit lists, ambiguous top-level boxes, unsupported sample-entry
versions, protected/encrypted tracks, unusual codecs and oversized
files remain UNSUPPORTED. This is metadata verification, **not**
full video/audio frame decoding, sync acceptance, or DCP validation.

## Native independent acceptance gates (OPEN until results inspected)

Workflow `.github/workflows/lu-e1-p12-independent-semantic-verifiers.yml`
requires exact-head frozen source and **real** Windows 2025 and Ubuntu
24.04 native Rust compilation, unit regressions, and actual OS
resources:

- Git clean, dirty, deleted, forged count and staged-index refusal,
  using installed native Git only for fixture truth and setup.
- A real four-commit Git merge, native repacking, independent packed
  graph count and deliberate checksum corruption. A separate real Git
  fixture generates 20 nearly identical blobs, proves the presence of
  a native OFS_DELTA and requires exact-byte reconstruction of every
  historical blob through Lu's own independent pack reader.
- Installed FFmpeg-generated ordinary MPEG-4 Visual and AAC MP4
  files, probed by Lu's actual supervised FFprobe adapter, compared
  to the independent box parser and tampered-duration negative control.
- Complete five-operation reviewed Git/WAV model-tool loop, with the
  source scope now aiming for **five distinct VERIFIED observations**
  and ten original outer/inner durable receipts. A false or unsupported
  observation must never become VERIFIED.
- Existing SHA-1, PCM WAV, SQLite, conversation authority and WebView
  receipt constraints remain in force; regression jobs must be
  inspected at the *same final exact commit*.
- Physical installed Windows and genuinely autonomous GGUF tool
  choice remain separately gated near v1.0, as previously decided.
  E1-P08C descendant-cgroup custody is also a different security
  guarantee, not inferred from source-file verification.

Native passing results have **not** yet been obtained or inspected for
this E1-P12 branch. An implementation commit or job queue is not an
acceptance receipt.

## Later scope

Add robust staged/renamed status semantics, pack indexes with
additional formats or very large repositories, fragmented/edit-list
MP4 timeline math, additional audio/video codecs and independent
sample-table validation, and pinned-handle identity against concurrent
file changes. These are explicit follow-up gates, not silent claims.
