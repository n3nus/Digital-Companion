pub mod app_assets;
pub mod console;
pub mod desktop;

use anyhow::{Result, bail};

pub fn run_cli(mut args: impl Iterator<Item = String>) -> Result<()> {
    match args.next().as_deref() {
        None | Some("desktop") => desktop::run(),
        Some("console") => console::run(),
        Some("--help") | Some("-h") => {
            println!("Nøkk\n\nUsage:\n  nokk [desktop]\n  nokk console\n  nokk-console");
            Ok(())
        }
        Some(command) => {
            bail!("unknown command {command:?}; use `nokk`, `nokk desktop`, or `nokk console`")
        }
    }
}
