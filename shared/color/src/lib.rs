use std::convert::TryFrom;

use common::cgmath::num_traits::clamp;
use common::{Rng, RngCore};
use std::ops::Mul;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ColorRgb {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Copy, Clone, Debug)]
struct ColorHsl {
    h: f32,
    s: f32,
    l: f32,
}

impl ColorRgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Clamps inputs to 0-1
    pub fn new_float(r: f32, g: f32, b: f32) -> Self {
        let clamp = |f: f32| {
            let val = clamp(f, 0.0, 1.0);
            (val * 255.0).round() as u8
        };

        Self::new(clamp(r), clamp(g), clamp(b))
    }

    pub fn unique_randoms(
        saturation: f32,
        luminance: f32,
        randy: &mut dyn RngCore,
    ) -> Option<UniqueRandomColors> {
        let range = 0.0f32..=1.0;
        if range.contains(&saturation) && range.contains(&luminance) {
            Some(UniqueRandomColors {
                next_hue: randy.gen_range(0.0, 1.0),
                s: saturation,
                l: luminance,
            })
        } else {
            None
        }
    }

    pub fn new_hsl(hue: f32, saturation: f32, luminance: f32) -> Self {
        ColorHsl {
            h: hue,
            s: saturation,
            l: luminance,
        }
        .into()
    }

    pub fn array_with_alpha(self, alpha: u8) -> [u8; 4] {
        [self.r, self.g, self.b, alpha]
    }
}

pub struct UniqueRandomColors {
    next_hue: f32,
    s: f32,
    l: f32,
}

impl UniqueRandomColors {
    /// Iterator.next but can't fail
    pub fn next_please(&mut self) -> ColorRgb {
        // iterator is infinite
        self.next().unwrap()
    }
}

impl Iterator for UniqueRandomColors {
    type Item = ColorRgb;

    fn next(&mut self) -> Option<Self::Item> {
        let hue = self.next_hue;
        let color = ColorHsl {
            h: hue,
            s: self.s,
            l: self.l,
        };

        // prepare for next
        self.next_hue = (self.next_hue + (137.5077 / 360.0/* golden angle */)) % 1.0;

        Some(color.into())
    }
}

impl From<ColorRgb> for [f32; 3] {
    fn from(c: ColorRgb) -> Self {
        [
            f32::from(c.r) / 255.0,
            f32::from(c.g) / 255.0,
            f32::from(c.b) / 255.0,
        ]
    }
}

impl From<ColorRgb> for (u8, u8, u8) {
    fn from(c: ColorRgb) -> Self {
        (c.r, c.g, c.b)
    }
}

impl From<ColorRgb> for [u8; 3] {
    fn from(c: ColorRgb) -> Self {
        [c.r, c.g, c.b]
    }
}

impl From<ColorRgb> for [u8; 4] {
    fn from(c: ColorRgb) -> Self {
        c.array_with_alpha(255)
    }
}

/// Includes alpha value of 255
/// TODO will this work with big endian?
impl From<ColorRgb> for u32 {
    fn from(c: ColorRgb) -> Self {
        let rgba: [u8; 4] = [c.r, c.g, c.b, u8::MAX];
        u32::from_ne_bytes(rgba)
    }
}

impl From<u32> for ColorRgb {
    fn from(int: u32) -> Self {
        let [r, g, b, _]: [u8; 4] = int.to_be_bytes();
        Self::new(r, g, b)
    }
}

impl TryFrom<&[f32]> for ColorRgb {
    type Error = ();

    fn try_from(slice: &[f32]) -> Result<Self, Self::Error> {
        if slice.len() == 3 {
            Ok(Self::new_float(slice[0], slice[1], slice[2]))
        } else {
            Err(())
        }
    }
}

impl From<(u8, u8, u8)> for ColorRgb {
    fn from(tup: (u8, u8, u8)) -> Self {
        let (r, g, b) = tup;
        Self { r, g, b }
    }
}

impl Mul<f32> for ColorRgb {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            r: ((self.r as f32) * rhs) as u8,
            g: ((self.g as f32) * rhs) as u8,
            b: ((self.b as f32) * rhs) as u8,
        }
    }
}

impl From<ColorHsl> for ColorRgb {
    #[allow(clippy::many_single_char_names)]
    /// https://stackoverflow.com/a/9493060
    fn from(c: ColorHsl) -> Self {
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
            if c.s == 0.0 {
                (c.l, c.l, c.l) // acromatic
            } else {
                let q = if c.l < 0.5 {
                    c.l * (1.0 + c.s)
                } else {
                    c.l + c.s - c.l * c.s
                };
                let p = 2.0 * c.l - q;
                let r = hue_to_rgb(p, q, c.h + 1.0 / 3.0);
                let g = hue_to_rgb(p, q, c.h);
                let b = hue_to_rgb(p, q, c.h - 1.0 / 3.0);
                (r, g, b)
            }
        };

        Self::new_float(r, g, b)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::many_single_char_names)]

    use crate::ColorHsl;
    use crate::ColorRgb;
    use common::{random, Itertools};
    use std::convert::TryFrom;

    #[test]
    fn accurate_conversion() {
        let a = ColorRgb::new(200, 12, 49);
        let b: (u8, u8, u8) = a.into();
        let c: ColorRgb = b.into();
        let d: [f32; 3] = a.into();
        let e = ColorRgb::try_from(&d as &[f32]).unwrap();

        assert_eq!(a, c);
        assert_eq!(c, e);
    }

    #[test]
    fn try_convert_from_slice() {
        let a = [1.0f32, 0.0, 1.0];
        let b = [0.0f32];
        let c = [0.0f32; 100];

        assert_eq!(
            ColorRgb::try_from(&a as &[f32]),
            Ok(ColorRgb::new(255, 0, 255))
        );
        assert_eq!(ColorRgb::try_from(&b as &[f32]), Err(()));
        assert_eq!(ColorRgb::try_from(&c as &[f32]), Err(()));
    }

    #[test]
    fn hsl_to_rgb() {
        // random colors from wikipedia
        assert_eq!(
            ColorRgb::from(ColorHsl {
                h: 0.0397,
                s: 0.817,
                l: 0.624
            }),
            ColorRgb::new_float(0.931, 0.463, 0.316)
        );
        assert_eq!(
            ColorRgb::from(ColorHsl {
                h: 0.667,
                s: 0.29,
                l: 0.608
            }),
            ColorRgb::new_float(0.495, 0.493, 0.721)
        );
    }

    #[test]
    fn random_uniques() {
        let mut randy = random::get();
        assert!(ColorRgb::unique_randoms(2.2, -0.8, &mut *randy).is_none());

        let uniques = ColorRgb::unique_randoms(0.2, 0.8, &mut *randy)
            .unwrap()
            .take(50)
            .collect_vec();
        assert_eq!(uniques.len(), 50);
        for (a, b) in uniques.iter().tuple_windows() {
            assert_ne!(a, b);
        }
    }
}
