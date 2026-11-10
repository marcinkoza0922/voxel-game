//! Opt-in movement comparisons; timing observations, never a CI performance gate.
use crate::game::{Game, Input, STEP};
use glam::Vec3;
use std::{
    io::Write,
    time::{Duration, Instant},
};

#[test]
#[ignore = "offscreen GPU comparison; writes target/movement-render-* profiles"]
fn movement_render_review() {
    use crate::{performance::Profile, render::Renderer};
    for mode in ["idle", "walk", "jump", "walk_jump"] {
        let mut game = Game::new(314159);
        let mut renderer = pollster::block_on(Renderer::headless(1280, 800)).unwrap();
        let result = (|| -> Result<(), String> {
            renderer.apply_settings(&crate::settings::Settings::default())?;
            let start = Instant::now();
            loop {
                renderer.update_world(&mut game.world, Vec3::from_array(game.player.pos));
                let [_, queued, active] = game.world.stream_counts();
                if queued + active == 0 {
                    break;
                }
                if start.elapsed() > Duration::from_secs(120) {
                    return Err("Movement review streaming timed out".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let path = format!("target/movement-render-{mode}.csv");
            let mut profile = Profile::new(std::path::Path::new(&path))?;
            let mut phases = std::io::BufWriter::new(
                std::fs::File::create(format!("target/movement-render-{mode}.stream.csv"))
                    .map_err(|e| e.to_string())?,
            );
            writeln!(phases, "frame,roofs_ms,world_ms,uploads_ms,frontiers_ms")
                .map_err(|e| e.to_string())?;
            renderer.enable_gpu_profile(profile.gpu_sender());
            let mut previous = Instant::now();
            for frame in 0..360 {
                let start = Instant::now();
                let interval = start.duration_since(previous).as_secs_f64() * 1000.;
                previous = start;
                let input = Input {
                    forward: if frame >= 60 && mode.contains("walk") {
                        if (frame / 60) % 2 == 0 { -1. } else { 1. }
                    } else {
                        0.
                    },
                    jump: frame >= 60 && mode.contains("jump"),
                    ..Default::default()
                };
                game.tick(&input, STEP);
                game.feedback.sounds.clear();
                let simulation = start.elapsed().as_secs_f64() * 1000.;
                let stream = Instant::now();
                renderer.update_world(&mut game.world, Vec3::from_array(game.player.pos));
                let streaming = stream.elapsed().as_secs_f64() * 1000.;
                let [roofs, world, uploads, frontiers] = renderer.stream_times;
                writeln!(
                    phases,
                    "{frame},{roofs:.6},{world:.6},{uploads:.6},{frontiers:.6}"
                )
                .map_err(|e| e.to_string())?;
                renderer.profile_draw = true;
                renderer.profile_frame = frame;
                let draw = Instant::now();
                renderer.draw(&game, false, false, 60., None)?;
                profile.record(
                    frame,
                    [
                        interval,
                        simulation,
                        streaming,
                        draw.elapsed().as_secs_f64() * 1000.,
                    ],
                    game.world.stream_counts(),
                    true,
                    false,
                    renderer.draw_times,
                )?;
            }
            renderer.drain_gpu_profile()?;
            phases.flush().map_err(|e| e.to_string())?;
            profile.finish()
        })();
        renderer.finish();
        result.unwrap();
    }
}

#[test]
#[ignore = "writes target/movement-review.csv; run alone in release mode"]
fn movement_cpu_review() {
    std::fs::create_dir_all("target").unwrap();
    let mut output =
        std::io::BufWriter::new(std::fs::File::create("target/movement-review.csv").unwrap());
    writeln!(output, "mode,repetition,tick,ms,x,y,z").unwrap();
    for repetition in 0..17 {
        for mode in ["idle", "walk", "jump", "walk_jump"] {
            let mut game = Game::new(314159);
            let start = Instant::now();
            loop {
                game.world.stream(Vec3::from_array(game.player.pos), 1);
                game.world.meshes.clear();
                let [_, queued, active] = game.world.stream_counts();
                if queued + active == 0 {
                    break;
                }
                assert!(start.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(1));
            }
            // Ground the player before measuring, without simulating long enough
            // to spawn enemies. Each mode starts from the same seeded fixture.
            for _ in 0..30 {
                game.tick(&Input::default(), STEP);
            }
            for tick in 0..120 {
                let input = Input {
                    forward: if mode.contains("walk") {
                        if tick < 60 { 1. } else { -1. }
                    } else {
                        0.
                    },
                    jump: mode.contains("jump"),
                    ..Default::default()
                };
                let start = Instant::now();
                game.tick(&input, STEP);
                let ms = start.elapsed().as_secs_f64() * 1000.;
                game.feedback.sounds.clear();
                if repetition > 0 {
                    let [x, y, z] = game.player.pos;
                    writeln!(output, "{mode},{repetition},{tick},{ms:.6},{x},{y},{z}").unwrap();
                }
            }
        }
    }
    output.flush().unwrap();
}
