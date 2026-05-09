use anyhow::{Context, Result};
use nokk_core::{AssetManifest, SpriteSheet};

pub const MANIFEST_RON: &str = include_str!("../../../assets/nokk/manifest.ron");
pub const SPRITESHEET_PNG: &[u8] = include_bytes!("../../../assets/nokk/spritesheet.png");

pub fn load_sprites() -> Result<SpriteSheet> {
    let manifest = AssetManifest::from_ron(MANIFEST_RON).context("load Nøkk asset manifest")?;
    SpriteSheet::from_png_bytes(SPRITESHEET_PNG, manifest).context("load Nøkk spritesheet")
}

