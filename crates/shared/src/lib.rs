use std::sync::{Arc, Mutex};
use std::{fs, panic, path::Path};

use anyhow::Result;
use backtrace::Backtrace;
use chrono::prelude::*;
use hudhook::Hudhook;
use log::*;
use simplelog::{ColorChoice, CombinedLogger, Config, SharedLogger, TermLogger, TerminalMode, WriteLogger};
use windows::Win32::{Foundation::*, UI::WindowsAndMessaging::MessageBoxW};
use windows::core::*;

mod clipboard;
mod config;
mod core;
mod error_display;
mod game;
mod input_blocker;
mod overlay;
pub mod utils;

pub use core::*;
use error_display::*;
pub use game::*;
pub use input_blocker::*;

/// Handle panics by both logging and popping up a message box, which is the
/// most reliable way to make something visible to the end user.
pub fn handle_panics() {
    panic::set_hook(Box::new(|panic_info| {
        let mut message = String::new();
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            message.push_str(&format!("Rust panic: {s}"));
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            message.push_str(&format!("Rust panic: {s}"));
        } else {
            message.push_str(&format!("Rust panic: {:?}", panic_info.payload()));
        }

        message.push_str(&format!("\n{:?}", Backtrace::new()));

        error!("{}", message);
        message_box(message);
    }));
}

/// Displays a message box with the given message.
fn message_box(message: impl Into<String>) {
    unsafe {
        MessageBoxW(
            HWND(0),
            &HSTRING::from(message.into()),
            w!("DS3 Archipelago Client"),
            Default::default(),
        );
    }
}

/// Starts the logger which logs to both stdout and a file which users can send
/// to the devs for debugging.
pub fn start_logger(game: String) {
    let terminal_logger = TermLogger::new(
        LevelFilter::Warn, 
        Config::default(),
        TerminalMode::Mixed, 
        ColorChoice::Auto
    );
    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![terminal_logger];

    let dir = match utils::mod_directory() {
        Ok(path) => path,
        Err(why) => {
            warn!("Error locating mod directory {} using current dir as default", why);
            Path::new(".")
        },
    };

    let filename = format!("{}-archipelago-{}.log", game, Local::now().format("%Y-%m-%d"));

    if let Ok(file_logger) = create_file_logger(filename, dir) {
        loggers.push(file_logger);
    } else {
        warn!("Error creating file logger at {:?}", dir)
    }

    match CombinedLogger::init(loggers) {
        Ok(_) => info!("Logger initialized"),
        Err(why) => error!("Failed to initialize logger {}", why),
    }
}

/// Creates a write logger that writes to files in [dir].
fn create_file_logger(filename: String, dir: impl AsRef<Path>) -> Result<Box<WriteLogger<fs::File>>> {
    let dir = dir.as_ref().join("log");
    fs::create_dir_all(&dir)?;

    let logger = WriteLogger::new(
        LevelFilter::Info,
        Config::default(),
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(filename))?,
    );

    Ok(logger)
}

/// Initializes the basic hooks into the underlying rendering system for the
/// current mod.
pub fn initialize<G: Game>(hmodule: HINSTANCE, blocker: G::InputBlocker) {
    std::thread::spawn(move || {
        info!("Worker thread initialized.");

        let mut core = G::wait_for_system_init().and_then(|_| {
            info!("Game system initialized.");

            // This mutex isn't strictly necessary since in practice we're only
            // ever touching this on DS3's main thread. But Rust doesn't have
            // any way of knowing that and using a Mutex is simpler than
            // creating a newtype that implements Sync, so we do it anyway.
            // Because there won't be any contention, it should be very
            // inexpensive.
            G::Core::new().map(|core| Arc::new(Mutex::new(core)))
        });

        if let Ok(core2) = core.as_ref() {
            let core2 = core2.clone();
            // Safety: We're playing a little fast and loose here, not
            // scheduling the task on the main thread. It seems to work, but
            // really we should probably handle it in the error display.
            if let Err(err) = unsafe {
                G::run_recurring_task(move || core2.lock().unwrap().update(G::is_main_menu()))
            } {
                core = Err(err);
            }
        }

        if let Err(e) = Hudhook::builder()
            .with::<G::GraphicsHooks>(ErrorDisplay::<G>::new(core, blocker))
            .with_hmodule(hmodule)
            .build()
            .apply()
        {
            panic!("Couldn't apply hooks: {e:?}");
        }
    });
}
