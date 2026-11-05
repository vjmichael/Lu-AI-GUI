//! IDE workspace desktop adapter (IDE-01B).
//!
//! This host is the only place a native folder picker is invoked. The WebView
//! cannot select an arbitrary project path, invoke an IDE worker, or grant
//! any filesystem/tool permissions through these lifecycle commands.
use std::{path::PathBuf, sync::Arc};

use lu_ide_workspace::{
    wire::{
        self,
        proto::{
            self, workspace_reply::Outcome, workspace_request::Operation, CloseProject,
            ListProjects, ReopenProject, SelectDirectory, WorkspaceRequest,
        },
    },
    IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker,
};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{next_id, DesktopState};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeProjectView {
    project_id: String,
    display_name: String,
    active: bool,
    available: bool,
}

impl From<proto::ProjectView> for IdeProjectView {
    fn from(view: proto::ProjectView) -> Self {
        Self {
            project_id: view.project_id,
            display_name: view.display_name,
            active: view.active,
            available: view.available,
        }
    }
}

/// The picker must run in a blocking task, never on the Tauri UI thread.
struct OsDirectoryPicker(AppHandle);

impl NativeDirectoryPicker for OsDirectoryPicker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        let selected = self.0.dialog().file().blocking_pick_folder();
        selected
            .map(|value| value.into_path().map_err(|_| IdeError::InvalidSelection))
            .transpose()
    }
}

struct NoDirectoryPicker;

impl NativeDirectoryPicker for NoDirectoryPicker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        // Only the user-invoked Select command can open a native chooser.
        Err(IdeError::InvalidSelection)
    }
}

fn dispatch(
    registry: &IdeWorkspaceRegistry,
    operation: Operation,
    picker: &mut dyn NativeDirectoryPicker,
) -> Result<Outcome, String> {
    let request = WorkspaceRequest {
        protocol_version: wire::VERSION,
        request_id: next_id("ide-ui-"),
        operation: Some(operation),
    };
    // Host constructs and validates the complete wire frame. JS cannot supply
    // arbitrary protobuf fields, a directory path, or a trusted origin token.
    let payload = wire::encode_request(&request).map_err(|_| "Invalid IDE request")?;
    let response = registry
        .handle_ui_request(&payload, picker)
        .map_err(|_| "IDE protocol unavailable")?;
    let reply = wire::decode_reply(&response).map_err(|_| "Invalid IDE reply")?;
    if reply.request_id != request.request_id {
        return Err("IDE correlation mismatch".into());
    }
    match reply.outcome.ok_or("IDE reply missing outcome")? {
        Outcome::Failure(failure) => Err(format!("IDE operation rejected: {}", failure.code)),
        success => Ok(success),
    }
}

fn projects(registry: &IdeWorkspaceRegistry) -> Result<Vec<IdeProjectView>, String> {
    match dispatch(
        registry,
        Operation::List(ListProjects {}),
        &mut NoDirectoryPicker,
    )? {
        Outcome::Projects(list) => Ok(list.projects.into_iter().map(Into::into).collect()),
        _ => Err("Unexpected IDE project list response".into()),
    }
}

fn finish_lifecycle(
    registry: &IdeWorkspaceRegistry,
    operation: Operation,
    picker: &mut dyn NativeDirectoryPicker,
) -> Result<Vec<IdeProjectView>, String> {
    dispatch(registry, operation, picker)?;
    projects(registry)
}

#[tauri::command]
pub async fn lu_ide_projects(
    state: State<'_, Arc<DesktopState>>,
) -> Result<Vec<IdeProjectView>, String> {
    let registry = Arc::clone(&state.ide);
    tauri::async_runtime::spawn_blocking(move || projects(&registry))
        .await
        .map_err(|_| "IDE catalog task unavailable".to_string())?
}

#[tauri::command]
pub async fn lu_ide_select_project(
    app: AppHandle,
    state: State<'_, Arc<DesktopState>>,
) -> Result<Vec<IdeProjectView>, String> {
    let registry = Arc::clone(&state.ide);
    tauri::async_runtime::spawn_blocking(move || {
        finish_lifecycle(
            &registry,
            Operation::Select(SelectDirectory {}),
            &mut OsDirectoryPicker(app),
        )
    })
    .await
    .map_err(|_| "IDE directory selection unavailable".to_string())?
}

#[tauri::command]
pub async fn lu_ide_open_project(
    state: State<'_, Arc<DesktopState>>,
    project_id: String,
) -> Result<Vec<IdeProjectView>, String> {
    // Project ID is opaque and validated again by the strict protobuf codec.
    let registry = Arc::clone(&state.ide);
    tauri::async_runtime::spawn_blocking(move || {
        finish_lifecycle(
            &registry,
            Operation::Reopen(ReopenProject { project_id }),
            &mut NoDirectoryPicker,
        )
    })
    .await
    .map_err(|_| "IDE project open unavailable".to_string())?
}

#[tauri::command]
pub async fn lu_ide_close_project(
    state: State<'_, Arc<DesktopState>>,
    project_id: String,
) -> Result<Vec<IdeProjectView>, String> {
    let registry = Arc::clone(&state.ide);
    tauri::async_runtime::spawn_blocking(move || {
        finish_lifecycle(
            &registry,
            Operation::Close(CloseProject { project_id }),
            &mut NoDirectoryPicker,
        )
    })
    .await
    .map_err(|_| "IDE project close unavailable".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_project_replies_hide_host_paths() {
        let view = proto::ProjectView {
            project_id: "ide-0123456789abcdef0123456789abcdef".into(),
            display_name: "project".into(),
            active: false,
            available: true,
        };
        let public: IdeProjectView = view.into();
        let json = serde_json::to_string(&public).unwrap();
        assert!(json.contains("\"projectId\""));
        assert!(!json.contains("root"));
        assert!(!json.contains("path"));
    }

    #[test]
    fn invalid_project_id_never_reaches_core_registry() {
        let request = WorkspaceRequest {
            protocol_version: wire::VERSION,
            request_id: "test-id".into(),
            operation: Some(Operation::Reopen(ReopenProject {
                project_id: "../../outside".into(),
            })),
        };
        assert!(wire::encode_request(&request).is_err());
    }
}
