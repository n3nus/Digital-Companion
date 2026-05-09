pub mod assets;
pub mod config;
pub mod gesture;
pub mod pet;
pub mod render;

pub use assets::{AnimationId, AnimationSpec, AssetManifest, HitZone, Rect, SpriteSheet};
pub use config::AppConfig;
pub use gesture::{GestureEvent, GestureTracker};
pub use pet::{Bounds, HeartParticle, PetBrain, PetMood, PetSnapshot};
pub use render::Surface;

