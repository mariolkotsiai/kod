use clap::Parser;
use omnimobile_toolkit::core::device_pool::DevicePool;
use omnimobile_toolkit::{start_gui_mode, start_tui_mode};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "omnimobile-toolkit", about = "Dual-device Android & iOS toolkit")]
struct CliArgs {
    /// Launch standalone terminal user interface instead of GUI
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
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(start_tui_mode(pool))?;
        return Ok(());
    }

    start_gui_mode(pool)?;
    Ok(())
}