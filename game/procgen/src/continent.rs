use crate::PlanetParams;
use common::*;
use std::f32::consts::PI;

pub struct Continent {
    pub pos: (i32, i32),
    pub radius: i32,
}

pub struct ContinentMap {
    size: i32,
    max_continents: usize,
    pub continents: Vec<Continent>,
    /// (continent idx, start idx, end idx (exclusive))
    continent_range: Vec<(ContinentIdx, usize, usize)>,
}

type ContinentIdx = usize;

const STARTING_RADIUS: f32 = 14.0;
const DECREMENT_RANGE: (f32, f32) = (0.4, 0.9);
const MIN_RADIUS: i32 = 2;

impl ContinentMap {
    pub fn new(params: &PlanetParams) -> Self {
        Self {
            size: params.planet_size as i32,
            max_continents: params.max_continents,

            continents: Vec::with_capacity(128),
            continent_range: Vec::with_capacity(params.max_continents),
        }
    }

    pub fn generate(&mut self, rando: &mut dyn RngCore) -> usize {
        macro_rules! new_decrement {
            () => {
                rando.gen_range(DECREMENT_RANGE.0, DECREMENT_RANGE.1)
            };
        }

        let mut radius = STARTING_RADIUS;
        let mut parent_start_idx = 0;
        let mut count: ContinentIdx = 0;
        let mut decrement = new_decrement!();

        loop {
            let mut next_continent_pls = false;

            let this_radius = radius.ceil() as i32;
            let mut this_pos = None; // initialized before use

            if this_radius <= MIN_RADIUS {
                next_continent_pls = true;
            } else {
                match self.place_new_continent(this_radius, parent_start_idx, rando) {
                    Some(pos) => {
                        this_pos = Some(pos);
                    }
                    None => {
                        next_continent_pls = true;
                    }
                }
            }

            if next_continent_pls {
                // this continent is finished
                self.continent_range
                    .push((count, parent_start_idx, self.continents.len()));

                count += 1;
                let blob_count = self.continents.len() - parent_start_idx;
                debug!("continent finished"; "index" => count, "blobs" => blob_count);

                if count >= self.max_continents {
                    // all done
                    break;
                }

                // prepare for next
                parent_start_idx = self.continents.len();
                radius = STARTING_RADIUS;
                decrement = new_decrement!();
                continue;
            }

            self.continents.push(Continent {
                pos: this_pos.expect("position not initialized"),
                radius: this_radius,
            });

            debug!("placing shape on continent"; "pos" => ?this_pos, "radius" => ?this_radius, "continent" => parent_start_idx);

            radius -= decrement;
        }

        count
    }

    fn place_new_continent(
        &self,
        radius: i32,
        parent_start_idx: usize,
        rando: &mut dyn RngCore,
    ) -> Option<(i32, i32)> {
        const MAX_ATTEMPTS: usize = 10;
        const MAX_ATTEMPTS_PER_PARENT: usize = 5;

        // assume parent continent is up to the end of the vec
        let max_parent = (self.continents.len()) as f32;
        let min_parent = parent_start_idx as f32;

        for _ in 0..MAX_ATTEMPTS {
            // choose a parent to attach to
            let parent_idx = if self.continents[parent_start_idx..].is_empty() {
                None
            } else {
                // later indices are more likely than early ones, i.e. attach to the branch shapes
                // more often than the big roots
                let r = (1.0 - rando.gen::<f32>()).sqrt();
                let idx = (min_parent + (r * (max_parent - min_parent))).floor() as usize;
                Some(idx)
            };

            for _ in 0..MAX_ATTEMPTS_PER_PARENT {
                let pos = match parent_idx {
                    None => {
                        // random
                        let x = rando.gen_range(radius, self.size - radius);
                        let y = rando.gen_range(radius, self.size - radius);
                        (x, y)
                    }
                    Some(idx) => {
                        // on parent circumference
                        let parent = &self.continents[idx];
                        let angle = rando.gen_range(0.0, PI * 2.0);
                        let parent_radius = parent.radius as f32 * 0.75;
                        let x = parent_radius * angle.cos();
                        let y = parent_radius * angle.sin();

                        (
                            parent.pos.0 + x.ceil() as i32,
                            parent.pos.1 + y.ceil() as i32,
                        )
                    }
                };

                if self.check_valid_circle(pos, radius, parent_start_idx) {
                    return Some(pos);
                }
            }
        }
        None
    }

    fn check_valid_circle(&self, pos: (i32, i32), radius: i32, continent_start_idx: usize) -> bool {
        for (i, other) in self.continents.iter().enumerate() {
            let d = (other.pos.0 - pos.0).pow(2) + (other.pos.1 - pos.1).pow(2);

            if d > (other.radius + radius).pow(2) {
                // no overlap
                continue;
            }

            if d <= (radius - other.radius).abs().pow(2) {
                // contained entirely by another, reject
                return false;
            } else {
                // overlaps
                let is_my_continent = i >= continent_start_idx;
                if !is_my_continent {
                    return false;
                }
            }
        }

        true
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &Continent)> + '_ {
        self.continent_range
            .iter()
            .flat_map(move |(idx, start, end)| {
                let blobs = &self.continents[*start..*end];
                blobs.iter().map(move |b| (*idx, b))
            })
    }
}
