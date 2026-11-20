# Lu Continuity Registry (LCR)

**Architecture proposal:** v0.1\
**Status:** Proposed; not implemented\
**Scope:** Lu desktop, web, and mobile; agent tasks; development
workspaces; recoverable artifacts\
**Priority:** Design now; implement incrementally without disturbing the
active 13-file profile-isolation patch recovery.

## 1. Problem statement

A Lu-related Work task, "Work - DX'ing Lu," was created on the user's
desktop. Later, a mobile conversation and another Work task could not
reliably discover or access the original workspace. A new scratch
checkout did not establish whether the original 13-file uncommitted
patch survived. Device-of-origin metadata alone would have made the
conversation easier to locate, but would not have granted access to its
filesystem or recovered unsaved work.

The design must solve **discovery**, **conversation access**,
**execution-location identification**, and **state recovery** as four
separate problems.

## 2. Design principles

1.  **Account-owned conversation:** A conversation's stable identity is
    independent of the device, app surface, or execution environment
    that created it.
2.  **Separate conversation from task and workspace:** Reading messages
    is not equivalent to accessing files. Every task and workspace has
    its own stable identifier and lifecycle.
3.  **Immutable provenance, derived recency:** Preserve origin
    device/client and event history; calculate last
    opened/edited/executed from recorded events.
4.  **No misleading recovery claims:** A git commit, matching file, or
    conversation transcript does not prove that all uncommitted and
    untracked files have survived.
5.  **Recoverability by design:** Store verified, portable snapshots of
    task state and file changes, not merely references to ephemeral
    compute environments.
6.  **Explicit authorization:** A task must not access another task's
    private workspace simply because both belong to the same account.
    Grant, handoff, or restore through an auditable authorization path.
7.  **Privacy first:** "Location" means logical storage or execution
    host; do not collect precise geolocation to implement continuity.
8.  **Offline-capable:** Local access should work without a network;
    sync resumes idempotently. Clearly label unsynced work.
9.  **Multi-account isolation:** Show a unified view of authorized
    accounts only after explicit account linking or switching. Never
    silently merge permissions or data.

## 3. Domain model

-   **Account:** Authentication and ownership boundary. A person may
    explicitly link or switch between multiple accounts, but records
    remain account-scoped.
-   **Device:** Stable pseudonymous device ID, friendly label, client
    platform, last seen time, and trust/revocation state. No mandatory
    GPS or IP history.
-   **Conversation:** Stable account-scoped ID, message timeline,
    attachment references, origin metadata, and associated task IDs.
-   **Conversation event:** Immutable `created`, `opened`,
    `message_added`, `task_started`, `task_resumed`,
    `workspace_attached`, `checkpoint_created`, `sync_failed`, etc.,
    with actor, device/client, and timestamp.
-   **Task:** Agent execution identity, status, originating
    conversation, and workspace references. A conversation may have
    multiple tasks; a task can be linked to a parent task.
-   **Workspace:** Identified filesystem/runtime environment:
    desktop-local, cloud-managed, remote machine, or imported snapshot.
    Records repository identity, branch, commit, accessibility, and
    retention where known.
-   **Checkpoint:** Durable manifest plus file snapshot/patch and
    verified checksums; references the task/workspace and exact source
    revision. Distinguishes verified artifacts from inferred artifacts.
-   **Access grant:** Explicit permission for another device, task, or
    agent to view, resume, or restore a resource, scoped by account and
    expiry.

### Relationship sketch

``` text
Account 1 ── N Conversations ── N Tasks ── N Workspace attachments
   │               │               │                   │
   └── N Devices   └── N Events    └── N Checkpoints    └── N Artifacts

An account may be displayed alongside other explicitly authorized accounts,
but authorization remains isolated per account and per workspace.
```

## 4. Metadata: provenance, activity, and execution are distinct

  --------------------------------------------------------------------------
  Field                      Meaning                 Source of truth
  -------------------------- ----------------------- -----------------------
  `origin_device_id`         Device that created the Immutable creation
                             conversation            event

  `origin_client`            Lu Desktop, Lu Web, Lu  Immutable creation
                             Mobile, etc.            event

  `last_opened_device_id`    Device that last viewed Derived from `opened`
                             it                      events

  `last_edited_device_id`    Device that last        Derived from message
                             added/edited content    events

  `last_execution_host_id`   Host where agent tools  Derived from execution
                             last ran                events

  `workspace_id`             Actual task filesystem  Workspace attachment
                             reference               records

  `workspace_kind`           Local, cloud, remote,   Workspace record
                             snapshot                

  `workspace_availability`   Online, offline,        Last verified probe,
                             inaccessible, expired,  with timestamp
                             unknown                 

  `sync_state`               Synced, pending,        Per-resource sync
                             local-only, failed,     ledger
                             conflict                

  `checkpoint_id`            Latest verified         Checkpoint record
                             portable recovery point 

  `retention_expires_at`     Known expiry of         Provider metadata, if
                             ephemeral workspace     available
  --------------------------------------------------------------------------

