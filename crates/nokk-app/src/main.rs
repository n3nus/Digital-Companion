#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use anyhow::Result;

fn main() -> Result<()> {
    nokk::run_cli(std::env::args().skip(1))
}
