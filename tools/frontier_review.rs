//! Offscreen mining-update timings against a read-only save fixture.
use crate::{Options, render::Renderer, save, world::Block};
use glam::{IVec3, Vec3};
use std::{
    io::Write,
    time::{Duration, Instant},
};

pub fn run(options: &Options) -> Result<(), String> {
    let mut game = save::read(&options.save)?.ok_or("Mining review needs an existing --save")?;
    let mut renderer = pollster::block_on(Renderer::headless(1280, 800))?;
    let result = (|| {
        let pos = Vec3::from_array(game.player.pos);
        let settle =
            |renderer: &mut Renderer, world: &mut crate::world::World| -> Result<(), String> {
                let start = Instant::now();
                loop {
                    renderer.update_world(world, pos);
                    let [_, queued, active] = world.stream_counts();
                    if queued + active == 0 {
                        return Ok(());
                    }
                    if start.elapsed() > Duration::from_secs(120) {
                        return Err("Mining review streaming timed out".into());
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            };
        settle(&mut renderer, &mut game.world)?;
        let center = pos.floor().as_ivec3();
        let mut samples = Vec::new();
        for i in 0..12 {
            let p = center + IVec3::new(i % 3 - 1, -1, i / 3 - 1);
            game.world.set(p, Block::Stone);
            settle(&mut renderer, &mut game.world)?;
            // Exclude background remesh settling; measure the immediate update
            // after removing one block, which runs on the interactive frame.
            game.world.set(p, Block::Air);
            let start = Instant::now();
            renderer.update_world(&mut game.world, pos);
            samples.push(start.elapsed().as_secs_f64() * 1000.);
            settle(&mut renderer, &mut game.world)?;
        }
        if let Some(path) = &options.profile {
            let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            writeln!(file, "edit,update_ms").map_err(|e| e.to_string())?;
            for (i, ms) in samples.iter().enumerate() {
                writeln!(file, "{i},{ms:.6}").map_err(|e| e.to_string())?;
            }
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "Mining update: median={:.3} max={:.3} ms (12 edits; save unchanged)",
            samples[6], samples[11]
        );
        Ok(())
    })();
    renderer.finish();
    result
}