**Do not conflate** last opened device, last message authoring device,
and execution host. Do not infer workspace host solely from the desktop
where a task was launched.

## 5. Protobuf contracts (illustrative v0.1)

Internal Lu events and contracts should use Protobuf, consistent with
Lu's architectural direction. The app may continue to transport bytes
over Tauri IPC; external services may expose JSON adapters.

``` proto
syntax = "proto3";
package lu.continuity.v1;

import "google/protobuf/timestamp.proto";

enum SyncState {
  SYNC_STATE_UNSPECIFIED = 0;
  SYNCED = 1;
  PENDING = 2;
  LOCAL_ONLY = 3;
  FAILED = 4;
  CONFLICT = 5;
}

enum WorkspaceKind {
  WORKSPACE_KIND_UNSPECIFIED = 0;
  DESKTOP_LOCAL = 1;
  CLOUD_MANAGED = 2;
  REMOTE_HOST = 3;
  RESTORED_SNAPSHOT = 4;
}

enum WorkspaceAvailability {
  WORKSPACE_AVAILABILITY_UNSPECIFIED = 0;
  ONLINE = 1;
  OFFLINE = 2;
  INACCESSIBLE = 3;
  EXPIRED = 4;
  UNKNOWN = 5;
}

message ConversationMetadata {
  string conversation_id = 1;
  string account_id = 2;
  string origin_device_id = 3;
  string origin_client = 4;
  google.protobuf.Timestamp created_at = 5;
  optional string last_opened_device_id = 6; // projection from events
  optional string last_edited_device_id = 7; // projection from events
  repeated string task_ids = 8;
  SyncState sync_state = 9;
  uint64 revision = 10;
}

message WorkspaceLocator {
  string workspace_id = 1;
  string account_id = 2;
  WorkspaceKind kind = 3;
  optional string execution_host_id = 4;
  optional string repository_id = 5;
  optional string branch = 6;
  optional string head_commit = 7;
  optional string latest_checkpoint_id = 8;
  WorkspaceAvailability availability = 9;
  optional google.protobuf.Timestamp last_verified_at = 10;
  optional google.protobuf.Timestamp retention_expires_at = 11;
}

message ContinuityEvent {
  string event_id = 1; // globally unique for idempotency
  string account_id = 2;
  string conversation_id = 3;
  optional string task_id = 4;
  optional string workspace_id = 5;
  optional string device_id = 6;
  string event_type = 7;
  google.protobuf.Timestamp occurred_at = 8;
  uint64 device_sequence = 9;
}
```

**Schema governance:** Use `buf` breaking-change checks, reserve retired
field numbers, and use `optional` where unset differs from zero/false.
The above is a design sketch, not production-complete authentication or
synchronization schema.

## 6. Conversation sync architecture

**Account conversation index:** A cloud-backed, account-scoped index
allows a logged-in web/mobile/desktop client to discover conversation
IDs and status. Search and filters may include origin device, client,
task, and workspace availability.

**Message content:** Sync messages and permitted attachments through
authenticated APIs; cache locally in encrypted storage for offline
access. Use stable message IDs, per-conversation revision/cursors,
idempotent writes, and explicit conflict handling. Preserve a monotonic
server ordering while retaining client timestamps for provenance.

**Local-only mode:** If a user chooses not to sync content, other
devices cannot promise full access. Offer an opt-in metadata-only stub
or no remote listing, according to privacy settings; clearly label that
content is not remotely accessible.

**Multiple accounts:** A user may switch accounts or explicitly link an
aggregate index. Each request carries the correct account scope and is
independently authorized. Do not duplicate or expose conversations
between accounts without an explicit share/import operation.

## 7. Workspace continuity and recovery

**Workspace locator:** Record workspace ID, execution host, access
mechanism, availability, repository/branch/HEAD when known, and the last
verification time. A conversation opened on mobile can show that its
task files are on an offline desktop or a different isolated cloud task.

**Authorized handoff:** Provide a `request_workspace_handoff` flow that
either (a) resumes the original reachable environment, (b) transfers an
authorized snapshot into a new workspace, or (c) queues a request for
the desktop when it reconnects. Never assume a new task can mount the
original task's filesystem.

**Durable checkpoints:** At meaningful milestones, before risky changes,
and before task shutdown, create a private, content-addressed snapshot
manifest. Capture tracked modifications, untracked files, file hashes,
git HEAD and branch, intended patch files, tests and results, and
necessary reproducible environment metadata. Exclude secrets and
credentials by default; never push to GitHub without authorization.

**Checkpoint verification:** A checkpoint is marked verified only after
the stored artifact can be read back, integrity-checked, and its
manifest inspected. "Patch recovered" requires checking the expected
file set and content, not merely finding a similar commit.

**Retention:** Surface known ephemeral-workspace expiration; create
warnings or automatic private checkpoints according to user preferences.
Distinguish `UNKNOWN` from `EXPIRED`.

**Recovery statuses:** `DISCOVERED`, `WORKSPACE_REACHABLE`,
`SNAPSHOT_AVAILABLE`, `SNAPSHOT_VERIFIED`, `RESTORED`, and
`NOT_RECOVERABLE` must be separately reportable; a newly cloned repo is
not a recovered worktree.

