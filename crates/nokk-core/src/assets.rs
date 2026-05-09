use std::collections::BTreeMap;
use std::fmt;
use std::io::Cursor;

use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum AssetError {
    Manifest(String),
    Png(String),
    InvalidSheet(String),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(message) => write!(f, "asset manifest error: {message}"),
            Self::Png(message) => write!(f, "png decode error: {message}"),
            Self::InvalidSheet(message) => write!(f, "invalid spritesheet: {message}"),
        }
    }
}

impl std::error::Error for AssetError {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitZone {
    pub name: String,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameRef {
    pub index: u16,
    pub duration_ms: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationSpec {
    pub frames: Vec<FrameRef>,
    #[serde(default)]
    pub looped: bool,
}

impl AnimationSpec {
    pub fn frame_at(&self, elapsed_ms: u64) -> Option<u16> {
        if self.frames.is_empty() {
            return None;
        }

        let total = self.total_duration_ms();
        let mut cursor = if self.looped && total > 0 {
            elapsed_ms % total
        } else {
            elapsed_ms.min(total.saturating_sub(1))
        };

        for frame in &self.frames {
            let duration = u64::from(frame.duration_ms.max(1));
            if cursor < duration {
                return Some(frame.index);
            }
            cursor = cursor.saturating_sub(duration);
        }

        self.frames.last().map(|frame| frame.index)
    }

    pub fn total_duration_ms(&self) -> u64 {
        self.frames
            .iter()
            .map(|frame| u64::from(frame.duration_ms.max(1)))
            .sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationId {
    Idle,
    Blink,
    Walk,
    WalkDown,
    WalkUp,
    WalkLeft,
    WalkRight,
    Sit,
    Sleep,
    Happy,
    Poke,
    Dance,
}

impl AnimationId {
    pub fn as_key(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Blink => "blink",
            Self::Walk => "walk",
            Self::WalkDown => "walk_down",
            Self::WalkUp => "walk_up",
            Self::WalkLeft => "walk_left",
            Self::WalkRight => "walk_right",
            Self::Sit => "sit",
            Self::Sleep => "sleep",
            Self::Happy => "happy",
            Self::Poke => "poke",
            Self::Dance => "dance",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssetManifest {
    pub frame_size: u32,
    pub sheet_columns: u32,
    pub animations: BTreeMap<String, AnimationSpec>,
    pub hit_zones: Vec<HitZone>,
    pub heart_spawn_points: Vec<Point>,
    pub heart_frames: Vec<u16>,
}

impl AssetManifest {
    pub fn from_ron(input: &str) -> Result<Self, AssetError> {
        let manifest: Self =
            ron::from_str(input).map_err(|err| AssetError::Manifest(err.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn animation(&self, id: AnimationId) -> &AnimationSpec {
        self.animations
            .get(id.as_key())
            .unwrap_or_else(|| panic!("missing required animation {}", id.as_key()))
    }

    pub fn hit_zone(&self, name: &str) -> Option<Rect> {
        self.hit_zones
            .iter()
            .find(|zone| zone.name == name)
            .map(|zone| zone.rect)
    }

    pub fn is_stroke_zone(&self, x: i32, y: i32) -> bool {
        self.hit_zones
            .iter()
            .any(|zone| matches!(zone.name.as_str(), "head" | "back") && zone.rect.contains(x, y))
    }

    pub fn is_body_zone(&self, x: i32, y: i32) -> bool {
        self.hit_zones
            .iter()
            .any(|zone| zone.name == "body" && zone.rect.contains(x, y))
    }

    pub fn validate(&self) -> Result<(), AssetError> {
        if self.frame_size == 0 {
            return Err(AssetError::Manifest("frame_size must be greater than zero".into()));
        }
        if self.sheet_columns == 0 {
            return Err(AssetError::Manifest("sheet_columns must be greater than zero".into()));
        }

        for required in [
            AnimationId::Idle,
            AnimationId::Blink,
            AnimationId::Walk,
            AnimationId::WalkDown,
            AnimationId::WalkUp,
            AnimationId::WalkLeft,
            AnimationId::WalkRight,
            AnimationId::Sit,
            AnimationId::Sleep,
            AnimationId::Happy,
            AnimationId::Poke,
            AnimationId::Dance,
        ] {
            let Some(animation) = self.animations.get(required.as_key()) else {
                return Err(AssetError::Manifest(format!(
                    "missing required animation {}",
                    required.as_key()
                )));
            };
            if animation.frames.is_empty() {
                return Err(AssetError::Manifest(format!(
                    "animation {} has no frames",
                    required.as_key()
                )));
            }
        }

        for required in ["head", "back", "body"] {
            if self.hit_zone(required).is_none() {
                return Err(AssetError::Manifest(format!(
                    "missing required hit zone {required}"
                )));
            }
        }

        if self.heart_spawn_points.is_empty() {
            return Err(AssetError::Manifest(
                "at least one heart spawn point is required".into(),
            ));
        }
        if self.heart_frames.is_empty() {
            return Err(AssetError::Manifest(
                "at least one heart frame is required".into(),
            ));
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Debug)]
pub struct SpriteSheet {
    width: u32,
    height: u32,
    pixels: Vec<Rgba>,
    manifest: AssetManifest,
}

impl SpriteSheet {
    pub fn from_png_bytes(bytes: &[u8], manifest: AssetManifest) -> Result<Self, AssetError> {
        let decoder = png::Decoder::new(Cursor::new(bytes));
        let mut reader = decoder
            .read_info()
            .map_err(|err| AssetError::Png(err.to_string()))?;
        let mut buffer = vec![
            0;
            reader
                .output_buffer_size()
                .ok_or_else(|| AssetError::Png("unknown output buffer size".into()))?
        ];
        let info = reader
            .next_frame(&mut buffer)
            .map_err(|err| AssetError::Png(err.to_string()))?;

        if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
            return Err(AssetError::InvalidSheet(
                "spritesheet must be 8-bit RGBA PNG".into(),
            ));
        }

        let data = &buffer[..info.buffer_size()];
        let mut pixels = Vec::with_capacity((info.width * info.height) as usize);
        for px in data.chunks_exact(4) {
            pixels.push(Rgba {
                r: px[0],
                g: px[1],
                b: px[2],
                a: px[3],
            });
        }

        let sheet = Self {
            width: info.width,
            height: info.height,
            pixels,
            manifest,
        };
        sheet.validate_dimensions()?;
        Ok(sheet)
    }

    pub fn manifest(&self) -> &AssetManifest {
        &self.manifest
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn frame_size(&self) -> u32 {
        self.manifest.frame_size
    }

    pub fn frame_pixel(&self, frame_index: u16, x: u32, y: u32) -> Rgba {
        let frame_size = self.manifest.frame_size;
        let columns = self.manifest.sheet_columns;
        let frame_x = u32::from(frame_index) % columns;
        let frame_y = u32::from(frame_index) / columns;
        let sx = frame_x * frame_size + x;
        let sy = frame_y * frame_size + y;
        self.pixel(sx, sy)
    }

    pub fn pixel(&self, x: u32, y: u32) -> Rgba {
        if x >= self.width || y >= self.height {
            return Rgba::default();
        }
        self.pixels[(y * self.width + x) as usize]
    }

    fn validate_dimensions(&self) -> Result<(), AssetError> {
        let frame_size = self.manifest.frame_size;
        if self.width % frame_size != 0 || self.height % frame_size != 0 {
            return Err(AssetError::InvalidSheet(format!(
                "sheet dimensions {}x{} are not multiples of frame size {frame_size}",
                self.width, self.height
            )));
        }

        let rows = self.height / frame_size;
        let frame_count = rows * self.manifest.sheet_columns;
        for (name, animation) in &self.manifest.animations {
            for frame in &animation.frames {
                if u32::from(frame.index) >= frame_count {
                    return Err(AssetError::InvalidSheet(format!(
                        "animation {name} references missing frame {}",
                        frame.index
                    )));
                }
            }
        }
        for frame in &self.manifest.heart_frames {
            if u32::from(*frame) >= frame_count {
                return Err(AssetError::InvalidSheet(format!(
                    "heart frame {frame} does not exist"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looped_animation_wraps() {
        let animation = AnimationSpec {
            frames: vec![
                FrameRef {
                    index: 1,
                    duration_ms: 100,
                },
                FrameRef {
                    index: 2,
                    duration_ms: 100,
                },
            ],
            looped: true,
        };
        assert_eq!(animation.frame_at(0), Some(1));
        assert_eq!(animation.frame_at(150), Some(2));
        assert_eq!(animation.frame_at(250), Some(1));
    }

    #[test]
    fn non_looped_animation_holds_last_frame() {
        let animation = AnimationSpec {
            frames: vec![
                FrameRef {
                    index: 4,
                    duration_ms: 50,
                },
                FrameRef {
                    index: 5,
                    duration_ms: 50,
                },
            ],
            looped: false,
        };
        assert_eq!(animation.frame_at(999), Some(5));
    }

    #[test]
    fn generated_assets_validate_and_decode() {
        let manifest =
            AssetManifest::from_ron(include_str!("../../../assets/nokk/manifest.ron")).unwrap();
        let sheet = SpriteSheet::from_png_bytes(
            include_bytes!("../../../assets/nokk/spritesheet.png"),
            manifest,
        )
        .unwrap();
        assert_eq!(sheet.frame_size(), 192);
        assert_eq!(sheet.width(), 1536);
        assert_eq!(sheet.height(), 1920);
    }
}
