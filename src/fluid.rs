//! Discrete source-driven fluids. Updates read the old world, then commit together.
use crate::world::{Block, MAX_Y, MIN_Y, World, hash};
use glam::IVec3;

const SIDES: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];
const NEIGHBORS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Z,
    IVec3::NEG_Z,
    IVec3::Y,
    IVec3::NEG_Y,
];

/// Only exposed sources, contacts and existing flow need an initial pulse.
/// The worker's halo includes neighboring chunks, even when they are unloaded.
pub(crate) fn exposed_frontier(key: IVec3, halo: &[Block]) -> Vec<[i32; 3]> {
    let at = |p: IVec3| halo[(p.x + 1 + 18 * (p.z + 1 + 18 * (p.y + 1))) as usize];
    let mut frontier = Vec::new();
    for y in 0..crate::world::SIZE {
        for z in 0..crate::world::SIZE {
            for x in 0..crate::world::SIZE {
                let p = IVec3::new(x, y, z);
                let Some((lava, level)) = at(p).fluid() else {
                    continue;
                };
                let exposed = NEIGHBORS.iter().any(|d| {
                    let b = at(p + *d);
                    if let Some((kind, _)) = b.fluid() {
                        kind != lava
                    } else {
                        *d != IVec3::Y && b.replaceable()
                    }
                });
                if level != 0 || exposed {
                    frontier.push((key * crate::world::SIZE + p).to_array());
                }
            }
        }
    }
    frontier
}

impl World {
    pub fn fluid_at(&self, pos: glam::Vec3) -> Option<(bool, u8)> {
        let p = pos.floor().as_ivec3();
        let block = self.get(p);
        let fluid = block.fluid()?;
        let height = if self.get(p + IVec3::Y).fluid().is_some() {
            1.
        } else {
            block.fluid_height()
        };
        (pos.y - (p.y as f32) < height).then_some(fluid)
    }

    pub fn lava_lights(&self, center: IVec3) -> Vec<glam::Vec3> {
        let mut lights: Vec<_> = self
            .edits
            .range([center.x - 24, i32::MIN, i32::MIN]..=[center.x + 24, i32::MAX, i32::MAX])
            .filter(|(p, b)| {
                (p[1] - center.y).abs() <= 24
                    && (p[2] - center.z).abs() <= 24
                    && b.fluid().is_some_and(|(lava, _)| lava)
            })
            .map(|(p, _)| IVec3::from_array(*p).as_vec3() + glam::Vec3::splat(0.5))
            .collect();
        for cx in center.x.div_euclid(64) - 1..=center.x.div_euclid(64) + 1 {
            for cz in center.z.div_euclid(64) - 1..=center.z.div_euclid(64) + 1 {
                let h = hash(self.seed ^ 0x706f6f6c, cx, 0, cz);
                let x = cx * 64 + 16 + ((h >> 8) % 32) as i32;
                let z = cz * 64 + 16 + ((h >> 24) % 32) as i32;
                let p = IVec3::new(x, self.height(x, z) - 1, z);
                if (p - center).abs().max_element() <= 24 && self.get(p) == Block::Lava {
                    lights.push(p.as_vec3() + glam::Vec3::splat(0.5));
                }
            }
        }
        let mut groups = std::collections::BTreeMap::new();
        lights.retain(|p| {
            !self
                .get(p.floor().as_ivec3() + IVec3::Y)
                .fluid()
                .is_some_and(|(lava, _)| lava)
        });
        for p in lights {
            let cell = p.floor().as_ivec3().div_euclid(IVec3::splat(4)).to_array();
            groups.entry(cell).or_insert(p);
        }
        groups.into_values().collect()
    }

    pub fn wake_fluid(&mut self, p: IVec3) {
        self.fluid_pending.insert(p.to_array());
        for d in NEIGHBORS {
            self.fluid_pending.insert((p + d).to_array());
        }
    }

