# PRD: `/add-dir` Slash Command

## Status Update (2026-04-08)

### Implemented in code

- Added persisted config support under `permissions.additionalDirectories`.
- Added runtime `Config.additional_working_directories` state so explicit extra directories are part of the live session configuration, not just CLI/bootstrap setup.
- Threaded `additional_working_directories` through session configuration, per-turn config rebuilding, and the TUI/core override pipeline.
- Fixed live-session sandbox refresh so changing `additional_working_directories` recomputes the effective filesystem allowlists immediately.
- Preserved existing `--add-dir` behavior by merging CLI-provided directories with persisted config directories.
- Updated project-doc discovery so `AGENTS.md` lookup walks explicit additional working directories.
- Updated skill loading so repo skill roots from explicit additional working directories are included.
- Registered `/add-dir` in the TUI slash-command surface with optional inline path arguments.
- Implemented `/add-dir <path>`:
  - trims and resolves the path, including `~`
  - validates existence with a single metadata/stat lookup
  - rejects empty input, non-directories, and paths already covered by an existing working directory
  - opens a confirmation popup with `Yes, for this session`, `Yes, and remember this directory`, and `No`
- Implemented `/add-dir` with no argument:
  - opens a dedicated directory-entry prompt
  - shows debounced filesystem directory suggestions
  - supports `Tab` completion and `Up`/`Down` suggestion navigation
  - emits the cancellation message `Did not add a working directory.` when dismissed
- Implemented session-only success, remembered success, and partial-success messaging, including the `/permissions` follow-up hint.
- Implemented persistence for remembered directories under `permissions.additionalDirectories` without duplicate writes.
- On Windows, remembered/session additions also reuse the existing non-elevated read-root grant flow.
- Added TUI and core test coverage for:
  - slash-command prompt/confirmation behavior
  - remembered vs session-only flows
  - config editing for `permissions.additionalDirectories`
  - sandbox recomputation when additional working directories change

### Verification completed

- `cargo fmt`
- `cargo test -p codex-tui slash_add_dir`
- targeted `codex-core` tests for the config schema and legacy sandbox recomputation

### Remaining notes

- The user-facing PRD flow is implemented.
- The main remaining work is standard integration follow-through, not missing feature behavior:
  - run a broader non-filtered verification sweep when desired
  - stage/commit/push the latest no-arg prompt changes
- Internally, the implementation uses the existing `OverrideTurnContext` / session-update pipeline rather than introducing a literal `addDirectories` permission-update type. Functionally this satisfies the PRD goals, but the internal shape differs from the original technical note.
## Summary

Implement a slash command, `/add-dir`, that allows the user to add an additional working directory to the current Claude Code session. The command must grant tool access to the selected directory, optionally persist that access in local settings, and update runtime sandbox configuration immediately so the new directory is usable without restarting the session.

## Problem

Claude Code starts with a primary working directory, but users often need to work across multiple repositories or sibling directories during a single session. Without an explicit way to expand the workspace, tool access is limited and the user must restart the session from a different directory or manually modify settings.

## Goals

- Let users add a directory from within the REPL using `/add-dir`.
- Support both direct invocation with a path and interactive invocation without a path.
- Validate the target path before granting access.
- Distinguish between session-only access and persisted access.
- Ensure file tools and sandboxed shell commands can access the new directory immediately.
- Keep the UX consistent with the existing permissions/settings flows.

## Non-Goals

- Managing removal of directories. This belongs to `/permissions` or a separate removal flow.
- Granting access to files directly; the command is directory-only.
- Persisting by default from the freeform input flow unless explicitly selected.
- Bypassing policy restrictions or locked settings behavior.

## User Stories

- As a user, I can run `/add-dir ../shared-lib` to add a sibling repository to my current session.
- As a user, I can run `/add-dir` and browse or type a path interactively.
- As a user, I can choose whether the directory is available only for the current session or remembered in local settings.
- As a user, I receive a clear error if the path does not exist, is not a directory, or is already covered by an existing working directory.

## User Experience

### Invocation

- `/add-dir <path>`
  - Validate the path.
  - If valid, show a confirmation dialog with options:
    - `Yes, for this session`
    - `Yes, and remember this directory`
    - `No`
