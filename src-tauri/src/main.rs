#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    LogicalSize, Manager, PhysicalPosition, WebviewWindow,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

mod awake_clock;
mod focus_switch;
use awake_clock::AwakeClock;

#[tauri::command]
fn awake_millis(clock: tauri::State<'_, AwakeClock>) -> u64 {
    clock.millis()
}

#[tauri::command]
fn open_task_link(url: &str) -> Result<(), String> {
    let url = tauri::Url::parse(url).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Expected an HTTP or HTTPS link without credentials.".into());
    }

    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(target_os = "windows")]
    let opener = "explorer.exe";

    std::process::Command::new(opener)
        .arg(url.as_str())
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn place_widget(window: &WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = window.current_monitor()?.or(window.primary_monitor()?) {
        let area = monitor.work_area();
        let size = window.outer_size()?;
        let inset = (16.0 * monitor.scale_factor()).round() as i32;
        let y = (area.size.height as i32 - size.height as i32 - inset).max(0);
        window.set_position(PhysicalPosition::new(
            area.position.x + inset,
            area.position.y + y,
        ))?;
    }
    Ok(())
}

#[tauri::command]
fn resize_widget(window: WebviewWindow, height: u32) -> Result<(), String> {
    window
        .set_size(LogicalSize::new(420, height.clamp(96, 540)))
        .map_err(|error| error.to_string())?;
    // Wayland compositors may choose the window position themselves.
    let _ = place_widget(&window);
    Ok(())
}

#[tauri::command]
fn show_widget(window: WebviewWindow) -> Result<(), String> {
    let _ = place_widget(&window);
    window.show().map_err(|error| error.to_string())
}

#[tauri::command]
fn hide_widget(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn pin_widget(window: WebviewWindow, pinned: bool) -> Result<(), String> {
    window
        .set_always_on_top(pinned)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn schedule_checkin(
    window: WebviewWindow,
    generation: tauri::State<'_, Arc<AtomicU64>>,
    seconds: u64,
) {
    let generation = Arc::clone(generation.inner());
    let ticket = generation.fetch_add(1, Ordering::Relaxed) + 1;
    if seconds == 0 {
        return;
    }
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(seconds.clamp(60, 3600)));
        if generation.load(Ordering::Relaxed) != ticket {
            break;
        }
        if window
            .eval("window.dispatchEvent(new Event('desktop-checkin'))")
            .is_err()
        {
            break;
        }
    });
}

#[tauri::command]
async fn set_task_shortcut(
    window: WebviewWindow,
    action: String,
    binding: String,
    previous: Option<String>,
) -> Result<bool, String> {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return Ok(false);
    }
    let detail = match action.as_str() {
        "previous" => "-1",
        "next" => "1",
        "compose" => "0",
        "pause" => "'pause'",
        _ => return Err("Unknown shortcut action".into()),
    };
    let shortcut = binding
        .parse::<Shortcut>()
        .map_err(|error| error.to_string())?;
    let previous = previous
        .map(|value| value.parse::<Shortcut>())
        .transpose()
        .map_err(|error| error.to_string())?;
    if previous == Some(shortcut) {
        return Ok(true);
    }
    let manager = window.global_shortcut();
    let focus = action == "compose";
    let script = format!(
        "window.dispatchEvent(new CustomEvent('desktop-shortcut', {{ detail: {detail} }}))"
    );
    let target = window.clone();
    manager
        .on_shortcut(shortcut, move |_, _, event| {
            if event.state() == ShortcutState::Pressed {
                if focus {
                    let _ = show_widget(target.clone());
                    let _ = target.set_focus();
                }
                let _ = target.eval(&script);
            }
        })
        .map_err(|error| error.to_string())?;
    if let Some(previous) = previous {
        if let Err(error) = manager.unregister(previous) {
            let _ = manager.unregister(shortcut);
            return Err(error.to_string());
        }
    }
    Ok(true)
}

fn main() {
    let has_global_shortcuts = !cfg!(target_os = "linux") || std::env::var_os("DISPLAY").is_some();
    let clock = AwakeClock::default();
    clock.start();
    let mut builder = tauri::Builder::default()
        .manage(Arc::new(AtomicU64::new(0)))
        .manage(focus_switch::BrowserPreference::default())
        .manage(clock);
    if has_global_shortcuts {
        builder = builder.plugin(tauri_plugin_global_shortcut::Builder::new().build());
    }
    builder
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Focused(false)) {
                if let Some(webview) = window.get_webview_window(window.label()) {
                    let _ = webview.eval("window.dispatchEvent(new Event('desktop-blur'))");
                }
            }
        })
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                focus_switch::start(window);
            }
            let show = MenuItem::with_id(app, "show", "Show Next Up", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Next Up", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &settings, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Next Up")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" | "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = show_widget(window.clone());
                            let _ = window.set_focus();
                            let script = if event.id().as_ref() == "settings" {
                                "window.dispatchEvent(new Event('desktop-show')); window.dispatchEvent(new Event('desktop-settings'))"
                            } else {
                                "window.dispatchEvent(new Event('desktop-show'))"
                            };
                            let _ = window.eval(script);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            if let Err(error) = tray.build(app) {
                eprintln!("Could not create the Next Up tray icon: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_task_link,
            set_task_shortcut,
            resize_widget,
            show_widget,
            hide_widget,
            pin_widget,
            awake_millis,
            focus_switch::set_work_browser,
            schedule_checkin
        ])
        .run(tauri::generate_context!())
        .expect("could not run Next Up");
}
