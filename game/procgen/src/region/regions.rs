use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{watch, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use common::derive_more::Deref;
use common::*;
use core::array::IntoIter;
use grid::DynamicGrid;

use crate::continent::ContinentMap;
use crate::region::feature::SharedRegionalFeature;
use crate::region::region::{Region, RegionContinuations, SlabContinuations};
use crate::region::unit::RegionLocation;
use crate::region::SlabContinuation;
use crate::{PlanetParams, PlanetParamsRef};
use futures::prelude::stream::FuturesUnordered;
use futures::{Future, StreamExt};
use line_drawing::Bresenham;
use std::hint::unreachable_unchecked;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use strum_macros::EnumDiscriminants;
use unit::world::SlabLocation;

pub struct Regions<const SIZE: usize, const SIZE_2: usize> {
    params: PlanetParamsRef,

    /// 2d grid of all regions on the planet, each containing its own RwLock
    region_grid: DynamicGrid<RegionEntry<SIZE, SIZE_2>>,

    region_continuations: RegionContinuations<SIZE>,
    slab_continuations: SlabContinuations,
    is_initial_region: AtomicBool,

    /// Keep track of all regions created in tests
    /// TODO use a global vec/channel instead (in tests only)
    #[cfg(test)]
    created_regions: Mutex<Vec<RegionLocation<SIZE>>>,
}

#[derive(EnumDiscriminants)]
pub(crate) enum RegionLoadState<const SIZE: usize, const SIZE_2: usize> {
    /// Is not loaded
    Unloaded,
    /// Has been requested but is not yet in progress
    Requested(RequestedType),
    /// Is currently loading. The channel is not actually used to send anything, its closure when
    /// upgrading the state signals to the receivers
    InProgress(RequestedType, watch::Sender<()>, watch::Receiver<()>),
    /// Is loaded and has some neighbours partially/fully loaded, cannot yet generate slabs
    Partially(Region<SIZE, SIZE_2>),
    /// Is loaded and has all neighbours partially/fully loaded, can generate slabs
    Fully(Region<SIZE, SIZE_2>),
}

#[derive(Copy, Clone)]
#[cfg_attr(test, derive(Debug))]
pub(crate) enum RequestedType {
    /// Requested to be fully loaded with all neighbours loaded
    Central,
    /// Requested as a neighbour only
    Neighbour,
}

enum WaitOnCentral<'a, const SIZE: usize, const SIZE_2: usize> {
    /// Do not wait on central region as it's already in progress/finished, wait on neighbours only
    Nope,
    /// Wait on central region as it's already reserved
    Wait,
    /// Wait on central region and request it as it's unloaded
    RequestAndWait(RwLockWriteGuard<'a, RegionLoadState<SIZE, SIZE_2>>),
}

/// A reference to a Region within its Partially or Loaded state
pub struct LoadedRegionRef<'a, const SIZE: usize, const SIZE_2: usize> {
    /// Ensures the state can't change while this reference exists
    _guard: RwLockReadGuard<'a, RegionLoadState<SIZE, SIZE_2>>,
    region: &'a Region<SIZE, SIZE_2>,
}

#[derive(Default, Deref)]
struct RegionEntry<const SIZE: usize, const SIZE_2: usize>(RwLock<RegionLoadState<SIZE, SIZE_2>>);

/// Ensures that region loading occurs in an outward pattern from the initial set. This prevents
/// random access to regions on the planet (after the initial load), but ensures regional features
/// always spread out and merge properly, instead of multiple distinct features having to merge late
/// after generation.
#[derive(Debug, Clone, Copy)]
enum LoadSource<const SIZE: usize> {
    /// No extra loading needed for initial region
    Initial,

    /// Load regions in a straight line from the given nearest region to the one being loaded
    Nearest(RegionLocation<SIZE>),
}

