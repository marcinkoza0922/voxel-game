//! Deterministic 32px material tiles. Face order: side, top, underside.
use crate::world::Block;

pub const TILE_SIZE: u32 = 32;

pub const BLOCKS: [Block; 21] = [
    Block::Air,
    Block::Grass,
    Block::Dirt,
    Block::Stone,
    Block::Log,
    Block::Leaves,
    Block::Copper,
    Block::Coal,
    Block::Planks,
    Block::Cobble,
    Block::Furnace,
    Block::Table,
    Block::Torch,
    Block::Sand,
    Block::Bed,
    Block::Iron,
    Block::Smelter,
    Block::Snow,
    Block::RuinStone,
    Block::Reliquary,
    Block::Obsidian,
];
pub fn layer(block: Block, face: usize) -> u32 {
    (BLOCKS.iter().position(|b| *b == block).unwrap_or(0) * 3 + face) as u32
}
fn noise(x: i32, y: i32, salt: u32) -> u32 {
    let mut n = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ salt.wrapping_mul(2246822519);
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    n ^ (n >> 16)
}
fn tint(c: [f32; 3], shade: f32) -> [f32; 3] {
    c.map(|v| (v * shade).clamp(0., 1.))
}

// Overlapping narrow, bent blades at native texel resolution, wrapping at tile edges.
fn grass_texel(x: i32, y: i32) -> [f32; 3] {
    let mut color = tint(
        [0.26, 0.41, 0.12],
        0.94 + (noise(x, y, 71) % 13) as f32 * 0.01,
    );
    for blade in 0..90 {
        let h = noise(blade, 0, 83);
        let root_x = (h % 32) as i32;
        let root_y = ((h >> 8) % 32) as i32;
        let length = 3 + ((h >> 16) % 7) as i32;
        let along = (root_y - y).rem_euclid(32);
        let bend = ((h >> 24) % 5) as i32 - 2;
        let spine = root_x + bend * along / length;
        let across = (x - spine + 16).rem_euclid(32) - 16;
        if along < length && (across == 0 || (across == 1 && along < length / 3)) {
            let tip = along as f32 / length as f32;
            let shade = 0.80 + ((h >> 12) % 35) as f32 * 0.01 + tip * 0.23;
            color = tint([0.36, 0.53, 0.16], shade);
        }
    }
    color
}

