use anyhow::Result;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub fn run() -> Result<()> {
    linux::run()
}

#[cfg(target_os = "windows")]
pub fn run() -> Result<()> {
    windows::run()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn run() -> Result<()> {
    anyhow::bail!("Nøkk desktop V1 supports Linux/Wayland and Windows; use `nokk console` here")
}