impl<const SIZE: usize, const SIZE_2: usize> Regions<SIZE, SIZE_2> {
    pub fn new(params: PlanetParamsRef) -> Self {
        let planet_size = params.planet_size as usize;
        Regions {
            params,
            region_grid: DynamicGrid::new([planet_size, planet_size, 1]),
            region_continuations: Mutex::new(HashMap::with_capacity(64)),
            slab_continuations: Arc::new(Mutex::new(HashMap::with_capacity(64))),
            is_initial_region: AtomicBool::new(true),
            #[cfg(test)]
            created_regions: Default::default(),
        }
    }

    /// Loads requested region and all neighbours in preparation for slab generation
    pub(crate) async fn get_or_create(
        &self,
        location: RegionLocation<SIZE>,
        continents: &ContinentMap,
    ) -> Option<LoadedRegionRef<'_, SIZE, SIZE_2>> {
        let entry = self.entry_checked(location)?;

        // safety: MUST be fully loaded
        macro_rules! fully_loaded {
            ($entry_ro:expr, $extra_log:literal) => {
                {
                    let region_ref = entry.region_ref_with_guard($entry_ro).await;
                    trace!(concat!("region is already fully loaded", $extra_log); "region" => ?location);

                    // ensure no dangling continuations for this completed region
                    if cfg!(debug_assertions) {
                        let continuations_guard = self.region_continuations().lock().await;
                        let continuations = continuations_guard.get(&location);
                        assert!(continuations.is_none(), "dangling region continuations for {:?}: {:?}", location, continuations.unwrap());
                    }

                    Some(region_ref)
                }
            };
        }

        // fast path for when the region is already fully loaded, take ro lock only
        {
            let entry_ro = entry.0.read().await;
            if let RegionLoadState::Fully(region) = &*entry_ro {
                #[cfg(debug_assertions)]
                {
                    // ensure that no gutted features remain
                    assert!(
                        region.all_features().all(|f| !f.is_boundary_empty()),
                        "empty boundary on feature in {:?}: {:#?}",
                        location,
                        region.all_features().collect_vec()
                    );
                }

                // already fully loaded, nothing to do
                // safety: in fully loaded branch
                return unsafe { fully_loaded!(entry_ro, " (fast path)") };
            }
        }

        // lookup existing state with rw lock
        let entry_rw = entry.0.write().await;

