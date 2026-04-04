mod app;
mod bot;
mod config;
mod core;
mod ui;

use anyhow::Result;
use std::{
    sync::{atomic::{AtomicBool, Ordering}, Arc},
    time::Duration,
};
use tokio::task::LocalSet;

use app::{init_terminal, restore_terminal, spawn_event_reader, App, Args};
use bot::run_bot;
use config::ConfigWatcher;
use core::{EventBus, UiEvent, BotCommand};

/// Guard untuk merestore stderr saat program selesai
struct StderrGuard;

impl Drop for StderrGuard {
    fn drop(&mut self) {
        // Restore stderr - on Windows this requires re-opening CONOUT$
        #[cfg(windows)]
        unsafe {
            use std::os::windows::io::IntoRawHandle;
            use windows::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE};
            use windows::Win32::Foundation::HANDLE;

            if let Ok(conout) = std::fs::OpenOptions::new()
                .write(true)
                .open("CONOUT$")
            {
                let raw_handle = conout.into_raw_handle();
                let handle = std::mem::transmute::<*mut std::ffi::c_void, HANDLE>(raw_handle);
                let _ = SetStdHandle(STD_ERROR_HANDLE, handle);
            }
        }
    }
}

/// Redirect stderr ke null untuk menghindari output error external crates
fn redirect_stderr_to_null() -> StderrGuard {
    #[cfg(windows)]
    unsafe {
        use std::os::windows::io::IntoRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE};

        if let Ok(null_file) = std::fs::OpenOptions::new().write(true).open("NUL") {
            let raw_handle = null_file.into_raw_handle();
            let handle = std::mem::transmute::<*mut std::ffi::c_void, HANDLE>(raw_handle);
            let _ = SetStdHandle(STD_ERROR_HANDLE, handle);
        }
    }

    #[cfg(not(windows))]
    unsafe {
        use std::os::unix::io::IntoRawFd;

        if let Ok(null_file) = std::fs::OpenOptions::new().write(true).open("/dev/null") {
            let raw_fd = null_file.into_raw_fd();
            libc::dup2(raw_fd, libc::STDERR_FILENO);
            libc::close(raw_fd);
        }
    }

    StderrGuard
}

#[tokio::main]
async fn main() -> Result<()> {
    // Redirect stderr to suppress external error output from Azalea/bevy
    let _stderr_guard = redirect_stderr_to_null();

    // Parse arguments
    let mut args = Args::parse(std::env::args().skip(1))?;

    // Load config if specified (before TUI setup)
    if let Some(config_path) = &args.config_file {
        match crate::core::Config::load_from_file(config_path) {
            Ok(config) => {
                args.merge_with_config(&config);
            }
            Err(e) => {
                eprintln!("Warning: Failed to load config file: {}", e);
            }
        }
    }

    // Run without TUI if requested
    if args.no_tui {
        run_simple_mode(args).await?;
        return Ok(());
    }

    // Create event bus for decoupled communication
    let event_bus = EventBus::new();
    let bot_cmd_tx = event_bus.cmd_tx.clone();
    let mut bot_event_rx = event_bus.ui_rx;

    // Setup TUI
    let mut terminal = init_terminal()?;
    let (mut app, app_event_tx) = App::new(bot_cmd_tx, args.server.clone(), args.username.clone(), args.theme.clone());

    // Setup config file watcher after app_event_tx is available
    let mut _config_watcher = None;
    if let Some(config_path) = &args.config_file {
        _config_watcher = ConfigWatcher::new(config_path, app_event_tx.clone()).ok();
    }

    // Setup event reader thread
    spawn_event_reader(app_event_tx.clone(), Duration::from_millis(100));

    // Bridge bot events to app
    let event_tx = app_event_tx.clone();
    tokio::spawn(async move {
        while let Some(event) = bot_event_rx.recv().await {
            if event_tx.send(app::AppEvent::Minecraft(event)).is_err() {
                break;
            }
        }
    });

    // Shared flag to signal app exit
    let app_running = Arc::new(AtomicBool::new(true));
    let app_running_clone = app_running.clone();

    // Run app in a separate thread
    let app_handle = std::thread::spawn(move || {
        let result = app.run(&mut terminal);
        app_running_clone.store(false, Ordering::Relaxed);
        result
    });

    // Run bot with timeout checks
    let local = LocalSet::new();

    local.spawn_local(async move {
        let bot_future = run_bot(args, event_bus.cmd_tx, event_bus.ui_tx);

        let exit_future = async {
            while app_running.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };

        tokio::select! {
            _ = bot_future => {}
            _ = exit_future => {}
        }
    });

    local.await;

    let _ = restore_terminal();
    let _ = app_handle.join();

    Ok(())
}

async fn run_simple_mode(args: Args) -> Result<()> {
    let event_bus = EventBus::new();
    let bot_cmd_tx = event_bus.cmd_tx.clone();
    let mut bot_event_rx = event_bus.ui_rx;

    println!("Starting Minecraft AFK TUI (Simple Mode)...");
    println!("Server: {}", args.server);
    println!("Username: {}", args.username);
    if args.email.is_some() {
        println!("Auth: Microsoft");
    } else {
        println!("Auth: Offline");
    }
    println!();

    // Handle events
    tokio::spawn(async move {
        while let Some(event) = bot_event_rx.recv().await {
            match event {
                UiEvent::Connected { server, username } => {
                    println!("[CONNECTED] {} as {}", server, username);
                }
                UiEvent::Disconnected { reason } => {
                    println!("[DISCONNECTED] {}", reason.unwrap_or_default());
                }
                UiEvent::ChatReceived { username, message } => {
                    println!("[CHAT] <{}> {}", username, message);
                }
                UiEvent::SystemMessage(msg) => {
                    println!("[SYSTEM] {}", msg);
                }
                UiEvent::Error(err) => {
                    eprintln!("[ERROR] {}", err);
                }
                _ => {}
            }
        }
    });

    // Simple input loop
    let tx = bot_cmd_tx.clone();
    std::thread::spawn(move || {
        use std::io::{self, BufRead};
        let stdin = io::stdin();
        let reader = stdin.lock();

        for line in reader.lines().map_while(Result::ok) {
            let input = line.trim();
            if input.is_empty() {
                continue;
            }

            if let Some(stripped) = input.strip_prefix('/') {
                let _ = tx.send(BotCommand::Command(stripped.to_string()));
            } else {
                let _ = tx.send(BotCommand::Chat(input.to_string()));
            }
        }
    });

    // Run bot
    let local = LocalSet::new();
    local.spawn_local(async move {
        let _ = run_bot(args, event_bus.cmd_tx, event_bus.ui_tx).await;
    });

    local.await;

    Ok(())
}
