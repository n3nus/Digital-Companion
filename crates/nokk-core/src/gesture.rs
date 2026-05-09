use std::collections::VecDeque;

use crate::assets::AssetManifest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureEvent {
    Stroked,
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    x: i32,
    y: i32,
    at_ms: u64,
}

#[derive(Clone, Debug)]
pub struct GestureTracker {
    samples: VecDeque<Sample>,
    last_stroke_ms: u64,
}

impl Default for GestureTracker {
    fn default() -> Self {
        Self {
            samples: VecDeque::new(),
            last_stroke_ms: 0,
        }
    }
}

impl GestureTracker {
    pub fn pointer_moved(
        &mut self,
        x: i32,
        y: i32,
        at_ms: u64,
        manifest: &AssetManifest,
    ) -> Option<GestureEvent> {
        if !manifest.is_stroke_zone(x, y) {
            self.samples.clear();
            return None;
        }

        self.samples.push_back(Sample { x, y, at_ms });
        while self
            .samples
            .front()
            .is_some_and(|sample| at_ms.saturating_sub(sample.at_ms) > 900)
        {
            self.samples.pop_front();
        }

        if at_ms.saturating_sub(self.last_stroke_ms) < 1_200 {
            return None;
        }

        if self.samples.len() < 6 {
            return None;
        }

        let mut distance = 0.0_f32;
        let mut direction_changes = 0;
        let mut last_sign = 0;
        let mut previous: Option<Sample> = None;
        for sample in &self.samples {
            if let Some(previous) = previous {
                let dx = sample.x - previous.x;
                let dy = sample.y - previous.y;
                distance += ((dx * dx + dy * dy) as f32).sqrt();
                let sign = dx.signum();
                if sign != 0 && last_sign != 0 && sign != last_sign {
                    direction_changes += 1;
                }
                if sign != 0 {
                    last_sign = sign;
                }
            }
            previous = Some(*sample);
        }

        if distance >= 24.0 && direction_changes >= 1 {
            self.last_stroke_ms = at_ms;
            self.samples.clear();
            Some(GestureEvent::Stroked)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::assets::{AnimationSpec, FrameRef, HitZone, Point, Rect};

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
                        x: 10,
                        y: 5,
                        w: 28,
                        h: 20,
                    },
                },
                HitZone {
                    name: "back".into(),
                    rect: Rect {
                        x: 8,
                        y: 22,
                        w: 32,
                        h: 10,
                    },
                },
                HitZone {
                    name: "body".into(),
                    rect: Rect {
                        x: 8,
                        y: 16,
                        w: 32,
                        h: 28,
                    },
                },
            ],
            heart_spawn_points: vec![Point { x: 24, y: 12 }],
            heart_frames: vec![24],
        }
    }

    #[test]
    fn stroke_requires_repeated_motion_inside_stroke_zone() {
        let manifest = manifest();
        let mut tracker = GestureTracker::default();
        let mut event = None;
        for (i, x) in [12, 19, 26, 18, 11, 22].into_iter().enumerate() {
            event = tracker.pointer_moved(x, 12, 10_000 + i as u64 * 80, &manifest);
        }
        assert_eq!(event, Some(GestureEvent::Stroked));
    }

    #[test]
    fn outside_motion_resets_stroke() {
        let manifest = manifest();
        let mut tracker = GestureTracker::default();
        for (i, x) in [12, 19, 26].into_iter().enumerate() {
            tracker.pointer_moved(x, 12, 10_000 + i as u64 * 80, &manifest);
        }
        assert_eq!(tracker.pointer_moved(45, 45, 10_400, &manifest), None);
        assert_eq!(tracker.pointer_moved(15, 12, 10_480, &manifest), None);
    }
}