        match &*entry_rw {
            RegionLoadState::Unloaded => {
                let source = self.find_load_source(location);
                trace!("load source for region"; "region" => ?location, "source" => ?source);

                if let LoadSource::Nearest(nearest) = source {
                    let (this_x, this_y) = location.xy_i();
                    let (nearest_x, nearest_y) = nearest.xy_i();
                    let (dx, dy) = (this_x - nearest_x, this_y - nearest_y);

                    if dx.abs() <= 2 && dy.abs() <= 2 {
                        trace!("unloaded region is a direct neighbour of an existing region"; "region" => ?location);
                    } else {
                        trace!("loading regions in a line between this region and nearest loaded"; "region" => ?location, "nearest" => ?nearest, "distance" => ?(dx,dy));

                        let line = {
                            let mut line = Bresenham::new((nearest_x, nearest_y), (this_x, this_y));

                            // skip repeat of nearest region
                            let _skipped = line.next();
                            debug_assert_eq!(_skipped, Some((nearest_x, nearest_y)));

                            // skip last end point too, via a painful temporary allocation
                            let full_line = line.collect::<SmallVec<[(i32, i32); 8]>>();

                            let len = full_line.len();
                            full_line.into_iter().take(len - 1)
                        };

                        // load all regions along the line concurrently, blocking until they're complete
                        join_on(line.map(|(rx, ry)| {

                            // safety: both live longer than local scope, as we are joining on these tasks
                            let self_local;
                            let continents_local;
                            unsafe {
                                self_local = &*(self as *const Self);
                                continents_local = &*(continents as *const ContinentMap)
                            }

                            let region = RegionLocation::new(rx as u32, ry as u32);
                                let entry = self.entry_unchecked(region);

                                if let Ok(mut w) = entry.0.try_write() {
                                    // don't overwrite more advanced states
                                    if let RegionLoadState::Unloaded = *w {
                                        trace!("marking nearby region as requested"; "region" => ?region);
                                        *w = RegionLoadState::Requested(RequestedType::Neighbour);
                                    }
                                } else {
                                    trace!("can't mark nearby region as requested, it must be currently being requested already"; "region" => ?region);
                                }

                            self_local.request_region(region, continents_local)

                        })).await;
                    }
                }

                // load self and all neighbours now that adjacent regions are definitely loaded
                trace!("region is unloaded, loading self and all neighbours"; "region" => ?location);
                self.request_all_neighbours(
                    continents,
                    location,
                    WaitOnCentral::RequestAndWait(entry_rw),
                )
                .await;
            }
            RegionLoadState::Requested(_) => {
                // already requested but not yet started
                drop(entry_rw);
                trace!("region is already requested, loading its neighbours"; "region" => ?location);
                self.request_all_neighbours(continents, location, WaitOnCentral::Wait)
                    .await;
            }
            RegionLoadState::InProgress(ty, _, rx) => {
                // already loading, wait for it to finish
                let channel = rx.clone();
                let ty = *ty;
                drop(entry_rw);

                // ensure all neighbours are loading/loaded
                let load_task = async {
                    if let RequestedType::Neighbour = ty {
                        trace!("region is already requested as just a neighbour, loading its neighbours"; "region" => ?location);
                        self.request_all_neighbours(continents, location, WaitOnCentral::Nope)
                            .await;
                    }
                };

                let wait = wait_for_loading_region(location, channel);

                // wait for central and neighbours concurrently
                futures::join!(load_task, wait);
            }
            RegionLoadState::Partially(_) => {
                drop(entry_rw);

                // self is already loaded and maybe some neighbours, load all neighbours
                trace!("region is partially loaded, loading all neighbours"; "region" => ?location);
                self.request_all_neighbours(continents, location, WaitOnCentral::Nope)
                    .await;
            }
            RegionLoadState::Fully(_) => {
                // already fully loaded, nothing to do
                // safety: in fully loaded branch
                return unsafe { fully_loaded!(entry_rw.downgrade(), "") };
            }
        }

