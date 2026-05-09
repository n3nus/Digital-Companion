mod app_assets;
mod console;
mod desktop;

use anyhow::{bail, Result};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("desktop") => desktop::run(),
        Some("console") => console::run(),
        Some("--help") | Some("-h") => {
            println!("Nøkk\n\nUsage:\n  nokk [desktop]\n  nokk console");
            Ok(())
        }
        Some(command) => bail!("unknown command {command:?}; use `nokk`, `nokk desktop`, or `nokk console`"),
    }
}

