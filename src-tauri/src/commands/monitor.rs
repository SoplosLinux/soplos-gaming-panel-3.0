// What the window asks about the machine when it opens, before the first
// reading of the sampler arrives.

#[tauri::command]
pub fn monitor_snapshot() -> Option<crate::monitor::Snapshot> {
    crate::monitor::latest()
}