- `/add-dir`
  - Open an input dialog prompting for a directory path.
  - Provide directory autocomplete suggestions.
  - Support `Tab` to complete and `Enter` to submit.
  - On successful validation, add the directory for the current session.

### Messaging

- Success, session-only:
  - `Added <path> as a working directory for this session`
- Success, persisted:
  - `Added <path> as a working directory and saved to local settings`
- Cancellation:
  - `Did not add a working directory.`
  - Or, when a validated path was provided:
  - `Did not add <path> as a working directory.`
- Include a follow-up hint to use `/permissions` to manage workspace access.

## Functional Requirements

### 1. Command Registration

- Register a local JSX command named `add-dir`.
- Expose description text equivalent to `Add a new working directory`.
- Accept an optional `<path>` argument.

### 2. Path Validation

- Trim the provided argument before validation.
- Expand user-relative paths such as `~`.
- Resolve the directory to an absolute normalized path.
- Validate with a single filesystem stat call.
- Reject empty input.
- Reject non-existent or inaccessible paths as `not found`.
- Reject paths that exist but are not directories.
- Reject paths already contained within any existing working directory.

### 3. Interactive Input Flow

- When no argument is supplied, render a dialog for entering a directory path.
- Show autocomplete suggestions for filesystem directories.
- Debounce suggestion fetching to keep input responsive.
- Allow keyboard navigation through suggestions.

### 4. Permission Update

- On confirmation, create a permission update of type `addDirectories`.
- Add the directory to the in-memory tool permission context.
- Tag the directory source as:
  - `session` for temporary access
  - `localSettings` when the user chooses to remember it

### 5. Persistence

- If the user chooses the remember option, persist the directory to local settings under:
  - `permissions.additionalDirectories`
- Avoid writing duplicate entries.
- If persistence fails, keep the session update and show a partial-success message.

### 6. Sandbox Refresh

- Update bootstrap state used for additional directories.
- Refresh sandbox runtime configuration immediately after the directory is added.
- Ensure sandboxed shell commands can access the new directory in the same session without restart.

### 7. Runtime Scope of Access

- The added directory must be treated as an additional working directory by tool permission checks.
- The added directory must be included in sandbox filesystem allowlists.
- The directory should participate in features that consume explicit additional directories, including:
  - optional `CLAUDE.md` loading from additional directories
  - skill loading from explicit additional directories
  - `--bare` flows that honor explicit directories while skipping auto-discovery

## Error Handling Requirements

- Empty path:
  - `Please provide a directory path.`
- Path not found or inaccessible:
  - `Path <absolutePath> was not found.`
- Not a directory:
  - `<input> is not a directory. Did you mean to add the parent directory <parentDir>?`
- Already covered:
  - `<input> is already accessible within the existing working directory <workingDir>.`

Errors should be rendered in the command flow and then returned to the REPL cleanly.

## Technical Notes

- Use the existing permission update pipeline rather than introducing command-specific permission state.
- Reuse the workspace-directory dialog component for both direct and no-arg flows.
- Keep session-only additional directories in bootstrap state as the runtime source of truth for immediate refresh behavior.
- Persisted additional directories should still be refreshed eagerly to avoid a race between settings reload and the user’s next action.

## Acceptance Criteria

- Running `/add-dir <valid-directory>` opens a confirmation flow and adds the directory when approved.
- Running `/add-dir` opens a path-entry dialog with autocomplete.
- Choosing session-only access adds the directory for the current session only.
- Choosing remember adds the directory to the current session and local settings.
- The newly added directory is immediately accessible to file tools and sandboxed shell commands.
- Invalid paths produce deterministic, user-friendly error messages.
- Adding a directory already covered by an existing working directory is rejected.
- Duplicate persisted entries are not written.
- The command behaves correctly in `--bare` mode for explicit additional directories.

## Open Questions

- Should the no-argument interactive input flow also offer the remember choice after validation, for consistency with the direct-path flow?
- Should persisted scope support `projectSettings` or `userSettings` in the future, or remain local-only from this command?
- Should telemetry be added to measure command usage, cancellation rate, and validation failures?






