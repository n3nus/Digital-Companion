use serde::{Deserialize, Serialize};

use crate::assets::{AnimationId, AssetManifest};

const FIRST_AMBIENT_MIN_MS: u64 = 1_200;
const FIRST_AMBIENT_MAX_MS: u64 = 2_800;
const AMBIENT_MIN_MS: u64 = 5_000;
const AMBIENT_MAX_MS: u64 = 13_000;
const WALK_MIN_MS: u64 = 3_500;
const WALK_MAX_MS: u64 = 7_000;
const WALK_VELOCITY_PX_PER_SEC: i32 = 58;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetMood {
    Calm,
    Happy,
    Sleepy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub width: i32,
    pub height: i32,
    pub pet_size: i32,
}

impl Bounds {
    fn clamp_x(&self, x: i32) -> i32 {
        x.clamp(0, (self.width - self.pet_size).max(0))
    }

    fn clamp_y(&self, y: i32) -> i32 {
        y.clamp(0, (self.height - self.pet_size).max(0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PetSnapshot {
    pub x: i32,
    pub y: i32,
    pub animation: AnimationId,
    pub mood: PetMood,
    pub facing: i32,
    pub paused: bool,
    pub animation_started_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeartParticle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub age_ms: u64,
    pub lifetime_ms: u64,
    pub frame: u16,
}

impl HeartParticle {
    pub fn tick(&mut self, dt_ms: u64) {
        self.age_ms = self.age_ms.saturating_add(dt_ms);
        let dt = dt_ms as f32 / 1000.0;
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        self.vy -= 8.0 * dt;
    }

    pub fn alive(&self) -> bool {
        self.age_ms < self.lifetime_ms
    }

    pub fn alpha(&self) -> u8 {
        if self.lifetime_ms == 0 {
            return 0;
        }
        let remaining = self.lifetime_ms.saturating_sub(self.age_ms);
        ((remaining * 255) / self.lifetime_ms) as u8
    }
}

#[derive(Clone, Debug)]
pub struct PetBrain {
    snapshot: PetSnapshot,
    rng: TinyRng,
    next_ambient_ms: u64,
    action_until_ms: u64,
    poke_cooldown_until_ms: u64,
    last_tick_ms: Option<u64>,
    walk_velocity: i32,
    walk_dx: i32,
    walk_dy: i32,
    first_ambient: bool,
    particles: Vec<HeartParticle>,
}

impl Default for PetBrain {
    fn default() -> Self {
        Self::new(0x4e6f6b6b)
    }
}

impl PetBrain {
    pub fn new(seed: u64) -> Self {
        let mut rng = TinyRng::new(seed);
        let next_ambient_ms = rng.range(FIRST_AMBIENT_MIN_MS, FIRST_AMBIENT_MAX_MS);
        Self {
            snapshot: PetSnapshot {
                x: 32,
                y: 32,
                animation: AnimationId::Idle,
                mood: PetMood::Calm,
                facing: 1,
                paused: false,
                animation_started_ms: 0,
            },
            rng,
            next_ambient_ms,
            action_until_ms: 0,
            poke_cooldown_until_ms: 0,
            last_tick_ms: None,
            walk_velocity: WALK_VELOCITY_PX_PER_SEC,
            walk_dx: 1,
            walk_dy: 0,
            first_ambient: true,
            particles: Vec::new(),
        }
    }

    pub fn from_config(
        seed: u64,
        position: Option<(i32, i32)>,
        animation: AnimationId,
        mood: PetMood,
        paused: bool,
    ) -> Self {
        let mut brain = Self::new(seed);
        if let Some((x, y)) = position {
            brain.snapshot.x = x;
            brain.snapshot.y = y;
        }
        brain.snapshot.animation = animation;
        brain.snapshot.mood = mood;
        brain.snapshot.paused = paused;
        brain.apply_direction_for_animation(animation);
        brain
    }

    pub fn snapshot(&self) -> PetSnapshot {
        self.snapshot
    }

    pub fn particles(&self) -> &[HeartParticle] {
        &self.particles
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.snapshot.paused = paused;
    }

    pub fn toggle_paused(&mut self) {
        self.snapshot.paused = !self.snapshot.paused;
    }

    pub fn reset_position(&mut self, bounds: Bounds) {
        self.snapshot.x = bounds.clamp_x(24);
        self.snapshot.y = bounds.clamp_y(bounds.height - bounds.pet_size - 24);
    }

    pub fn tick(&mut self, now_ms: u64, bounds: Bounds) {
        let dt_ms = self
            .last_tick_ms
            .map(|last| now_ms.saturating_sub(last))
            .unwrap_or(16)
            .min(250);
        self.last_tick_ms = Some(now_ms);

        for particle in &mut self.particles {
            particle.tick(dt_ms);
        }
        self.particles.retain(HeartParticle::alive);

        if self.snapshot.paused {
            return;
        }

        if now_ms >= self.action_until_ms
            && matches!(
                self.snapshot.animation,
                AnimationId::Blink
                    | AnimationId::Walk
                    | AnimationId::WalkDown
                    | AnimationId::WalkUp
                    | AnimationId::WalkLeft
                    | AnimationId::WalkRight
                    | AnimationId::Sit
                    | AnimationId::Sleep
                    | AnimationId::Happy
                    | AnimationId::Poke
                    | AnimationId::Dance
            )
        {
            self.set_animation(AnimationId::Idle, now_ms);
            self.snapshot.mood = PetMood::Calm;
        }

        if now_ms >= self.next_ambient_ms && now_ms >= self.action_until_ms {
            self.pick_ambient(now_ms);
        }

        if self.is_walking() {
            let step = (self.walk_velocity as i64 * dt_ms as i64 / 1000) as i32;
            self.snapshot.x += step * self.walk_dx;
            self.snapshot.y += step * self.walk_dy;

            if self.snapshot.x <= 0 || self.snapshot.x >= bounds.width - bounds.pet_size {
                self.walk_dx *= -1;
                self.snapshot.facing = self.walk_dx.signum();
                self.snapshot.x = bounds.clamp_x(self.snapshot.x);
                self.apply_walk_animation_for_direction(now_ms);
            }
            if self.snapshot.y <= 0 || self.snapshot.y >= bounds.height - bounds.pet_size {
                self.walk_dy *= -1;
                self.snapshot.y = bounds.clamp_y(self.snapshot.y);
                self.apply_walk_animation_for_direction(now_ms);
            }
        } else {
            self.snapshot.x = bounds.clamp_x(self.snapshot.x);
            self.snapshot.y = bounds.clamp_y(self.snapshot.y);
        }
    }

    pub fn poke(&mut self, now_ms: u64) -> bool {
        if now_ms < self.poke_cooldown_until_ms {
            return false;
        }
        self.set_animation(AnimationId::Poke, now_ms);
        self.snapshot.mood = PetMood::Happy;
        self.action_until_ms = now_ms + 900;
        self.poke_cooldown_until_ms = now_ms + 1_200;
        true
    }

    pub fn stroke(&mut self, now_ms: u64, manifest: &AssetManifest) -> usize {
        self.set_animation(AnimationId::Happy, now_ms);
        self.snapshot.mood = PetMood::Happy;
        self.action_until_ms = now_ms + 1_800;
        let count = self.rng.range(3, 6) as usize;
        self.spawn_hearts(count, manifest);
        count
    }

    pub fn current_frame(&self, manifest: &AssetManifest, now_ms: u64) -> u16 {
        let animation = manifest.animation(self.snapshot.animation);
        let elapsed = now_ms.saturating_sub(self.snapshot.animation_started_ms);
        animation.frame_at(elapsed).unwrap_or(0)
    }

    fn pick_ambient(&mut self, now_ms: u64) {
        if self.first_ambient {
            self.first_ambient = false;
            let animation = self.pick_walk_direction();
            self.set_animation(animation, now_ms);
            self.snapshot.mood = PetMood::Calm;
            self.action_until_ms = now_ms + self.rng.range(WALK_MIN_MS, WALK_MAX_MS);
            self.next_ambient_ms = self.next_ambient_time(now_ms);
            return;
        }

        let roll = self.rng.range(0, 100);
        let (animation, duration, mood) = match roll {
            0..=31 => {
                let animation = self.pick_walk_direction();
                (
                    animation,
                    self.rng.range(WALK_MIN_MS, WALK_MAX_MS),
                    PetMood::Calm,
                )
            }
            32..=55 => (AnimationId::Idle, 2_000, PetMood::Calm),
            56..=73 => (AnimationId::Sit, 4_500, PetMood::Calm),
            74..=86 => (AnimationId::Blink, 700, PetMood::Calm),
            87..=94 => (AnimationId::Dance, 2_800, PetMood::Happy),
            _ => (AnimationId::Sleep, 6_500, PetMood::Sleepy),
        };

        self.set_animation(animation, now_ms);
        self.snapshot.mood = mood;
        self.action_until_ms = now_ms + duration;
        self.next_ambient_ms = self.next_ambient_time(now_ms);
    }

    fn next_ambient_time(&mut self, now_ms: u64) -> u64 {
        now_ms + self.rng.range(AMBIENT_MIN_MS, AMBIENT_MAX_MS)
    }

    fn set_animation(&mut self, animation: AnimationId, now_ms: u64) {
        if self.snapshot.animation != animation {
            self.snapshot.animation = animation;
            self.snapshot.animation_started_ms = now_ms;
        }
    }

    fn is_walking(&self) -> bool {
        matches!(
            self.snapshot.animation,
            AnimationId::Walk
                | AnimationId::WalkDown
                | AnimationId::WalkUp
                | AnimationId::WalkLeft
                | AnimationId::WalkRight
        )
    }

    fn pick_walk_direction(&mut self) -> AnimationId {
        match self.rng.range(0, 4) {
            0 => {
                self.walk_dx = 0;
                self.walk_dy = 1;
                self.snapshot.facing = 1;
                AnimationId::WalkDown
            }
            1 => {
                self.walk_dx = 0;
                self.walk_dy = -1;
                self.snapshot.facing = -1;
                AnimationId::WalkUp
            }
            2 => {
                self.walk_dx = -1;
                self.walk_dy = 0;
                self.snapshot.facing = -1;
                AnimationId::WalkLeft
            }
            _ => {
                self.walk_dx = 1;
                self.walk_dy = 0;
                self.snapshot.facing = 1;
                AnimationId::WalkRight
            }
        }
    }

    fn apply_walk_animation_for_direction(&mut self, now_ms: u64) {
        let animation = if self.walk_dx < 0 {
            AnimationId::WalkLeft
        } else if self.walk_dx > 0 {
            AnimationId::WalkRight
        } else if self.walk_dy < 0 {
            AnimationId::WalkUp
        } else {
            AnimationId::WalkDown
        };
        self.set_animation(animation, now_ms);
    }

    fn apply_direction_for_animation(&mut self, animation: AnimationId) {
        match animation {
            AnimationId::Walk | AnimationId::WalkDown => {
                self.walk_dx = 0;
                self.walk_dy = 1;
                self.snapshot.facing = 1;
            }
            AnimationId::WalkUp => {
                self.walk_dx = 0;
                self.walk_dy = -1;
                self.snapshot.facing = -1;
            }
            AnimationId::WalkLeft => {
                self.walk_dx = -1;
                self.walk_dy = 0;
                self.snapshot.facing = -1;
            }
            AnimationId::WalkRight => {
                self.walk_dx = 1;
                self.walk_dy = 0;
                self.snapshot.facing = 1;
            }
            _ => {}
        }
    }

    fn spawn_hearts(&mut self, count: usize, manifest: &AssetManifest) {
        for i in 0..count {
            let spawn = manifest.heart_spawn_points
                [self.rng.range(0, manifest.heart_spawn_points.len() as u64) as usize];
            let frame = manifest.heart_frames
                [self.rng.range(0, manifest.heart_frames.len() as u64) as usize];
            let spread = self.rng.range(0, 21) as f32 - 10.0;
            self.particles.push(HeartParticle {
                x: self.snapshot.x as f32 + spawn.x as f32 + spread,
                y: self.snapshot.y as f32 + spawn.y as f32 - (i as f32 * 2.0),
                vx: spread * 0.45,
                vy: -18.0 - self.rng.range(0, 10) as f32,
                age_ms: 0,
                lifetime_ms: 1_300 + self.rng.range(0, 400),
                frame,
            });
        }
    }
}

#[derive(Clone, Debug)]
struct TinyRng {
    state: u64,
}

impl TinyRng {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn range(&mut self, min: u64, max: u64) -> u64 {
        debug_assert!(min < max);
        min + self.next() % (max - min)
    }
}

pub fn unix_time_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0x4e6f6b6b)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::assets::{AnimationSpec, AssetManifest, FrameRef, HitZone, Point, Rect};

    use super::*;

    fn manifest() -> AssetManifest {
        let mut animations = BTreeMap::new();
        for key in [
            "idle",
            "blink",
            "walk",
            "walk_down",
            "walk_up",
            "walk_left",
            "walk_right",
            "sit",
            "sleep",
            "happy",
            "poke",
            "dance",
        ] {
            animations.insert(
                key.to_string(),
                AnimationSpec {
                    frames: vec![FrameRef {
                        index: 0,
                        duration_ms: 100,
                    }],
                    looped: true,
                },
            );
        }
        AssetManifest {
            frame_size: 48,
            sheet_columns: 8,
            animations,
            hit_zones: vec![
                HitZone {
                    name: "head".into(),
                    rect: Rect {
                        x: 14,
                        y: 8,
                        w: 20,
                        h: 16,
                    },
                },
                HitZone {
                    name: "back".into(),
                    rect: Rect {
                        x: 10,
                        y: 22,
                        w: 28,
                        h: 10,
                    },
                },
                HitZone {
                    name: "body".into(),
                    rect: Rect {
                        x: 9,
                        y: 18,
                        w: 30,
                        h: 24,
                    },
                },
            ],
            heart_spawn_points: vec![Point { x: 24, y: 10 }],
            heart_frames: vec![24],
        }
    }

    #[test]
    fn ambient_interval_stays_in_requested_range() {
        let mut brain = PetBrain::new(1);
        let bounds = Bounds {
            width: 800,
            height: 600,
            pet_size: 144,
        };
        brain.next_ambient_ms = 1;
        brain.tick(1, bounds);
        assert!(brain.next_ambient_ms >= AMBIENT_MIN_MS + 1);
        assert!(brain.next_ambient_ms < AMBIENT_MAX_MS + 1);
    }

    #[test]
    fn first_ambient_is_scheduled_quickly() {
        let brain = PetBrain::new(1);
        assert!(brain.next_ambient_ms >= FIRST_AMBIENT_MIN_MS);
        assert!(brain.next_ambient_ms < FIRST_AMBIENT_MAX_MS);
    }

    #[test]
    fn first_ambient_starts_walking_and_moves_pet() {
        let mut brain = PetBrain::new(2);
        brain.snapshot.x = 220;
        brain.snapshot.y = 180;
        brain.next_ambient_ms = 1;
        let bounds = Bounds {
            width: 800,
            height: 600,
            pet_size: 144,
        };

        brain.tick(1, bounds);
        assert!(brain.is_walking());

        let start = (brain.snapshot.x, brain.snapshot.y);
        brain.tick(1_001, bounds);
        let end = (brain.snapshot.x, brain.snapshot.y);
        assert_ne!(start, end);
    }

    #[test]
    fn poke_has_cooldown_and_does_not_spawn_hearts() {
        let mut brain = PetBrain::new(2);
        assert!(brain.poke(100));
        assert!(!brain.poke(200));
        assert!(brain.particles().is_empty());
    }

    #[test]
    fn stroke_spawns_three_to_five_hearts() {
        let mut brain = PetBrain::new(3);
        let count = brain.stroke(100, &manifest());
        assert!((3..=5).contains(&count));
        assert_eq!(brain.particles().len(), count);
        assert_eq!(brain.snapshot().animation, AnimationId::Happy);
    }
}
