# PRD: `/add-dir` Slash Command

## Status Update (2026-04-03)

### Implemented in code so far

- Added a persisted config field under `permissions.additionalDirectories` in the Rust config model.
- Added runtime `Config.additional_working_directories` state so explicit extra directories are no longer only a CLI/bootstrap concern.
- Threaded additional working directories into `SessionConfiguration`, `SessionSettingsUpdate`, and per-turn config rebuilding.
- Threaded `additional_working_directories` through `Op::OverrideTurnContext`, the TUI `AppCommand` wrapper, and the core override handler so live session updates can carry extra directories.
- Updated project-doc discovery so `AGENTS.md` lookup now also walks explicit additional working directories.
- Updated skill loading so repo skill roots from explicit additional working directories are included.
- Preserved existing `--add-dir` / additional writable roots behavior by merging CLI-provided directories with persisted config directories.
- Registered `/add-dir` in the TUI slash-command surface, including popup visibility and inline-argument support.
- Added `/add-dir` dispatch in `chatwidget.rs`.
  - Bare `/add-dir` currently reports that interactive directory entry is not wired yet.
  - `/add-dir <path>` currently performs session-only path validation and applies an immediate `OverrideTurnContext` update for `additional_working_directories`.
- Added TUI coverage for the new command and fixed the dependent compile fallout in `codex-app-server` / TUI test fixtures exposed by the new override field.
- Ran `just fmt` and `cargo test -p codex-tui` successfully after this slice.

### Not implemented yet

- Interactive no-arg directory entry UI with directory autocomplete.
- Confirmation popup with `session`, `remember`, and `cancel` actions.
- Persistence write path for appending to `permissions.additionalDirectories` without duplicates.
- Persisted-scope success / partial-success messaging.
- Sandbox refresh plumbing for the current session after adding a directory.
- Docs updates for the final remembered-directory UX, if needed.

### Important follow-up / cleanup items

- The current `/add-dir <path>` flow is intentionally session-only and skips the PRD's confirmation step until the picker / remember UI exists.
- Full workspace verification has not been rerun; this slice is verified with `cargo test -p codex-tui`.
- The PRD originally called the command `/add-dir`, while the filename says `dir-slash-command`; the code work so far has followed `/add-dir`.

### Suggested next-session order

1. Implement the confirmation popup and remember/persist flow for `/add-dir <path>`.
2. Implement the interactive no-arg directory prompt with autocomplete and keyboard navigation.
3. Add sandbox refresh plumbing so newly added directories are immediately usable by sandboxed shell commands.
4. Update docs/schema if the persistence flow changes config output, then rerun the appropriate verification sweep.
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