        // now self state can be updated to fully loaded
        trace!("upgrading from partially to fully loaded"; "region" => ?location);
        match entry.upgrade_from_partially_to_fully().await {
            Ok(region) => Some(region),
            Err(_) => panic!("expected region to be partially loaded by now"),
        }
    }

    async fn request_all_neighbours(
        &self,
        continents: &ContinentMap,
        centre: RegionLocation<SIZE>,
        central: WaitOnCentral<'_, SIZE, SIZE_2>,
    ) {
        // calculate neighbours
        let neighbours = self
            .neighbouring_regions(centre)
            .collect::<ArrayVec<_, 8>>();

        // mark neighbours as requested
        for neighbour in neighbours.iter().copied() {
            let entry = self.entry_unchecked(neighbour);
            if let Ok(mut w) = entry.0.try_write() {
                // don't overwrite more advanced states
                if let RegionLoadState::Unloaded = *w {
                    trace!("marking neighbour region as requested"; "region" => ?neighbour);
                    *w = RegionLoadState::Requested(RequestedType::Neighbour);
                }
            } else {
                trace!("can't mark neighbour region as requested, it must be currently being requested already"; "region" => ?neighbour);
            }
        }

        // mark centre as requested if needed
        let centre_iter = match central {
            WaitOnCentral::Nope => None,
            WaitOnCentral::Wait => Some(centre),
            WaitOnCentral::RequestAndWait(mut lock) => {
                trace!("marking central region as requested"; "region" => ?centre);
                debug_assert!(matches!(&*lock, RegionLoadState::Unloaded));
                *lock = RegionLoadState::Requested(RequestedType::Central);
                Some(centre)
            }
        };

        join_on(
            centre_iter
                .into_iter()
                .chain(neighbours.into_iter())
                .map(|region| {
                    let self_local: &Self;
                    let continents_local: &ContinentMap;
                    // safety: both live longer than local scope, as we are joining on these tasks
                    unsafe {
                        self_local = &*(self as *const Self);
                        continents_local = &*(continents as *const ContinentMap)
                    }
                    self_local.request_region(region, continents_local)
                }),
        )
        .await;
    }

    /// Expects to be requested already
    async fn request_region(&self, region: RegionLocation<SIZE>, continents: &ContinentMap) {
        let entry = self.entry_unchecked(region);
        let mut inner = entry.0.write().await;

        match &*inner {
            RegionLoadState::Unloaded => {
                unreachable!("region {:?} should be already requested", region)
            }
            RegionLoadState::Requested(ty) => {
                // requested already, load in this function
                trace!("reserving request"; "region" => ?region);
                *inner = RegionLoadState::in_progress(*ty);
                drop(inner);
            }
            RegionLoadState::InProgress(_, _, rx) => {
                // just wait
                let channel = rx.clone();
                drop(inner);

                wait_for_loading_region(region, channel).await;
                return;
            }
            RegionLoadState::Partially(_) | RegionLoadState::Fully(_) => {
                // already loaded
                return;
            }
        };

        // do actual loading
        let self_local: &Self;
        let continents_local: &ContinentMap;
        // safety: both live longer than local scope, as we are joining on these tasks
        unsafe {
            self_local = &*(self as *const Self);
            continents_local = &*(continents as *const ContinentMap)
        }
        self_local
            .create_single_region(region, continents_local)
            .await;
    }

    /// Loads the given region only, no neighbours. Assumes already unloaded and state is already
    /// set to InProgress
    async fn create_single_region(
        &self,
        location: RegionLocation<SIZE>,
        continents: &ContinentMap,
    ) {
        debug_assert!(
            matches!(
                *self.entry_unchecked(location).0.read().await,
                RegionLoadState::InProgress(_, _, _)
            ),
            "region {:?} should already be reserved",
            location
        );

        #[cfg(test)]
        {
            // log for tests
            self.created_regions.lock().await.push(location);
        }

        // init region chunks and discover regional features
        let region = Region::<SIZE, SIZE_2>::create(location, continents, self).await;

        // update state to partial
        let entry = self.entry_unchecked(location);
        let mut entry_rw = entry.0.write().await;
        match &*entry_rw {
            RegionLoadState::InProgress(_, _, _) => {
                // as expected
            }
            state => unreachable!(
                "finished creation in bad state: {:?}",
                RegionLoadStateDiscriminants::from(state)
            ),
        }

        trace!("upgrading region state to partially loaded"; "region" => ?location);
        *entry_rw = RegionLoadState::Partially(region);
    }

    pub async fn get_existing(
        &self,
        region: RegionLocation<SIZE>,
    ) -> Option<LoadedRegionRef<'_, SIZE, SIZE_2>> {
        let entry = self.entry_checked(region)?;
        let ro = entry.0.read().await;
        match &*ro {
            RegionLoadState::Partially(_) | RegionLoadState::Fully(_) => {
                // safety: partially or fully loaded
                let region_ref = unsafe { entry.region_ref_with_guard(ro).await };
                Some(region_ref)
            }
            _ => None,
        }
    }

    pub fn slab_continuations(&self) -> SlabContinuations {
        Arc::clone(&self.slab_continuations)
    }

    pub(in crate::region) fn region_continuations(&self) -> &RegionContinuations<SIZE> {
        &self.region_continuations
    }

    pub fn params(&self) -> &PlanetParamsRef {
        &self.params
    }

    /// None if out of range of the planet
    fn entry_checked(&self, region: RegionLocation<SIZE>) -> Option<&RegionEntry<SIZE, SIZE_2>> {
        self.params
            .is_region_in_range(region)
            .as_some_from(|| self.entry_unchecked(region))
    }

    fn entry_unchecked(&self, region: RegionLocation<SIZE>) -> &RegionEntry<SIZE, SIZE_2> {
        debug_assert!(
            self.params.is_region_in_range(region),
            "region is out of range: {:?}",
            region
        );

        let (rx, ry) = region.xy();
        &self.region_grid[[rx as usize, ry as usize, 0]]
    }

    fn find_load_source(&self, region: RegionLocation<SIZE>) -> LoadSource<SIZE> {
        if self.is_initial_region.swap(false, Ordering::Relaxed) {
            LoadSource::Initial
        } else {
            // find nearest loaded region
            let (rx, ry) = region.xy();
            let bounds = 0..self.params.planet_size as i32;
            let nearby = spiral::ManhattanIterator::new(
                rx as i32,
                ry as i32,
                self.params.planet_size as u16,
            )
            .skip(1) // skip self
            .filter_map(|(x, y)| {
                if bounds.contains(&x) && bounds.contains(&y) {
                    Some(RegionLocation::new(x as u32, y as u32))
                } else {
                    None
                }
            })
            .find(|region| {
                let entry = self.entry_unchecked(*region);
                match entry.0.try_read() {
                    Err(_) => {
                        // if the lock is already taken it must be in progress
                        true
                    }
                    Ok(guard) => !matches!(*guard, RegionLoadState::Unloaded),
                }
            });

            match nearby {
                Some(nearby) if nearby != region => LoadSource::Nearest(nearby),
                _ => unreachable!(
                    "should have found a nearby region already loaded that isn't itself: {:?}",
                    nearby
                ),
            }
        }
    }

    fn neighbouring_regions(
        &self,
        centre: RegionLocation<SIZE>,
    ) -> impl Iterator<Item = RegionLocation<SIZE>> + Clone + '_ {
        neighbouring_regions_with_params(centre, &self.params)
    }

    /// Partially or fully loaded. Region assumed to be valid
    pub async fn is_region_loaded(&self, region: RegionLocation<SIZE>) -> bool {
        let entry = self.entry_unchecked(region);
        let ro = entry.0.read().await;
        matches!(&*ro, RegionLoadState::Partially(_) | RegionLoadState::Fully(_))
    }

    pub async fn try_replace_feature(
        &self,
        region: RegionLocation<SIZE>,
        current: &SharedRegionalFeature,
        new: SharedRegionalFeature,
    ) {
        // ensure modified region has not yet generated any terrain
        if cfg!(debug_assertions) {
            let region_chunk_bounds = {
                let (min, max) = region.chunk_bounds();
                (min.0..max.0, min.1..max.1)
            };

            let is_in_region = {
                |slab: &SlabLocation| {
                    let (xs, ys) = &region_chunk_bounds;
                    let (x, y) = slab.chunk.xy();
                    xs.contains(&x) && ys.contains(&y)
                }
            };

            let bad_slabs = self
                .slab_continuations
                .lock()
                .await
                .iter()
                .filter(|(slab, _)| is_in_region(slab))
                .filter_map(|(slab, state)| {
                    if let SlabContinuation::Loaded = state {
                        Some(*slab)
                    } else {
                        None
                    }
                })
                .collect_vec();

            assert_eq!(
                bad_slabs,
                vec![],
                "some slabs have already been generated in region {:?} where feature {:?} is being replaced",
                region,
                current.ptr_debug()
            );
        }

        debug!("applying feature replacement"; "region" => ?region, "current" => ?current.ptr_debug(), "new" => ?new.ptr_debug());
        self.with_loaded_region_mut(region, |r| {
            if !r.replace_feature(current, new) {
                warn!("feature not found for replacement"; "region" => ?region, "feature" => ?current.ptr_debug());
            }
        }).await;
    }

    /// Partially or fully loaded. Region assumed to be valid
    pub async fn with_loaded_region_mut(
        &self,
        region: RegionLocation<SIZE>,
        dew_it: impl FnOnce(&mut Region<SIZE, SIZE_2>),
    ) {
        let entry = self.entry_unchecked(region);
        let mut guard = entry.0.write().await;
        match &mut *guard {
            RegionLoadState::Partially(r) | RegionLoadState::Fully(r) => dew_it(r),
            _ => {}
        };
    }
}

