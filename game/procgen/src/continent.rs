use crate::params::PlanetParams;
use crate::{map_range, RegionLocation};
use common::cgmath::num_traits::clamp;
use common::*;
use grid::DynamicGrid;
use noise::{Fbm, MultiFractal, NoiseFn, Seedable};
use std::cell::Cell;
use std::f64::consts::TAU;
use std::num::NonZeroUsize;
use std::sync::Arc;

use geo::{LineString, Polygon};
#[cfg(feature = "cache")]
use serde::{Deserialize, Serialize};

// TODO agree api and stop making everything public

#[cfg_attr(feature = "cache", derive(Serialize, Deserialize))]
pub struct ContinentMap {
    params: PlanetParams,

    continent_polygons: Vec<(ContinentIdx, Polygon<f64>)>,

    pub grid: DynamicGrid<RegionTile>,

    /// None until init_generator()
    #[cfg_attr(feature = "cache", serde(skip))]
    generator: Option<Arc<Generator>>,
}

type ContinentIdx = NonZeroUsize;

#[cfg_attr(feature = "cache", derive(Serialize, Deserialize))]
pub struct RegionTile {
    density: Cell<f64>,
    continent: Option<ContinentIdx>,
    height: f64,
}

/// density: Cell<f64> is only written to during initial generation outside of any async functions
unsafe impl Sync for RegionTile {}

pub struct Generator {
    height: Fbm,
    scale: f64,
    /// (min, max) after sampling a large number of points
    limits: (f64, f64),
}

impl ContinentMap {
    pub fn new(params: &PlanetParams) -> Self {
        // TODO validate values with result type
        assert!(params.planet_size > 0);

        Self {
            params: params.clone(),

            continent_polygons: Vec::new(),

            grid: DynamicGrid::<RegionTile>::new([
                params.planet_size as usize,
                params.planet_size as usize,
                1,
            ]),

            generator: None,
        }
    }

    pub fn init_generator(&mut self, rando: &mut dyn RngCore) {
        self.generator = Some(Arc::new(Generator::new(rando, &self.params)))
    }

    #[cfg(test)]
    pub fn new_with_rng(params: &PlanetParams, rando: &mut dyn RngCore) -> Self {
        let mut this = Self::new(params);
        this.init_generator(rando);
        this
    }

    pub fn generate(&mut self, rando: &mut dyn RngCore) {
        // place continents as a load of circle blobs
        // TODO reject if continent or land blob count is too low
        let mut blobby = mr_blobby::BlobPlacement::new(&self.params);
        let (continents, total_blobs) = blobby.place_blobs(rando);
        info!(
            "placed {count} continents with {blobs} land blobs",
            count = continents,
            blobs = total_blobs
        );

        // convert blobs to polygons in region space
        let polygons = self.derive_polygons(&blobby);
        self.continent_polygons = polygons;
    }

    fn derive_polygons(
        &self,
        blobs: &mr_blobby::BlobPlacement,
    ) -> Vec<(ContinentIdx, Polygon<f64>)> {
        const CIRCLE_VERTICES: usize = 16;
        fn polygon_from_blob(blob: &mr_blobby::LandBlob) -> [(f64, f64); CIRCLE_VERTICES] {
            // ty http://slabode.exofire.net/circle_draw.shtml

            let mut vertices = [(0.0, 0.0); CIRCLE_VERTICES]; // TODO could be uninitialized

            let n = CIRCLE_VERTICES - 1; // -1 so last == first
            let pos = (blob.pos.0 as f64, blob.pos.1 as f64);
            let theta = TAU / n as f64;
            let tangential_factor = theta.tan();
            let radial_factor = theta.cos();

            // start at angle = 0
            let mut x = blob.radius as f64;
            let mut y = 0.0;

            vertices.iter_mut().for_each(|v| {
                *v = (pos.0 + x, pos.1 + y);

                let tx = -y;
                let ty = x;

                x += tx * tangential_factor;
                y += ty * tangential_factor;

                x *= radial_factor;
                y *= radial_factor;
            });

            vertices[CIRCLE_VERTICES - 1] = vertices[0];

            vertices
        }

        let epsilon = self.params.continent_polygon_epsilon;
        let mut continent_polygons = Vec::with_capacity(blobs.continent_count());
        for (continent_idx, blobs) in blobs.iter().group_by(|(c, _)| *c).into_iter() {
            use geo::simplify::Simplify;
            use geo_booleanop::boolean::BooleanOp;

            let mut polygons = blobs.map(|(_, blob): (_, &mr_blobby::LandBlob)| {
                let vertices = polygon_from_blob(blob);
                let exterior = vertices.iter().copied().collect::<LineString<f64>>();
                debug_assert!(exterior.is_closed());
                Polygon::new(exterior, vec![])
            });

            let mut continent_polygon = polygons.next().unwrap(); // continent is not empty

            for polygon in polygons {
                let mut multi = continent_polygon.union(&polygon);
                debug_assert_eq!(multi.0.len(), 1);
                continent_polygon = multi.0.remove(0);
            }

            // simplify vertices
            let prev_count = continent_polygon.exterior().num_coords();
            let simplified = continent_polygon.simplify(&epsilon);
            debug_assert!(simplified.exterior().is_closed(), "over-simplified!");

            debug!(
                "polygon for continent {continent} has {vertices} vertices, simplified from {}",
                prev_count,
                continent = continent_idx.get(),
                vertices = simplified.exterior().num_coords(),
            );
            continent_polygons.push((continent_idx, simplified));
        }

        continent_polygons
    }

