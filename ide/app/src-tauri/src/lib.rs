//! Standalone IDE desktop. All project directories originate in the OS picker,
//! not from JavaScript, model output, or a Lu IPC call.
#![forbid(unsafe_code)]
use std::{path::PathBuf, sync::Arc};

use lu_ide_workspace::{IdeError, IdeWorkspaceRegistry, NativeDirectoryPicker};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

struct NativePicker(AppHandle);
impl NativeDirectoryPicker for NativePicker {
    fn choose_project_directory(&mut self) -> Result<Option<PathBuf>, IdeError> {
        self.0
            .dialog()
            .file()
            .blocking_pick_folder()
            .map(|folder| folder.into_path().map_err(|_| IdeError::InvalidSelection))
            .transpose()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSummary {
    project_id: String,
    display_name: String,
    active: bool,
    available: bool,
}

impl From<lu_ide_workspace::wire::proto::ProjectView> for ProjectSummary {
    fn from(project: lu_ide_workspace::wire::proto::ProjectView) -> Self {
        Self {
            project_id: project.project_id,
            display_name: project.display_name,
            active: project.active,
            available: project.available,
        }
    }
}

fn summarize(registry: &IdeWorkspaceRegistry) -> Result<Vec<ProjectSummary>, String> {
    registry
        .projects()
        .map(|list| list.into_iter().map(Into::into).collect())
        .map_err(|error| format!("Project registry unavailable: {}", stable_error(&error)))
}

fn stable_error(error: &IdeError) -> &'static str {
    match error {
        IdeError::Io(_) => "IO_FAILURE",
        IdeError::Format(_) | IdeError::InvalidCatalog => "CATALOG_INVALID",
        IdeError::Poisoned => "HOST_UNAVAILABLE",
        IdeError::InvalidSelection => "INVALID_DIRECTORY",
        IdeError::UnknownProject => "PROJECT_NOT_FOUND",
        IdeError::Capacity => "PROJECT_CAPACITY",
        IdeError::AlreadyActive => "PROJECT_ALREADY_OPEN",
        IdeError::NotActive => "PROJECT_NOT_OPEN",
    }
}

#[tauri::command]
async fn ide_list_projects(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || summarize(&registry))
        .await
        .map_err(|_| "PROJECT_TASK_UNAVAILABLE".to_string())?
}

#[tauri::command]
async fn ide_choose_project(
    app: AppHandle,
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .register_from_native_picker(&mut NativePicker(app))
            .map_err(|error| stable_error(&error).to_string())?;
        summarize(&registry)
    })
    .await
    .map_err(|_| "PROJECT_PICKER_UNAVAILABLE".to_string())?
}

#[tauri::command]
async fn ide_open_project(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .reopen(&project_id)
            .map_err(|error| stable_error(&error).to_string())?;
        summarize(&registry)
    })
    .await
    .map_err(|_| "PROJECT_TASK_UNAVAILABLE".to_string())?
}

#[tauri::command]
async fn ide_close_project(
    state: State<'_, Arc<IdeWorkspaceRegistry>>,
    project_id: String,
) -> Result<Vec<ProjectSummary>, String> {
    let registry = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || {
        registry
            .close(&project_id)
            .map_err(|error| stable_error(&error).to_string())?;
        summarize(&registry)
    })
    .await
    .map_err(|_| "PROJECT_TASK_UNAVAILABLE".to_string())?
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let registry = IdeWorkspaceRegistry::open(data.join("projects.json"))
                .map_err(|_| std::io::Error::other("standalone IDE registry failed to open"))?;
            app.manage(Arc::new(registry));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ide_list_projects,
            ide_choose_project,
            ide_open_project,
            ide_close_project
        ])
        .run(tauri::generate_context!())
        .expect("standalone Lu IDE failed to initialize");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_summary_exposes_no_host_path() {
        let entry = lu_ide_workspace::wire::proto::ProjectView {
            project_id: "ide-0123456789abcdef0123456789abcdef".into(),
            display_name: "example".into(),
            active: false,
            available: true,
        };
        let summary: ProjectSummary = entry.into();
        assert_eq!(summary.display_name, "example");
        assert_eq!(summary.project_id.len(), 36);
        // The public type has no PathBuf or path fields by construction.
    }
}