impl<const SIZE: usize, const SIZE_2: usize> Default for RegionLoadState<SIZE, SIZE_2> {
    fn default() -> Self {
        Self::Unloaded
    }
}

async fn wait_for_loading_region<const SIZE: usize>(
    region: RegionLocation<SIZE>,
    mut channel: watch::Receiver<()>,
) {
    trace!("waiting for loading region to finish"; "region" => ?region);

    // ignore error returned when channel is closed, because that indicates the region has
    // been upgraded from Requested to Partially loaded
    let _ = channel.changed().await;
    trace!("woke up from waiting for requested region to finish"; "region" => ?region);
}

fn neighbouring_regions_with_params<const SIZE: usize>(
    centre: RegionLocation<SIZE>,
    params: &PlanetParams,
) -> impl Iterator<Item = RegionLocation<SIZE>> + Clone + '_ {
    const NEIGHBOUR_OFFSETS: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];

    IntoIter::new(NEIGHBOUR_OFFSETS)
        .filter_map(move |offset| centre.try_add_offset_with_params(offset, params))
}

impl<const SIZE: usize, const SIZE_2: usize> RegionEntry<SIZE, SIZE_2> {
    /// # Safety
    /// Must be in the Partially or Fully loaded state
    async unsafe fn region_ref_with_guard<'a>(
        &self,
        guard: RwLockReadGuard<'a, RegionLoadState<SIZE, SIZE_2>>,
    ) -> LoadedRegionRef<'a, SIZE, SIZE_2> {
        use RegionLoadState::*;

        let region = match &*guard {
            Fully(region) | Partially(region) => &*(region as *const Region<SIZE, SIZE_2>),
            _ => {
                if cfg!(debug_assertions) {
                    panic!("region must be partially or fully loaded to get a reference");
                }
                unreachable_unchecked()
            }
        };

        LoadedRegionRef {
            _guard: guard,
            region,
        }
    }

    async fn upgrade_from_partially_to_fully(
        &self,
    ) -> Result<LoadedRegionRef<'_, SIZE, SIZE_2>, ()> {
        let mut guard = self.0.write().await;

        // ensure partially loaded currently
        match &*guard {
            RegionLoadState::Partially(_) => { /* needs upgrade */ }
            RegionLoadState::Fully(_) => unsafe {
                // already fully loaded
                return Ok(self.region_ref_with_guard(guard.downgrade()).await);
            },
            state => {
                error!("state is {:?}", RegionLoadStateDiscriminants::from(state));
                return Err(());
            }
        }

        // steal region out of enum, temporarily replacing with Unloaded
        // TODO move directly with pointer magic instead
        let partially = std::mem::replace(&mut *guard, RegionLoadState::Unloaded);
        match partially {
            RegionLoadState::Partially(region) => {
                // set to fully loaded
                *guard = RegionLoadState::Fully(region);

                unsafe { Ok(self.region_ref_with_guard(guard.downgrade()).await) }
            }
            _ => {
                if cfg!(debug_assertions) {
                    unreachable!()
                }
                // safety: just checked state
                unsafe { unreachable_unchecked() }
            }
        }
    }
}

