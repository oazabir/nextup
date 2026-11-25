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
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

mod awake_clock;
mod focus_switch;
use awake_clock::AwakeClock;

#[tauri::command]
fn awake_millis(clock: tauri::State<'_, AwakeClock>) -> u64 {
    clock.millis()
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
fn schedule_checkin(window: WebviewWindow, generation: tauri::State<'_, Arc<AtomicU64>>) {
    let generation = Arc::clone(generation.inner());
    let ticket = generation.fetch_add(1, Ordering::Relaxed) + 1;
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(300));
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

fn main() {
    let modifiers = Some(Modifiers::CONTROL | Modifiers::ALT);
    let previous = Shortcut::new(modifiers, Code::BracketLeft);
    let next = Shortcut::new(modifiers, Code::BracketRight);
    let compose = Shortcut::new(modifiers, Code::KeyN);

    let has_global_shortcuts = !cfg!(target_os = "linux") || std::env::var_os("DISPLAY").is_some();
    let clock = AwakeClock::default();
    clock.start();
    let mut builder = tauri::Builder::default()
        .manage(Arc::new(AtomicU64::new(0)))
        .manage(clock);
    if has_global_shortcuts {
        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if let Some(window) = app.get_webview_window("main") {
                        let script = if shortcut == &compose {
                            let _ = show_widget(window.clone());
                            let _ = window.set_focus();
                            "window.dispatchEvent(new CustomEvent('desktop-shortcut', { detail: 0 }))"
                        } else if shortcut == &previous {
                            "window.dispatchEvent(new CustomEvent('desktop-shortcut', { detail: -1 }))"
                        } else {
                            "window.dispatchEvent(new CustomEvent('desktop-shortcut', { detail: 1 }))"
                        };
                        let _ = window.eval(script);
                    }
                })
                .build(),
        );
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
            if has_global_shortcuts {
                for shortcut in [previous, next, compose] {
                    if let Err(error) = app.global_shortcut().register(shortcut) {
                        eprintln!("Could not register desktop shortcut {shortcut:?}: {error}");
                    }
                }
            }
            let show = MenuItem::with_id(app, "show", "Show Next Up", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Next Up", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Next Up")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = show_widget(window.clone());
                            let _ = window.set_focus();
                            let _ = window.eval("window.dispatchEvent(new Event('desktop-show'))");
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
            resize_widget,
            show_widget,
            hide_widget,
            pin_widget,
            awake_millis,
            schedule_checkin
        ])
        .run(tauri::generate_context!())
        .expect("could not run Next Up");
}
