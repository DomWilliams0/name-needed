use crate::PlanetParams;
use common::*;
use grid::DynamicGrid;
use std::cell::Cell;
use std::f32::consts::PI;

pub struct LandBlob {
    pub pos: (i32, i32),
    pub radius: i32,
}

// TODO agree api and stop making everything public

pub struct ContinentMap {
    size: i32,
    max_continents: usize,
    /// Consecutive blobs belong to the same continent, partitioned by continent_range
    land_blobs: Vec<LandBlob>,
    /// (continent idx, start idx, end idx (exclusive))
    continent_range: Vec<(ContinentIdx, usize, usize)>,

    pub grid: DynamicGrid<Tile>,
}

type ContinentIdx = usize;

const STARTING_RADIUS: f32 = 14.0;
const DECREMENT_RANGE: (f32, f32) = (0.3, 0.6);
const MIN_RADIUS: i32 = 2;
const NEW_CONTINENT_MIN_DISTANCE: i32 = 30;

pub struct Tile {
    pub is_land: bool,
    pub depth: Cell<u8>,
}

impl ContinentMap {
    pub fn new(params: &PlanetParams) -> Self {
        Self {
            size: params.planet_size as i32,
            max_continents: params.max_continents,

            land_blobs: Vec::with_capacity(128),
            continent_range: Vec::with_capacity(params.max_continents),

            grid: DynamicGrid::<Tile>::new([
                params.planet_size as usize,
                params.planet_size as usize,
                1,
            ]),
        }
    }

    pub fn generate(&mut self, rando: &mut dyn RngCore) -> (usize, usize) {
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
                let blob_count = self.land_blobs.len() - parent_start_idx;

                if blob_count == 0 {
                    // empty continent
                    break;
                }

                self.continent_range
                    .push((count, parent_start_idx, self.land_blobs.len()));

                count += 1;
                debug!("continent finished"; "index" => count, "blobs" => blob_count);

                if count >= self.max_continents {
                    // all done
                    break;
                }

                // prepare for next
                parent_start_idx = self.land_blobs.len();
                radius = STARTING_RADIUS;
                decrement = new_decrement!();
                continue;
            }

            self.land_blobs.push(LandBlob {
                pos: this_pos.expect("position not initialized"),
                radius: this_radius,
            });

            debug!("placing shape on continent"; "pos" => ?this_pos, "radius" => ?this_radius, "continent" => parent_start_idx);

            // possibly reduce radius, gets less likely as it gets smaller so we have fewer large continents
            let decrement_threshold = (radius / STARTING_RADIUS).max(0.1).min(0.8);
            if rando.gen::<f32>() < decrement_threshold {
                radius -= decrement;
            }
        }

        (count, self.land_blobs.len())
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
        let max_parent = (self.land_blobs.len()) as f32;
        let min_parent = parent_start_idx as f32;

        for _ in 0..MAX_ATTEMPTS {
            // choose a parent to attach to
            let parent_idx = if self.land_blobs[parent_start_idx..].is_empty() {
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
                        // new continent - must not be too close to others
                        let x = rando.gen_range(radius, self.size - radius);
                        let y = rando.gen_range(radius, self.size - radius);
                        let pos = (x, y);

                        if self.min_distance_2_from_others(pos) <= NEW_CONTINENT_MIN_DISTANCE.pow(2)
                        {
                            // too close
                            continue;
                        }
                        pos
                    }
                    Some(idx) => {
                        // on parent circumference
                        let parent = &self.land_blobs[idx];
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

                if self.check_valid_blob(pos, radius, parent_start_idx) {
                    return Some(pos);
                }
            }
        }
        None
    }

    fn check_valid_blob(&self, pos: (i32, i32), radius: i32, blob_start_idx: usize) -> bool {
        for (i, other) in self.land_blobs.iter().enumerate() {
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
                let is_my_continent = i >= blob_start_idx;
                if !is_my_continent {
                    return false;
                }
            }
        }

        true
    }

    fn min_distance_2_from_others(&self, pos: (i32, i32)) -> i32 {
        let mut dist = i32::MAX;
        for other in &self.land_blobs {
            let d2 = (other.pos.0 - pos.0).pow(2) + (other.pos.1 - pos.1).pow(2);
            dist = dist.min(d2);
        }

        dist
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &LandBlob)> + '_ {
        self.continent_range
            .iter()
            .flat_map(move |(idx, start, end)| {
                let blobs = &self.land_blobs[*start..*end];
                blobs.iter().map(move |b| (*idx, b))
            })
    }

    pub fn populate_depth(&mut self) {
        macro_rules! set {
            ($pos:expr) => {
                let (x, y) = $pos;
                let coord = [x as usize, y as usize, 0];
                if self.grid.is_coord_in_range(coord) {
                    self.grid[coord].is_land = true;
                }
            };
        }

        // rasterize blobs onto grid
        for blob in &self.land_blobs {
            // draw filled in circle
            // https://stackoverflow.com/a/14976268
            let mut x = blob.radius;
            let mut y = 0;
            let mut x_change = 1 - (blob.radius << 1);
            let mut y_change = 0;
            let mut radius_error = 0;

            let x0 = blob.pos.0;
            let y0 = blob.pos.1;
            while x >= y {
                for _x in (x0 - x)..=(x0 + x) {
                    set!((_x, y0 + y));
                    set!((_x, y0 - y));
                }

                for _x in (x0 - y)..=(x0 + y) {
                    set!((_x, y0 + x));
                    set!((_x, y0 - x));
                }

                y += 1;
                radius_error += y_change;
                y_change += 2;

                if ((radius_error << 1) + x_change) > 0 {
                    x -= 1;
                    radius_error += x_change;
                    x_change += 2;
                }
            }
        }

        let mut frontier = Vec::with_capacity((self.size * self.size / 2) as usize);
        for idx in self
            .grid
            .iter()
            .enumerate()
            .filter_map(|(idx, tile)| tile.is_land.as_some(idx))
        {
            for n in self.grid.neighbours(idx) {
                let n_tile = &self.grid[n];
                if n_tile.is_land {
                    continue;
                }

                // this is border between land and sea
                frontier.push((n, 1));
            }

            while let Some((idx, new_val)) = frontier.pop() {
                let propagate = |tile: &Tile| {
                    let current = tile.depth.get();
                    if current == 0 || new_val < current {
                        tile.depth.set(new_val);
                        true
                    } else {
                        false
                    }
                };

                let this_tile = &self.grid[idx];
                if propagate(this_tile) {
                    for n in self.grid.neighbours(idx) {
                        frontier.push((n, new_val.saturating_add(1)));
                    }
                }
            }
        }
    }
}

impl Default for Tile {
    fn default() -> Self {
        Tile {
            is_land: false,
            depth: Cell::new(0),
        }
    }
}
