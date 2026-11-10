//! Discrete source-driven fluids. Updates read the old world, then commit together.
use crate::world::{Biome, Block, MAX_Y, MIN_Y, World, hash};
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
        let ready: Vec<_> = self
            .fluid_pending
            .iter()
            .copied()
            .filter(|p| (IVec3::from_array(*p) - center).abs().max_element() <= 64)
            .take(512)
            .collect();
        let mut candidates = std::collections::BTreeSet::new();
        for key in ready {
            self.fluid_pending.remove(&key);
            let p = IVec3::from_array(key);
            candidates.insert(key);
            for d in NEIGHBORS {
                candidates.insert((p + d).to_array());
            }
        }
        let mut changes = Vec::new();
        for key in candidates {
            let p = IVec3::from_array(key);
            if !(MIN_Y + 1..=MAX_Y).contains(&p.y) {
                continue;
            }
            let old = self.get(p);
            if !old.replaceable() {
                continue;
            }
            let neighbors = SIDES.map(|d| self.get(p + d));
            if old.fluid().is_some_and(|(lava, _)| lava)
                && NEIGHBORS
                    .iter()
                    .any(|d| self.get(p + *d).fluid().is_some_and(|(lava, _)| !lava))
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
                            .any(|d| self.get(p + *d).fluid().is_some_and(|(l, _)| l))
                    {
                        self.fluid_pending.insert(key);
                    }
                    continue;
                }
                let source = if lava { Block::Lava } else { Block::Water };
                let below = self.get(p - IVec3::Y);
                let renewable = !lava
                    && neighbors.iter().filter(|b| **b == Block::Water).count() >= 2
                    && (below.solid() || below == Block::Water);
                let level = if old == source || renewable {
                    Some(0)
                } else if self
                    .get(p + IVec3::Y)
                    .fluid()
                    .is_some_and(|(l, _)| l == lava)
                {
                    Some(8)
                } else {
                    SIDES
                        .iter()
                        .filter_map(|d| {
                            let n = p + *d;
                            let (kind, level) = self.get(n).fluid()?;
                            let floor = self.get(n - IVec3::Y);
                            if kind != lava || !(floor.solid() || floor.fluid() == Some((lava, 0)))
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
        for (p, b) in changes {
            self.set(p, b);
        }
    }

    /// Inland lake pairs joined by a winding river, contained within 384-block cells.
    /// A shared elevation keeps source water settled; beds and banks seal caves.
    pub(crate) fn waterway_column(&self, px: i32, pz: i32) -> Option<(f32, i32)> {
        let cx = px.div_euclid(384);
        let cz = pz.div_euclid(384);
        let h = hash(self.seed ^ 0x7269_7665, cx, 0, cz);
        if !h.is_multiple_of(2) {
            return None;
        }
        let x = cx * 384 + 144 + ((h >> 8) % 96) as i32;
        let z = cz * 384 + 144 + ((h >> 24) % 96) as i32;
        if x.abs() < 160 && z.abs() < 160 {
            return None;
        }
        let (u, v) = if h & 4 == 0 {
            (px - x, pz - z)
        } else {
            (pz - z, px - x)
        };
        if u.abs() > 106 || v.abs() > 48 {
            return None;
        }
        let u = u as f32;
        let v = v as f32;
        let bend = if h & 8 == 0 { 20. } else { -20. };
        let nodes = [(-70., 0.), (-25., bend), (25., -bend), (70., 0.)];
        let mut edge = f32::INFINITY;
        for pair in nodes.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (dx, dz) = (b.0 - a.0, b.1 - a.1);
            let t = ((u - a.0) * dx + (v - a.1) * dz) / (dx * dx + dz * dz);
            let t = t.clamp(0., 1.);
            edge = edge.min((u - a.0 - t * dx).hypot(v - a.1 - t * dz) - 5.);
        }
        // Broader, irregular lake basins at both ends of the river.
        for (a, radius) in [(-70., 22.), (70., 30.)] {
            let shore = 1.5 * (u * 0.19 + v * 0.13).sin();
            edge = edge.min((u - a).hypot(v * 1.15) - radius - shore);
        }
        if edge > 2. {
            return None;
        }
        let level = self.height(x, z) - 2;
        for (a, b) in nodes {
            let (dx, dz) = if h & 4 == 0 {
                (a as i32, b as i32)
            } else {
                (b as i32, a as i32)
            };
            if (self.height(x + dx, z + dz) - level).abs() > 18 {
                return None;
            }
        }
        Some((edge, level))
    }

    pub(crate) fn waterway_block(&self, p: IVec3, height: i32) -> Option<Block> {
        let (edge, level) = self.waterway_column(p.x, p.z)?;
        let depth = if edge < -12. {
            7
        } else if edge < -3. {
            4
        } else {
            2
        };
        if p.y < level - 8 || p.y > height.max(level) + 7 {
            return None;
        }
        Some(if p.y <= level - depth || (edge >= 0. && p.y <= level) {
            Block::Sand
        } else if p.y <= level {
            Block::Water
        } else {
            Block::Air
        })
    }

    /// Small stone-lined pools in gentle terrain, outside the protected spawn.
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
        let dx = (p.x - x).abs();
        let dz = (p.z - z).abs();
        if dx > 5 || dz > 5 {
            return None;
        }
        let y = self.height(x, z);
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
        if dx == 5 || dz == 5 || p.y == y - 3 {
            return (p.y <= y).then_some(Block::Stone);
        }
        Some(if p.y < y {
            if lava {
                Block::Lava
            } else if p.y == y - 1 && self.biome(x, z) == Biome::Tundra {
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
        assert_eq!(w.biome(p.x, p.z), Biome::Tundra);
        assert!(w.get(p).solid());
        assert_eq!(w.get(p - IVec3::Y), Block::Water);
        assert_eq!(w.get(p - IVec3::Y * 2), Block::Stone);
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
                let h = hash(w.seed ^ 0x7269_7665, cx, 0, cz);
                let x = cx * 384 + 144 + ((h >> 8) % 96) as i32;
                let z = cz * 384 + 144 + ((h >> 24) % 96) as i32;
                let level = w.height(x, z) - 2;
                for (i, u) in [0, 70].into_iter().enumerate() {
                    let (px, pz) = if h & 4 == 0 { (x + u, z) } else { (x, z + u) };
                    let p = IVec3::new(px, level, pz);
                    if !found[i]
                        && w.biome(px, pz) == Biome::Tundra
                        && w.waterway_block(p, w.height(px, pz)) == Some(Block::Water)
                        && w.get(p) == Block::Water
                    {
                        found[i] = true;
                        assert_eq!(w.get(p - IVec3::Y), Block::Water);
                        assert_eq!(w.get(p + IVec3::Y), Block::Air);
                        assert_eq!(w.get(p - IVec3::Y * 8), Block::Sand);
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
                    Block::Water => water = Some(p),
                    Block::Lava => lava = Some(p),
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
            assert_eq!(w.get(p - IVec3::Y * 2), Block::Stone);
            assert_eq!(w.get(p + IVec3::X * 5), Block::Stone);
        }
    }
}
