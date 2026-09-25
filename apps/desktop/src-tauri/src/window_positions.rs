//! Window placement is a local preference, independent of visibility and zoom.
use tauri::Emitter;
use tauri_plugin_window_state::{AppHandleExt, StateFlags, WindowExt};

pub fn restore(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    window.restore_state(StateFlags::POSITION)?;
    // The plugin checks monitor intersection; also keep the title bar reachable
    // when a display resolution/arrangement changes or only an edge intersects.
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    let monitors = window.available_monitors()?;
    let monitor = monitors
        .iter()
        .find(|monitor| {
            let area = monitor.work_area();
            i64::from(position.x) >= i64::from(area.position.x)
                && i64::from(position.x) < i64::from(area.position.x) + i64::from(area.size.width)
                && i64::from(position.y) >= i64::from(area.position.y)
                && i64::from(position.y) < i64::from(area.position.y) + i64::from(area.size.height)
        })
        .or_else(|| monitors.first());
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let x = position.x.clamp(
            area.position.x,
            area.position
                .x
                .saturating_add(area.size.width.saturating_sub(size.width) as i32),
        );
        let y = position.y.clamp(
            area.position.y,
            area.position
                .y
                .saturating_add(area.size.height.saturating_sub(size.height) as i32),
        );
        window.set_position(tauri::PhysicalPosition::new(x, y))?;
    }
    Ok(())
}

pub fn save(app: &tauri::AppHandle) {
    if app.save_window_state(StateFlags::POSITION).is_err() {
        let _ = app.emit(
            "runtime-error",
            "Window positions could not be saved on this PC.",
        );
    }
}
