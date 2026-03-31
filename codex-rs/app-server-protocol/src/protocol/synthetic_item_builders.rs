//! Shared builders for synthetic [`ThreadItem`] values emitted by the app-server layer.
//!
//! These items do not come from first-class core `ItemStarted` / `ItemCompleted` events.
//! Instead, the app-server synthesizes them so clients can render a coherent lifecycle for
//! approvals and other pre-execution flows before the underlying tool has started or when the
//! tool never starts at all.
//!
//! Keeping these builders in one place is useful for two reasons:
//! - Live notifications and rebuilt `thread/read` history both need to construct the same
//!   synthetic items, so sharing the logic avoids drift between those paths.
//! - The projection is presentation-specific. Core protocol events stay generic, while the
//!   app-server protocol decides how to surface those events as `ThreadItem`s for clients.
use crate::protocol::v2::CommandAction;
use crate::protocol::v2::CommandExecutionSource;
use crate::protocol::v2::CommandExecutionStatus;
use crate::protocol::v2::FileUpdateChange;
use crate::protocol::v2::PatchApplyStatus;
use crate::protocol::v2::ThreadItem;
use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_shell_command::parse_command::parse_command;
use codex_shell_command::parse_command::shlex_join;
use std::path::PathBuf;

pub fn synthetic_file_change_item(
    item_id: String,
    changes: Vec<FileUpdateChange>,
    status: PatchApplyStatus,
) -> ThreadItem {
    ThreadItem::FileChange {
        id: item_id,
        changes,
        status,
    }
}

pub fn synthetic_command_execution_item(
    item_id: String,
    command: String,
    cwd: PathBuf,
    command_actions: Vec<CommandAction>,
    source: CommandExecutionSource,
    status: CommandExecutionStatus,
) -> ThreadItem {
    ThreadItem::CommandExecution {
        id: item_id,
        command,
        cwd,
        process_id: None,
        source,
        status,
        command_actions,
        aggregated_output: None,
        exit_code: None,
        duration_ms: None,
    }
}

pub fn guardian_command_execution_item(
    assessment: &GuardianAssessmentEvent,
    status: CommandExecutionStatus,
) -> Option<ThreadItem> {
    match &assessment.action {
        GuardianAssessmentAction::Command { command, cwd, .. } => {
            let command = command.clone();
            let command_actions = vec![CommandAction::Unknown {
                command: command.clone(),
            }];
            Some(synthetic_command_execution_item(
                assessment.id.clone(),
                command,
                cwd.clone(),
                command_actions,
                CommandExecutionSource::Agent,
                status,
            ))
        }
        GuardianAssessmentAction::Execve {
            program, argv, cwd, ..
        } => {
            let argv = if argv.is_empty() {
                vec![program.clone()]
            } else {
                std::iter::once(program.clone())
                    .chain(argv.iter().skip(1).cloned())
                    .collect::<Vec<_>>()
            };
            let command = shlex_join(&argv);
            let parsed_cmd = parse_command(&argv);
            let command_actions = if parsed_cmd.is_empty() {
                vec![CommandAction::Unknown {
                    command: command.clone(),
                }]
            } else {
                parsed_cmd.into_iter().map(CommandAction::from).collect()
            };
            Some(synthetic_command_execution_item(
                assessment.id.clone(),
                command,
                cwd.clone(),
                command_actions,
                CommandExecutionSource::Agent,
                status,
            ))
        }
        GuardianAssessmentAction::ApplyPatch { .. }
        | GuardianAssessmentAction::NetworkAccess { .. }
        | GuardianAssessmentAction::McpToolCall { .. } => None,
    }
}
