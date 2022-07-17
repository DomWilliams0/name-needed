use misc::cgmath::num_traits::clamp;
use misc::{NormalizedFloat, Rng, RngCore};
use std::convert::TryFrom;
use std::ops::Mul;

/// RGBA
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Color([u8; 4]);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, u8::MAX)
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    /// Clamps inputs to 0-1
    pub fn rgb_f(r: f32, g: f32, b: f32) -> Self {
        Self::rgba_f(r, g, b, 1.0)
    }

    /// Clamps inputs to 0-1
    pub fn rgba_f(r: f32, g: f32, b: f32, a: f32) -> Self {
        let clamp = |f: f32| {
            let val = clamp(f, 0.0, 1.0);
            (val * 255.0).round() as u8
        };

        Self::rgba(clamp(r), clamp(g), clamp(b), clamp(a))
    }

    pub fn unique_randoms(
        saturation: NormalizedFloat,
        luminance: NormalizedFloat,
        randy: &mut dyn RngCore,
    ) -> impl Iterator<Item = Color> {
        UniqueRandomColors {
            next_hue: randy.gen_range(0.0, 1.0),
            s: saturation.value(),
            l: luminance.value(),
        }
    }

    pub fn hsl(hue: f32, saturation: f32, luminance: f32) -> Self {
        hsl_to_rgb(hue, saturation, luminance)
    }

    pub fn alpha(&mut self) -> &mut u8 {
        &mut self.0[3]
    }
}

impl From<Color> for [u8; 4] {
    fn from(c: Color) -> Self {
        c.0
    }
}

pub struct UniqueRandomColors {
    next_hue: f32,
    s: f32,
    l: f32,
}

impl Iterator for UniqueRandomColors {
    type Item = Color;

    fn next(&mut self) -> Option<Self::Item> {
        let hue = self.next_hue;

        // prepare for next
        self.next_hue = (self.next_hue + (137.5077 / 360.0/* golden angle */)) % 1.0;

        Some(Color::hsl(hue, self.s, self.l))
    }
}

impl From<Color> for [f32; 4] {
    fn from(c: Color) -> Self {
        let [r, g, b, a] = c.0;
        [
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            f32::from(a) / 255.0,
        ]
    }
}

impl From<Color> for u32 {
    fn from(c: Color) -> Self {
        u32::from_ne_bytes(c.0)
    }
}

impl From<u32> for Color {
    fn from(int: u32) -> Self {
        Self(int.to_be_bytes())
    }
}

impl TryFrom<&[f32]> for Color {
    type Error = ();

    fn try_from(slice: &[f32]) -> Result<Self, Self::Error> {
        match slice.len() {
            3 => Ok(Self::rgb_f(slice[0], slice[1], slice[2])),
            4 => Ok(Self::rgba_f(slice[0], slice[1], slice[2], slice[3])),
            _ => Err(()),
        }
    }
}

impl Mul<f32> for Color {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        let [r, g, b, a] = self.0;
        Self([
            ((r as f32) * rhs) as u8,
            ((g as f32) * rhs) as u8,
            ((b as f32) * rhs) as u8,
            a,
        ])
    }
}

#[allow(clippy::many_single_char_names)]
/// https://stackoverflow.com/a/9493060
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color {
    fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
        let t = match t {
            t if t < 0.0 => t + 1.0,
            t if t > 1.0 => t - 1.0,
            t => t,
        };
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 1.0 / 2.0 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    }
    let (r, g, b) = {
        if s == 0.0 {
            (l, l, l) // acromatic
        } else {
            let q = if l < 0.5 {
                l * (1.0 + s)
            } else {
                l + s - l * s
            };
            let p = 2.0 * l - q;
            let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
            let g = hue_to_rgb(p, q, h);
            let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
            (r, g, b)
        }
    };

    Color::rgb_f(r, g, b)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::many_single_char_names)]

    use super::*;
    use misc::{thread_rng, Itertools};

    #[test]
    fn try_convert_from_slice() {
        let a = [1.0f32, 0.0, 1.0];
        let b = [0.0f32];
        let c = [0.0f32; 100];
        let d = [1.0f32, 0.0, 1.0, 0.5];

        assert_eq!(Color::try_from(&a as &[f32]), Ok(Color::rgb(255, 0, 255)));
        assert_eq!(Color::try_from(&b as &[f32]), Err(()));
        assert_eq!(Color::try_from(&c as &[f32]), Err(()));
        assert_eq!(
            Color::try_from(&d as &[f32]),
            Ok(Color::rgba(255, 0, 255, 128))
        );
    }

    #[test]
    fn hsl_to_rgb() {
        // random colors from wikipedia
        assert_eq!(
            Color::hsl(0.0397, 0.817, 0.624),
            Color::rgb_f(0.931, 0.463, 0.316)
        );
        assert_eq!(
            Color::hsl(0.667, 0.29, 0.608),
            Color::rgb_f(0.495, 0.493, 0.721)
        );
    }

    #[test]
    fn random_uniques() {
        let mut randy = thread_rng();

        let uniques = Color::unique_randoms(
            NormalizedFloat::new(0.2),
            NormalizedFloat::new(0.8),
            &mut randy,
        )
        .take(50)
        .collect_vec();
        assert_eq!(uniques.len(), 50);
        for (a, b) in uniques.iter().tuple_windows() {
            assert_ne!(a, b);
        }
    }
}
