use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

pub const MIN_Y: i32 = -512;
pub const MAX_Y: i32 = 1535;
pub const SIZE: i32 = 16;
pub const VIEW_RADIUS: i32 = 10;
/// Highest source-water voxel in oceans and terrain-carved rivers.
pub const SEA_LEVEL: i32 = 24;

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
    Water,
    FlowingWater(u8),
    Lava,
    FlowingLava(u8),
    Obsidian,
    Ice,
    Gravel,
}
impl Block {
    pub fn solid(self) -> bool {
        !matches!(self, Self::Air | Self::Torch) && self.fluid().is_none() && !self.plant()
    }
    /// Source is level zero; falling columns use level eight.
    pub fn fluid(self) -> Option<(bool, u8)> {
        match self {
            Self::Water => Some((false, 0)),
            Self::FlowingWater(n) => Some((false, n)),
            Self::Lava => Some((true, 0)),
            Self::FlowingLava(n) => Some((true, n)),
            _ => None,
        }
    }
    pub fn replaceable(self) -> bool {
        self == Self::Air || self == Self::Torch || self.plant() || self.fluid().is_some()
    }
    pub fn fluid_height(self) -> f32 {
        match self.fluid() {
            Some((_, 0 | 8)) => 0.9,
            Some((_, n)) => (8 - n.min(7)) as f32 / 9.,
            None => 1.,
        }
    }
    pub fn dirt(self) -> bool {
        matches!(self, Self::Dirt | Self::Grass)
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
            Self::Water | Self::FlowingWater(_) => [0.10, 0.37, 0.74],
            Self::Lava | Self::FlowingLava(_) => [1., 0.24, 0.015],
            Self::Ice => [0.56, 0.79, 0.91],
            Self::Obsidian => [0.13, 0.08, 0.20],
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
            Self::Gravel => [0.52, 0.49, 0.44],
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
            Self::Water | Self::FlowingWater(_) => "Water",
            Self::Lava | Self::FlowingLava(_) => "Lava",
            Self::Ice => "Ice",
            Self::Obsidian => "Obsidian",
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
            Self::Gravel => "Gravel",
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
    let meadow = noise(
        seed ^ 0x6d656164,
        support.x as f32 / 24.,
        support.z as f32 / 24.,
    );
    // Flowers gather in meadows, with occasional blooms between them.
    if !h.is_multiple_of(5) && (h >> 32).is_multiple_of(if meadow > 0.55 { 8 } else { 43 }) {
        return FLOWERS[((h >> 48) % 6) as usize];
    }
    if meadow < 0.48 && (h >> 20).is_multiple_of(61) {
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

/// Immutable procedural inputs shared by queries at different column heights.
struct TerrainColumn {
    height: i32,
    tree: Option<(i32, i32, i32)>,
    ruin: Option<IVec3>,
    cave: Option<(i32, i32)>,
}

pub(crate) struct SkyColumn {
    terrain: TerrainColumn,
    pub roof: i32,
}
impl SkyColumn {
    pub fn blocked(&self, world: &World, p: IVec3) -> bool {
        let block = world
            .edits
            .get(&p.to_array())
            .copied()
            .or_else(|| world.chunks.get(&chunk(p)).map(|c| c[index(p)]))
            .unwrap_or_else(|| {
                world.natural_column(
                    p,
                    self.terrain.height,
                    self.terrain.tree,
                    self.terrain.ruin,
                    self.terrain.cave,
                )
            });
        // Plant support never changes whether a block is opaque.
        block.solid() && block != Block::Leaves
    }
}

/// Snapshot queries for one read phase. Discard before committing world changes.
#[derive(Default)]
pub(crate) struct BlockQueries {
    blocks: HashMap<IVec3, Block>,
    columns: HashMap<(i32, i32), TerrainColumn>,
}
impl BlockQueries {
    pub fn get(&mut self, world: &World, p: IVec3) -> Block {
        if let Some(&block) = self.blocks.get(&p) {
            return block;
        }
        let block = world
            .edits
            .get(&p.to_array())
            .copied()
            .or_else(|| world.chunks.get(&chunk(p)).map(|c| c[index(p)]))
            .unwrap_or_else(|| {
                let column = self
                    .columns
                    .entry((p.x, p.z))
                    .or_insert_with(|| world.terrain_column(p.x, p.z));
                world.natural_column(p, column.height, column.tree, column.ruin, column.cave)
            });
        let block = if block.plant() && !self.get(world, p - IVec3::Y).dirt() {
            Block::Air
        } else {
            block
        };
        self.blocks.insert(p, block);
        block
    }
}

pub struct World {
    pub seed: u64,
    pub edits: BTreeMap<[i32; 3], Block>,
    pub(crate) physics_pending: std::collections::BTreeSet<[i32; 3]>,
    pub fluid_pending: std::collections::BTreeSet<[i32; 3]>,
    pub(crate) fluid_cursor: Option<[i32; 3]>,
    pub(crate) waterways: crate::hydrology::SharedCache,
    pub torch_mounts: BTreeMap<[i32; 3], [i32; 3]>,
    chunks: HashMap<IVec3, Box<[Block]>>,
    pending: VecDeque<IVec3>,
    pub dirty: Vec<IVec3>,
    pub revision: u64,
    center: IVec3,
    radius: i32,
    desired: HashSet<IVec3>,
    pub(crate) surface_bounds: HashMap<(i32, i32), (i32, i32)>,
    workers: Option<crate::streaming::Workers>,
    planner: Option<crate::streaming::Planner>,
    planning: bool,
    plan_ready: bool,
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
            fluid_pending: Default::default(),
            fluid_cursor: None,
            waterways: Default::default(),
            physics_pending: Default::default(),
            chunks: HashMap::new(),
            pending: VecDeque::new(),
            dirty: vec![],
            revision: 0,
            center: IVec3::splat(i32::MAX),
            radius: -1,
            desired: HashSet::new(),
            surface_bounds: HashMap::new(),
            workers: None,
            planner: None,
            planning: false,
            plan_ready: false,
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
        let inland =
            24. + hills + ridge.powf(1.7) * 750. * ((distance - 100.).max(0.) / 500.).min(1.);
        let exposure = smooth(((distance - 192.) / 256.).clamp(0., 1.));
        let continent = noise(self.seed ^ 0x736561, x / 1200., z / 1200.);
        // Bound individual ocean basins instead of only changing their frequency.
        // Dry shoulders between basins keep a shoreline within ordinary view range.
        let cell_x = (x / 320.).floor() as i32;
        let cell_z = (z / 320.).floor() as i32;
        let sea_hash = hash(self.seed ^ 0x73686f7265, cell_x, 0, cell_z);
        let sea_x = cell_x as f32 * 320. + 144. + (sea_hash % 33) as f32;
        let sea_z = cell_z as f32 * 320. + 144. + ((sea_hash >> 8) % 33) as f32;
        let radius = 96. + ((sea_hash >> 16) % 33) as f32;
        let shoreline = (noise(self.seed ^ 0x636f617374, x / 55., z / 55.) - 0.5) * 16.;
        let shore =
            smooth(((radius + shoreline - (x - sea_x).hypot(z - sea_z)) / 24.).clamp(0., 1.));
        let ocean = smooth(((0.54 - continent) / 0.20).clamp(0., 1.)) * exposure * shore;
        let basin = SEA_LEVEL as f32 - 8. - (0.54 - continent).max(0.) * 100.
            + noise(self.seed ^ 0x626564, x / 70., z / 70.) * 4.;
        let terrain = inland * (1. - ocean) + basin * ocean;
        // A warped noise contour gives continuous valleys across chunk boundaries.
        let warp = noise(self.seed ^ 0x77617270, x / 420., z / 420.) * 80.;
        let channel =
            (noise(self.seed ^ 0x72697672, (x + warp) / 260., (z - warp) / 260.) - 0.5).abs();
        // Higher terrain needs wider valley shoulders to avoid narrow wall-like banks.
        let shoulder = (0.035 + (terrain - SEA_LEVEL as f32).max(0.) * 0.0005).min(0.16);
        let river = smooth(((0.018 + shoulder - channel) / shoulder).clamp(0., 1.)) * exposure;
        (terrain - (terrain - (SEA_LEVEL - 4) as f32).max(0.) * river) as i32
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
        if y <= SEA_LEVEL + 2 {
            return None;
        }
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
        let tx = x.div_euclid(7);
        let tz = z.div_euclid(7);
        let mut best = None;
        let mut distance = i32::MAX;
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
                if h.is_multiple_of(chance)
                    && self.biome(cx, cz) != Biome::Tundra
                    && (x - cx).abs() <= 3
                    && (z - cz).abs() <= 3
                {
                    let height = self.height(cx, cz);
                    let root = IVec3::new(cx, height + 1, cz);
                    if height > SEA_LEVEL + 1
                        && self
                            .natural_column(
                                root,
                                height,
                                None,
                                self.nearby_ruin(cx, cz),
                                self.cave_column(cx, cz),
                            )
                            .replaceable()
                        && self.waterway_column(cx, cz).is_none()
                        && self
                            .pool_block(IVec3::new(cx, height, cz), height)
                            .is_none()
                        && !self
                            .cave_column(cx, cz)
                            .is_some_and(|(lo, hi)| (lo..=hi).contains(&height))
                    {
                        let d = (x - cx).pow(2) + (z - cz).pow(2);
                        if d < distance {
                            distance = d;
                            best = Some((cx, height, cz));
                        }
                    }
                }
            }
        }
        best
    }
    /// A column's cave interval, shared by direct queries and chunk generation.
    /// Features fit inside 160-block cells, including their entrance ramps.
    pub(crate) fn cave_column(&self, x: i32, z: i32) -> Option<(i32, i32)> {
        let (lo, mut hi) = self.raw_cave_column(x, z)?;
        // Hydrology planning only uses raw geometry, so this cannot recurse.
        if let Some((_, level)) = self.waterway_column(x, z) {
            hi = hi.min(level - 12);
        }
        if !self.cave_sink(x, z) {
            for (dx, dz) in [(0, 0), (-12, 0), (12, 0), (0, -12), (0, 12)] {
                let height = self.height(x + dx, z + dz);
                if height < SEA_LEVEL {
                    hi = hi.min(SEA_LEVEL - 12);
                } else if self
                    .pool_block(IVec3::new(x + dx, height - 1, z + dz), height)
                    .is_some_and(|b| b.fluid().is_some() || b == Block::Ice)
                {
                    hi = hi.min(height - 13);
                }
            }
        }
        (hi >= lo).then_some((lo, hi))
    }

