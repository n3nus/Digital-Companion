use crate::assets::{Rgba, SpriteSheet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Surface {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Surface {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    pub fn blit_frame(&mut self, sheet: &SpriteSheet, frame: u16, x: i32, y: i32, scale: u32) {
        let frame_size = sheet.frame_size();
        let scale = scale.max(1);

        for sy in 0..frame_size {
            for sx in 0..frame_size {
                let src = sheet.frame_pixel(frame, sx, sy);
                if src.a == 0 {
                    continue;
                }
                let dx0 = x + (sx * scale) as i32;
                let dy0 = y + (sy * scale) as i32;
                for oy in 0..scale {
                    for ox in 0..scale {
                        self.blend_pixel(dx0 + ox as i32, dy0 + oy as i32, src);
                    }
                }
            }
        }
    }

    pub fn blit_frame_with_alpha(
        &mut self,
        sheet: &SpriteSheet,
        frame: u16,
        x: i32,
        y: i32,
        scale: u32,
        alpha: u8,
    ) {
        let frame_size = sheet.frame_size();
        let scale = scale.max(1);

        for sy in 0..frame_size {
            for sx in 0..frame_size {
                let mut src = sheet.frame_pixel(frame, sx, sy);
                src.a = ((u16::from(src.a) * u16::from(alpha)) / 255) as u8;
                if src.a == 0 {
                    continue;
                }
                let dx0 = x + (sx * scale) as i32;
                let dy0 = y + (sy * scale) as i32;
                for oy in 0..scale {
                    for ox in 0..scale {
                        self.blend_pixel(dx0 + ox as i32, dy0 + oy as i32, src);
                    }
                }
            }
        }
    }

    pub fn blend_pixel(&mut self, x: i32, y: i32, src: Rgba) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 || src.a == 0 {
            return;
        }

        let index = ((y as u32 * self.width + x as u32) * 4) as usize;
        let sa = u16::from(src.a);
        let inv = 255 - sa;

        let sr = premul(src.r, src.a);
        let sg = premul(src.g, src.a);
        let sb = premul(src.b, src.a);

        let dr = u16::from(self.pixels[index]);
        let dg = u16::from(self.pixels[index + 1]);
        let db = u16::from(self.pixels[index + 2]);
        let da = u16::from(self.pixels[index + 3]);

        self.pixels[index] = (sr + dr * inv / 255).min(255) as u8;
        self.pixels[index + 1] = (sg + dg * inv / 255).min(255) as u8;
        self.pixels[index + 2] = (sb + db * inv / 255).min(255) as u8;
        self.pixels[index + 3] = (sa + da * inv / 255).min(255) as u8;
    }

    pub fn as_argb8888_native_endian(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len());
        for px in self.pixels.chunks_exact(4) {
            let color = ((u32::from(px[3])) << 24)
                | ((u32::from(px[0])) << 16)
                | ((u32::from(px[1])) << 8)
                | u32::from(px[2]);
            out.extend_from_slice(&color.to_ne_bytes());
        }
        out
    }
}

fn premul(channel: u8, alpha: u8) -> u16 {
    u16::from(channel) * u16::from(alpha) / 255
}

#[cfg(test)]
mod tests {
    use crate::assets::Rgba;

    use super::*;

    #[test]
    fn transparent_clear_sets_all_zero() {
        let mut surface = Surface::new(4, 4);
        surface.pixels.fill(255);
        surface.clear();
        assert!(surface.pixels.iter().all(|value| *value == 0));
    }

    #[test]
    fn alpha_blend_uses_premultiplied_output() {
        let mut surface = Surface::new(1, 1);
        surface.blend_pixel(
            0,
            0,
            Rgba {
                r: 100,
                g: 50,
                b: 0,
                a: 128,
            },
        );
        assert_eq!(surface.pixels[3], 128);
        assert!(surface.pixels[0] <= 51);
        assert!(surface.pixels[1] <= 26);
    }

    #[test]
    fn particle_lifetime_fades() {
        let mut particle = crate::pet::HeartParticle {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            age_ms: 0,
            lifetime_ms: 100,
            frame: 0,
        };
        assert_eq!(particle.alpha(), 255);
        particle.tick(50);
        assert!(particle.alpha() < 255);
        assert!(particle.alive());
        particle.tick(60);
        assert!(!particle.alive());
    }
}

