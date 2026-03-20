// Fragment queue commands — state, fragments, skip, stop, clear.

use crate::fragment_queue::{FragmentQueue, QueueStatus};
use crate::pagination::TextFragment;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

use super::PaginationEvent;

/// Get the current fragment queue state.
#[tauri::command]
pub fn get_queue_state(queue: State<'_, Mutex<FragmentQueue>>) -> QueueState {
    let q = queue.lock().unwrap();
    QueueState {
        status: q.status(),
        current_index: q.current_index(),
        total: q.len(),
        fragments: q.fragments(),
    }
}

/// Get the list of fragments in the queue.
#[tauri::command]
pub fn get_queue_fragments(queue: State<'_, Mutex<FragmentQueue>>) -> Vec<TextFragment> {
    let q = queue.lock().unwrap();
    q.fragments()
}

/// Skip to a specific fragment in the queue.
#[tauri::command]
pub fn skip_to_fragment(
    app: AppHandle,
    queue: State<'_, Mutex<FragmentQueue>>,
    index: usize,
) -> Result<(), String> {
    let q = queue.lock().unwrap();
    let total = q.len();
    let is_paginated = total > 1;
    q.skip_to(index)?;

    // Emit skip event
    let _ = app.emit("pagination:skipped", PaginationEvent {
        total,
        current_index: index,
        is_paginated,
    });

    Ok(())
}

/// Stop fragment queue playback.
#[tauri::command]
pub fn stop_queue(
    app: AppHandle,
    queue: State<'_, Mutex<FragmentQueue>>,
) -> Result<(), String> {
    let q = queue.lock().unwrap();
    let total = q.len();
    let current_index = q.current_index().unwrap_or(0);
    let is_paginated = total > 1;
    q.stop();

    // Emit stopped event
    let _ = app.emit("pagination:stopped", PaginationEvent {
        total,
        current_index,
        is_paginated,
    });

    Ok(())
}

/// Clear fragment queue.
#[tauri::command]
pub fn clear_queue(
    app: AppHandle,
    queue: State<'_, Mutex<FragmentQueue>>,
) -> Result<(), String> {
    let q = queue.lock().unwrap();
    let total = q.len();
    let current_index = q.current_index().unwrap_or(0);
    let is_paginated = total > 1;
    q.clear();

    // Emit cleared event
    let _ = app.emit("pagination:cleared", PaginationEvent {
        total,
        current_index,
        is_paginated,
    });

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueState {
    pub status: QueueStatus,
    pub current_index: Option<usize>,
    pub total: usize,
    pub fragments: Vec<TextFragment>,
}