    /// Rare cave formations may receive a natural underground waterfall.
    fn cave_sink(&self, x: i32, z: i32) -> bool {
        hash(
            self.seed ^ 0xca7e_2026,
            x.div_euclid(160),
            0,
            z.div_euclid(160),
        )
        .is_multiple_of(32)
    }

    pub(crate) fn raw_cave_column(&self, x: i32, z: i32) -> Option<(i32, i32)> {
        if (x as f64).hypot(z as f64) < 32. {
            return None;
        }
        let cell_x = x.div_euclid(160);
        let cell_z = z.div_euclid(160);
        let h = hash(self.seed ^ 0xca7e_2026, cell_x, 0, cell_z);
        let cx = cell_x * 160 + 72 + (h % 17) as i32;
        let cz = cell_z * 160 + 72 + ((h >> 8) % 17) as i32;
        let (along, across) = if h & 0x10000 == 0 {
            (x - cx, z - cz)
        } else {
            (z - cz, x - cx)
        };
        if !(-56..=66).contains(&along) || across.abs() > 24 {
            return None;
        }
        let (width, ceiling) = match (h >> 20) % 3 {
            0 => (3., 6.),   // Low, winding passages.
            1 => (2.5, 24.), // Tall, narrow crevices.
            _ => (22., 44.), // Vast underground chasms.
        };
        let bend = (along as f32 * 0.065 + (h % 31) as f32).sin() * 2.;
        let lateral = across as f32 - bend;
        let shape = ((along as f32 + 12.) / 44.).powi(2) + (lateral / width).powi(2);
        let base = self.height(cx, cz) - 48;
        let mut interval = if shape < 1. {
            let radius = (1. - shape).sqrt();
            Some((
                base - (radius * 4.) as i32,
                base + (radius * ceiling) as i32,
            ))
        } else {
            None
        };
        // About one in three formations has a walkable descending entrance.
        // End above the local surface so grass/dirt cannot seal the opening.
        if (h >> 28).is_multiple_of(3) && (0..=66).contains(&along) && lateral.abs() < 3.5 {
            let (ex, ez) = if h & 0x10000 == 0 {
                (cx + 60, cz)
            } else {
                (cx, cz + 60)
            };
            let exit = self.height(ex, ez);
            if (exit - (base + 48)).abs() > 12 {
                return interval;
            }
            let floor = base + (((exit - base - 2) as f32 * along as f32 / 60.).floor() as i32);
            let ramp = (floor + 1, floor + 6);
            interval = Some(match interval {
                Some((lo, hi)) => (lo.min(ramp.0), hi.max(ramp.1)),
                None => ramp,
            });
        }
        interval
    }
    fn natural_column(
        &self,
        p: IVec3,
        height: i32,
        tree: Option<(i32, i32, i32)>,
        ruin: Option<IVec3>,
        cave: Option<(i32, i32)>,
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
        if let Some(water) = self.waterway_block(p, height) {
            return water;
        }
        // A rare cave that actually intersects a submerged bed receives a falling
        // column supplied by the ordinary sea-level source above it. Other beds seal.
        if height < SEA_LEVEL
            && self.cave_sink(p.x, p.z)
            && let Some((lo, hi)) = cave
            && hi >= height - 3
            && p.y >= lo
            && p.y <= height
        {
            return Block::FlowingWater(8);
        }
        // Seal the upper bed before carving caves; surface water is generated as sources.
        if height <= SEA_LEVEL + 1 {
            if p.y > height && p.y <= SEA_LEVEL {
                return Block::Water;
            }
            if p.y <= height && p.y >= height - 3 {
                return if hash(
                    self.seed ^ 0x73686f72,
                    p.x.div_euclid(3),
                    0,
                    p.z.div_euclid(3),
                )
                .is_multiple_of(5)
                {
                    Block::Gravel
                } else {
                    Block::Sand
                };
            }
            if height < SEA_LEVEL && p.y > SEA_LEVEL {
                return Block::Air;
            }
        }
        if let Some(pool) = self.pool_block(p, height) {
            return pool;
        }
        if p.y > MIN_Y + 3 && cave.is_some_and(|(lo, hi)| (lo..=hi).contains(&p.y)) {
            return Block::Air;
        }
        // Keep each formation inside its cell so neighboring outcrops stay separated.
        if p.y >= height && p.y <= height + 4 && self.biome(p.x, p.z) == Biome::Grassland {
            let cx = p.x.div_euclid(48);
            let cz = p.z.div_euclid(48);
            let h = hash(self.seed ^ 0x726f636b, cx, 0, cz);
            if h.is_multiple_of(3) {
                let rx = cx * 48 + 10 + ((h >> 8) % 28) as i32;
                let rz = cz * 48 + 10 + ((h >> 24) % 28) as i32;
                let top = outcrop_top(h, p.x - rx, p.z - rz);
                if top > 0 {
                    let base = self.height(rx, rz);
                    if (height - base).abs() <= 1 && p.y <= base + top {
                        return Block::Stone;
                    }
                }
            }
        }
        if p.y > height {
            if let Some((tx, ty, tz)) = tree {
                let h = hash(self.seed ^ 0x74726565, tx, 0, tz);
                let tall = 5 + (h % 5) as i32;
                let dx = p.x - tx;
                let dz = p.z - tz;
                let dy = p.y - ty;
                let branch_y = tall - 2;
                let axis_x = h & 8 == 0;
                let branch = dy == branch_y
                    && if axis_x {
                        dz == 0 && dx.abs() <= 2
                    } else {
                        dx == 0 && dz.abs() <= 2
                    };
                if (dx == 0 && dz == 0 && dy <= tall) || branch {
                    return Block::Log;
                }
                let radius = 2 + ((h >> 5) % 2) as i32;
                let crown = dx.abs() + dz.abs() + (dy - tall + 1).abs();
                if dy >= tall - 3
                    && dy <= tall + 1
                    && crown <= radius + 1
                    && (crown <= 2 || !hash(h, dx, dy, dz).is_multiple_of(7))
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
                self.cave_column(p.x, p.z),
            ),
        )
    }
    fn terrain_column(&self, x: i32, z: i32) -> TerrainColumn {
        TerrainColumn {
            height: self.height(x, z),
            tree: self.tree(x, z),
            ruin: self.nearby_ruin(x, z),
            cave: self.cave_column(x, z),
        }
    }
    /// Prepare one column for both roof searches and ray occupancy queries.
    pub(crate) fn sky_column(&self, x: i32, z: i32) -> SkyColumn {
        let mut column = SkyColumn {
            terrain: self.terrain_column(x, z),
            roof: MIN_Y,
        };
        let top = self
            .edits
            .range([x, i32::MIN, i32::MIN]..=[x, i32::MAX, i32::MAX])
            .filter(|(p, _)| p[2] == z)
            .map(|(p, _)| p[1])
            .max()
            .unwrap_or(MIN_Y)
            .max(column.terrain.height + 16)
            .min(MAX_Y);
        column.roof = (MIN_Y..=top)
            .rev()
            .find(|&y| column.blocked(self, IVec3::new(x, y, z)))
            .unwrap_or(MIN_Y);
        column
    }
    #[cfg(test)]
    pub fn sky_roof(&self, x: i32, z: i32) -> i32 {
        self.sky_column(x, z).roof
    }
    fn supported_plant(&self, p: IVec3, b: Block) -> Block {
        if b.plant() && !self.get(p - IVec3::Y).dirt() {
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
                let cave = self.cave_column(wx, wz);
                for y in -1..=SIZE {
                    let p = IVec3::new(wx, base.y + y, wz);
                    let block = self
                        .edits
                        .get(&p.to_array())
                        .copied()
                        .or_else(|| self.chunks.get(&chunk(p)).map(|c| c[index(p)]))
                        .unwrap_or_else(|| self.natural_column(p, h, t, ruin, cave));
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
        if !b.dirt() && self.get(p + IVec3::Y).plant() {
            self.set(p + IVec3::Y, Block::Air);
        }
        self.wake_fluid(p);
        self.wake_physics(p);
        self.edits.insert(p.to_array(), b);
        if b != Block::Torch {
            self.torch_mounts.remove(&p.to_array());
        }
        let key = chunk(p);
        if let Some(c) = self.chunks.get_mut(&key) {
            c[index(p)] = b;
        }
        self.dirty.extend(
            self.chunks
                .keys()
                .copied()
                .filter(|k| (k.x - key.x).abs() <= 1 && (k.z - key.z).abs() <= 1),
        );
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
                if self.get(p) != Block::Dirt
                    || (self.get(p + IVec3::Y).solid() || self.get(p + IVec3::Y).fluid().is_some())
                {
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
    pub(crate) fn plan_surface(&mut self, c: IVec3, radius: i32) -> HashSet<IVec3> {
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
                            high = high.max(h.max(SEA_LEVEL) + 6);
                            if let Some((bed, top)) =
                                self.waterway_bounds(x * SIZE + dx, z * SIZE + dz, h)
                            {
                                low = low.min(bed);
                                high = high.max(top);
                            }
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
        desired
    }
    pub fn stream(&mut self, pos: Vec3, radius: i32) -> Vec<IVec3> {
        let c = chunk(pos.floor().as_ivec3());
        let mut removed = vec![];
        let radius = radius.max(0);
        if c != self.center || radius != self.radius {
            self.center = c;
            self.radius = radius;
            self.plan_ready = false;
            self.desired.clear();
            if let Some(workers) = &self.workers {
                for key in workers.cancel_pending() {
                    self.active.remove(&key);
                    if self.chunks.contains_key(&key) && !self.pending.contains(&key) {
                        self.pending.push_back(key);
                    }
                }
            }
            self.chunks.retain(|key, _| {
                let keep = (key.x - c.x).abs() <= radius && (key.z - c.z).abs() <= radius;
                if !keep {
                    removed.push(*key);
                }
                keep
            });
            self.pending
                .retain(|key| (key.x - c.x).abs() <= radius && (key.z - c.z).abs() <= radius);
            self.meshes.retain(|(key, _)| self.chunks.contains_key(key));
        }
        let planner = self
            .planner
            .get_or_insert_with(crate::streaming::Planner::new);
        if let Some(plan) = planner.receive() {
            self.planning = false;
            self.surface_bounds = plan.bounds;
            if plan.center == c && plan.radius == radius {
                self.plan_ready = true;
                let desired = plan.desired;
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
        }
        if !self.plan_ready && !self.planning {
            planner.request(
                self.seed,
                c,
                radius,
                std::mem::take(&mut self.surface_bounds),
                self.waterways.clone(),
            );
            self.planning = true;
        }
        let mut priorities_changed = false;
        self.meshes.retain(|(key, _)| !self.dirty.contains(key));
        for key in if self.plan_ready {
            std::mem::take(&mut self.dirty)
        } else {
            Vec::new()
        } {
            if self.desired.contains(&key) && !self.pending.contains(&key) {
                self.pending.push_back(key);
                priorities_changed = true;
            }
        }
        let workers = self
            .workers
            .get_or_insert_with(crate::streaming::Workers::new);
        let accept_start = std::time::Instant::now();
        while self.meshes.len() < crate::streaming::UPLOAD_BACKLOG
            && accept_start.elapsed() < std::time::Duration::from_millis(2)
        {
            let Some(result) = workers.receive() else {
                break;
            };
            self.active.remove(&result.key);
            if !self.desired.contains(&result.key) {
                if !self.plan_ready
                    && self.chunks.contains_key(&result.key)
                    && !self.pending.contains(&result.key)
                {
                    self.pending.push_back(result.key);
                }
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
            // Edits already wake fluids. Remeshing must not repeatedly requeue
            // settled ocean interiors and starve spill edges.
            if !self.chunks.contains_key(&result.key) {
                self.fluid_pending.extend(result.fluid_frontier);
            }
            for (i, b) in result.blocks.iter().enumerate() {
                if matches!(b, Block::Sand | Block::Gravel | Block::Leaves) {
                    let p = result.key * SIZE
                        + IVec3::new(
                            i as i32 % SIZE,
                            i as i32 / (SIZE * SIZE),
                            i as i32 / SIZE % SIZE,
                        );
                    self.physics_pending.insert(p.to_array());
                }
            }
            self.chunks.insert(result.key, result.blocks);
            self.meshes.retain(|(key, _)| *key != result.key);
            self.meshes.push((result.key, result.mesh));
        }
        if priorities_changed {
            self.pending
                .make_contiguous()
                .sort_by_key(|p| ((*p - c).length_squared(), p.x, p.y, p.z));
        }
        // A bounded backlog keeps workers running between redraws.
        let dispatch_start = std::time::Instant::now();
        while self.plan_ready
            && self.active.len() < crate::streaming::MAX_OUTSTANDING
            && dispatch_start.elapsed() < std::time::Duration::from_millis(2)
        {
            let Some(i) = self.pending.iter().position(|k| !self.active.contains(k)) else {
                break;
            };
            let key = self.pending.remove(i).unwrap();
            if !self.desired.contains(&key) {
                continue;
            }
            let base = key * SIZE;
            let mut snapshot = World::new(self.seed);
            snapshot.waterways = self.waterways.clone();
            snapshot.torch_mounts = self.torch_mounts.clone();
            // Include the full vertical columns for sky occlusion above the AO halo.
            snapshot.edits = self
                .edits
                .range(
                    [base.x - crate::render::SKY_REACH - 1, i32::MIN, i32::MIN]
                        ..=[
                            base.x + SIZE + crate::render::SKY_REACH + 1,
                            i32::MAX,
                            i32::MAX,
                        ],
                )
                .filter(|(p, _)| {
                    (base.z - crate::render::SKY_REACH - 1
                        ..=base.z + SIZE + crate::render::SKY_REACH + 1)
                        .contains(&p[2])
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
        [
            self.chunks.len(),
            self.pending.len() + self.meshes.len() + usize::from(!self.plan_ready),
            self.active.len(),
        ]
    }
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
    pub fn raycast(&self, origin: Vec3, dir: Vec3, reach: f32) -> Option<(IVec3, IVec3)> {
        self.raycast_fluid(origin, dir, reach, false)
    }
    pub fn raycast_fluid(
        &self,
        origin: Vec3,
        dir: Vec3,
        reach: f32,
        sources: bool,
    ) -> Option<(IVec3, IVec3)> {
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
                && (block.fluid().is_none()
                    || (sources && matches!(block, Block::Water | Block::Lava)))
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

// Union of offset, elliptical lobes avoids the repeated diamond-and-cap silhouette.
// Zero means outside the footprint; positive values are heights above the ground.
fn outcrop_top(h: u64, mut x: i32, mut z: i32) -> i32 {
    for _ in 0..((h >> 40) & 3) {
        (x, z) = (-z, x);
    }
    if (h >> 42) & 1 != 0 {
        x = -x;
    }
    let size = ((h >> 44) & 1) as i32;
    let lobes: &[(i32, i32, i32, i32, i32)] = match (h >> 48) % 5 {
        0 => &[(0, 0, 2 + size, 2, 3), (1, 0, 2, 2, 2)],
        1 => &[(0, 0, 3 + size, 2 + size, 1), (-1, 0, 2, 2, 2)],
        2 => &[(-1, 0, 3 + size, 1 + size, 2), (2, 0, 2, 1, 3)],
        3 => &[(-2, 0, 1 + size, 2, 3), (2, 1, 1 + size, 2, 2)],
        _ => &[(-2, -1, 2, 2, 2), (1, 1, 2 + size, 2, 3), (2, -2, 1, 1, 1)],
    };
    lobes.iter().fold(0, |top, &(cx, cz, rx, rz, height)| {
        let distance = (x - cx).pow(2) * rz * rz + (z - cz).pow(2) * rx * rx;
        let radius = rx * rx * rz * rz;
        if distance > radius {
            top
        } else {
            // Broad tops and uneven shoulders instead of a mandatory single-block peak.
            top.max(height - i32::from(distance * 3 > radius * 2 && height > 1))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_water_keeps_land_within_the_normal_view_distance() {
        for seed in [1, 41, 314159] {
            let w = World::new(seed);
            let mut checked = 0;
            for x in (-2048..=2048).step_by(96) {
                for z in (-2048..=2048).step_by(96) {
                    if w.height(x, z) >= SEA_LEVEL {
                        continue;
                    }
                    checked += 1;
                    let shore = (0..16).any(|ray| {
                        let angle = ray as f32 * std::f32::consts::TAU / 16.;
                        (16..=144).step_by(16).any(|distance| {
                            let qx = x + (angle.cos() * distance as f32).round() as i32;
                            let qz = z + (angle.sin() * distance as f32).round() as i32;
                            w.height(qx, qz) >= SEA_LEVEL
                                && w.get(IVec3::new(qx, SEA_LEVEL, qz)).solid()
                        })
                    });
                    assert!(shore, "no nearby shore at ({x}, {z}), seed={seed}");
                }
            }
            assert!(checked > 30);
        }
    }

    #[test]
    fn ordinary_caves_leave_thick_roofs_near_surface_water() {
        let w = World::new(41);
        let mut checked = 0;
        for x in (-1600..1600).step_by(8) {
            for z in (-1600..1600).step_by(8) {
                if w.cave_sink(x, z) {
                    continue;
                }
                let Some((_, raw_hi)) = w.raw_cave_column(x, z) else {
                    continue;
                };
                if raw_hi <= SEA_LEVEL - 12 {
                    continue;
                }
                let near_water = [(0, 0), (-12, 0), (12, 0), (0, -12), (0, 12)]
                    .iter()
                    .any(|&(dx, dz)| w.height(x + dx, z + dz) < SEA_LEVEL);
                if near_water {
                    checked += 1;
                    assert!(
                        w.cave_column(x, z)
                            .is_none_or(|(_, hi)| hi <= SEA_LEVEL - 12)
                    );
                    assert_ne!(w.get(IVec3::new(x, SEA_LEVEL - 1, z)), Block::Air);
                }
            }
        }
        assert!(checked > 10, "water-adjacent caves must be exercised");
    }

    #[test]
    fn rare_submerged_cave_openings_receive_a_supplied_waterfall() {
        let mut w = World::new(41);
        let mut fall = None;
        'cells: for cx in -24..24 {
            for cz in -24..24 {
                if !w.cave_sink(cx * 160, cz * 160) {
                    continue;
                }
                for dx in (16..144).step_by(4) {
                    for dz in (16..144).step_by(4) {
                        let (x, z) = (cx * 160 + dx, cz * 160 + dz);
                        let height = w.height(x, z);
                        if height >= SEA_LEVEL {
                            continue;
                        }
                        let Some((lo, hi)) = w.cave_column(x, z) else {
                            continue;
                        };
                        if hi < height - 3 || lo >= height - 2 {
                            continue;
                        }
                        let p = IVec3::new(x, height, z);
                        if w.get(p) == Block::FlowingWater(8) {
                            fall = Some((p, lo));
                            break 'cells;
                        }
                    }
                }
            }
        }
        let (p, lo) = fall.expect("occasional natural underground waterfall");
        for y in lo..=p.y {
            assert_eq!(w.get(IVec3::new(p.x, y, p.z)), Block::FlowingWater(8));
        }
        assert_eq!(w.get(p + IVec3::Y), Block::Water);
        w.wake_fluid(p);
        for _ in 0..12 {
            w.tick_fluids(p, false);
        }
        assert!(w.get(p).fluid().is_some());
        let key = chunk(p);
        let halo = w.halo(key);
        let local = p - key * SIZE + IVec3::ONE;
        assert_eq!(
            halo[(local.x + 18 * (local.z + 18 * local.y)) as usize],
            w.get(p)
        );
    }

    #[test]
    fn seas_have_flat_sources_sealed_beds_and_streamed_surfaces() {
        let mut w = World::new(314159);
        let mut deep = None;
        let mut shallow = None;
        for x in (-4096..=4096).step_by(32) {
            for z in (-4096..=4096).step_by(32) {
                let h = w.height(x, z);
                if h < SEA_LEVEL - 12 && deep.is_none() {
                    deep = Some(IVec3::new(x, SEA_LEVEL, z));
                }
                if h == SEA_LEVEL - 4 && shallow.is_none() {
                    shallow = Some(IVec3::new(x, SEA_LEVEL, z));
                }
            }
        }
        for p in [deep.expect("deep ocean"), shallow.expect("shallow water")] {
            let h = w.height(p.x, p.z);
            for y in h + 1..=SEA_LEVEL {
                assert_eq!(w.get(IVec3::new(p.x, y, p.z)), Block::Water);
            }
            assert_eq!(w.get(p + IVec3::Y), Block::Air);
            for y in h - 3..=h {
                assert!(matches!(
                    w.get(IVec3::new(p.x, y, p.z)),
                    Block::Sand | Block::Gravel
                ));
            }
            let key = chunk(p);
            assert!(w.plan_surface(key, 0).contains(&key));
            let halo = w.halo(key);
            for y in -1..=SIZE {
                for z in -1..=SIZE {
                    for x in -1..=SIZE {
                        assert_eq!(
                            halo[((x + 1) + 18 * ((z + 1) + 18 * (y + 1))) as usize],
                            w.get(key * SIZE + IVec3::new(x, y, z))
                        );
                    }
                }
            }
            w.set(p, Block::Stone);
            assert_eq!(w.get(p), Block::Stone);
        }
    }

    #[test]
    fn river_valleys_cut_continental_land_to_sea_level() {
        let w = World::new(41);
        let mut river = None;
        for x in (-4096..=4096).step_by(16) {
            for z in (-4096..=4096).step_by(16) {
                if noise(w.seed ^ 0x736561, x as f32 / 1200., z as f32 / 1200.) > 0.6
                    && w.height(x, z) == SEA_LEVEL - 4
                {
                    river = Some(IVec3::new(x, SEA_LEVEL, z));
                    break;
                }
            }
            if river.is_some() {
                break;
            }
        }
        let p = river.expect("river crossing continental land");
        assert_eq!(w.get(p), Block::Water);
        assert_eq!(w.get(p - IVec3::Y * 3), Block::Water);
        assert_eq!(w.get(p + IVec3::Y), Block::Air);
    }

    #[test]
    fn sea_generation_preserves_dry_spawn_across_seeds() {
        for seed in [0, 1, 41, 314159, u64::MAX] {
            let w = World::new(seed);
            for x in (-128..=128).step_by(16) {
                for z in (-128..=128).step_by(16) {
                    assert!(w.height(x, z) >= SEA_LEVEL);
                }
            }
        }
    }

    #[test]
    fn outcrop_shapes_have_distinct_footprints_and_broad_tops() {
        let mut shapes = HashSet::new();
        for family in 0..5_u64 {
            for size in 0..2_u64 {
                let h = (family << 48) | (size << 44);
                let mut shape = Vec::new();
                for x in -10..=10 {
                    for z in -10..=10 {
                        let top = outcrop_top(h, x, z);
                        if top > 0 {
                            assert!(x.abs() <= 5 && z.abs() <= 5);
                            assert!((1..=3).contains(&top));
                            shape.push((x, z, top));
                        }
                    }
                }
                let peak = shape.iter().map(|p| p.2).max().unwrap();
                assert!(shape.iter().filter(|p| p.2 == peak).count() > 1);
                shapes.insert(shape);
            }
        }
        assert_eq!(shapes.len(), 10);
    }

    #[test]
    fn grassland_outcrops_are_sparse_and_match_chunk_halos() {
        let w = World::new(314159);
        let mut rocks = Vec::new();
        for x in -64..64 {
            for z in -64..64 {
                let p = IVec3::new(x, w.height(x, z) + 1, z);
                if w.get(p) == Block::Stone {
                    assert_eq!(w.biome(x, z), Biome::Grassland);
                    rocks.push(p);
                }
            }
        }
        assert!(!rocks.is_empty() && rocks.len() < 128 * 128 / 50);
        for p in rocks.iter().step_by(7) {
            let key = chunk(*p);
            let local = *p - key * SIZE;
            assert_eq!(
                w.halo(key)[((local.x + 1) + 18 * ((local.z + 1) + 18 * (local.y + 1))) as usize],
                Block::Stone
            );
        }
    }
    #[test]
    fn cached_surface_plans_match_fresh_worlds_and_stay_bounded() {
        let mut cached = World::new(314159);
        for pos in [
            Vec3::new(-32., 50., -16.),
            Vec3::new(-16., 90., -16.),
            Vec3::new(-16., -100., -16.),
            Vec3::new(1024., 500., 1024.),
        ] {
            wait_for_plan(&mut cached, pos, 2);
            let mut fresh = World::new(314159);
            wait_for_plan(&mut fresh, pos, 2);
            assert_eq!(cached.desired, fresh.desired);
            assert_eq!(cached.surface_bounds, fresh.surface_bounds);
            assert_eq!(cached.surface_bounds.len(), 25);
        }
        wait_for_plan(&mut cached, Vec3::ZERO, 0);
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
        wait_for_plan(&mut w, pos, 0);
        let surface = chunk(IVec3::new(x, w.height(x, z), z));
        assert!(surface.y > chunk(pos.as_ivec3()).y + 4);
        assert!(w.desired.contains(&surface));
        wait_for_plan(&mut w, pos, 2);
        let edge = IVec3::new(surface.x + 2, surface.y, surface.z);
        assert!(
            w.pending.iter().any(|k| k.x == edge.x && k.z == edge.z)
                || w.chunks.keys().any(|k| k.x == edge.x && k.z == edge.z)
        );
        wait_for_plan(&mut w, Vec3::new(0., 40., 0.), 0);
        assert!(w.pending.iter().all(|k| k.x == 0 && k.z == 0));
    }
    #[test]
    fn background_results_preserve_edits_and_match_synchronous_meshes() {
        let mut w = World::new(42);
        let pos = Vec3::new(-1., 40., 16.);
        let key = chunk(pos.as_ivec3());
        wait_for_plan(&mut w, pos, 1);
        assert!(!w.active.is_empty());
        assert!(w.active.len() <= crate::streaming::MAX_OUTSTANDING);
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
            w.meshes.clear();
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
        wait_for_plan(&mut w, Vec3::new(0., 40., 0.), 0);
        let old = w.active.clone();
        let pos = Vec3::new(1000., 40., 1000.);
        let key = chunk(pos.as_ivec3());
        wait_for_chunk(&mut w, pos, key);
        assert!(old.iter().all(|k| !w.loaded(*k)));
        assert!(w.chunks.keys().all(|k| w.desired.contains(k)));
        assert!(w.active.len() <= crate::streaming::MAX_OUTSTANDING);
    }
    #[test]
    fn obsolete_plans_are_rejected_and_edits_during_planning_survive() {
        let mut w = World::new(42);
        w.stream(Vec3::new(-1000., 40., -1000.), 2);
        assert!(w.planning);
        // Move before accepting the first plan. No old plan or result may be applied.
        let pos = Vec3::new(1024., 40., 1024.);
        let key = chunk(pos.as_ivec3());
        let edit = key * SIZE;
        w.set(edit, Block::Planks);
        wait_for_chunk(&mut w, pos, key);
        assert!(w.plan_ready);
        assert!(
            w.desired
                .iter()
                .all(|k| (k.x - key.x).abs() <= 1 && (k.z - key.z).abs() <= 1)
        );
        assert_eq!(w.chunks[&key][index(edit)], Block::Planks);
        let expected = crate::render::mesh_chunk(&w, key);
        let actual = &w.meshes.iter().find(|(k, _)| *k == key).unwrap().1;
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(actual),
            bytemuck::cast_slice::<_, u8>(&expected)
        );
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
    fn wait_for_plan(w: &mut World, pos: Vec3, radius: i32) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            w.meshes.clear();
            w.stream(pos, radius);
            if w.plan_ready {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "planning timed out");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    fn wait_for_chunk(w: &mut World, pos: Vec3, key: IVec3) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !w.loaded(key) {
            assert!(std::time::Instant::now() < deadline, "streaming timed out");
            w.meshes.clear();
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
            for dx in -12..=12 {
                // River valleys can sit far below the nearby ruin's elevation.
                let surface = world.height(home.x + dx, home.z + 12);
                for y in surface - 45..surface - 12 {
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
    fn all_plants_require_dirt_in_queries_and_halos() {
        let mut w = World::new(314159);
        let p = IVec3::new(0, 100, 0);
        for support in [
            Block::Dirt,
            Block::Grass,
            Block::Stone,
            Block::Sand,
            Block::Snow,
            Block::Log,
            Block::Air,
        ] {
            for plant in [Block::TallGrass, Block::Shrub, Block::RedFlower] {
                w.set(p - IVec3::Y, support);
                w.set(p, plant);
                let expected = if support.dirt() { plant } else { Block::Air };
                assert_eq!(w.get(p), expected);
                let local = p - chunk(p) * SIZE;
                assert_eq!(
                    w.halo(chunk(p))
                        [((local.x + 1) + 18 * ((local.z + 1) + 18 * (local.y + 1))) as usize],
                    expected
                );
            }
        }
    }
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
        assert_eq!(world.get(support + IVec3::Y), Block::Shrub);
        world.set(support, Block::Stone);
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

#[cfg(test)]
mod cave_tests {
    use super::*;

    #[test]
    fn caves_have_varied_scale_surface_access_and_protected_bedrock() {
        for seed in [1, 42, 314159] {
            let world = World::new(seed);
            let mut kinds = [false; 3];
            let mut entrance = None;
            for cx in -4..=4 {
                for cz in -4..=4 {
                    let h = hash(seed ^ 0xca7e_2026, cx, 0, cz);
                    let x = cx * 160 + 72 + (h % 17) as i32;
                    let z = cz * 160 + 72 + ((h >> 8) % 17) as i32;
                    let (raw_lo, raw_hi) = world.raw_cave_column(x, z).unwrap();
                    let kind = ((h >> 20) % 3) as usize;
                    assert!(raw_hi - raw_lo >= [4, 12, 36][kind]);
                    let Some((lo, hi)) = world.cave_column(x, z) else {
                        continue;
                    };
                    kinds[kind] |= hi - lo >= [4, 12, 36][kind];
                    assert_eq!(world.get(IVec3::new(x, (lo + hi) / 2, z)), Block::Air);
                    assert_eq!(world.get(IVec3::new(x, MIN_Y, z)), Block::Stone);
                    for d in 48..=60 {
                        let (ex, ez) = if h & 0x10000 == 0 {
                            (x + d, z)
                        } else {
                            (x, z + d)
                        };
                        let surface = IVec3::new(ex, world.height(ex, ez), ez);
                        if world.get(surface) == Block::Air {
                            entrance = Some(surface);
                        }
                    }
                }
            }
            assert!(kinds.into_iter().all(|found| found));
            let mouth = entrance.expect("occasional open surface entrances");
            let key = chunk(mouth);
            let halo = world.halo(key);
            for y in -1..=SIZE {
                for z in -1..=SIZE {
                    for x in -1..=SIZE {
                        let p = key * SIZE + IVec3::new(x, y, z);
                        assert_eq!(
                            halo[((x + 1) + 18 * ((z + 1) + 18 * (y + 1))) as usize],
                            world.get(p)
                        );
                    }
                }
            }
            assert_eq!(
                world.get(IVec3::new(2, world.height(2, 2), 2)),
                Block::Grass
            );
        }
    }
}

#[cfg(test)]
mod sky_column_tests {
    use super::*;

    #[test]
    fn pulse_queries_match_direct_reads_across_world_layers() {
        for seed in [42, 314159, 987654] {
            let mut world = World::new(seed);
            for (x, z) in [(-17, -16), (0, 0), (255, 256), (1024, -1024)] {
                let y = world.height(x, z);
                let p = IVec3::new(x, y + 1, z);
                for resident in [false, true] {
                    if resident {
                        world
                            .chunks
                            .insert(chunk(p), vec![Block::Planks; 4096].into_boxed_slice());
                    }
                    for support in [Block::Dirt, Block::Stone, Block::Air] {
                        world.set(p - IVec3::Y, support);
                        for block in [
                            Block::Air,
                            Block::Water,
                            Block::Lava,
                            Block::FlowingWater(7),
                            Block::Leaves,
                            Block::RedFlower,
                        ] {
                            world.set(p, block);
                            let mut queries = BlockQueries::default();
                            for dy in (-64..=32).chain([MIN_Y - y - 2, MAX_Y - y + 2]) {
                                for dx in -1..=1 {
                                    for dz in -1..=1 {
                                        let q = IVec3::new(x + dx, y + dy, z + dz);
                                        let expected = world.get(q);
                                        // Both fresh queries and cache hits must match.
                                        for _ in 0..2 {
                                            assert_eq!(
                                                queries.get(&world, q),
                                                expected,
                                                "seed={seed} q={q} resident={resident}"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cached_occupancy_matches_world_queries_with_edits_and_resident_chunks() {
        for seed in [42, 314159, 987654] {
            let mut world = World::new(seed);
            for (x, z) in [(-17, -16), (0, 0), (255, 256)] {
                let y = world.height(x, z);
                for resident in [false, true] {
                    if resident {
                        // Deliberately different from procedural terrain to verify precedence.
                        world.chunks.insert(
                            chunk(IVec3::new(x, y, z)),
                            vec![Block::Planks; 4096].into_boxed_slice(),
                        );
                    }
                    for block in [
                        Block::Stone,
                        Block::Air,
                        Block::Leaves,
                        Block::Water,
                        Block::RedFlower,
                    ] {
                        world.set(IVec3::new(x, y + 1, z), block);
                        let column = world.sky_column(x, z);
                        for dy in -64..=32 {
                            let p = IVec3::new(x, y + dy, z);
                            let expected = world.get(p);
                            assert_eq!(
                                column.blocked(&world, p),
                                expected.solid() && expected != Block::Leaves,
                                "seed={seed} p={p} resident={resident} edit={block:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

impl World {
    pub(crate) fn wake_physics(&mut self, p: IVec3) {
        self.physics_pending.insert(p.to_array());
        for d in [
            IVec3::X,
            IVec3::NEG_X,
            IVec3::Y,
            IVec3::NEG_Y,
            IVec3::Z,
            IVec3::NEG_Z,
        ] {
            self.physics_pending.insert((p + d).to_array());
        }
    }

    /// Bounded active frontier; one falling voxel step per 200ms pulse.
    pub fn tick_block_physics(&mut self, center: IVec3, pulse: u32) {
        let nearby: Vec<_> = self
            .physics_pending
            .iter()
            .copied()
            .filter(|p| (IVec3::from_array(*p) - center).abs().max_element() <= 64)
            .collect();
        let count = nearby.len();
        let ready: Vec<_> = nearby
            .into_iter()
            .cycle()
            .skip(if count == 0 {
                0
            } else {
                pulse as usize * 256 % count
            })
            .take(count.min(256))
            .collect();
        let mut queries = BlockQueries::default();
        let mut supported = HashSet::new();
        let mut changes = Vec::new();
        for key in ready {
            self.physics_pending.remove(&key);
            let p = IVec3::from_array(key);
            let b = queries.get(self, p);
            if matches!(b, Block::Sand | Block::Gravel) && p.y > MIN_Y + 1 {
                let below = queries.get(self, p - IVec3::Y);
                if below.replaceable() {
                    changes.push((p, Block::Air));
                    changes.push((p - IVec3::Y, b));
                }
            } else if b == Block::Leaves && supported.contains(&p) {
                self.physics_pending.insert(key);
            } else if b == Block::Leaves {
                let mut seen = HashSet::from([p]);
                let mut todo = VecDeque::from([p]);
                let mut attached = false;
                while let Some(q) = todo.pop_front() {
                    for d in [
                        IVec3::X,
                        IVec3::NEG_X,
                        IVec3::Y,
                        IVec3::NEG_Y,
                        IVec3::Z,
                        IVec3::NEG_Z,
                    ] {
                        let n = q + d;
                        match queries.get(self, n) {
                            Block::Log => {
                                attached = true;
                                break;
                            }
                            Block::Leaves if seen.insert(n) => todo.push_back(n),
                            _ => {}
                        }
                    }
                    if attached || seen.len() >= 4096 {
                        break;
                    }
                }
                // Large unresolved components remain alive and retry later.
                if attached || !todo.is_empty() {
                    supported.extend(seen);
                    self.physics_pending.insert(key);
                } else if hash(self.seed ^ pulse as u64, p.x, p.y, p.z).is_multiple_of(16) {
                    changes.push((p, Block::Air));
                } else {
                    self.physics_pending.insert(key);
                }
            }
        }
        drop(queries);
        for (p, b) in changes {
            self.set(p, b);
        }
    }
}

#[cfg(test)]
mod vegetation_physics_tests {
    use super::*;

    #[test]
    fn sediment_falls_and_keeps_its_material() {
        for block in [Block::Sand, Block::Gravel] {
            let mut w = World::new(41);
            let p = IVec3::new(0, 200, 0);
            w.set(p - IVec3::Y * 3, Block::Stone);
            w.set(p - IVec3::Y, Block::Water);
            w.set(p, block);
            for tick in 0..30 {
                w.tick_block_physics(p, tick);
            }
            assert_eq!(w.get(p), Block::Air);
            assert_eq!(w.get(p - IVec3::Y * 2), block);
        }
    }

    #[test]
    fn leaves_follow_support_chains_and_decay_after_wood_is_removed() {
        let mut w = World::new(41);
        let p = IVec3::new(15, 200, -16);
        w.set(p, Block::Log);
        for x in 1..=8 {
            w.set(p + IVec3::X * x, Block::Leaves);
        }
        for tick in 0..100 {
            w.tick_block_physics(p, tick);
        }
        for x in 1..=8 {
            assert_eq!(w.get(p + IVec3::X * x), Block::Leaves);
        }
        w.set(p, Block::Air);
        w.tick_block_physics(p, 100);
        assert!((1..=8).any(|x| w.get(p + IVec3::X * x) == Block::Leaves));
        for tick in 101..600 {
            w.tick_block_physics(p, tick);
        }
        for x in 1..=8 {
            assert_eq!(w.get(p + IVec3::X * x), Block::Air);
        }
    }

    #[test]
    fn trees_have_varied_heights_and_roots_across_biomes() {
        let w = World::new(41);
        let mut roots = HashSet::new();
        let mut heights = HashSet::new();
        for x in (-400..400).step_by(3) {
            for z in (-400..400).step_by(3) {
                if let Some((tx, ty, tz)) = w.tree(x, z) {
                    if roots.insert((tx, tz)) {
                        assert_ne!(w.biome(tx, tz), Biome::Tundra);
                        assert_eq!(w.get(IVec3::new(tx, ty + 1, tz)), Block::Log);
                        heights.insert(5 + hash(w.seed ^ 0x74726565, tx, 0, tz) % 5);
                    }
                }
            }
        }
        assert!(roots.len() > 100);
        assert_eq!(heights.len(), 5);
    }
}