    pub fn continent_polygons(&self) -> impl Iterator<Item = &(ContinentIdx, Polygon<f64>)> {
        self.continent_polygons.iter()
    }

    pub fn discover(&mut self) {
        // TODO reimplement or add back density if needed
        todo!();
        // self.rasterize_land_blobs();
        // self.discover_density();
        // self.generate_initial_heightmap();
    }

    /// Discovers density and scales to 0.0-1.0
    fn discover_density(&mut self) {
        let increment = 0.1;
        let limit = 10.0;

        let size = self.params.planet_size as i32;
        let mut frontier = Vec::with_capacity((size * size / 2) as usize);
        for (idx, tile) in self.grid.iter().enumerate() {
            let is_land = tile.is_land();
            for (n, _) in self.grid.wrapping_neighbours(idx) {
                let n_tile = &self.grid[n];
                if is_land == n_tile.is_land() {
                    continue;
                }

                // this is border between land and sea
                frontier.push((n, increment));
            }

            while let Some((idx, new_val)) = frontier.pop() {
                let this_tile = &self.grid[idx];
                let current = this_tile.density.get();
                if current == 0.0 || new_val < current {
                    this_tile.density.set(new_val);
                    for (n, _) in self.grid.wrapping_neighbours(idx) {
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

        assert!(
            max_density > 0.0,
            "all density is 0, world might be too small"
        );

        debug!("original density limit"; "max" => ?max_density);

        for tile in self.grid.iter_mut() {
            let val = tile.density.get();
            let scaled_val = val / (max_density);
            tile.density.set(scaled_val);
        }

        // average density with gaussian blur filter
        apply_gaussian_filter(
            &mut self.grid,
            |tile| tile.density.get(),
            |tile, orig| tile.is_land() == orig.is_land(),
            |tile, val| tile.density.set(val),
        )
    }

    /// Generates noise and scales to 0.0-1.0
    fn generate_initial_heightmap(&mut self) {
        let height_gen = self
            .generator
            .as_mut()
            .expect("generator has not been initialized");

        let mut min = f64::MAX;
        let mut max = f64::MIN;

        for (coord, tile) in self.grid.iter_coords_mut() {
            let pos = (coord[0] as f64, coord[1] as f64);
            let height = height_gen.sample(pos);

            min = min.min(height);
            max = max.max(height);

            tile.height = height;
        }

        debug!("original noise limits"; "max" => max, "min" => min);

        for tile in self.grid.iter_mut() {
            let height = map_range((min, max), (0.0, 1.0), tile.height);
            let density = tile.density.get();

            // multiply together so that height is lower at the borders between land+sea (density=0)
            // and more diverse inland where density=1
            tile.height = height * density;
        }
    }

    pub fn generator_copy(&self) -> Arc<Generator> {
        self.generator.clone().expect("generator not initialized")
    }

    pub fn generator(&self) -> &Generator {
        self.generator.as_ref().expect("generator not initialized")
    }

    pub fn tile_at(&self, region: RegionLocation) -> &RegionTile {
        let RegionLocation(x, y) = region;
        &self.grid[[x as usize, y as usize, 0]]
    }
}

impl Generator {
    pub fn new(rando: &mut dyn RngCore, params: &PlanetParams) -> Self {
        // TODO adjust params for global height map
        let seed = rando.gen::<u64>();
        let noise = Fbm::new()
            .set_seed(seed as u32)
            .set_octaves(params.height_octaves)
            .set_frequency(params.height_freq);
        let scale = params.planet_size as f64;

        let mut this = Generator {
            height: noise,
            scale,
            limits: (0.0, 0.0),
        };

        this.limits = this.find_limits(seed);
        this
    }

    fn find_limits(&mut self, seed: u64) -> (f64, f64) {
        let (mut min, mut max) = (100.0, -100.0);
        let mut r = StdRng::seed_from_u64(seed);
        let iterations = 1_000; // _000;
        debug!("finding generator limits"; "iterations" => iterations);

        for _ in 0..iterations {
            let f = self.sample((r.gen_range(-100.0, 100.0), r.gen_range(-100.0, 100.0)));
            min = f.min(min);
            max = f.max(max);
        }

        debug!(
            "generator limits are {min:?} -> {max:?}",
            min = min,
            max = max
        );

        (min, max)
    }

    pub fn sample_normalized(&self, coord: (f64, f64)) -> f64 {
        let value = self.sample(coord);
        map_range(self.limits, (0.0, 1.0), value)
    }

    /// Produces seamlessly wrapping noise in the range self.limits
    fn sample(&self, (x, y): (f64, f64)) -> f64 {
        // thanks https://www.gamasutra.com/blogs/JonGallant/20160201/264587/Procedurally_Generating_Wrapping_World_Maps_in_Unity_C__Part_2.php

        // noise range
        let x1 = 0.0;
        let x2 = 2.0;
        let y1 = 0.0;
        let y2 = 2.0;
        let dx = x2 - x1;
        let dy = y2 - y1;

        // sample at smaller intervals
        let s = x / self.scale;
        let t = y / self.scale;

        // get 4d noise
        let nx = x1 + (s * TAU).cos() * dx / TAU;
        let ny = y1 + (t * TAU).cos() * dy / TAU;
        let nz = x1 + (s * TAU).sin() * dx / TAU;
        let nw = y1 + (t * TAU).sin() * dy / TAU;
        self.height.get([nx, ny, nz, nw])
    }
}

impl Default for RegionTile {
    fn default() -> Self {
        RegionTile {
            density: Cell::new(0.0),
            continent: None,
            height: 0.0,
        }
    }
}

impl RegionTile {
    pub fn is_land(&self) -> bool {
        self.continent.is_some()
    }

    /// density is not really Sync
    #[cfg(feature = "bin")]
    pub unsafe fn density(&self) -> f64 {
        self.density.get()
    }

    pub fn height(&self) -> f64 {
        self.height
    }

    pub fn land_height(&self) -> f64 {
        if self.is_land() {
            self.height
        } else {
            0.0
        }
    }
}

fn apply_gaussian_filter<T: Default>(
    grid: &mut DynamicGrid<T>,
    mut gimme_value: impl FnMut(&T) -> f64,
    mut should_average_value: impl FnMut(&T, &T) -> bool,
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
                let grid_entry = {
                    let coord = [h as isize, w as isize, 0];
                    let coord = grid.wrap_coord(coord);
                    &grid[coord]
                };

                let grid_val = gimme_value(grid_entry);

                let kernel_val = if should_average_value(grid_entry, value) {
                    let x = h - i;
                    let y = w - j;
                    kernel[x + (y * WIDTH)]
                } else {
                    0.0
                };

                val += kernel_val * grid_val;
            }
        }

        // ensure limits are maintained
        val = clamp(val, 0.0, 1.0);
        set_value(value, val);
    }
}

mod mr_blobby {
    use super::{ContinentIdx, RegionTile};
    use crate::PlanetParams;
    use common::*;
    use grid::DynamicGrid;
    use std::f32::consts::PI;
    use std::num::NonZeroUsize;