impl<const SIZE: usize, const SIZE_2: usize> Deref for LoadedRegionRef<'_, SIZE, SIZE_2> {
    type Target = Region<SIZE, SIZE_2>;

    fn deref(&self) -> &Self::Target {
        self.region
    }
}

impl<const SIZE: usize, const SIZE_2: usize> RegionLoadState<SIZE, SIZE_2> {
    fn in_progress(ty: RequestedType) -> Self {
        let (tx, rx) = watch::channel(());
        RegionLoadState::InProgress(ty, tx, rx)
    }
}

async fn join_on<F: Future<Output = O> + Send + 'static, O: Send + 'static>(
    futures: impl Iterator<Item = F>,
) {
    let mut futures = futures
        .map(tokio::task::spawn)
        .collect::<FuturesUnordered<_>>();

    let mut panics = Vec::new();
    while let Some(res) = futures.next().await {
        if let Err(err) = res {
            if let Ok(panic) = err.try_into_panic() {
                panics.push(panic);
            }
        }
    }

    if !panics.is_empty() {
        crit!("{n} region loading task(s) panicked", n = panics.len());

        // panic with the first only
        let panic = panics.swap_remove(0);
        std::panic::panic_any(panic);
    }
}

#[cfg(test)]
mod tests {
    use crate::PlanetParams;