    /// At most 512 frontier cells per pulse; distant frontiers sleep until approached.
    pub fn tick_fluids(&mut self, center: IVec3, lava_tick: bool) {
        let cursor = self.fluid_cursor.unwrap_or([i32::MIN; 3]);
        let ready: Vec<_> = self
            .fluid_pending
            .range((
                std::ops::Bound::Excluded(cursor),
                std::ops::Bound::Unbounded,
            ))
            .chain(self.fluid_pending.range(..=cursor))
            .copied()
            .filter(|p| (IVec3::from_array(*p) - center).abs().max_element() <= 64)
            .take(512)
            .collect();
        if let Some(&last) = ready.last() {
            self.fluid_cursor = Some(last);
        }
        let mut candidates = std::collections::BTreeSet::new();
        for key in ready {
            self.fluid_pending.remove(&key);
            let p = IVec3::from_array(key);
            candidates.insert(key);
            for d in NEIGHBORS {
                candidates.insert((p + d).to_array());
            }
        }
        let mut queries = crate::world::BlockQueries::default();
        let mut changes = Vec::new();
        for key in candidates {
            let p = IVec3::from_array(key);
            if !(MIN_Y + 1..=MAX_Y).contains(&p.y) {
                continue;
            }
            let old = queries.get(self, p);
            if !old.replaceable() {
                continue;
            }
            let neighbors = SIDES.map(|d| queries.get(self, p + d));
            if old.fluid().is_some_and(|(lava, _)| lava)
                && NEIGHBORS.iter().any(|d| {
                    queries
                        .get(self, p + *d)
                        .fluid()
                        .is_some_and(|(lava, _)| !lava)
                })
            {
                changes.push((
                    p,
                    if old == Block::Lava {
                        Block::Obsidian
                    } else {
                        Block::Cobble
                    },
                ));
                continue;
            }
            let mut next = Block::Air;
            for lava in [false, true] {
                if old.fluid().is_some_and(|(kind, _)| kind != lava) {
                    continue;
                }
                if lava && !lava_tick {
                    if old.fluid().is_some()
                        || NEIGHBORS
                            .iter()
                            .any(|d| queries.get(self, p + *d).fluid().is_some_and(|(l, _)| l))
                    {
                        self.fluid_pending.insert(key);
                    }
                    continue;
                }
                let source = if lava { Block::Lava } else { Block::Water };
                let below = queries.get(self, p - IVec3::Y);
                let renewable = !lava
                    && neighbors.iter().filter(|b| **b == Block::Water).count() >= 2
                    && (below.solid() || below == Block::Water)
                    && !self.natural_fall(p);
                let level = if old == source || renewable {
                    Some(0)
                } else if queries
                    .get(self, p + IVec3::Y)
                    .fluid()
                    .is_some_and(|(l, _)| l == lava)
                {
                    Some(8)
                } else {
                    SIDES
                        .iter()
                        .filter_map(|d| {
                            let n = p + *d;
                            let (kind, level) = queries.get(self, n).fluid()?;
                            let floor = queries.get(self, n - IVec3::Y);
                            // A falling/flowing column landing in a pool must
                            // join that pool, not spread a raised layer over it.
                            if kind != lava
                                || !(floor.solid()
                                    || (level == 0 && floor.fluid() == Some((lava, 0))))
                            {
                                return None;
                            }
                            let level = if level == 8 { 0 } else { level };
                            let level = level + if lava { 2 } else { 1 };
                            (level <= if lava { 6 } else { 7 }).then_some(level)
                        })
                        .min()
                };
                if let Some(level) = level {
                    next = match (lava, level) {
                        (false, 0) => Block::Water,
                        (true, 0) => Block::Lava,
                        (false, n) => Block::FlowingWater(n),
                        (true, n) => Block::FlowingLava(n),
                    };
                    break;
                }
            }
            // Non-fluid occupants survive unless displaced by incoming flow.
            if next == Block::Air && old.fluid().is_none() {
                continue;
            }
            if old.fluid().is_some_and(|(l, _)| l) && !lava_tick {
                continue;
            }
            if old != next {
                changes.push((p, next));
            }
        }
        drop(queries); // The cached snapshot ends before committing changes.
        for (p, b) in changes {
            self.set(p, b);
        }
    }

