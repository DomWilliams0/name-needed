use crate::params::PlanetParams;
use common::cgmath::num_traits::clamp;
use common::*;
use grid::DynamicGrid;
use noise::{Fbm, MultiFractal, NoiseFn, Seedable};
use std::cell::Cell;
use std::f32::consts::PI;
use std::f64::consts::TAU;
use std::num::NonZeroUsize;

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

type ContinentIdx = NonZeroUsize;

const STARTING_RADIUS: f32 = 14.0;
const DECREMENT_RANGE: (f32, f32) = (0.3, 0.6);
const MIN_RADIUS: i32 = 2;
const NEW_CONTINENT_MIN_DISTANCE: i32 = 30;

pub struct Tile {
    density: Cell<f64>,
    pub continent: Option<ContinentIdx>,
    pub height: f64,
}

impl ContinentMap {
    pub fn new(params: &PlanetParams) -> Self {
        // TODO validate values with result type
        assert!(params.planet_size > 0);

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
        let mut current_continent = ContinentIdx::new(1).unwrap();
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

                self.continent_range.push((
                    current_continent,
                    parent_start_idx,
                    self.land_blobs.len(),
                ));

                // increment, overflow would panic before it wraps around to 0
                current_continent =
                    unsafe { NonZeroUsize::new_unchecked(current_continent.get() + 1) };
                debug!("continent finished"; "index" => current_continent.get(), "blobs" => blob_count);

                if current_continent.get() >= self.max_continents {
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

        let count = current_continent.get() - 1;
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
                        let max = (self.size - radius).max(radius + 1);
                        let x = rando.gen_range(radius, max);
                        let y = rando.gen_range(radius, max);
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

    pub fn iter(&self) -> impl Iterator<Item = (ContinentIdx, &LandBlob)> + '_ {
        self.continent_range
            .iter()
            .flat_map(move |(idx, start, end)| {
                let blobs = &self.land_blobs[*start..*end];
                blobs.iter().map(move |b| (*idx, b))
            })
    }

    pub fn discover(&mut self, rando: &mut dyn RngCore) {
        self.rasterize_land_blobs();
        self.discover_density();
        self.generate_initial_heightmap(rando);
    }

    fn rasterize_land_blobs(&mut self) {
        for &(continent, start, end) in self.continent_range.iter() {
            macro_rules! set {
                ($pos:expr) => {
                    let (x, y) = $pos;
                    let coord = [x as isize, y as isize, 0];
                    let wrapped_coord = self.grid.wrap_coord(coord);
                    self.grid[wrapped_coord].continent = Some(continent);
                };
            }

            for blob in &self.land_blobs[start..end] {
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
        }
    }

    /// Discovers density and scales to 0.0-1.0
    fn discover_density(&mut self) {
        let increment = 1.0;
        let limit = self.size as f64 / 4.0;

        let mut frontier = Vec::with_capacity((self.size * self.size / 2) as usize);
        for idx in self
            .grid
            .iter()
            .enumerate()
            .filter_map(|(idx, tile)| tile.is_land().as_some(idx))
        {
            for n in self.grid.wrapping_neighbours(idx) {
                let n_tile = &self.grid[n];
                if n_tile.is_land() {
                    continue;
                }

                // this is border between land and sea
                frontier.push((n, 1.0));
            }

            while let Some((idx, new_val)) = frontier.pop() {
                let this_tile = &self.grid[idx];
                let current = this_tile.density.get();
                if current == 0.0 || new_val < current {
                    this_tile.density.set(new_val);
                    for n in self.grid.wrapping_neighbours(idx) {
                        let incremented = (new_val + increment).min(limit);
                        frontier.push((n, incremented));
                    }
                }
            }
        }

        // normalize density values between 0 to 1
        let max_density = self
            .grid
            .iter()
            .map(|tile| OrderedFloat(tile.density.get()))
            .max()
            .unwrap() // not empty
            .0;

        info!("original density limit"; "max" => ?max_density);

        for tile in self.grid.iter_mut() {
            let val = tile.density.get();
            let scaled_val = val / (max_density);
            tile.density.set(scaled_val);
        }

        // average density with gaussian blur filter
        apply_gaussian_filter(
            &mut self.grid,
            |tile| tile.density.get(),
            |tile, val| tile.density.set(val),
        )
    }

    /// Generates noise and scales to 0.0-1.0
    fn generate_initial_heightmap(&mut self, rando: &mut dyn RngCore) {
        // thanks https://www.gamasutra.com/blogs/JonGallant/20160201/264587/Procedurally_Generating_Wrapping_World_Maps_in_Unity_C__Part_2.php
        let noise = Fbm::new()
            .set_seed(rando.gen())
            .set_octaves(4)
            .set_frequency(9.0);

        let mut min = f64::MAX;
        let mut max = f64::MIN;

        let size = self.size as f64;
        for (coord, tile) in self.grid.iter_coords_mut() {
            let (x, y) = (coord[0] as f64, coord[1] as f64);

            // noise range
            let x1 = 0.0;
            let x2 = 2.0;
            let y1 = 0.0;
            let y2 = 2.0;
            let dx = x2 - x1;
            let dy = y2 - y1;

            // sample at smaller intervals
            let s = x / size;
            let t = y / size;

            // get 4d noise
            let nx = x1 + (s * TAU).cos() * dx / TAU;
            let ny = y1 + (t * TAU).cos() * dy / TAU;
            let nz = x1 + (s * TAU).sin() * dx / TAU;
            let nw = y1 + (t * TAU).sin() * dy / TAU;
            let height = noise.get([nx, ny, nz, nw]);

            // scale to density of land

            min = min.min(height);
            max = max.max(height);

            tile.height = height;
        }

        debug!("original noise limits"; "max" => max, "min" => min);

        let rescale = |val: f64| {
            // https://rosettacode.org/wiki/Map_range#Rust
            let from = (min, max);
            let to = (0.0, 1.0);
            to.0 + (val - from.0) * (to.1 - to.0) / (from.1 - from.0)
        };

        for tile in self.grid.iter_mut() {
            let height = rescale(tile.height);
            let density = tile.density.get();

            // multiply together so that height is lower at the borders between land+sea (density=0)
            // and more diverse inland where density=1
            tile.height = height * density;
        }
    }
}

impl Default for Tile {
    fn default() -> Self {
        Tile {
            density: Cell::new(0.0),
            continent: None,
            height: 0.0,
        }
    }
}

impl Tile {
    pub fn is_land(&self) -> bool {
        self.continent.is_some()
    }

    pub fn density(&self) -> f64 {
        self.density.get()
    }
}

fn apply_gaussian_filter<T: Default>(
    grid: &mut DynamicGrid<T>,
    mut gimme_value: impl FnMut(&T) -> f64,
    mut set_value: impl FnMut(&T, f64),
) {
    const HEIGHT: usize = 5;
    const WIDTH: usize = 5;
    const SIGMA: f64 = 3.0;

    let kernel = {
        let mut kernel = [0.0; HEIGHT * WIDTH];

        let mut sum = 0.0;
        for (idx, val) in kernel.iter_mut().enumerate() {
            let i = (idx / WIDTH) as f64;
            let j = (idx % WIDTH) as f64;

            *val = (-(i * i + j * j) / (2.0 * SIGMA * SIGMA)).exp() / (TAU * SIGMA * SIGMA);
            sum += *val;
        }

        for val in kernel.iter_mut() {
            *val /= sum;
        }

        kernel
    };

    for ([i, j, _], value) in grid.iter_coords() {
        let mut val = 0.0;

        for h in i..i + HEIGHT {
            for w in j..j + WIDTH {
                let kernel_val = {
                    let x = h - i;
                    let y = w - j;
                    kernel[x + (y * WIDTH)]
                };

                let grid_val = {
                    let coord = [h as isize, w as isize, 0];
                    let coord = grid.wrap_coord(coord);
                    gimme_value(&grid[coord])
                };
                val += kernel_val * grid_val;
            }
        }

        // ensure limits are maintained
        val = clamp(val, 0.0, 1.0);
        set_value(value, val);
    }
}