    use super::*;
    use std::time::Duration;
    use unit::dim::SmallUnsignedConstant;

    const SIZE: SmallUnsignedConstant = SmallUnsignedConstant::new(2);
    const PLANET_SIZE: u32 = 32;
    type SmolRegionLocation = RegionLocation<2>;
    type SmolRegion = Region<2, 4>;
    type SmolRegions = Regions<2, 4>;

    fn init() -> (&'static mut SmolRegions, &'static mut ContinentMap) {
        let params = {
            let mut params = PlanetParams::dummy();
            let mut params_mut = PlanetParamsRef::get_mut(&mut params).unwrap();
            params_mut.planet_size = PLANET_SIZE;
            params_mut.max_continents = 4;
            params
        };
        let regions = Box::new(SmolRegions::new(params.clone()));
        let continents = Box::new(ContinentMap::new_with_rng(
            params.clone(),
            &mut thread_rng(),
        ));

        // by boxing and leaking these, they are not destroyed prematurely during a panic and
        // therefore don't cause other non-panicked worker threads to segfault
        (Box::leak(regions), Box::leak(continents))
    }

    async fn load_regions(regions_to_request: impl Iterator<Item = (u32, u32)>) -> SmolRegions {
        let (regions, continents) = init();

        let mut regions_to_request = regions_to_request.sorted().collect_vec();
        join_on(regions_to_request.iter().copied().map(|(x, y)| {
            let regions_local = unsafe { &*(regions as *const SmolRegions) };
            let continents_local = unsafe { &*(continents as *const ContinentMap) };
            regions_local.get_or_create(SmolRegionLocation::new(x, y), continents_local)
        }))
        .await;

        // collect all loaded regions
        let mut partial = vec![];
        let mut full = vec![];
        for ([x, y, _], entry) in regions.region_grid.iter_coords() {
            let guard = entry
                .0
                .try_read()
                .expect("should be able to lock immediately");
            let loc = (x as u32, y as u32);
            match &*guard {
                RegionLoadState::Unloaded => continue,
                RegionLoadState::InProgress(ty, _, _) | RegionLoadState::Requested(ty) => {
                    panic!("region is still requested as {:?}", ty)
                }
                RegionLoadState::Partially(_) => partial.push(loc),
                RegionLoadState::Fully(_) => full.push(loc),
            }
        }
        partial.sort_unstable_by_key(|(reg, _)| *reg);
        full.sort_unstable_by_key(|(reg, _)| *reg);

        regions_to_request.dedup();
        partial.dedup();
        full.dedup();

        // do some common checks
        assert_eq!(full, regions_to_request, "incorrect fully loaded regions");
        assert_eq!(
            partial,
            sorted_neighbours_for_all(regions_to_request.iter().copied()),
            "incorrect partially loaded regions"
        );

        // ensure no dupe loading
        let mut all_creations = regions.created_regions.lock().await;
        let len_before = all_creations.len();
        all_creations.sort();
        all_creations.dedup();
        assert_eq!(len_before, all_creations.len(), "duplicate slab creation");

        drop(all_creations);

        // safety: came from a leaked box
        unsafe { *Box::from_raw(regions) }
    }

    fn sorted_neighbours(centre: (u32, u32)) -> Vec<(u32, u32)> {
        let mut params = PlanetParams::dummy();
        let mut params_mut = PlanetParamsRef::get_mut(&mut params).unwrap();
        params_mut.planet_size = PLANET_SIZE;

        neighbouring_regions_with_params(SmolRegionLocation::new(centre.0, centre.1), &params)
            .map(|r| r.xy())
            .sorted()
            .collect()
    }

