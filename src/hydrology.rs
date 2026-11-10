//! Seeded standalone lakes and contained lake-to-lake drainage.
use crate::world::{Biome, Block, SEA_LEVEL, World, hash};
use glam::IVec3;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{Arc, Mutex},
};

pub(crate) const CELL: i32 = 384;
const CAPACITY: usize = 64;
const SIDES: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];
#[derive(Default)]
pub(crate) struct Cache {
    plans: BTreeMap<(i32, i32), Option<Arc<Waterway>>>,
    ponds: BTreeMap<(i32, i32), bool>,
    order: VecDeque<(i32, i32)>,
    pond_order: VecDeque<(i32, i32)>,
}
pub(crate) type SharedCache = Arc<Mutex<Cache>>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Column {
    pub edge: f32,
    pub level: i32,
    pub spill_top: i32,
    apron: f32,
}
impl Column {
    pub fn block(self, world: &World, p: IVec3, height: i32) -> Option<Block> {
        // Existing sea water owns submerged shore aprons.
        if self.edge >= 0. && height < SEA_LEVEL {
            return None;
        }
        let depth = if self.edge < -12. {
            7
        } else if self.edge < -3. {
            4
        } else {
            2
        };
        let bed = if self.edge < 0. {
            height.min(self.level - depth)
        } else {
            let t = (self.edge / self.apron).clamp(0., 1.);
            ((self.level + 2) as f32 + (height - self.level - 2) as f32 * t * t * (3. - 2. * t))
                .round()
                .min(height as f32) as i32
        };
        if p.y < bed - 3 || p.y > height.max(self.spill_top) + 7 {
            return None;
        }
        Some(if p.y <= bed {
            // Sediment needs a stable foundation above generated caves.
            if p.y == bed - 3 {
                Block::Stone
            } else if p.y == bed && self.edge > 3. {
                if world.biome(p.x, p.z) == Biome::Tundra {
                    Block::Snow
                } else {
                    Block::Grass
                }
            } else {
                Block::Sand
            }
        } else if self.edge < 0. && p.y <= self.level {
            Block::Water
        } else if self.edge < 0. && p.y <= self.spill_top {
            if p.y == self.spill_top {
                Block::FlowingWater(1)
            } else {
                Block::FlowingWater(8)
            }
        } else {
            Block::Air
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Lake {
    pub center: IVec3,
    radius: f32,
    aspect: f32,
    rotated: bool,
}
impl Lake {
    fn edge(self, x: i32, z: i32) -> f32 {
        let (u, v) = if self.rotated {
            (z - self.center.z, x - self.center.x)
        } else {
            (x - self.center.x, z - self.center.z)
        };
        (u as f32).hypot(v as f32 * self.aspect)
            - self.radius
            - 1.5 * (u as f32 * 0.19 + v as f32 * 0.13).sin()
    }
    fn shore_level(self, world: &World) -> i32 {
        let mut rim = world.height(self.center.x, self.center.z);
        let extent = (self.radius / self.aspect.min(1.)).ceil() as i32 + 4;
        for dx in -extent..=extent {
            for dz in -extent..=extent {
                let (x, z) = (self.center.x + dx, self.center.z + dz);
                if (0.0..2.0).contains(&self.edge(x, z)) {
                    rim = rim.min(world.height(x, z));
                }
            }
        }
        (rim - 2).max(SEA_LEVEL)
    }
}

pub(crate) struct Waterway {
    pub lakes: [Lake; 2],  // Upstream/downstream; identical for a standalone lake.
    pub route: Vec<IVec3>, // Four-connected, non-increasing water elevations.
    columns: HashMap<(i32, i32), Column>,
}
impl Waterway {
    pub fn column(&self, x: i32, z: i32) -> Option<Column> {
        self.columns.get(&(x, z)).copied()
    }
    fn build(world: &World, cx: i32, cz: i32) -> Option<Self> {
        let h = hash(world.seed ^ 0x7269_7665, cx, 0, cz);
        if !h.is_multiple_of(2) {
            return None;
        }
        let x = cx * CELL + 160 + ((h >> 8) % 64) as i32;
        let z = cz * CELL + 160 + ((h >> 24) % 64) as i32;
        if x.abs() < 160 && z.abs() < 160 {
            return None;
        }
        let rotated = h & 4 != 0;
        let point = |u, v| {
            if rotated {
                IVec3::new(x + v, 0, z + u)
            } else {
                IVec3::new(x + u, 0, z + v)
            }
        };
        let paired = h.is_multiple_of(6);
        let span = 70;
        let bend = if h & 8 == 0 { 20 } else { -20 };
        let mut nodes = [
            point(-span, 0),
            point(-25, bend),
            point(25, -bend),
            point(span, 0),
        ];
        let mut lakes = [
            Lake {
                center: nodes[0],
                radius: 22.,
                aspect: 1.15,
                rotated,
            },
            Lake {
                center: nodes[3],
                radius: 30.,
                aspect: 1.15,
                rotated,
            },
        ];
        if !paired {
            nodes = [point(0, 0); 4];
            lakes[0].center = nodes[0];
            lakes[0].aspect = 0.85 + ((h >> 36) % 6) as f32 * 0.1;
            // Most candidates are modest lakes; broad lakes are occasional landmarks.
            lakes[0].radius = match (h >> 48) % 8 {
                0 => 80. + ((h >> 16) % 25) as f32,
                1 | 2 => 49. + ((h >> 16) % 24) as f32,
                _ => 24. + ((h >> 16) % 25) as f32,
            };
            lakes[1] = lakes[0];
        }
        for lake in &mut lakes {
            lake.center.y = lake.shore_level(world);
        }
        if lakes[0].center.y < lakes[1].center.y {
            lakes.swap(0, 1);
            nodes.reverse();
        }
        if (paired && lakes[0].center.y <= SEA_LEVEL + 3)
            || (!paired && world.height(x, z) <= SEA_LEVEL + 3)
        {
            return None;
        }
        // Avoid perched basins on steep sites, preserving the existing gentle-site rule.
        if nodes
            .iter()
            .any(|p| (world.height(p.x, p.z) - lakes[0].center.y).abs() > 24)
        {
            return None;
        }
        let mut route = vec![nodes[0]];
        for pair in nodes.windows(2) {
            raster_line(&mut route, pair[0], pair[1]);
        }
        let bank_min = |p: IVec3| {
            SIDES
                .iter()
                .map(|d| world.height(p.x + d.x * 6, p.z + d.z * 6))
                .min()
                .unwrap()
        };
        let low = route.iter().map(|&p| bank_min(p) - 2).min().unwrap();
        if paired {
            lakes[1].center.y = lakes[1]
                .center
                .y
                .min(low)
                .min(lakes[0].center.y - 2)
                .max(SEA_LEVEL);
        }
        let mut level = lakes[0].center.y;
        for p in &mut route {
            if lakes[1].edge(p.x, p.z) < -1. {
                level = lakes[1].center.y;
            } else if lakes[0].edge(p.x, p.z) >= -1. {
                level = level.min(bank_min(*p) - 2).max(lakes[1].center.y);
            }
            p.y = level;
        }
        let mut columns = HashMap::new();
        let mut stamp = |x, z, column: Column| {
            let entry = columns.entry((x, z)).or_insert(column);
            // A lower reach's shore apron must not cut through the bank of an
            // adjacent higher reach at a cascade. Preserve the higher bank's
            // influence throughout their overlapping aprons, capped by native land.
            if entry.edge >= 0. && column.edge >= 0. {
                entry.level = entry.level.max(column.level);
                entry.edge = entry.edge.min(column.edge);
                entry.apron = entry.apron.max(column.apron);
                return;
            }
            if column.edge < entry.edge || (column.edge == entry.edge && column.level < entry.level)
            {
                *entry = column;
            }
        };
        for lake in lakes {
            let extent = (lake.radius / lake.aspect.min(1.)).ceil() as i32 + 21;
            for dx in -extent..=extent {
                for dz in -extent..=extent {
                    let (x, z) = (lake.center.x + dx, lake.center.z + dz);
                    let edge = lake.edge(x, z);
                    if edge <= 18. {
                        stamp(
                            x,
                            z,
                            Column {
                                edge,
                                level: lake.center.y,
                                spill_top: lake.center.y,
                                apron: 18.,
                            },
                        );
                    }
                }
            }
        }
        for p in &route {
            for dx in -17..=17 {
                for dz in -17..=17 {
                    let edge = (dx as f32).hypot(dz as f32) - 4.5;
                    if edge <= 12. {
                        stamp(
                            p.x + dx,
                            p.z + dz,
                            Column {
                                edge,
                                level: p.y,
                                spill_top: p.y,
                                apron: 12.,
                            },
                        );
                    }
                }
            }
        }
        // At a drop, vertical falling water joins adjacent source levels. It has a
        // supplying source on the high side and lands inside the lower wet channel.
        let wet: HashMap<_, _> = columns
            .iter()
            .filter(|(_, c)| c.edge < 0.)
            .map(|(&p, c)| (p, c.level))
            .collect();
        for (&(x, z), c) in &mut columns {
            if c.edge < 0. {
                c.spill_top = SIDES
                    .iter()
                    .filter_map(|d| wet.get(&(x + d.x, z + d.z)))
                    .copied()
                    .max()
                    .unwrap_or(c.level)
                    .max(c.level);
            }
        }
        // Reject any site whose actual carved shore would leave an unplanned
        // outlet. This includes diagonal low ground missed by profile probes.
        for (&(x, z), c) in &columns {
            if c.edge >= 0. {
                continue;
            }
            for d in SIDES {
                let (qx, qz) = (x + d.x, z + d.z);
                let shore = columns.get(&(qx, qz))?;
                if shore.edge < 0. {
                    continue;
                }
                let height = world.height(qx, qz);
                let contained = match shore.block(world, IVec3::new(qx, c.level, qz), height) {
                    Some(b) => b.solid() || b.fluid().is_some(),
                    None if height < SEA_LEVEL && c.level <= SEA_LEVEL => true,
                    None => {
                        height >= c.level
                            && !world
                                .raw_cave_column(qx, qz)
                                .is_some_and(|(lo, hi)| (lo..=hi).contains(&c.level))
                    }
                };
                if !contained {
                    return None;
                }
            }
        }
        for rx in (cx * CELL).div_euclid(256)..=((cx + 1) * CELL).div_euclid(256) {
            for rz in (cz * CELL).div_euclid(256)..=((cz + 1) * CELL).div_euclid(256) {
                if let Some(ruin) = world.ruin_in_cell(rx, rz) {
                    for dx in -7..=7 {
                        for dz in -7..=7 {
                            if columns
                                .get(&(ruin.x + dx, ruin.z + dz))
                                .is_some_and(|c| c.edge < 0.)
                            {
                                return None;
                            }
                        }
                    }
                }
            }
        }
        Some(Self {
            lakes,
            route,
            columns,
        })
    }
}

/// Shared shore shape for rendering and exact containment checks.
pub(crate) fn pond_edge(h: u64, dx: i32, dz: i32, radius: f32) -> f32 {
    let aspect = 0.7 + ((h >> 32) % 7) as f32 * 0.1;
    let (u, v) = if h & 8 == 0 { (dx, dz) } else { (dz, dx) };
    (u as f32 * aspect).hypot(v as f32 / aspect) - radius
        + (0.3 + ((h >> 44) % 4) as f32 * 0.2) * (u as f32 * 0.7 + v as f32 * 0.5).sin()
}

fn raster_line(route: &mut Vec<IVec3>, a: IVec3, b: IVec3) {
    let mut p = a;
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    while p.x != b.x || p.z != b.z {
        let step_x = p.x != b.x
            && (p.z == b.z
                || ((p.x + dx.signum() - a.x) * dz - (p.z - a.z) * dx).abs()
                    <= ((p.x - a.x) * dz - (p.z + dz.signum() - a.z) * dx).abs());
        if step_x {
            p.x += dx.signum();
        } else {
            p.z += dz.signum();
        }
        route.push(p);
    }
}

impl World {
    #[cfg(test)]
    pub(crate) fn waterway_cache_len(&self) -> usize {
        self.waterways.lock().unwrap().plans.len()
    }

    pub(crate) fn waterway_plan(&self, x: i32, z: i32) -> Option<Arc<Waterway>> {
        let key = (x.div_euclid(CELL), z.div_euclid(CELL));
        if let Some(plan) = self.waterways.lock().unwrap().plans.get(&key).cloned() {
            return plan;
        }
        let plan = Waterway::build(self, key.0, key.1).map(Arc::new);
        let mut cache = self.waterways.lock().unwrap();
        if let Some(existing) = cache.plans.get(&key) {
            return existing.clone();
        }
        if let Some(old) = (cache.plans.len() >= CAPACITY)
            .then(|| cache.order.pop_front())
            .flatten()
        {
            cache.plans.remove(&old);
        }
        cache.order.push_back(key);
        cache.plans.insert(key, plan.clone());
        plan
    }
    pub(crate) fn natural_fall(&self, p: IVec3) -> bool {
        self.waterway_plan(p.x, p.z)
            .and_then(|plan| plan.column(p.x, p.z))
            .is_some_and(|c| c.edge < 0. && p.y > c.level && p.y <= c.spill_top)
    }
    pub(crate) fn waterway_bounds(&self, x: i32, z: i32, height: i32) -> Option<(i32, i32)> {
        let c = self.waterway_plan(x, z)?.column(x, z)?;
        Some((height.min(c.level - 7) - 3, height.max(c.spill_top) + 1))
    }
    pub(crate) fn waterway_column(&self, x: i32, z: i32) -> Option<(f32, i32)> {
        let c = self.waterway_plan(x, z)?.column(x, z)?;
        Some((c.edge, c.level))
    }
    pub(crate) fn waterway_block(&self, p: IVec3, height: i32) -> Option<Block> {
        self.waterway_plan(p.x, p.z)?
            .column(p.x, p.z)?
            .block(self, p, height)
    }
    pub(crate) fn pond_contained(
        &self,
        cx: i32,
        cz: i32,
        x: i32,
        z: i32,
        y: i32,
        radius: f32,
    ) -> bool {
        let key = (cx, cz);
        if let Some(&safe) = self.waterways.lock().unwrap().ponds.get(&key) {
            return safe;
        }
        let h = hash(self.seed ^ 0x706f6f6c, cx, 0, cz);
        let edge = |dx, dz| pond_edge(h, dx, dz, radius);
        let mut safe = true;
        for dx in -16..=16 {
            for dz in -16..=16 {
                if edge(dx, dz) >= 0.
                    && SIDES.iter().any(|d| edge(dx + d.x, dz + d.z) < 0.)
                    && self.height(x + dx, z + dz) < y
                {
                    safe = false;
                }
            }
        }
        let mut cache = self.waterways.lock().unwrap();
        if let Some(&existing) = cache.ponds.get(&key) {
            return existing;
        }
        if let Some(old) = (cache.ponds.len() >= 128)
            .then(|| cache.pond_order.pop_front())
            .flatten()
        {
            cache.ponds.remove(&old);
        }
        cache.pond_order.push_back(key);
        cache.ponds.insert(key, safe);
        safe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plans(world: &World) -> Vec<Arc<Waterway>> {
        let mut found = Vec::new();
        for cx in -12..12 {
            for cz in -12..12 {
                if let Some(plan) = world.waterway_plan(cx * CELL + 192, cz * CELL + 192) {
                    if plan.route.len() == 1 {
                        continue;
                    }
                    found.push(plan);
                    if found.len() == 4 {
                        return found;
                    }
                }
            }
        }
        assert!(!found.is_empty());
        found
    }

    #[test]
    fn standalone_lakes_vary_in_size_and_preserve_continental_channels() {
        let w = World::new(41);
        let mut smallest = usize::MAX;
        let mut largest = 0;
        for cx in -16..16 {
            for cz in -16..16 {
                let Some(plan) = w.waterway_plan(cx * CELL + 192, cz * CELL + 192) else {
                    continue;
                };
                if plan.route.len() != 1 {
                    continue;
                }
                let area = plan.columns.values().filter(|c| c.edge < 0.).count();
                smallest = smallest.min(area);
                largest = largest.max(area);
                for (&(x, z), c) in &plan.columns {
                    assert_eq!((x.div_euclid(CELL), z.div_euclid(CELL)), (cx, cz));
                    if c.edge < 0. {
                        assert_eq!(w.get(IVec3::new(x, c.level, z)), Block::Water);
                    } else if w.height(x, z) < SEA_LEVEL {
                        assert_eq!(w.get(IVec3::new(x, SEA_LEVEL, z)), Block::Water);
                    }
                }
            }
        }
        assert!(
            smallest < 3500,
            "smaller standalone lakes must survive siting"
        );
        assert!(
            largest > 16000,
            "large standalone lakes must survive siting"
        );
        assert!(largest < 42000, "standalone lake area must remain bounded");
    }

    #[test]
    fn rivers_are_wet_and_descend_all_the_way_into_a_receiving_lake() {
        for seed in [1, 41, 314159] {
            let w = World::new(seed);
            for plan in sample_plans(&w) {
                assert!(plan.lakes[0].center.y > plan.lakes[1].center.y);
                assert_eq!(w.get(plan.lakes[0].center), Block::Water);
                assert_eq!(w.get(plan.lakes[1].center), Block::Water);
                assert_eq!(plan.route.first(), Some(&plan.lakes[0].center));
                assert_eq!(plan.route.last(), Some(&plan.lakes[1].center));
                for pair in plan.route.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    assert_eq!((a.x - b.x).abs() + (a.z - b.z).abs(), 1);
                    assert!(a.y >= b.y);
                    assert_eq!(w.get(a), Block::Water, "dry route at {a:?}, seed={seed}");
                    for y in b.y..=a.y {
                        let p = IVec3::new(b.x, y, b.z);
                        assert!(
                            w.get(p).fluid().is_some(),
                            "gap in cascade at {p:?}, seed={seed}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn natural_rivers_stay_in_their_channels_and_reach_the_receiver_after_simulation() {
        for seed in [41, 314159] {
            let mut w = World::new(seed);
            let plan = sample_plans(&w).remove(0);
            for (&(x, z), c) in &plan.columns {
                if c.edge < 0. && (c.edge > -1.5 || c.spill_top > c.level) {
                    for y in c.level..=c.spill_top {
                        w.wake_fluid(IVec3::new(x, y, z));
                    }
                }
            }
            for _ in 0..2 {
                for center in plan.route.iter().step_by(32) {
                    for pulse in 0..24 {
                        w.tick_fluids(*center, pulse % 3 == 0);
                    }
                }
            }
            for (&p, b) in &w.edits {
                if b.fluid().is_some() && w.height(p[0], p[2]) >= SEA_LEVEL {
                    assert!(
                        plan.column(p[0], p[2]).is_some_and(|c| c.edge < 0.),
                        "runoff escaped onto a field at {p:?}, seed={seed}"
                    );
                }
            }
            for &p in &plan.route {
                assert_eq!(w.get(p), Block::Water, "river dried at {p:?}");
            }
            assert_eq!(w.get(plan.lakes[1].center), Block::Water);
        }
    }

    #[test]
    fn natural_source_edges_are_contained_by_banks_or_another_wet_column() {
        for seed in [1, 41, 314159] {
            let w = World::new(seed);
            for plan in sample_plans(&w) {
                for (&(x, z), c) in &plan.columns {
                    if c.edge >= 0. {
                        continue;
                    }
                    let p = IVec3::new(x, c.level, z);
                    assert_eq!(w.get(p), Block::Water);
                    let bed = (w.height(x, z).min(c.level - 7) - 3..=c.level)
                        .rev()
                        .find(|&y| w.get(IVec3::new(x, y, z)).solid())
                        .expect("sealed bed below the water column");
                    for y in bed - 3..=bed {
                        assert!(w.get(IVec3::new(x, y, z)).solid());
                    }
                    for d in SIDES {
                        let q = p + d;
                        let b = w.get(q);
                        assert!(
                            b.solid() || b.fluid().is_some(),
                            "unplanned spill onto land at {q:?} from {p:?}, seed={seed}, source={c:?}, shore={:?}, native={}",
                            plan.column(q.x, q.z),
                            w.height(q.x, q.z)
                        );
                    }
                }
            }
        }
    }
}
