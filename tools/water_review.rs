//! Reproducible natural-waterfall review, isolated from normal saves/settings.
use crate::{
    Options,
    game::Game,
    world::{Block, World},
};
use glam::{IVec3, Vec3};
use std::path::PathBuf;

pub fn find_spill(w: &World) -> Option<(IVec3, IVec3)> {
    for cx in -8..8 {
        for cz in -8..8 {
            let plan = match w.waterway_plan(cx * 384 + 192, cz * 384 + 192) {
                Some(plan) => plan,
                None => continue,
            };
            for p in &plan.route {
                let Some(c) = plan.column(p.x, p.z) else {
                    continue;
                };
                let source = IVec3::new(p.x, c.level, p.z);
                if w.get(source) != Block::Water {
                    continue;
                }
                for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                    let q = source + d;
                    if w.get(q).fluid().is_some() && w.get(q - IVec3::Y) == Block::FlowingWater(8) {
                        return Some((source, q));
                    }
                }
            }
        }
    }
    None
}

pub fn run(mut options: Options) -> Result<(), String> {
    let mut game = Game::new(options.seed);
    let (source, spill) =
        find_spill(&game.world).ok_or("No natural spillway found for this seed")?;
    let plan = game.world.waterway_plan(source.x, source.z).unwrap();
    println!(
        "Drainage: {:?} -> {:?} ({} channel voxels)",
        plan.lakes[0].center,
        plan.lakes[1].center,
        plan.route.len()
    );
    // Activate through normal streaming, then run only the fluid simulation.
    let start = std::time::Instant::now();
    loop {
        game.world.stream(source.as_vec3(), 1);
        game.world.meshes.clear();
        let [_, queued, active] = game.world.stream_counts();
        if queued + active == 0 {
            break;
        }
        if start.elapsed().as_secs() > 30 {
            return Err("Water review streaming timed out".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    for pulse in 0..90 {
        game.world.tick_fluids(source, pulse % 3 == 0);
    }
    if game.world.get(spill - IVec3::Y) != Block::FlowingWater(8) {
        return Err("Natural spillway did not produce a falling water column".into());
    }
    let outward = (spill - source).as_vec3();
    let side = Vec3::new(-outward.z, 0., outward.x);
    let target = spill.as_vec3() - Vec3::Y * 2.;
    let mut camera = target + outward * 13. + side * 13. + Vec3::Y * 9.;
    camera.y = camera
        .y
        .max(game.world.height(camera.x as i32, camera.z as i32) as f32 + 3.);
    game.player.pos = camera.to_array();
    let direction = (target - game.player.eye()).normalize();
    game.player.yaw = direction.x.atan2(-direction.z);
    game.player.pitch = direction.y.asin();
    game.player.spawn = game.player.pos;
    game.player.inventory.selected = 8;
    game.message_time = 0.;
    options.save = PathBuf::from("target/water-review/world.json");
    options.settings_path = PathBuf::from("target/water-review/settings.json");
    options.capture = Some(PathBuf::from("target/water-review/waterfall.ppm"));
    options.frames = Some(1);
    options.title = false;
    options.pack = false;
    options.tour = false;
    options.profile = None;
    std::fs::create_dir_all("target/water-review").map_err(|e| e.to_string())?;
    // A plan-view voxel diagnostic shows both lakes and the complete wet route.
    use std::io::Write;
    let base_x = source.x.div_euclid(384) * 384;
    let base_z = source.z.div_euclid(384) * 384;
    let mut map =
        std::fs::File::create("target/water-review/watercourse.ppm").map_err(|e| e.to_string())?;
    map.write_all(b"P6\n384 384\n255\n")
        .map_err(|e| e.to_string())?;
    let mut pixels = Vec::with_capacity(384 * 384 * 3);
    for z in base_z..base_z + 384 {
        for x in base_x..base_x + 384 {
            let height = game.world.height(x, z);
            let shade = (height - crate::world::SEA_LEVEL).clamp(0, 100) as u8;
            let mut color = if height < crate::world::SEA_LEVEL {
                [35, 100, 165]
            } else {
                [85 + shade, 125 + shade / 2, 65 + shade / 2]
            };
            if let Some(c) = plan.column(x, z) {
                if c.edge < 0. && game.world.get(IVec3::new(x, c.level, z)).fluid().is_some() {
                    color = if c.spill_top > c.level {
                        [90, 220, 240]
                    } else {
                        [35, 135, 215]
                    };
                } else if c.edge < 2. {
                    color = [190, 177, 124];
                }
            }
            pixels.extend_from_slice(&color);
        }
    }
    map.write_all(&pixels).map_err(|e| e.to_string())?;
    println!(
        "Natural waterfall: seed={}, source={source:?}, spill={spill:?}",
        options.seed
    );
    crate::headless::run(game, &options)
}