    fn sorted_neighbours_for_all(regions: impl Iterator<Item = (u32, u32)>) -> Vec<(u32, u32)> {
        let central = regions.collect_vec();
        let mut all = central.iter().copied().fold(vec![], |mut acc, r| {
            acc.extend(sorted_neighbours(r));
            acc
        });

        // remove central regions from partial list
        all.retain(|r| !central.contains(r));
        all.sort();
        all.dedup();
        all
    }

    /// Just request a single region
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn region_requesting_control() {
        let regions_to_request = vec![(5, 5)];
        load_regions(regions_to_request.into_iter()).await;
    }

    /// 2 concurrent requests for the same central region
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn region_requesting_same_twice() {
        let regions_to_request = vec![
            (5, 5), // arbitrary region
            (5, 5), // repeated request
            (5, 5), // repeated request
        ];

        load_regions(regions_to_request.into_iter()).await;
    }

    /// Concurrent request to a region already requested as a neighbour
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn region_requesting_with_repeated_neighbours() {
        let regions_to_request = vec![
            (5, 5), // arbitrary region
            (6, 5), // a neighbour already requested
            (4, 5), // a neighbour already requested
        ];

        load_regions(regions_to_request.iter().copied()).await;
    }

    /// Concurrent request to a region that shares neighbours
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn region_requesting_with_common_neighbour() {
        let regions_to_request = vec![
            (5, 5), // arbitrary region
            (7, 5), // a further region that share neighbours
        ];

        load_regions(regions_to_request.iter().copied()).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn region_requesting_separate_directions() {
        let regions_to_request = vec![
            (5, 5), // arbitrary region
            (8, 5), // multiple regions away with no shared neighbours
        ];

        load_regions(regions_to_request.iter().copied()).await;
    }

    fn test_world_timeout() -> Duration {
        let seconds = std::env::var("NN_TEST_WORLD_TIMEOUT")
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(5);

        Duration::from_secs(seconds)
    }

    /// Try to find deadlocks
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn region_requesting_random_stress_test() {
        const COUNT: usize = 200;

        let (regions, continents) = init();

        common::logging::for_tests();
        let timeout_task = tokio::task::spawn(async {
            let seconds = std::env::var("NN_TEST_WORLD_TIMEOUT")
                .ok()
                .and_then(|val| val.parse().ok())
                .unwrap_or(60);

            tokio::time::sleep(Duration::from_secs(seconds)).await;
            panic!("TIMEOUT!!!");
        });

        let mut tasks = vec![];
        let mut rando = SmallRng::from_entropy();

        macro_rules! queue {
            ($region:expr) => {
                let (rx, ry) = $region;
                let region = RegionLocation::new(rx, ry);
                let fut = regions.get_or_create(region, continents);
                let task = tokio::task::spawn(fut);
                tasks.push(task);
            };
        };

        for _ in 0..COUNT {
            let rx = rando.gen_range(0, PLANET_SIZE);
            let ry = rando.gen_range(0, PLANET_SIZE);
            queue!((rx, ry));

            if rando.gen_bool(0.6) {
                queue!((rx + 1, ry));
            }
            if rando.gen_bool(0.6) {
                queue!((rx.saturating_sub(1), ry));
            }
            if rando.gen_bool(0.6) {
                queue!((rx, ry + 1));
            }
            if rando.gen_bool(0.6) {
                queue!((rx, ry.saturating_sub(1)));
            }
        }

        join_on(tasks.into_iter()).await;

        // cancel timeout
        timeout_task.abort();

        // generic checks
        for ([x, y, _], entry) in regions.region_grid.iter_coords() {
            let guard = entry
                .0
                .try_read()
                .expect("should be able to lock immediately");
            let loc = (x as u32, y as u32);
            match &*guard {
                RegionLoadState::InProgress(ty, _, _) | RegionLoadState::Requested(ty) => {
                    panic!("region is still requested as {:?}", ty)
                }
                _ => {}
            }
        }

        // ensure no dupe loading
        let mut all_creations = regions.created_regions.lock().await;
        let len_before = all_creations.len();
        all_creations.sort();
        all_creations.dedup();
        assert_eq!(len_before, all_creations.len(), "duplicate slab creation");
    }
}
