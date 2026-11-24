# Next Up

![Floating window showing the active task, its timer, and the next two priorities](docs/images/floating-window.jpg)

![Full task view showing the checklist and playback controls](docs/images/full-task-view.jpg)

Next Up is a desktop focus companion for people who jump between tasks and lose track of what they meant to do next. The floating panel keeps your current priority and timer in sight, and a five-minute check-in gives you a chance to return to it or deliberately switch. For people with ADHD and anyone who finds it hard to hold several priorities in mind, the panel provides a visible place to pick up again after an interruption.

- Pin the panel for a continuous reminder or expand it to reorder and complete tasks.
- Switch tasks with a keyboard shortcut and keep the time already spent on each one.
- Type or dictate a new task and optionally give it a deadline. Deadlines are highlighted during the final three hours.

Next Up is built with Rust and Tauri 2 and shares the same Mac-style interface across macOS, Windows, and Linux.

## Run and package

Install the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/) first. From this directory:

```sh
cargo run --manifest-path src-tauri/Cargo.toml
```

To build an installer on the target operating system:

```sh
cargo install tauri-cli --version "^2" --locked
cargo tauri build
```

The same three frontend files can also be served with `python3 -m http.server 8765` for browser preview. Desktop and browser have separate local task stores.

## Platform behavior

- The rounded glass tint is drawn inside a transparent, frameless Tauri window on all three platforms. On Linux, the compositor controls transparency, placement, and always-on-top behavior.
- Tauri's transparent macOS window support requires private APIs, which rules out Mac App Store distribution for this configuration.
- Ctrl+Alt/Option+[ and ] switch tasks. Ctrl+Alt/Option+N opens the composer. Global registration relies on the Tauri shortcut plugin and uses X11 on Linux. Focused shortcuts are available as a fallback.
- The microphone uses the webview's speech recognition API when exposed. Typing works without speech recognition. Parakeet V3 is not bundled.
