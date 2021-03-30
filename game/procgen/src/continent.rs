use crate::params::PlanetParams;
use crate::RegionLocation;
use common::cgmath::num_traits::clamp;
use common::*;
use grid::DynamicGrid;
use std::cell::Cell;
use std::f64::consts::TAU;
use std::num::NonZeroUsize;

use crate::biome::BiomeSampler;
use geo::{LineString, Point, Polygon};
#[cfg(feature = "cache")]
use serde::{Deserialize, Serialize};

// TODO agree api and stop making everything public

#[cfg_attr(feature = "cache", derive(Serialize, Deserialize))]
pub struct ContinentMap {
    params: PlanetParams,

    continent_polygons: Vec<(ContinentIdx, Polygon<f64>)>,

    /// Only used in climate
    pub grid: DynamicGrid<RegionTile>,

    /// None until init_generator()
    #[cfg_attr(feature = "cache", serde(skip))]
    biomes: Option<BiomeSampler>,
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

            biomes: None,
        }
    }

    pub fn init_generator(&mut self, rando: &mut dyn RngCore) -> BoxedResult<()> {
        let sampler = BiomeSampler::new(rando, &self.params)?;
        self.biomes = Some(sampler);
        Ok(())
    }

    #[cfg(any(test, feature = "benchmarking"))]
    pub fn new_with_rng(params: &PlanetParams, rando: &mut dyn RngCore) -> Self {
        let mut this = Self::new(params);

        // skip expensive generation with single dummy continent placement
        this.continent_polygons = dummy_continent_polygons();
        this.init_generator(rando).expect("failed");
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
        const CIRCLE_VERTICES: usize = 64;
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
                if multi.0.len() > 1 {
                    warn!(
                        "polygon union produced {count} polygons instead of 1. skipping extra",
                        count = multi.0.len()
                    );
                }

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

    /// -1.0: sea far away from coastline
    /// -0.2: sea close to coastline
    ///  0.0: coastline
    /// +0.2: land close to coastline
    /// +1.0: land far away from coastline
    ///
    pub fn coastline_proximity(&self, pos: (f64, f64)) -> f64 {
        use geo::contains::Contains;
        use geo::euclidean_distance::EuclideanDistance;

        let point = Point::from(pos);

        let (inland, polygons_to_check) = match self
            .continent_polygons()
            .enumerate()
            .find(|(_i, (_, polygon))| polygon.contains(&point))
        {
            Some((idx, _)) => {
                // contained by a polygon, only check its lines
                // TODO intersecting polygons!!
                (true, idx..idx + 1)
            }
            None => {
                // in the ocean, check all
                (false, 0..self.continent_polygons.len())
            }
        };

        let closest = (&self.continent_polygons[polygons_to_check])
            .iter()
            .enumerate()
            .flat_map(|(i, (_, polygon))| polygon.exterior().lines().map(move |line| (i, line)))
            .fold(
                (usize::MAX, f64::MAX),
                |(min_idx, min), (poly_idx, line)| {
                    let distance = point.euclidean_distance(&line);
                    if distance < min {
                        (poly_idx, distance)
                    } else {
                        (min_idx, min)
                    }
                },
            );

        debug_assert_ne!(closest, (usize::MAX, f64::MAX));
        let (_idx, distance) = closest;

        let mul = if inland { 1.0 } else { -1.0 };
        let coast_thickness = self.params.coastline_thickness;
        let scaled = if distance >= coast_thickness {
            // far away
            1.0
        } else {
            distance / coast_thickness
        };

        scaled * mul
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

    pub fn biome_sampler(&self) -> &BiomeSampler {
        self.biomes
            .as_ref()
            .expect("biome sampler not initialized with init_generator()")
    }

    pub fn tile_at(&self, region: RegionLocation) -> &RegionTile {
        let RegionLocation(x, y) = region;
        &self.grid[[x as usize, y as usize, 0]]
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

#[cfg(any(test, feature = "benchmarking"))]
fn dummy_continent_polygons() -> Vec<(ContinentIdx, Polygon<f64>)> {
    let c = |x, y| geo::Coordinate { x, y };
    vec![(
        ContinentIdx::new(1).unwrap(),
        Polygon::new(
            vec![
                c(-8.995027684875689, 84.80061645735721),
                c(-8.188530028891687, 82.58478235869809),
                c(-6.2739466010067355, 81.20829061533148),
                c(-4.701079625654302, 81.01118481127529),
                c(-3.538635902534419, 81.27650500542319),
                c(-2.2070527276557086, 82.13653260296113),
                c(-1.4188169719526034, 83.22880371615805),
                c(0.06246938307037773, 82.151173429136),
                c(0.2714700009296359, 81.24086534152255),
                c(0.014614170872986332, 79.74168380441088),
                c(-2.6699493778783205, 78.49550404572685),
                c(-4.826327053057245, 76.17148162703599),
                c(-5.752618289832623, 73.97405918152235),
                c(-5.951984182507903, 71.22076869602209),
                c(-7.398311230978958, 70.08103642662552),
                c(-8.63815572471545, 68.05212085995402),
                c(-8.992541527313534, 66.29907531396418),
                c(-8.944154131125643, 62.25478866911913),
                c(-8.26645440816078, 60.39282398310251),
                c(-7.308429187984299, 59.251093985161326),
                c(-3.9554599507550527, 57.156217961498605),
                c(0.42535689171152685, 57.21078542295812),
                c(1.3532566496125642, 56.000875188129065),
                c(2.354851587477556, 55.353976590753305),
                c(6.90027723304771, 54.001243271996735),
                c(9.389185421335446, 54.12153797590234),
                c(10.805062644817776, 54.513476488517725),
                c(12.378281476762574, 54.03880396699914),
                c(14.58393931450699, 54.26406628338391),
                c(16.694592710667724, 54.06076898795117),
                c(18.170185055463037, 54.63989630739692),
                c(21.585894544688582, 57.27306520592226),
                c(23.02577258696183, 59.17396817102946),
                c(23.7395314933448, 61.01484697892977),
                c(25.36863197205949, 61.330384573565595),
                c(26.478716322947985, 62.310039825809135),
                c(26.985092326096204, 63.70129646021255),
                c(26.794201524227834, 65.08350404963801),
                c(28.170185055463037, 65.63989630739691),
                c(29.06417777247591, 66.42884956125384),
                c(29.82229122314456, 67.82097930235639),
                c(29.999721097125924, 69.00558834840905),
                c(32.73726394411898, 69.66076914713119),
                c(34.95743264589597, 71.62007965161827),
                c(35.97018465219241, 74.4025929204251),
                c(35.58560551197249, 77.17469426759365),
                c(37.21476211870407, 80.11565203725306),
                c(37.80172487848544, 82.01853856800602),
                c(37.80172487848544, 85.98146143199398),
                c(37.21476211870408, 87.88434796274694),
                c(35.9522219591054, 90.04267627321171),
                c(35.55572805786141, 93.94755174410905),
                c(34.26238774315995, 96.63320058063621),
                c(32.88951706577241, 98.24061008784636),
                c(33.92068995139417, 100.2074154272024),
                c(33.657515999651615, 102.61268923504184),
                c(33.91517626274905, 104.69395958814341),
                c(34.86671841735842, 106.11573447676729),
                c(34.94051746354563, 107.5944384295982),
                c(33.8704694055762, 109.34549444740409),
                c(32.81252140442901, 109.88787274085004),
                c(31.661672264866162, 109.97931827851588),
                c(30.973724973347018, 110.47352997389778),
                c(30.907851850860958, 112.14415658573694),
                c(32.24069441756403, 113.00973274656776),
                c(34.17018505546304, 113.63989630739691),
                c(35.484816107031065, 115.04104926180484),
                c(36.8704694055762, 115.65450555259591),
                c(37.764428635611225, 116.83469561117592),
                c(37.98509232609621, 118.29870353978745),
                c(37.298133329356936, 119.92836282905962),
                c(35.484816107031065, 120.95895073819516),
                c(34.98296526270199, 121.65867670817718),
                c(34.86671841735842, 123.88426552323271),
                c(33.09471045425822, 127.14760054777915),
                c(30.7127313193288, 129.20012961575387),
                c(28.373650467932123, 129.9860189859059),
                c(26.40756674874158, 129.73963673083566),
                c(24.726762493047186, 128.77697044397132),
                c(22.129594067467604, 128.9456744462012),
                c(20.12099027808572, 128.3805409661817),
                c(17.868636897191216, 126.76120916439643),
                c(15.880254710207065, 124.128611220277),
                c(15.214363294719439, 122.57399926402154),
                c(14.067792512680695, 121.72069095108368),
                c(13.359094136221703, 120.64768848692921),
                c(9.044827461628266, 117.74803082670961),
                c(6.83000071028297, 114.75958155269178),
                c(5.8074884148695, 114.04699861762526),
                c(5.09471045425822, 115.14760054777915),
                c(4.096023073099185, 115.79262124593261),
                c(1.1099162641747415, 115.89971164872729),
                c(-0.9322074873193054, 114.72069095108368),
                c(-1.9047193335556412, 112.84649248603016),
                c(-3.8900837358252582, 112.89971164872729),
                c(-5.932207487319305, 111.72069095108368),
                c(-6.955323304900514, 109.5961690647047),
                c(-6.66250484965437, 107.4035503615885),
                c(-7.504844339512094, 106.1694186955878),
                c(-7.944154131125643, 104.74521133088088),
                c(-7.944154131125644, 103.25478866911912),
                c(-7.324819356196964, 101.5),
                c(-7.944154131125643, 99.74521133088088),
                c(-7.944154131125644, 98.25478866911912),
                c(-7.504844339512097, 96.8305813044122),
                c(-6.598769606026243, 95.53428403630706),
                c(-7.702906603707257, 94.30165121735267),
                c(-7.977843007645762, 92.66630465578471),
                c(-8.876309144916311, 90.98702959076117),
                c(-8.876309144916311, 89.01297040923882),
                c(-8.117389182977782, 87.5),
                c(-8.758770483143634, 86.36808057330268),
                c(-8.995027684875689, 84.80061645735721),
            ]
            .into_iter()
            .collect(),
            vec![],
        ),
    )]
}