pub fn texel(block: Block, face: usize, x: i32, y: i32) -> [f32; 3] {
    if block == Block::Grass
        && (face == 1 || (face == 0 && y < 5 + (noise(x / 2, 0, 3) % 7) as i32))
    {
        return grass_texel(x, y);
    }

    use Block::*;
    let fine = noise(x, y, 41);
    // Broad motifs occupy two-pixel clusters; full-resolution grain breaks their edges.
    let (x, y) = (x / 2, y / 2);
    let n = noise(x, y, 19);
    let coarse = noise(x / 3, y / 3, 7);
    let grain = 0.90 + (n % 9) as f32 * 0.025;
    let stone = || {
        let vein = (x + y / 3 + (y / 5) * 2).rem_euclid(13);
        tint(
            Stone.color(),
            if vein == 0 {
                0.70
            } else if n % 19 < 3 {
                1.22
            } else {
                grain
            },
        )
    };
    let c = match block {
        Shrub | TallGrass | RedFlower | YellowFlower | PurpleFlower | WhiteFlower | PinkFlower
        | BlueFlower => block.color(),
        Air => [0.; 3],
        Grass | Dirt => tint(
            [0.43, 0.28, 0.17],
            if n % 23 < 2 {
                1.40
            } else if coarse.is_multiple_of(5) {
                0.76
            } else {
                grain
            },
        ),
        Stone => stone(),
        Copper | Iron | Coal => {
            // Uneven clusters embedded in granite, with a dark rim and bright core.
            let cx = x % 8 - 3;
            let cy = (y + (x / 8) * 3) % 8 - 3;
            let d = cx * cx + cy * cy;
            if d < 8 && !coarse.is_multiple_of(7) {
                let ore = match block {
                    Copper => [0.86, 0.46, 0.23],
                    Iron => [0.68, 0.46, 0.34],
                    _ => [0.12, 0.14, 0.18],
                };
                tint(ore, if cx + cy < -1 { 1.28 } else { grain })
            } else if d < 13 {
                tint(stone(), 0.73)
            } else {
                stone()
            }
        }
        Log if face != 0 => {
            let r = (x - 7).abs().max((y - 8).abs());
            tint(
                [0.67, 0.47, 0.27],
                if r == 7 {
                    0.49
                } else if r % 3 == 0 {
                    0.73
                } else {
                    grain
                },
            )
        }
        Log => {
            let ridge = (x + (y / 6) % 2) % 5;
            tint(
                [0.40, 0.27, 0.15],
                if ridge == 0 {
                    0.61
                } else if ridge == 1 {
                    1.21
                } else {
                    grain
                },
            )
        }
        Leaves => {
            let leaf = (x + y * 2) % 5;
            tint(
                [0.23, 0.43, 0.18],
                if leaf == 0 {
                    1.28
                } else if coarse.is_multiple_of(4) {
                    0.66
                } else {
                    grain
                },
            )
        }
        Planks | Table => {
            let seam = y % 4 == 0 || (x + (y / 4 % 2) * 8) % 16 == 0;
            if block == Table && face == 1 && (x == 5 || x == 10 || y == 5 || y == 10) {
                [0.25, 0.15, 0.08]
            } else if block == Table && face == 0 && (!(2..=13).contains(&x) || y > 12) {
                [0.30, 0.19, 0.10]
            } else {
                tint(
                    [0.67, 0.47, 0.25],
                    if seam {
                        0.55
                    } else if y % 4 == 1 {
                        1.16
                    } else {
                        grain
                    },
                )
            }
        }
        Cobble | RuinStone | Furnace | Smelter => {
            let row = y / 5;
            let joint = y % 5 == 0 || (x + row % 2 * 4) % 8 == 0;
            let base = if block == RuinStone {
                [0.40, 0.48, 0.49]
            } else {
                block.color()
            };
            if matches!(block, Furnace | Smelter)
                && face == 0
                && (4..12).contains(&x)
                && (6..13).contains(&y)
            {
                if y > 10 && x > 5 && x < 10 {
                    tint([0.94, 0.34, 0.08], grain)
                } else {
                    [0.09, 0.10, 0.12]
                }
            } else if block == RuinStone
                && ((x == 7 && y > 4 && y < 12) || (y == 5 && x > 5 && x < 11))
            {
                [0.25, 0.61, 0.65]
            } else if block == RuinStone && coarse.is_multiple_of(7) {
                tint([0.26, 0.37, 0.25], grain)
            } else {
                tint(
                    base,
                    if joint {
                        0.58
                    } else if y % 5 == 1 {
                        1.20
                    } else {
                        grain
                    },
                )
            }
        }
        Sand => tint(
            [0.80, 0.72, 0.49],
            if (y + x / 5) % 6 == 0 {
                0.86
            } else {
                grain * 0.98
            },
        ),
        Snow => tint(
            [0.84, 0.91, 0.96],
            if coarse.is_multiple_of(7) {
                0.91
            } else if n.is_multiple_of(31) {
                1.08
            } else {
                0.99
            },
        ),
        Bed => {
            if y < 4 && face == 1 {
                tint([0.88, 0.82, 0.64], grain)
            } else if face == 0 && y > 11 {
                tint(Log.color(), grain)
            } else {
                tint([0.65, 0.25, 0.20], if x % 5 == 0 { 0.80 } else { grain })
            }
        }
        Reliquary => {
            if !(2..=13).contains(&x) || !(2..=13).contains(&y) {
                tint([0.31, 0.40, 0.48], grain)
            } else if (x - 8).abs() + (y - 8).abs() < 4 {
                [0.71, 0.93, 0.97]
            } else {
                tint([0.23, 0.51, 0.65], grain)
            }
        }
        Water | FlowingWater(_) | Lava | FlowingLava(_) => block.color(),
        Obsidian => tint(
            block.color(),
            if (x + y / 3).rem_euclid(11) == 0 {
                1.8
            } else {
                grain
            },
        ),
        Torch => {
            // Charred end grain and twisted linen on the head; long split wood grain below.
            if face == 1 {
                tint([0.22, 0.13, 0.065], grain)
            } else if y < 5 {
                tint(
                    [0.55, 0.37, 0.18],
                    if (x + y * 2) % 5 == 0 { 0.55 } else { grain },
                )
            } else {
                tint(
                    [0.38, 0.22, 0.10],
                    if (x + (y / 7)) % 4 == 0 { 0.65 } else { grain },
                )
            }
        }
    };
    tint(c, 0.97 + (fine % 7) as f32 * 0.01)
}
pub fn pixels() -> Vec<u8> {
    let mut out =
        Vec::with_capacity(BLOCKS.len() * 3 * TILE_SIZE as usize * TILE_SIZE as usize * 4);
    for b in BLOCKS {
        for face in 0..3 {
            for y in 0..TILE_SIZE as i32 {
                for x in 0..TILE_SIZE as i32 {
                    out.extend(texel(b, face, x, y).map(|v| (v * 255.).round() as u8));
                    out.push(255);
                }
            }
        }
    }
    out
}

/// Box-filtered linear-color mip levels, retaining crisp nearest texels nearby.
pub fn mipmaps() -> Vec<Vec<u8>> {
    let mut levels = vec![pixels()];
    let mut size = TILE_SIZE as usize;
    while size > 1 {
        let next = size / 2;
        let src = levels.last().unwrap();
        let mut dst = vec![0; BLOCKS.len() * 3 * next * next * 4];
        for layer in 0..BLOCKS.len() * 3 {
            for y in 0..next {
                for x in 0..next {
                    for c in 0..4 {
                        let mut sum = 0u32;
                        for dy in 0..2 {
                            for dx in 0..2 {
                                sum += src
                                    [((layer * size + y * 2 + dy) * size + x * 2 + dx) * 4 + c]
                                    as u32;
                            }
                        }
                        dst[((layer * next + y) * next + x) * 4 + c] = (sum / 4) as u8;
                    }
                }
            }
        }
        levels.push(dst);
        size = next;
    }
    levels
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_material_mips_and_distinct_faces() {
        let levels = mipmaps();
        assert_eq!(levels.len(), 6);
        for (level, bytes) in levels.iter().enumerate() {
            let size = TILE_SIZE as usize >> level;
            assert_eq!(bytes.len(), BLOCKS.len() * 3 * size * size * 4);
            assert!(bytes.chunks_exact(4).all(|p| p[3] == 255));
        }
        for block in [Block::Grass, Block::Log, Block::Table] {
            assert!((0..32).any(|x| texel(block, 0, x, 8) != texel(block, 1, x, 8)));
        }
        assert_ne!(layer(Block::Copper, 0), layer(Block::Iron, 0));
    }
}
