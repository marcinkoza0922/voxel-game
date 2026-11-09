use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

pub const MIN_Y: i32 = -512;
pub const MAX_Y: i32 = 1535;
pub const SIZE: i32 = 16;
pub const VIEW_RADIUS: i32 = 10;

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
    Iron,
    Smelter,
    Snow,
    RuinStone,
    Reliquary,
    TallGrass,
    RedFlower,
    YellowFlower,
    PurpleFlower,
    WhiteFlower,
    PinkFlower,
    BlueFlower,
    Shrub,
}
impl Block {
    pub fn solid(self) -> bool {
        !matches!(self, Self::Air | Self::Torch) && !self.plant()
    }
    pub fn plant(self) -> bool {
        matches!(self, Self::TallGrass | Self::Shrub) || self.flower_index().is_some()
    }
    pub fn flower_index(self) -> Option<usize> {
        FLOWERS.iter().position(|b| *b == self)
    }
    pub fn processor(self) -> bool {
        matches!(self, Self::Furnace | Self::Smelter)
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Shrub => [0.20, 0.39, 0.13],
            Self::TallGrass => [0.25, 0.45, 0.12],
            Self::RedFlower => FLOWER_COLORS[0],
            Self::YellowFlower => FLOWER_COLORS[1],
            Self::PurpleFlower => FLOWER_COLORS[2],
            Self::WhiteFlower => FLOWER_COLORS[3],
            Self::PinkFlower => FLOWER_COLORS[4],
            Self::BlueFlower => FLOWER_COLORS[5],
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
            Self::Iron => [0.59, 0.54, 0.49],
            Self::Smelter => [0.24, 0.27, 0.30],
            Self::Snow => [0.85, 0.91, 0.95],
            Self::RuinStone => [0.35, 0.43, 0.48],
            Self::Reliquary => [0.45, 0.72, 0.86],
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Shrub => "Shrub",
            Self::TallGrass => "Tall grass",
            Self::RedFlower => "Red flower",
            Self::YellowFlower => "Yellow flower",
            Self::PurpleFlower => "Purple flower",
            Self::WhiteFlower => "White flower",
            Self::PinkFlower => "Pink flower",
            Self::BlueFlower => "Blue flower",
            Self::Air => "Air",
            Self::Grass => "Dirt",
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
            Self::Iron => "Iron ore",
            Self::Smelter => "Crude smelter",
            Self::Snow => "Snow",
            Self::RuinStone => "Ancient masonry",
            Self::Reliquary => "Sealed iron cache",
        }
    }
}
pub const FLOWER_COLORS: [[f32; 3]; 6] = [
    [0.91, 0.15, 0.19],
    [0.98, 0.75, 0.10],
    [0.65, 0.28, 0.86],
    [0.95, 0.89, 0.77],
    [0.95, 0.34, 0.59],
    [0.24, 0.48, 0.94],
];
pub const FLOWERS: [Block; 6] = [
    Block::RedFlower,
    Block::YellowFlower,
    Block::PurpleFlower,
    Block::WhiteFlower,
    Block::PinkFlower,
    Block::BlueFlower,
];
pub fn vegetation_hash(seed: u64, support: IVec3) -> u64 {
    hash(seed ^ 0x767567, support.x, support.y, support.z)
}
pub fn grass_height(h: u64, tuft: u32) -> f32 {
    let bits = h.rotate_right(tuft * 17);
    let stature = 0.16 + ((h >> 56) & 255) as f32 / 255. * 0.79;
    stature * (0.72 + ((bits >> 16) & 255) as f32 / 255. * 0.28)
}
// Small selection bounds let rays pass above low grass or beside flower stems.
fn plant_hit(seed: u64, p: IVec3, block: Block, origin: Vec3, dir: Vec3, reach: f32) -> bool {
    let h = vegetation_hash(seed, p - IVec3::Y);
    let (min, max) = if block == Block::TallGrass {
        (
            Vec3::new(0.02, 0., 0.02),
            Vec3::new(
                0.98,
                (0..3).map(|i| grass_height(h, i)).fold(0., f32::max),
                0.98,
            ),
        )
    } else if block == Block::Shrub {
        (Vec3::new(0.06, 0., 0.06), Vec3::new(0.94, 0.7, 0.94))
    } else {
        (Vec3::new(0.15, 0., 0.15), Vec3::new(0.85, 0.85, 0.85))
    };
    let mut near = 0_f32;
    let mut far = reach;
    for axis in 0..3 {
        let low = p.as_vec3()[axis] + min[axis];
        let high = p.as_vec3()[axis] + max[axis];
        if dir[axis].abs() < 1e-6 {
            if origin[axis] < low || origin[axis] > high {
                return false;
            }
        } else {
            let a = (low - origin[axis]) / dir[axis];
            let b = (high - origin[axis]) / dir[axis];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
        }
    }
    near <= far
}
pub fn vegetation_block(seed: u64, support: IVec3) -> Block {
    let h = vegetation_hash(seed, support);
    // Keep flower abundance independent of the much sparser grass objects.
    if !h.is_multiple_of(5) && (h >> 32).is_multiple_of(13) {
        return FLOWERS[((h >> 48) % 6) as usize];
    }
    if (h >> 20).is_multiple_of(47) {
        return Block::Shrub;
    }
    let patch = noise(
        seed ^ 0x74756674,
        support.x as f32 / 14.,
        support.z as f32 / 14.,
    );
    let density = if patch > 0.65 {
        38
    } else if patch > 0.5 {
        12
    } else {
        2
    };
    if h % 100 < density {
        Block::TallGrass
    } else {
        Block::Air
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    Grassland,
    Forest,
    Tundra,
}
impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Self::Grassland => "Grasslands",
            Self::Forest => "Oak forest",
            Self::Tundra => "Tundra",
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
    pub torch_mounts: BTreeMap<[i32; 3], [i32; 3]>,
    chunks: HashMap<IVec3, Box<[Block]>>,
    pending: VecDeque<IVec3>,
    pub dirty: Vec<IVec3>,
    pub revision: u64,
    center: IVec3,
    radius: i32,
    desired: HashSet<IVec3>,
    surface_bounds: HashMap<(i32, i32), (i32, i32)>,
    workers: Option<crate::streaming::Workers>,
    active: HashSet<IVec3>,
    pub meshes: Vec<(IVec3, Vec<crate::render::Vertex>)>,
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
            torch_mounts: BTreeMap::new(),
            chunks: HashMap::new(),
            pending: VecDeque::new(),
            dirty: vec![],
            revision: 0,
            center: IVec3::splat(i32::MAX),
            radius: -1,
            desired: HashSet::new(),
            surface_bounds: HashMap::new(),
            workers: None,
            active: HashSet::new(),
            meshes: vec![],
        }
    }
    pub fn biome(&self, x: i32, z: i32) -> Biome {
        if (x as f64).hypot(z as f64) > 240.
            && noise(self.seed ^ 7621, x as f32 / 320., z as f32 / 320.) > 0.52
        {
            return Biome::Tundra;
        }
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
    /// One stable candidate per 256-block cell; only gentle tundra sites survive.
    pub fn ruin_in_cell(&self, x: i32, z: i32) -> Option<IVec3> {
        let h = hash(self.seed ^ 91283, x, 0, z);
        let cx = x * 256 + 48 + (h % 160) as i32;
        let cz = z * 256 + 48 + ((h >> 16) % 160) as i32;
        if self.biome(cx, cz) != Biome::Tundra {
            return None;
        }
        let y = self.height(cx, cz);
        if [-6, 6].into_iter().any(|dx| {
            [-6, 6]
                .into_iter()
                .any(|dz| (self.height(cx + dx, cz + dz) - y).abs() > 4)
        }) {
            return None;
        }
        Some(IVec3::new(cx, y, cz))
    }
    pub fn nearby_ruin(&self, x: i32, z: i32) -> Option<IVec3> {
        self.ruin_in_cell(x.div_euclid(256), z.div_euclid(256))
    }
    fn tree(&self, x: i32, z: i32) -> Option<(i32, i32, i32)> {
        if self.biome(x, z) == Biome::Tundra {
            return None;
        }
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
    fn natural_column(
        &self,
        p: IVec3,
        height: i32,
        tree: Option<(i32, i32, i32)>,
        ruin: Option<IVec3>,
    ) -> Block {
        if !(MIN_Y..=MAX_Y).contains(&p.y) {
            return if p.y < MIN_Y {
                Block::Stone
            } else {
                Block::Air
            };
        }
        if let Some(c) = ruin {
            let d = p - c;
            if d.x.abs() <= 6 && d.z.abs() <= 6 && p.y >= c.y - 6 && p.y <= height.max(c.y) + 6 {
                if d.y <= 0 {
                    return Block::RuinStone;
                }
                if d.x == 0 && d.z == 0 && d.y == 1 {
                    return Block::Reliquary;
                }
                let wall = (d.x.abs() == 5 && d.z.abs() <= 5) || (d.z.abs() == 5 && d.x.abs() <= 5);
                let doorway = d.z == 5 && d.x.abs() <= 1;
                let pillar = d.x.abs() == 5 && d.z.abs() == 5;
                if wall
                    && !doorway
                    && d.y
                        <= if pillar {
                            5
                        } else {
                            2 + (hash(self.seed, p.x, 0, p.z) % 2) as i32
                        }
                {
                    return Block::RuinStone;
                }
                return Block::Air;
            }
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
            if p.y == height + 1 && self.biome(p.x, p.z) != Biome::Tundra {
                return vegetation_block(self.seed, p - IVec3::Y);
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
            if self.biome(p.x, p.z) == Biome::Tundra {
                Block::Snow
            } else {
                Block::Grass
            }
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
            } else if vein < 13 && p.y < height - 12 {
                Block::Iron
            } else {
                Block::Stone
            }
        }
    }
    pub fn get(&self, p: IVec3) -> Block {
        if let Some(b) = self.edits.get(&p.to_array()) {
            return self.supported_plant(p, *b);
        }
        if let Some(c) = self.chunks.get(&chunk(p)) {
            return self.supported_plant(p, c[index(p)]);
        }
        self.supported_plant(
            p,
            self.natural_column(
                p,
                self.height(p.x, p.z),
                self.tree(p.x, p.z),
                self.nearby_ruin(p.x, p.z),
            ),
        )
    }
    fn supported_plant(&self, p: IVec3, b: Block) -> Block {
        if b.plant()
            && self
                .edits
                .get(&(p - IVec3::Y).to_array())
                .is_some_and(|support| *support != Block::Grass)
        {
            Block::Air
        } else {
            b
        }
    }
    /// Sample a chunk and its AO halo, reusing generation data for each column.
    pub fn halo(&self, key: IVec3) -> Vec<Block> {
        let base = key * SIZE;
        let mut blocks = vec![Block::Air; 18 * 18 * 18];
        for x in -1..=SIZE {
            for z in -1..=SIZE {
                let wx = base.x + x;
                let wz = base.z + z;
                let h = self.height(wx, wz);
                let t = self.tree(wx, wz);
                let ruin = self.nearby_ruin(wx, wz);
                for y in -1..=SIZE {
                    let p = IVec3::new(wx, base.y + y, wz);
                    let block = self
                        .edits
                        .get(&p.to_array())
                        .copied()
                        .or_else(|| self.chunks.get(&chunk(p)).map(|c| c[index(p)]))
                        .unwrap_or_else(|| self.natural_column(p, h, t, ruin));
                    blocks[((x + 1) + 18 * ((z + 1) + 18 * (y + 1))) as usize] =
                        self.supported_plant(p, block);
                }
            }
        }
        blocks
    }
    pub fn set(&mut self, p: IVec3, b: Block) -> bool {
        if !(MIN_Y + 1..=MAX_Y).contains(&p.y) {
            return false;
        }
        if b != Block::Grass && self.get(p + IVec3::Y).plant() {
            self.set(p + IVec3::Y, Block::Air);
        }
        self.edits.insert(p.to_array(), b);
        if b != Block::Torch {
            self.torch_mounts.remove(&p.to_array());
        }
        let key = chunk(p);
        if let Some(c) = self.chunks.get_mut(&key) {
            c[index(p)] = b;
        }
        self.dirty.push(key);
        // AO samples diagonal neighbors too, including across chunk corners.
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let other = chunk(p + IVec3::new(x, y, z));
                    if other != key {
                        self.dirty.push(other);
                    }
                }
            }
        }
        self.revision += 1;
        true
    }
    /// Spread cover one step per pulse around edits, where dirt can become exposed.
    /// Natural unedited surfaces already have grass; edits and the blocks below
    /// them cover both placed dirt and freshly dug surfaces without a world scan.
    pub fn spread_grass(&mut self, center: IVec3) -> usize {
        const RADIUS: i32 = 32;
        let mut candidates = std::collections::BTreeSet::new();
        for (&p, _) in self.edits.range(
            [center.x - RADIUS, i32::MIN, i32::MIN]..=[center.x + RADIUS, i32::MAX, i32::MAX],
        ) {
            let p = IVec3::from_array(p);
            if (p.z - center.z).abs() > RADIUS || (p.y - center.y).abs() > RADIUS {
                continue;
            }
            candidates.insert(p.to_array());
            candidates.insert((p - IVec3::Y).to_array());
        }
        let changes: Vec<_> = candidates
            .into_iter()
            .map(IVec3::from_array)
            .filter(|&p| {
                if self.get(p) != Block::Dirt || self.get(p + IVec3::Y).solid() {
                    return false;
                }
                [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
                    .into_iter()
                    .any(|side| {
                        (-1..=1).any(|dy| {
                            let neighbor = p + side + IVec3::Y * dy;
                            self.get(neighbor) == Block::Grass
                                && !self.get(neighbor + IVec3::Y).solid()
                        })
                    })
            })
            .take(32)
            .collect();
        let count = changes.len();
        for p in changes {
            self.set(p, Block::Grass);
        }
        count
    }
    pub fn stream(&mut self, pos: Vec3, radius: i32) -> Vec<IVec3> {
        self.meshes.clear();
        let c = chunk(pos.floor().as_ivec3());
        let mut removed = vec![];
        let radius = radius.max(0);
        if c != self.center || radius != self.radius {
            self.center = c;
            self.radius = radius;
            let mut desired = HashSet::new();
            // Only retain columns in this view: bounded memory even after long travel.
            self.surface_bounds
                .retain(|&(x, z), _| (x - c.x).abs() <= radius && (z - c.z).abs() <= radius);
            for x in c.x - radius..=c.x + radius {
                for z in c.z - radius..=c.z + radius {
                    // Heights depend only on the seed. Reuse overlapping columns
                    // across replans, while sampling all 256 heights in new columns.
                    let (low, high) = if let Some(bounds) = self.surface_bounds.get(&(x, z)) {
                        *bounds
                    } else {
                        let (mut low, mut high) = (MAX_Y, MIN_Y);
                        for dx in 0..SIZE {
                            for dz in 0..SIZE {
                                let h = self.height(x * SIZE + dx, z * SIZE + dz);
                                low = low.min(h);
                                high = high.max(h + 6);
                            }
                        }
                        self.surface_bounds.insert((x, z), (low, high));
                        (low, high)
                    };
                    let lo = (low.div_euclid(SIZE) - 1).max(MIN_Y / SIZE);
                    let hi = high.div_euclid(SIZE).min(MAX_Y / SIZE);
                    for y in lo..=hi {
                        desired.insert(IVec3::new(x, y, z));
                    }
                    // Underground gameplay needs nearby caves, not every cave
                    // beneath every distant mountain.
                    if (x - c.x).abs() <= 2 && (z - c.z).abs() <= 2 {
                        for y in (c.y - 2).max(MIN_Y / SIZE)..=(c.y + 2).min(MAX_Y / SIZE) {
                            desired.insert(IVec3::new(x, y, z));
                        }
                    }
                }
            }
            self.chunks.retain(|key, _| {
                let keep = desired.contains(key);
                if !keep {
                    removed.push(*key);
                }
                keep
            });
            self.desired = desired;
            let mut wanted: Vec<_> = self
                .desired
                .iter()
                .copied()
                .filter(|k| !self.chunks.contains_key(k))
                .collect();
            let mut queued: HashSet<_> = wanted.iter().copied().collect();
            // Keep queued remeshes of resident edited chunks when replanning.
            for key in &self.pending {
                if self.desired.contains(key) && queued.insert(*key) {
                    wanted.push(*key);
                }
            }
            wanted.sort_by_key(|p| ((*p - c).length_squared(), p.x, p.y, p.z));
            self.pending = wanted.into();
        }
        let mut priorities_changed = false;
        for key in self.dirty.drain(..) {
            if self.desired.contains(&key) && !self.pending.contains(&key) {
                self.pending.push_back(key);
                priorities_changed = true;
            }
        }
        let workers = self
            .workers
            .get_or_insert_with(crate::streaming::Workers::new);
        while let Some(result) = workers.receive() {
            self.active.remove(&result.key);
            if !self.desired.contains(&result.key) {
                continue;
            }
            if result.revision != self.revision {
                if !self.pending.contains(&result.key) {
                    self.pending.push_back(result.key);
                    priorities_changed = true;
                }
                continue;
            }
            self.pending.retain(|k| *k != result.key);
            self.chunks.insert(result.key, result.blocks);
            self.meshes.push((result.key, result.mesh));
        }
        if priorities_changed {
            self.pending
                .make_contiguous()
                .sort_by_key(|p| ((*p - c).length_squared(), p.x, p.y, p.z));
        }
        // At most two jobs (including completed results) can occupy memory.
        while self.active.len() < crate::streaming::WORKERS {
            let Some(i) = self.pending.iter().position(|k| !self.active.contains(k)) else {
                break;
            };
            let key = self.pending.remove(i).unwrap();
            if !self.desired.contains(&key) {
                continue;
            }
            let base = key * SIZE;
            let mut snapshot = World::new(self.seed);
            snapshot.torch_mounts = self.torch_mounts.clone();
            // Copy only edits intersecting the chunk and its one-block AO halo.
            snapshot.edits = self
                .edits
                .range([base.x - 1, i32::MIN, i32::MIN]..=[base.x + SIZE, i32::MAX, i32::MAX])
                .filter(|(p, _)| {
                    (base.y - 1..=base.y + SIZE).contains(&p[1])
                        && (base.z - 1..=base.z + SIZE).contains(&p[2])
                })
                .map(|(p, b)| (*p, *b))
                .collect();
            workers.submit(crate::streaming::Job {
                key,
                revision: self.revision,
                snapshot,
            });
            self.active.insert(key);
        }
        removed
    }
    pub fn loaded(&self, k: IVec3) -> bool {
        self.chunks.contains_key(&k)
    }
    pub fn stream_counts(&self) -> [usize; 3] {
        [self.chunks.len(), self.pending.len(), self.active.len()]
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
            let block = self.get(p);
            if block != Block::Air
                && (!block.plant() || plant_hit(self.seed, p, block, origin, dir, reach))
            {
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
    fn cached_surface_plans_match_fresh_worlds_and_stay_bounded() {
        let mut cached = World::new(314159);
        for pos in [
            Vec3::new(-32., 50., -16.),
            Vec3::new(-16., 90., -16.),
            Vec3::new(-16., -100., -16.),
            Vec3::new(1024., 500., 1024.),
        ] {
            cached.stream(pos, 2);
            let mut fresh = World::new(314159);
            fresh.stream(pos, 2);
            assert_eq!(cached.desired, fresh.desired);
            assert_eq!(cached.surface_bounds, fresh.surface_bounds);
            assert_eq!(cached.surface_bounds.len(), 25);
        }
        cached.stream(Vec3::ZERO, 0);
        assert_eq!(cached.surface_bounds.len(), 1);
    }
    #[test]
    fn cached_halo_matches_queries_with_edits_and_resident_chunks() {
        let mut w = World::new(42);
        let pos = Vec3::new(-1., 40., 16.);
        w.stream(pos, 1);
        w.set(IVec3::new(-1, 40, 16), Block::Planks);
        for key in [chunk(pos.as_ivec3()), IVec3::new(-2, 1, 0)] {
            let halo = w.halo(key);
            for x in -1..=SIZE {
                for z in -1..=SIZE {
                    for y in -1..=SIZE {
                        let i = ((x + 1) + 18 * ((z + 1) + 18 * (y + 1))) as usize;
                        assert_eq!(halo[i], w.get(key * SIZE + IVec3::new(x, y, z)));
                    }
                }
            }
        }
    }
    #[test]
    fn streaming_includes_high_surfaces_and_radius_changes() {
        let mut w = World::new(314159);
        let (x, z) = (-8000..=8000)
            .step_by(256)
            .flat_map(|x| (-8000..=8000).step_by(256).map(move |z| (x, z)))
            .max_by_key(|(x, z)| w.height(*x, *z))
            .unwrap();
        let pos = Vec3::new(x as f32, 40., z as f32);
        w.stream(pos, 0);
        let surface = chunk(IVec3::new(x, w.height(x, z), z));
        assert!(surface.y > chunk(pos.as_ivec3()).y + 4);
        assert!(w.desired.contains(&surface));
        w.stream(pos, 2);
        let edge = IVec3::new(surface.x + 2, surface.y, surface.z);
        assert!(
            w.pending.iter().any(|k| k.x == edge.x && k.z == edge.z)
                || w.chunks.keys().any(|k| k.x == edge.x && k.z == edge.z)
        );
        w.stream(Vec3::new(0., 40., 0.), 0);
        assert!(w.pending.iter().all(|k| k.x == 0 && k.z == 0));
    }
    #[test]
    fn background_results_preserve_edits_and_match_synchronous_meshes() {
        let mut w = World::new(42);
        let pos = Vec3::new(-1., 40., 16.);
        let key = chunk(pos.as_ivec3());
        w.stream(pos, 1);
        assert!(!w.active.is_empty());
        assert!(w.active.len() <= crate::streaming::WORKERS);
        // These edits happen after immutable snapshots were submitted. They
        // invalidate old results, including AO in diagonal neighboring chunks.
        let edit = key * SIZE;
        w.set(edit, Block::Planks);
        assert!(w.dirty.contains(&(key - IVec3::ONE)));
        wait_for_chunk(&mut w, pos, key);
        assert_eq!(w.get(edit), Block::Planks);
        let data = w.meshes.iter().find(|(k, _)| *k == key).unwrap();
        let expected = crate::render::mesh_chunk(&w, key);
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&data.1),
            bytemuck::cast_slice::<_, u8>(&expected)
        );
        let mut reference = World::new(w.seed);
        reference.edits = w.edits.clone();
        for y in 0..SIZE {
            for z in 0..SIZE {
                for x in 0..SIZE {
                    let p = key * SIZE + IVec3::new(x, y, z);
                    assert_eq!(w.chunks[&key][index(p)], reference.get(p));
                }
            }
        }
    }
    #[test]
    fn resident_edit_remesh_survives_stream_replanning() {
        let mut w = World::new(7);
        let pos = Vec3::new(0., 40., 0.);
        let key = chunk(pos.as_ivec3());
        wait_for_chunk(&mut w, pos, key);
        w.set(key * SIZE, Block::Planks);
        // A loaded chunk still needs its queued remesh after changing center.
        w.pending.push_back(key);
        let moved = pos + Vec3::X * SIZE as f32;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            w.stream(moved, 1);
            if let Some((_, mesh)) = w.meshes.iter().find(|(k, _)| *k == key) {
                let expected = crate::render::mesh_chunk(&w, key);
                assert_eq!(
                    bytemuck::cast_slice::<_, u8>(mesh),
                    bytemuck::cast_slice::<_, u8>(&expected)
                );
                break;
            }
            assert!(std::time::Instant::now() < deadline, "remesh timed out");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    #[test]
    fn obsolete_background_jobs_cannot_reload_evicted_chunks() {
        let mut w = World::new(42);
        w.stream(Vec3::new(0., 40., 0.), 0);
        let old = w.active.clone();
        let pos = Vec3::new(1000., 40., 1000.);
        let key = chunk(pos.as_ivec3());
        wait_for_chunk(&mut w, pos, key);
        assert!(old.iter().all(|k| !w.loaded(*k)));
        assert!(w.chunks.keys().all(|k| w.desired.contains(k)));
        assert!(w.active.len() <= crate::streaming::WORKERS);
    }
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
    fn wait_for_chunk(w: &mut World, pos: Vec3, key: IVec3) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !w.loaded(key) {
            assert!(std::time::Instant::now() < deadline, "streaming timed out");
            w.stream(pos, 1);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    #[test]
    fn chunk_eviction_does_not_erase_edits() {
        let mut w = World::new(1);
        let p = IVec3::new(0, 100, 0);
        w.set(p, Block::Planks);
        wait_for_chunk(&mut w, Vec3::new(0., 100., 0.), chunk(p));
        w.stream(Vec3::new(1000., 100., 1000.), 1);
        assert!(!w.loaded(chunk(p)));
        wait_for_chunk(&mut w, Vec3::new(0., 100., 0.), chunk(p));
        assert!(w.loaded(chunk(p)));
        assert_eq!(w.get(p), Block::Planks);
    }
}

#[cfg(test)]
mod expansion_tests {
    use super::*;
    #[test]
    fn tundra_ruin_is_deterministic_accessible_and_matches_chunk_halo() {
        for seed in [1, 42, 314159] {
            let world = World::new(seed);
            assert_ne!(world.biome(2, 2), Biome::Tundra);
            let home = (-8..=8)
                .find_map(|x| (-8..=8).find_map(|z| world.ruin_in_cell(x, z)))
                .unwrap();
            let other = World::new(seed);
            assert_eq!(world.biome(home.x, home.z), Biome::Tundra);
            assert_eq!(other.nearby_ruin(home.x, home.z), Some(home));
            assert_eq!(world.get(home + IVec3::Y), Block::Reliquary);
            assert_eq!(world.get(home + IVec3::new(0, 1, 5)), Block::Air);
            assert_eq!(world.get(home + IVec3::new(0, 0, 5)), Block::RuinStone);
            assert_eq!(world.get(home + IVec3::new(5, 5, 5)), Block::RuinStone);
            for dx in -7..=7 {
                let p = home + IVec3::new(dx, 1, 0);
                let key = chunk(p);
                let local = p - key * SIZE + IVec3::ONE;
                let halo = world.halo(key);
                assert_eq!(
                    halo[(local.x + 18 * (local.z + 18 * local.y)) as usize],
                    world.get(p)
                );
            }
            let mut iron = false;
            for y in home.y - 45..home.y - 12 {
                for dx in -12..=12 {
                    iron |= world.get(IVec3::new(home.x + dx, y, home.z + 12)) == Block::Iron;
                }
            }
            assert!(iron, "Iron must occur naturally below the surface");
        }
    }
}

#[cfg(test)]
mod vegetation_tests {
    use super::*;
    #[test]
    fn plants_are_non_solid_selectable_and_lose_support() {
        let mut w = World::new(314159);
        let p = IVec3::new(-16, 100, 16);
        w.set(p - IVec3::Y, Block::Grass);
        w.set(p, Block::RedFlower);
        assert!(!w.get(p).solid());
        assert_eq!(
            w.raycast(p.as_vec3() + Vec3::new(0.5, 0.2, 2.), Vec3::NEG_Z, 3.)
                .unwrap()
                .0,
            p
        );
        assert!(
            w.raycast(p.as_vec3() + Vec3::new(0.1, 0.9, 2.), Vec3::NEG_Z, 3.)
                .is_none()
        );
        w.set(p - IVec3::Y, Block::Air);
        assert_eq!(w.get(p), Block::Air);
        w.set(p - IVec3::Y, Block::Grass);
        assert_eq!(w.get(p), Block::Air);
    }
    #[test]
    fn vegetation_includes_short_tall_grass_and_non_solid_shrubs() {
        let (mut short, mut tall, mut shrubs) = (0, 0, 0);
        for x in 0..4096 {
            let p = IVec3::new(x, 100, 0);
            match vegetation_block(314159, p) {
                Block::TallGrass => {
                    let height = grass_height(vegetation_hash(314159, p), 0);
                    short += usize::from(height < 0.25);
                    tall += usize::from(height > 0.8);
                }
                Block::Shrub => shrubs += 1,
                _ => {}
            }
        }
        assert!(short > 0 && tall > 0 && shrubs > 0);
        assert!(Block::Shrub.plant() && !Block::Shrub.solid());
        let mut world = World::new(314159);
        let support = IVec3::new(0, 100, 0);
        world.set(support, Block::Grass);
        world.set(support + IVec3::Y, Block::Shrub);
        world.set(support, Block::Dirt);
        assert_eq!(world.get(support + IVec3::Y), Block::Air);
    }
    #[test]
    fn tall_grass_is_sparse_with_denser_patches() {
        let mut grass = 0;
        let mut total = 0;
        let mut dense = (0, 0);
        let mut sparse = (0, 0);
        for x in -96..96 {
            for z in -96..96 {
                let p = IVec3::new(x, 100, z);
                let block = vegetation_block(314159, p);
                let patch = noise(314159 ^ 0x74756674, x as f32 / 14., z as f32 / 14.);
                let bucket = if patch > 0.65 {
                    Some(&mut dense)
                } else if patch <= 0.5 {
                    Some(&mut sparse)
                } else {
                    None
                };
                if let Some(bucket) = bucket {
                    bucket.1 += 1;
                    if block == Block::TallGrass {
                        bucket.0 += 1;
                    }
                }
                if block == Block::TallGrass {
                    grass += 1;
                    let h = vegetation_hash(314159, p);
                    assert!((0..3).all(|i| (0.11..=0.95).contains(&grass_height(h, i))));
                }
                total += 1;
            }
        }
        assert!(grass > total / 30 && grass < total / 5);
        assert!(dense.0 as f32 / dense.1 as f32 > 5. * sparse.0 as f32 / sparse.1 as f32);
    }
    #[test]
    fn exposed_dirt_spreads_in_steps_across_slopes_and_chunk_boundaries() {
        let mut w = World::new(314159);
        let source = IVec3::new(-1, 100, 0);
        w.set(source, Block::Grass);
        let first = source + IVec3::X;
        let second = first + IVec3::X;
        let uphill = source + IVec3::Z + IVec3::Y;
        let covered = source + IVec3::NEG_Z;
        let isolated = source + IVec3::X * 5;
        for p in [first, second, uphill, covered, isolated] {
            w.set(p, Block::Dirt);
        }
        w.set(covered + IVec3::Y, Block::Stone);
        assert_eq!(w.spread_grass(source), 2);
        assert_eq!(w.get(first), Block::Grass);
        assert_eq!(w.get(uphill), Block::Grass);
        assert_eq!(w.get(second), Block::Dirt);
        assert_eq!(w.get(covered), Block::Dirt);
        assert_eq!(w.get(isolated), Block::Dirt);
        assert_eq!(w.spread_grass(source), 1);
        assert_eq!(w.get(second), Block::Grass);
        w.set(covered + IVec3::Y, Block::Air);
        assert_eq!(w.spread_grass(source), 1);
        assert_eq!(w.get(covered), Block::Grass);
        assert_eq!(Block::Grass.name(), Block::Dirt.name());
    }
    #[test]
    fn dug_natural_dirt_can_gain_cover_without_a_dirt_edit() {
        let mut w = World::new(314159);
        let p = IVec3::new(0, w.height(0, 0) - 1, 0);
        assert_eq!(w.get(p), Block::Dirt);
        w.set(p + IVec3::Y, Block::Air);
        w.set(p + IVec3::X, Block::Grass);
        w.set(p + IVec3::X + IVec3::Y, Block::Air);
        assert!(!w.edits.contains_key(&p.to_array()));
        assert!(w.spread_grass(p) > 0);
        assert_eq!(w.get(p), Block::Grass);
    }
    #[test]
    fn natural_plants_match_chunk_halos_and_tall_grass_is_sparse() {
        let w = World::new(314159);
        let mut grass = 0;
        let mut tall = 0;
        let mut flowers = 0;
        for x in -128..128 {
            let y = w.height(x, 0) + 1;
            let p = IVec3::new(x, y, 0);
            let block = w.get(p);
            if block.plant() {
                assert_eq!(block, vegetation_block(w.seed, p - IVec3::Y));
                let key = chunk(p);
                let local = p - key * SIZE;
                assert_eq!(
                    w.halo(key)
                        [((local.x + 1) + 18 * ((local.z + 1) + 18 * (local.y + 1))) as usize],
                    block
                );
                if block == Block::TallGrass {
                    grass += 1;
                    let h = vegetation_hash(w.seed, p - IVec3::Y);
                    if (0..3).any(|i| grass_height(h, i) > 0.4) {
                        tall += 1;
                    }
                } else {
                    flowers += 1;
                }
            }
        }
        assert!(grass > 0 && tall > 0 && tall < grass && flowers > 0);
    }
}