## 8. User experience

The conversation list should be unified within the selected account and
searchable across Lu Desktop, Web, and Mobile. Each entry can show
concise provenance and continuity indicators:

> **Work - DX'ing Lu**\
> Created on: Windows desktop\
> Last opened on: iPhone (if verified)\
> Workspace: Unverified / not yet located\
> Latest verified checkpoint: None known\
> Actions: Open conversation · Locate workspace · Request handoff

The above is an **illustrative status**, not an assertion that the
original workspace was desktop-local or that the 13-file patch was
recovered.

A details panel should show the event timeline, linked tasks, workspace
IDs, exact last-verified time, artifact manifests, sync errors, and
recovery options. "Conversation accessible" and "Workspace accessible"
should have separate indicators.

## 9. Security and privacy

-   Authenticate every device and authorize every account-scoped
    conversation/task/workspace operation.
-   Encrypt transport and stored content; maintain revocable device
    registrations and scoped task grants.
-   Keep credentials, environment secrets, private keys, and unrelated
    filesystem data out of checkpoints.
-   Make workspace access auditable; require user approval for
    cross-task handoffs that expose additional files or permissions.
-   Treat device labels and client versions as operational metadata, not
    identity proof. Avoid GPS tracking and unnecessary IP retention.
-   Give users visibility into device registrations, synchronized data,
    retention, and deletion/export options.
-   If end-to-end encryption is added later, explicitly design how
    server-side search and agent execution work without assuming the
    server can decrypt content.

## 10. Suggested internal interfaces

``` text
list_conversations(account_scope, filters, cursor)
get_conversation_metadata(conversation_id)
get_conversation_events(conversation_id, cursor)
get_linked_tasks(conversation_id)
locate_workspace(task_id)
verify_workspace_access(workspace_id)
list_verified_checkpoints(workspace_id)
create_checkpoint(workspace_id, include_untracked=true)
request_workspace_handoff(workspace_id, target_host_id, grant)
restore_checkpoint(checkpoint_id, target_workspace_id, grant)
```

Each operation returns typed status and structured failure details.
`locate_workspace` must never imply that a workspace is mountable or
that a patch exists without verification.

## 11. Implementation sequence

**Phase A --- Discoverability:** Introduce stable
conversation/task/workspace IDs, device registry, provenance events, a
local SQLite metadata index, and searchable conversation history. Expose
exact workspace unknown/unavailable states. No filesystem transfer
required.

**Phase B --- Cross-device conversation access:** Add authenticated
account-scoped cloud index, message/attachment sync, encrypted offline
cache, idempotent event synchronization, and clear local-only behavior.

**Phase C --- Portable task state:** Implement workspace locator,
artifact manifest, verified checkpoint creation/readback, and authorized
restore/handoff. Test with modified tracked files plus untracked files
and a disconnected desktop.

**Phase D --- Multi-account and resilience:** Add explicit account
switching/linking, scoped aggregation, device revocation, conflict
resolution, retention warnings, and recovery drill automation.

**Integration discipline:** Preserve Lu's existing architecture and
avoid changing the active Jan/Lu profile-isolation recovery worktree
until the original patch has been located and protected. This document
is a design proposal, not permission to modify GitHub or user AppData.

## 12. Acceptance tests

1.  Create a conversation on desktop; verify the same account lists and
    opens it on mobile and web after sync.
2.  Open on mobile; verify `origin_device_id` remains desktop while
    `last_opened_device_id` becomes mobile, without overwriting the last
    execution host.
3.  Start a desktop-local task; verify mobile can read the conversation
    but sees workspace `OFFLINE` when desktop is disconnected, not a
    false "ready" state.
4.  Create a task with modified tracked and untracked files; checkpoint;
    restore in a fresh isolated workspace; verify every expected hash
    and test artifact.
5.  Attempt cross-task workspace access without a grant; verify denial
    and audit event.
6.  Disconnect during sync; reconnect; verify idempotent replay without
    duplicate messages or events.
7.  Use two accounts; verify no cross-account content leakage and
    explicit authorization for any aggregate view.
8.  Test local-only conversation; verify remote clients do not falsely
    promise content access.
9.  Verify expired/unknown workspace states are distinct, and absence of
    a verified snapshot never becomes a "recovered" claim.
10. Revoke a device; verify new sync and handoff requests are denied.

## 13. Current Lu incident: immediate handling

The user reports that "Work - DX'ing Lu" was created on the desktop. The
original task's execution workspace location and recoverability have
**not** been verified from the available screenshots. A later scratch
checkout is a different workspace. Do not reset, clean, overwrite, or
rebuild anything while searching for the original 13-file patch. When
desktop access is available, identify the original task and workspace
first; capture a verified private patch/checkpoint before attempting any
migrations or integration.

**Design takeaway:** Device provenance helps find a task; a workspace
locator tells us where the work is; a verified checkpoint makes the work
portable. All three are necessary.
