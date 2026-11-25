// Release builds use the Windows GUI subsystem, so no console window opens behind the app.
// Debug builds keep the console: `cargo run -- --tui` and log output still work there.
// Crate-level attribute: it must stay above every `use` and item in this file.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use omnimobile_toolkit::core::device_pool::DevicePool;
use omnimobile_toolkit::{start_gui_mode, start_tui_mode};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "kod", about = "Dual-device Android & iOS toolkit")]
struct CliArgs {
    /// Launch the terminal user interface instead of the GUI (debug builds only)
    #[arg(long)]
    tui: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = CliArgs::parse();
    let pool = Arc::new(DevicePool::new());

    if args.tui {
        // A release build has no console (see windows_subsystem above), so the TUI cannot draw.
        // Shipping the TUI with the installer is a backlog item (Phase 2a decision, option C).
        // cfg! keeps both branches type-checked in every build profile (CI clippy runs in debug).
        if !cfg!(debug_assertions) {
            return Err("--tui is only available in debug builds (cargo run -- --tui)".into());
        }
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(start_tui_mode(pool))?;
        return Ok(());
    }

    start_gui_mode(pool)?;
    Ok(())
}
