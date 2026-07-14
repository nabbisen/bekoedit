use bekoedit_core::AppState;
use bekoedit_ui_contract::source_editor::SourceEditorId;
use dioxus::prelude::*;

use crate::source_sync::{
    EditorMountHandle, SourceSyncState, mount_source_editor, unmount_source_editor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEditorStatus {
    Loading,
    Ready,
    Unavailable,
}

pub fn use_source_editor_lifecycle(editor_id: SourceEditorId) -> SourceEditorStatus {
    let state = use_context::<Signal<AppState>>();
    let sync = use_context::<Signal<SourceSyncState>>();
    let mut mount_handle = use_signal(|| None::<EditorMountHandle>);

    use_effect(move || {
        let session = state.read();
        if let Some(session) = session.session.as_ref() {
            mount_source_editor(sync, editor_id, session.document_id, session.revision);
            if let Some(current) = sync.read().mount_handle(editor_id, session.document_id)
                && *mount_handle.read() != Some(current)
            {
                mount_handle.set(Some(current));
            }
        }
    });

    use_drop(move || {
        if let Some(handle) = *mount_handle.read() {
            unmount_source_editor(sync, handle);
        }
    });

    let document_id = state
        .read()
        .session
        .as_ref()
        .map(|session| session.document_id);
    let controller = sync.read();
    if document_id.is_some_and(|id| controller.is_ready(editor_id, id)) {
        SourceEditorStatus::Ready
    } else if controller.is_unavailable() {
        SourceEditorStatus::Unavailable
    } else {
        SourceEditorStatus::Loading
    }
}