    /// Irregular sediment-lined ponds (stone-lined lava) outside the protected spawn.
    pub(crate) fn pool_block(&self, p: IVec3, height: i32) -> Option<Block> {
        if p.x.abs() < 32 && p.z.abs() < 32 {
            return None;
        }
        let cx = p.x.div_euclid(64);
        let cz = p.z.div_euclid(64);
        let h = hash(self.seed ^ 0x706f6f6c, cx, 0, cz);
        if !h.is_multiple_of(3) {
            return None;
        }
        let x = cx * 64 + 16 + ((h >> 8) % 32) as i32;
        let z = cz * 64 + 16 + ((h >> 24) % 32) as i32;
        let dx = p.x - x;
        let dz = p.z - z;
        let radius = 2.8 + ((h >> 48) % 12) as f32 * 0.5;
        let edge = crate::hydrology::pond_edge(h, dx, dz, radius);
        if edge > 1. {
            return None;
        }
        let y = self.height(x, z);
        if y <= crate::world::SEA_LEVEL + 4 || !self.pond_contained(cx, cz, x, z, y, radius) {
            return None;
        }
        // Reject the whole pool on steep terrain, keeping a complete containing rim.
        if [(-5, -5), (-5, 5), (5, -5), (5, 5)]
            .iter()
            .any(|(a, b)| (self.height(x + a, z + b) - y).abs() > 2)
        {
            return None;
        }
        let lava = (h >> 40).is_multiple_of(5);
        if !(y - 3..=height.max(y) + 1).contains(&p.y) {
            return None;
        }
        let floor = y - 2 - i32::from(edge < -1.8);
        if p.y <= floor || edge >= 0. {
            return (p.y <= height.min(y - i32::from(edge < 0.))).then_some(if lava {
                Block::Stone
            } else {
                match hash(
                    self.seed ^ 0x62616e6b,
                    p.x.div_euclid(2),
                    0,
                    p.z.div_euclid(2),
                ) % 5
                {
                    0 => Block::Gravel,
                    1 => Block::Dirt,
                    _ => Block::Sand,
                }
            });
        }
        Some(if p.y < y {
            if lava {
                Block::Lava
            } else if p.y == y - 1 && self.biome(x, z).is_snow() {
                Block::Ice
            } else {
                Block::Water
            }
        } else {
            Block::Air
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn basin() -> (World, IVec3) {
        let mut w = World::new(41);
        let p = IVec3::new(15, 200, -16);
        for x in -9..=9 {
            for z in -9..=9 {
                w.set(p + IVec3::new(x, -1, z), Block::Stone);
            }
        }
        w.fluid_pending.clear();
        (w, p)
    }
    fn advance(w: &mut World, p: IVec3, ticks: usize) {
        for _ in 0..ticks {
            w.tick_fluids(p, true);
        }
    }
    #[test]
    fn untouched_lake_spillways_activate_when_streamed() {
        let mut w = World::new(41);
        let (source, spill) = crate::water_review::find_spill(&w).expect("natural lake spillway");
        assert!(w.edits.is_empty());
        let start = std::time::Instant::now();
        loop {
            w.stream(source.as_vec3(), 1);
            w.meshes.clear();
            let [_, queued, active] = w.stream_counts();
            if queued + active == 0 {
                break;
            }
            assert!(
                start.elapsed().as_secs() < 10,
                "spillway streaming timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(w.fluid_pending.contains(&spill.to_array()));
        let pending = std::mem::take(&mut w.fluid_pending);
        // An unchanged resident remesh must not requeue the entire body of water.
        w.dirty.push(crate::world::chunk(source));
        let start = std::time::Instant::now();
        loop {
            w.stream(source.as_vec3(), 1);
            w.meshes.clear();
            let [_, queued, active] = w.stream_counts();
            if queued + active == 0 {
                break;
            }
            assert!(start.elapsed().as_secs() < 10, "spillway remesh timed out");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(w.fluid_pending.is_empty());
        w.fluid_pending = pending;
        advance(&mut w, source, 60);
        assert_eq!(w.get(spill - IVec3::Y), Block::FlowingWater(8));
        assert_eq!(w.get(source), Block::Water);
    }

    #[test]
    fn shoreline_level_cache_is_bounded_and_does_not_change_generation() {
        let w = World::new(41);
        let (source, _) = crate::water_review::find_spill(&w).unwrap();
        let before = w.waterway_column(source.x, source.z);
        for cx in -16..16 {
            for cz in -16..16 {
                let h = hash(w.seed ^ 0x7269_7665, cx, 0, cz);
                w.waterway_column(
                    cx * 384 + 144 + ((h >> 8) % 96) as i32,
                    cz * 384 + 144 + ((h >> 24) % 96) as i32,
                );
                assert!(w.waterway_cache_len() <= 64);
            }
        }
        assert_eq!(w.waterway_column(source.x, source.z), before);
        assert_eq!(World::new(41).waterway_column(source.x, source.z), before);
    }

    #[test]
    fn lake_banks_blend_into_terrain_without_retaining_walls() {
        let mut checked_bank = false;
        let mut sloped_bank = false;
        for seed in [1, 41, 314159] {
            let w = World::new(seed);
            for cx in -4..4 {
                for cz in -4..4 {
                    let h = hash(w.seed ^ 0x7269_7665, cx, 0, cz);
                    let x = cx * 384 + 144 + ((h >> 8) % 96) as i32;
                    let z = cz * 384 + 144 + ((h >> 24) % 96) as i32;
                    if w.waterway_column(x, z).is_none() {
                        continue;
                    }
                    for dx in (-124..=124).step_by(4) {
                        for dz in (-124..=124).step_by(4) {
                            let (px, pz) = (x + dx, z + dz);
                            let Some((edge, level)) = w.waterway_column(px, pz) else {
                                continue;
                            };
                            if edge < 0. || level == crate::world::SEA_LEVEL {
                                continue;
                            }
                            checked_bank = true;
                            let height = w.height(px, pz);
                            // No solid bank may be generated above the original ground.
                            assert!(matches!(
                                w.waterway_block(IVec3::new(px, height + 1, pz), height),
                                None | Some(Block::Air)
                            ));
                            if (1.0..10.0).contains(&edge) && height > level + 3 {
                                let above_shore = IVec3::new(px, height, pz);
                                sloped_bank |=
                                    w.waterway_block(above_shore, height) == Some(Block::Air);
                            }
                        }
                    }
                }
            }
        }
        assert!(checked_bank, "contained lake shores must be generated");
        assert!(sloped_bank, "high shores must be carved into a broad slope");
    }

    #[test]
    fn busy_frontiers_cannot_starve_waterfalls() {
        let (mut w, p) = basin();
        let source = p + IVec3::Y * 4;
        w.set(source, Block::Water);
        let busy: Vec<_> = (-20..=-10)
            .flat_map(|x| (200..=240).flat_map(move |y| [-16, -15].map(|z| [x, y, z])))
            .collect();
        for _ in 0..6 {
            // Simulate continuing activity at lexicographically earlier positions.
            w.fluid_pending.extend(busy.iter().copied());
            w.tick_fluids(p, true);
        }
        assert_eq!(w.get(source - IVec3::Y), Block::FlowingWater(8));
    }

    #[test]
    fn exposed_sources_spill_over_ledges_across_chunk_boundaries_and_drain() {
        let (mut w, p) = basin(); // Source sits at x=15, z=-16.
        for y in 182..=199 {
            w.set(
                IVec3::new(16, y, -16),
                if y == 182 { Block::Stone } else { Block::Air },
            );
        }
        w.set(p, Block::Water);
        w.fluid_pending.clear();
        // Simulate first chunk acceptance, with the spill into an unloaded neighbor.
        let key = crate::world::chunk(p);
        let frontier = exposed_frontier(key, &w.halo(key));
        assert!(frontier.contains(&p.to_array()));
        w.fluid_pending.extend(frontier);
        advance(&mut w, p, 60);
        assert_eq!(w.get(IVec3::new(16, 184, -16)), Block::FlowingWater(8));
        w.set(p, Block::Air);
        advance(&mut w, p, 160);
        assert!(w.edits.values().all(|b| b.fluid().is_none()));
    }

    #[test]
    fn settled_ocean_interiors_do_not_seed_the_frontier() {
        let mut halo = vec![Block::Water; 18 * 18 * 18];
        // An open water surface above this full chunk is also settled.
        for z in 0..18 {
            for x in 0..18 {
                halo[x + 18 * (z + 18 * 17)] = Block::Air;
            }
        }
        assert!(exposed_frontier(IVec3::ZERO, &halo).is_empty());
        // A downward opening on the adjacent chunk must wake the edge source.
        halo[17 + 18 * (1 + 18)] = Block::Air;
        assert!(exposed_frontier(IVec3::ZERO, &halo).contains(&[15, 0, 0]));
        // Contact on the upper face still wakes lava/water reactions.
        halo[1 + 18 * (1 + 18 * 17)] = Block::Lava;
        assert!(exposed_frontier(IVec3::ZERO, &halo).contains(&[0, 15, 0]));
    }

    #[test]
    fn water_spreads_seven_blocks_across_chunks_and_drains() {
        let (mut w, p) = basin();
        w.set(p, Block::Water);
        advance(&mut w, p, 40);
        assert_eq!(w.get(p + IVec3::X * 7), Block::FlowingWater(7));
        assert_eq!(w.get(p + IVec3::X * 8), Block::Air);
        assert!(!w.get(p + IVec3::X).solid());
        w.set(p, Block::Air);
        advance(&mut w, p, 80);
        assert!(w.edits.values().all(|b| b.fluid().is_none()));
    }
    #[test]
    fn water_falls_before_spreading_and_reacts_to_dams() {
        let (mut w, p) = basin();
        w.set(p + IVec3::Y * 3, Block::Water);
        advance(&mut w, p, 25);
        assert_eq!(w.get(p + IVec3::Y * 2), Block::FlowingWater(8));
        assert_eq!(w.get(p + IVec3::X + IVec3::Y * 3), Block::Air);
        assert!(w.get(p + IVec3::X).fluid().is_some());
        // Enclose the source; the disconnected waterfall must drain.
        w.set(p + IVec3::Y * 2, Block::Stone);
        for d in SIDES {
            w.set(p + IVec3::Y * 3 + d, Block::Stone);
        }
        advance(&mut w, p, 90);
        assert_eq!(w.get(p), Block::Air);
    }
    #[test]
    fn waterfalls_land_in_water_without_spreading_a_raised_layer_over_the_pool() {
        let (mut w, p) = basin();
        for dx in -2..=2 {
            for dz in -2..=2 {
                w.set(p + IVec3::new(dx, 0, dz), Block::Water);
            }
        }
        w.set(p + IVec3::Y * 5, Block::Water);
        advance(&mut w, p, 60);
        assert_eq!(w.get(p + IVec3::Y), Block::FlowingWater(8));
        assert_eq!(w.get(p + IVec3::Y + IVec3::X), Block::Air);
        assert_eq!(w.get(p + IVec3::X), Block::Water);
    }

    #[test]
    fn two_sources_fill_supported_water_but_not_open_air_or_lava() {
        let (mut w, p) = basin();
        w.set(p - IVec3::X, Block::Water);
        w.set(p + IVec3::Z, Block::Water);
        w.tick_fluids(p, true);
        assert_eq!(w.get(p), Block::Water);
        w.set(p, Block::Air);
        w.tick_fluids(p, true);
        assert_eq!(w.get(p), Block::Water); // renewable bucket refill
        let q = p + IVec3::Y * 4;
        w.set(q - IVec3::X, Block::Water);
        w.set(q + IVec3::Z, Block::Water);
        w.tick_fluids(p, true);
        assert_eq!(w.get(q), Block::Air);
        let (mut w, p) = basin();
        w.set(p - IVec3::X, Block::Lava);
        w.set(p + IVec3::Z, Block::Lava);
        advance(&mut w, p, 20);
        assert_ne!(w.get(p), Block::Lava);
    }
    #[test]
    fn two_by_two_pool_refills_and_sources_below_support_renewal() {
        let (mut w, p) = basin();
        for d in [IVec3::ZERO, IVec3::X, IVec3::Z, IVec3::X + IVec3::Z] {
            w.set(p + d, Block::Water);
        }
        w.set(p, Block::Air);
        w.tick_fluids(p, true);
        assert_eq!(w.get(p), Block::Water);
        w.set(p + IVec3::Y + IVec3::X, Block::Water);
        w.set(p + IVec3::Y + IVec3::Z, Block::Water);
        w.tick_fluids(p, true);
        assert_eq!(w.get(p + IVec3::Y), Block::Water);
    }
    #[test]
    fn lava_is_slower_shorter_and_solidifies_against_water() {
        let (mut w, p) = basin();
        w.set(p, Block::Lava);
        w.tick_fluids(p, false);
        assert_eq!(w.get(p + IVec3::X), Block::Air);
        advance(&mut w, p, 30);
        assert_eq!(w.get(p + IVec3::X * 3), Block::FlowingLava(6));
        assert_eq!(w.get(p + IVec3::X * 4), Block::Air);
        w.set(p - IVec3::X, Block::Water);
        w.set(p + IVec3::X + IVec3::Z, Block::Water);
        w.tick_fluids(p, false);
        assert_eq!(w.get(p), Block::Obsidian);
        assert_eq!(w.get(p + IVec3::X), Block::Cobble);
    }
    #[test]
    fn distant_updates_sleep_and_fluid_surface_controls_submersion() {
        let (mut w, p) = basin();
        w.set(p, Block::Water);
        w.tick_fluids(p + IVec3::X * 1000, true);
        assert_eq!(w.get(p + IVec3::X), Block::Air);
        assert!(!w.fluid_pending.is_empty());
        w.set(p, Block::FlowingWater(7));
        assert!(
            w.fluid_at(p.as_vec3() + glam::Vec3::new(0.5, 0.05, 0.5))
                .is_some()
        );
        assert!(w.fluid_at(p.as_vec3() + glam::Vec3::splat(0.5)).is_none());
    }
    #[test]
    fn tundra_ponds_have_solid_ice_over_water_and_warm_ponds_stay_liquid() {
        let w = World::new(41);
        let mut cold = None;
        let mut warm = None;
        for cx in -20..20 {
            for cz in -20..20 {
                let h = hash(w.seed ^ 0x706f6f6c, cx, 0, cz);
                let x = cx * 64 + 16 + ((h >> 8) % 32) as i32;
                let z = cz * 64 + 16 + ((h >> 24) % 32) as i32;
                let p = IVec3::new(x, w.height(x, z) - 1, z);
                if w.get(p) == Block::Ice {
                    cold = Some(p);
                }
                if w.get(p) == Block::Water && w.pool_block(p, p.y + 1) == Some(Block::Water) {
                    warm = Some(p);
                }
            }
        }
        let p = cold.expect("frozen tundra pond");
        assert!(w.biome(p.x, p.z).is_snow());
        assert!(w.get(p).solid());
        assert_eq!(w.get(p - IVec3::Y), Block::Water);
        assert!(matches!(
            w.get(p - IVec3::Y * 2),
            Block::Stone | Block::Sand | Block::Gravel | Block::Dirt
        ));
        assert!(warm.is_some());
        let mut broken = World::new(w.seed);
        broken.set(p, Block::Air);
        assert_eq!(broken.get(p), Block::Air);
        assert_eq!(broken.get(p - IVec3::Y), Block::Water);
        assert!(
            broken
                .fluid_at((p - IVec3::Y).as_vec3() + glam::Vec3::splat(0.5))
                .is_some()
        );
        let k = crate::world::chunk(p);
        let local = p - k * 16 + IVec3::ONE;
        assert_eq!(
            w.halo(k)[(local.x + 18 * (local.z + 18 * local.y)) as usize],
            Block::Ice
        );
    }

    #[test]
    fn lakes_and_rivers_remain_liquid_in_tundra_and_match_halos() {
        let w = World::new(41);
        let mut found = [false; 2];
        for cx in -12..12 {
            for cz in -12..12 {
                let Some(plan) = w.waterway_plan(cx * 384 + 192, cz * 384 + 192) else {
                    continue;
                };
                if plan.route.len() < 2 {
                    continue;
                }
                for (i, site) in [plan.route[plan.route.len() / 2], plan.lakes[1].center]
                    .into_iter()
                    .enumerate()
                {
                    let (px, pz) = (site.x, site.z);
                    let (_, level) = w.waterway_column(px, pz).unwrap();
                    let p = IVec3::new(px, level, pz);
                    if !found[i]
                        && w.biome(px, pz).is_snow()
                        && w.waterway_block(p, w.height(px, pz)) == Some(Block::Water)
                        && w.get(p) == Block::Water
                        && w.get(p + IVec3::Y) == Block::Air
                    {
                        found[i] = true;
                        assert_eq!(w.get(p - IVec3::Y), Block::Water);
                        assert_eq!(w.get(p + IVec3::Y), Block::Air);
                        let bed = (1..=64)
                            .map(|depth| w.get(p - IVec3::Y * depth))
                            .find(|b| b.fluid().is_none())
                            .expect("sealed lake/river bed");
                        assert_eq!(bed, Block::Sand);
                        let k = crate::world::chunk(p);
                        let halo = w.halo(k);
                        for y in -1..=16 {
                            for z in -1..=16 {
                                for x in -1..=16 {
                                    assert_eq!(
                                        halo[((x + 1) + 18 * ((z + 1) + 18 * (y + 1))) as usize],
                                        w.get(k * 16 + IVec3::new(x, y, z))
                                    );
                                }
                            }
                        }
                    }
                }
                if found == [true; 2] {
                    return;
                }
            }
        }
        assert_eq!(found, [true; 2], "liquid tundra river and lake");
    }

    #[test]
    fn pond_banks_are_irregular_and_mix_sediments() {
        let w = World::new(41);
        let mut sand = false;
        let mut gravel = false;
        let mut dirt = false;
        let mut irregular = false;
        for cx in 1..12 {
            for cz in 1..12 {
                let h = hash(w.seed ^ 0x706f6f6c, cx, 0, cz);
                let x = cx * 64 + 16 + ((h >> 8) % 32) as i32;
                let z = cz * 64 + 16 + ((h >> 24) % 32) as i32;
                let y = w.height(x, z);
                if w.pool_block(IVec3::new(x, y - 1, z), y) != Some(Block::Water) {
                    continue;
                }
                let mut widths = std::collections::HashSet::new();
                for dz in -6..=6 {
                    let mut width = 0;
                    for dx in -6..=6 {
                        let p = IVec3::new(x + dx, y - 1, z + dz);
                        width +=
                            i32::from(w.pool_block(p, w.height(p.x, p.z)) == Some(Block::Water));
                        match w.pool_block(p - IVec3::Y * 2, w.height(p.x, p.z)) {
                            Some(Block::Sand) => sand = true,
                            Some(Block::Gravel) => gravel = true,
                            Some(Block::Dirt) => dirt = true,
                            _ => {}
                        }
                    }
                    if width > 0 {
                        widths.insert(width);
                    }
                }
                irregular |= widths.len() > 1;
            }
        }
        assert!(sand && gravel && dirt && irregular);
    }

    #[test]
    fn natural_pools_are_repeatable_and_match_chunk_halos() {
        let w = World::new(41);
        let mut water = None;
        let mut lava = None;
        for cx in -20..20 {
            for cz in -20..20 {
                let h = hash(w.seed ^ 0x706f6f6c, cx, 0, cz);
                let x = cx * 64 + 16 + ((h >> 8) % 32) as i32;
                let z = cz * 64 + 16 + ((h >> 24) % 32) as i32;
                let p = IVec3::new(x, w.height(x, z) - 1, z);
                match w.get(p) {
                    Block::Water if w.pool_block(p, p.y + 1) == Some(Block::Water) => {
                        water = Some(p)
                    }
                    Block::Lava if w.pool_block(p, p.y + 1) == Some(Block::Lava) => lava = Some(p),
                    _ => {}
                }
                if water.is_some() && lava.is_some() {
                    break;
                }
            }
        }
        for p in [water.unwrap(), lava.unwrap()] {
            let k = crate::world::chunk(p);
            let local = p - k * 16 + IVec3::ONE;
            let halo = w.halo(k);
            assert_eq!(
                halo[(local.x + 18 * (local.z + 18 * local.y)) as usize],
                w.get(p)
            );
            assert!(matches!(
                w.get(p - IVec3::Y * 2),
                Block::Stone | Block::Sand | Block::Gravel | Block::Dirt
            ));
            assert!(
                (1..=16).any(|dx| w.get(p + IVec3::X * dx).solid()),
                "pond must have a containing rim"
            );
        }
    }
}
