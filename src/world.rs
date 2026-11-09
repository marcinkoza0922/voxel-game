use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};

pub const MIN_Y: i32 = -512;
pub const MAX_Y: i32 = 1535;
pub const SIZE: i32 = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    #[default]
    Air,
    Grass,
    Dirt,
    Stone,
    Log,
    Leaves,
    Copper,
    Coal,
    Planks,
    Cobble,
    Furnace,
    Table,
    Torch,
    Sand,
    Bed,
}
impl Block {
    pub fn solid(self) -> bool {
        !matches!(self, Self::Air | Self::Torch)
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Air => [0.; 3],
            Self::Grass => [0.34, 0.52, 0.23],
            Self::Dirt => [0.38, 0.25, 0.16],
            Self::Stone => [0.48, 0.49, 0.51],
            Self::Log => [0.30, 0.20, 0.12],
            Self::Leaves => [0.19, 0.37, 0.19],
            Self::Copper => [0.64, 0.38, 0.22],
            Self::Coal => [0.22, 0.23, 0.25],
            Self::Planks => [0.65, 0.47, 0.28],
            Self::Cobble => [0.42, 0.43, 0.45],
            Self::Furnace => [0.29, 0.31, 0.34],
            Self::Table => [0.49, 0.31, 0.16],
            Self::Torch => [1., 0.65, 0.18],
            Self::Sand => [0.76, 0.69, 0.47],
            Self::Bed => [0.63, 0.24, 0.20],
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Air => "Air",
            Self::Grass => "Grass",
            Self::Dirt => "Dirt",
            Self::Stone => "Granite",
            Self::Log => "Oak log",
            Self::Leaves => "Oak leaves",
            Self::Copper => "Copper ore",
            Self::Coal => "Coal ore",
            Self::Planks => "Oak planks",
            Self::Cobble => "Granite cobble",
            Self::Furnace => "Furnace",
            Self::Table => "Crafting table",
            Self::Torch => "Torch",
            Self::Sand => "Sand",
            Self::Bed => "Bedroll",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    Grassland,
    Forest,
}
impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Self::Grassland => "Grasslands",
            Self::Forest => "Oak forest",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Edit {
    pub pos: [i32; 3],
    pub block: Block,
}

pub struct World {
    pub seed: u64,
    pub edits: BTreeMap<[i32; 3], Block>,
    chunks: HashMap<IVec3, Box<[Block]>>,
    pending: VecDeque<IVec3>,
    pub dirty: Vec<IVec3>,
    pub revision: u64,
    center: IVec3,
}

pub fn hash(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9e3779b185ebca87)
        ^ (y as u64).wrapping_mul(0xc2b2ae3d27d4eb4f)
        ^ (z as u64).wrapping_mul(0x165667b19e3779f9);
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58476d1ce4e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d049bb133111eb);
    h ^ (h >> 31)
}
fn smooth(t: f32) -> f32 {
    t * t * (3. - 2. * t)
}
fn noise(seed: u64, x: f32, z: f32) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let fx = smooth(x - x.floor());
    let fz = smooth(z - z.floor());
    let v = |a, b| (hash(seed, a, 0, b) >> 40) as f32 / 16777215.;
    let a = v(ix, iz) * (1. - fx) + v(ix + 1, iz) * fx;
    let b = v(ix, iz + 1) * (1. - fx) + v(ix + 1, iz + 1) * fx;
    a * (1. - fz) + b * fz
}
fn index(p: IVec3) -> usize {
    (p.x.rem_euclid(SIZE) + SIZE * (p.z.rem_euclid(SIZE) + SIZE * p.y.rem_euclid(SIZE))) as usize
}
pub fn chunk(p: IVec3) -> IVec3 {
    IVec3::new(
        p.x.div_euclid(SIZE),
        p.y.div_euclid(SIZE),
        p.z.div_euclid(SIZE),
    )
}
impl World {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            edits: BTreeMap::new(),
            chunks: HashMap::new(),
            pending: VecDeque::new(),
            dirty: vec![],
            revision: 0,
            center: IVec3::splat(i32::MAX),
        }
    }
    pub fn biome(&self, x: i32, z: i32) -> Biome {
        if noise(self.seed ^ 456, x as f32 / 110., z as f32 / 110.) > 0.53 {
            Biome::Forest
        } else {
            Biome::Grassland
        }
    }
    pub fn height(&self, x: i32, z: i32) -> i32 {
        let x = x as f32;
        let z = z as f32;
        let hills =
            noise(self.seed, x / 90., z / 90.) * 22. + noise(self.seed ^ 15, x / 25., z / 25.) * 5.;
        let ridge = (noise(self.seed ^ 914, x / 650., z / 650.) - 0.52).max(0.) * 2.1;
        // A gentle spawn region grows into mountains hundreds of blocks high.
        let distance = (x * x + z * z).sqrt();
        (24. + hills + ridge.powf(1.7) * 750. * ((distance - 100.).max(0.) / 500.).min(1.)) as i32
    }
    fn tree(&self, x: i32, z: i32) -> Option<(i32, i32, i32)> {
        let tx = x.div_euclid(7);
        let tz = z.div_euclid(7);
        for a in tx - 1..=tx + 1 {
            for b in tz - 1..=tz + 1 {
                let h = hash(self.seed ^ 834, a, 0, b);
                let cx = a * 7 + 2 + (h % 3) as i32;
                let cz = b * 7 + 2 + ((h >> 4) % 3) as i32;
                let chance = if self.biome(cx, cz) == Biome::Forest {
                    4
                } else {
                    19
                };
                if h.is_multiple_of(chance) && (x - cx).abs() <= 2 && (z - cz).abs() <= 2 {
                    return Some((cx, self.height(cx, cz), cz));
                }
            }
        }
        None
    }
    fn natural_column(&self, p: IVec3, height: i32, tree: Option<(i32, i32, i32)>) -> Block {
        if !(MIN_Y..=MAX_Y).contains(&p.y) {
            return if p.y < MIN_Y {
                Block::Stone
            } else {
                Block::Air
            };
        }
        if p.y > height {
            if let Some((tx, ty, tz)) = tree {
                if p.x == tx && p.z == tz && p.y <= ty + 5 {
                    return Block::Log;
                }
                if (ty + 3..=ty + 6).contains(&p.y)
                    && (p.x - tx).abs() + (p.z - tz).abs() + (p.y - ty - 4).abs() < 5
                {
                    return Block::Leaves;
                }
            }
            return Block::Air;
        }
        if p.y == MIN_Y {
            return Block::Stone;
        }
        if p.y < height - 4 {
            let a = (p.x as f32 * 0.075 + (p.y as f32 * 0.065).sin() * 2.).sin();
            let b = (p.z as f32 * 0.085 + (p.y as f32 * 0.051).cos() * 2.).cos();
            if a + b > 1.56 && p.y > MIN_Y + 3 {
                return Block::Air;
            }
        }
        if p.y == height {
            Block::Grass
        } else if p.y > height - 4 {
            Block::Dirt
        } else {
            let vein = hash(
                self.seed ^ 879,
                p.x.div_euclid(2),
                p.y.div_euclid(2),
                p.z.div_euclid(2),
            ) % 101;
            if vein < 5 {
                Block::Copper
            } else if vein < 10 {
                Block::Coal
            } else {
                Block::Stone
            }
        }
    }
    pub fn get(&self, p: IVec3) -> Block {
        if let Some(b) = self.edits.get(&p.to_array()) {
            return *b;
        }
        if let Some(c) = self.chunks.get(&chunk(p)) {
            return c[index(p)];
        }
        self.natural_column(p, self.height(p.x, p.z), self.tree(p.x, p.z))
    }
    pub fn set(&mut self, p: IVec3, b: Block) -> bool {
        if !(MIN_Y + 1..=MAX_Y).contains(&p.y) {
            return false;
        }
        self.edits.insert(p.to_array(), b);
        let key = chunk(p);
        if let Some(c) = self.chunks.get_mut(&key) {
            c[index(p)] = b;
        }
        self.dirty.push(key);
        for d in [
            IVec3::X,
            -IVec3::X,
            IVec3::Y,
            -IVec3::Y,
            IVec3::Z,
            -IVec3::Z,
        ] {
            let other = chunk(p + d);
            if other != key {
                self.dirty.push(other);
            }
        }
        self.revision += 1;
        true
    }
    pub fn stream(&mut self, pos: Vec3, radius: i32) -> Vec<IVec3> {
        let c = chunk(pos.floor().as_ivec3());
        let mut removed = vec![];
        if c != self.center {
            self.center = c;
            self.chunks.retain(|key, _| {
                let keep = (key.x - c.x).abs() <= radius + 1
                    && (key.z - c.z).abs() <= radius + 1
                    && (key.y - c.y).abs() <= 5;
                if !keep {
                    removed.push(*key);
                }
                keep
            });
            self.pending.clear();
            let mut wanted = vec![];
            for x in c.x - radius..=c.x + radius {
                for z in c.z - radius..=c.z + radius {
                    let surface = self.height(x * SIZE + 8, z * SIZE + 8).div_euclid(SIZE);
                    let lo = (c.y - 2).min(surface - 1).max(c.y - 4).max(MIN_Y / SIZE);
                    let hi = (c.y + 1).max(surface).min(c.y + 4).min(MAX_Y / SIZE);
                    for y in lo..=hi {
                        let k = IVec3::new(x, y, z);
                        if !self.chunks.contains_key(&k) {
                            wanted.push(k);
                        }
                    }
                }
            }
            wanted.sort_by_key(|p| (*p - c).length_squared());
            self.pending.extend(wanted);
        }
        // Limit generation work per frame; meshes are separately budgeted.
        for _ in 0..3 {
            if let Some(k) = self.pending.pop_front() {
                let mut blocks = vec![Block::Air; 4096].into_boxed_slice();
                for x in 0..SIZE {
                    for z in 0..SIZE {
                        let wx = k.x * SIZE + x;
                        let wz = k.z * SIZE + z;
                        let h = self.height(wx, wz);
                        let t = self.tree(wx, wz);
                        for y in 0..SIZE {
                            let p = IVec3::new(wx, k.y * SIZE + y, wz);
                            blocks[index(p)] = self
                                .edits
                                .get(&p.to_array())
                                .copied()
                                .unwrap_or_else(|| self.natural_column(p, h, t));
                        }
                    }
                }
                self.chunks.insert(k, blocks);
                self.dirty.push(k);
            }
        }
        removed
    }
    pub fn loaded(&self, k: IVec3) -> bool {
        self.chunks.contains_key(&k)
    }
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
    pub fn raycast(&self, origin: Vec3, dir: Vec3, reach: f32) -> Option<(IVec3, IVec3)> {
        if dir.length_squared() < 0.0001 {
            return None;
        }
        let dir = dir.normalize();
        let mut p = origin.floor().as_ivec3();
        let step = dir.signum().as_ivec3();
        let delta = Vec3::new(
            if dir.x != 0. {
                1. / dir.x.abs()
            } else {
                f32::INFINITY
            },
            if dir.y != 0. {
                1. / dir.y.abs()
            } else {
                f32::INFINITY
            },
            if dir.z != 0. {
                1. / dir.z.abs()
            } else {
                f32::INFINITY
            },
        );
        let boundary = p.as_vec3()
            + Vec3::new(
                if step.x > 0 { 1. } else { 0. },
                if step.y > 0 { 1. } else { 0. },
                if step.z > 0 { 1. } else { 0. },
            );
        let mut t = Vec3::new(
            if dir.x != 0. {
                (boundary.x - origin.x) / dir.x
            } else {
                f32::INFINITY
            },
            if dir.y != 0. {
                (boundary.y - origin.y) / dir.y
            } else {
                f32::INFINITY
            },
            if dir.z != 0. {
                (boundary.z - origin.z) / dir.z
            } else {
                f32::INFINITY
            },
        );
        let mut prev = p;
        loop {
            if self.get(p) != Block::Air {
                return Some((p, prev));
            }
            let axis = if t.x <= t.y && t.x <= t.z {
                0
            } else if t.y <= t.z {
                1
            } else {
                2
            };
            if t[axis] > reach {
                return None;
            }
            prev = p;
            p[axis] += step[axis];
            t[axis] += delta[axis];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_chunks() {
        assert_eq!(chunk(IVec3::new(-1, -17, 16)), IVec3::new(-1, -2, 1));
    }
    #[test]
    fn deterministic_and_tall() {
        let a = World::new(42);
        let b = World::new(42);
        for x in [-4000, -16, 0, 16, 4000] {
            assert_eq!(a.height(x, 91), b.height(x, 91));
            assert_eq!(a.get(IVec3::new(x, 20, 91)), b.get(IVec3::new(x, 20, 91)));
        }
        assert!(MAX_Y - MIN_Y + 1 >= 2048);
    }
    #[test]
    fn edits_at_boundaries() {
        let mut w = World::new(1);
        w.set(IVec3::new(-1, 100, 16), Block::Planks);
        assert_eq!(w.get(IVec3::new(-1, 100, 16)), Block::Planks);
        assert!(w.dirty.contains(&IVec3::new(0, 6, 1)));
        assert!(!w.set(IVec3::new(0, MIN_Y, 0), Block::Air));
    }
    #[test]
    fn ray_hits_exact_face() {
        let mut w = World::new(1);
        w.set(IVec3::new(2, 100, 0), Block::Stone);
        let (hit, before) = w.raycast(Vec3::new(0.5, 100.5, 0.5), Vec3::X, 5.).unwrap();
        assert_eq!(hit, IVec3::new(2, 100, 0));
        assert_eq!(before, IVec3::new(1, 100, 0));
        assert!(w.raycast(Vec3::new(0.5, 100.5, 0.5), Vec3::X, 1.).is_none());
    }
    #[test]
    fn distant_mountains_exceed_legacy_world_heights() {
        let w = World::new(314159);
        let peak = (-8000..=8000)
            .step_by(256)
            .flat_map(|x| (-8000..=8000).step_by(256).map(move |z| (x, z)))
            .map(|(x, z)| w.height(x, z))
            .max()
            .unwrap();
        assert!(peak > 384, "peak was {peak}");
        assert!(peak < MAX_Y);
    }
    #[test]
    fn chunk_eviction_does_not_erase_edits() {
        let mut w = World::new(1);
        let p = IVec3::new(0, 100, 0);
        w.set(p, Block::Planks);
        for _ in 0..10 {
            w.stream(Vec3::new(0., 100., 0.), 1);
        }
        w.stream(Vec3::new(1000., 100., 1000.), 1);
        assert!(!w.loaded(chunk(p)));
        for _ in 0..10 {
            w.stream(Vec3::new(0., 100., 0.), 1);
        }
        assert!(w.loaded(chunk(p)));
        assert_eq!(w.get(p), Block::Planks);
    }
}