    const MIN_RADIUS: i32 = 2;

    pub struct LandBlob {
        pub pos: (i32, i32),
        pub radius: i32,
    }

    pub struct BlobPlacement<'a> {
        params: &'a PlanetParams,

        /// Consecutive blobs belong to the same continent, partitioned by continent_range
        land_blobs: Vec<LandBlob>,

        /// (continent idx, start idx, end idx (exclusive))
        continent_range: Vec<(ContinentIdx, usize, usize)>,
    }

    impl<'a> BlobPlacement<'a> {
        pub fn new(params: &'a PlanetParams) -> Self {
            BlobPlacement {
                params,
                land_blobs: Vec::with_capacity(512),
                continent_range: Vec::new(),
            }
        }

        pub fn place_blobs(&mut self, rando: &mut dyn RngCore) -> (usize, usize) {
            macro_rules! new_decrement {
                () => {
                    rando.gen_range(self.params.continent_dec_min, self.params.continent_dec_max)
                };
            }

            let mut radius = self.params.continent_start_radius;
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

                    if current_continent.get() >= self.params.max_continents {
                        // all done
                        break;
                    }

                    // prepare for next
                    parent_start_idx = self.land_blobs.len();
                    radius = self.params.continent_start_radius;
                    decrement = new_decrement!();
                    continue;
                }

                self.land_blobs.push(LandBlob {
                    pos: this_pos.expect("position not initialized"),
                    radius: this_radius,
                });

                trace!("placing shape on continent"; "pos" => ?this_pos, "radius" => ?this_radius, "continent" => parent_start_idx);

                // possibly reduce radius, gets less likely as it gets smaller so we have fewer large continents
                let decrement_threshold = (radius / self.params.continent_start_radius)
                    .max(0.1)
                    .min(0.8);
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
                            let max = (self.params.planet_size as i32 - radius).max(radius + 1);
                            let x = rando.gen_range(radius, max);
                            let y = rando.gen_range(radius, max);
                            let pos = (x, y);

                            if self.min_distance_2_from_others(pos)
                                <= self.params.continent_min_distance.pow(2)
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

        pub fn continent_count(&self) -> usize {
            self.continent_range.len()
        }

        #[deprecated]
        fn rasterize_land_blobs(&self, grid: &mut DynamicGrid<RegionTile>) {
            for &(continent, start, end) in self.continent_range.iter() {
                macro_rules! set {
                    ($pos:expr) => {
                        let (x, y) = $pos;
                        let coord = [x as isize, y as isize, 0];
                        let wrapped_coord = grid.wrap_coord(coord);
                        grid[wrapped_coord].continent = Some(continent);
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
    }
}
