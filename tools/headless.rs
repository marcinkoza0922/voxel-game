//! Offscreen GPU runner; never initializes winit or audio.
use crate::{
    Options,
    game::{Game, Input, STEP},
    performance,
    render::Renderer,
    save,
};
use std::time::{Duration, Instant};

pub fn run(mut game: Game, options: &Options) -> Result<(), String> {
    let mut renderer = pollster::block_on(Renderer::headless(1280, 800))?;
    let result = (|| {
        let settings = crate::settings::Settings::read(&options.settings_path)?;
        renderer.apply_settings(&settings);
        if options.title {
            renderer.ui.screen = crate::ui::Screen::Title;
        }
        let mut profile = options
            .profile
            .as_deref()
            .map(performance::Profile::new)
            .transpose()?;
        if let Some(profile) = &profile {
            renderer.enable_gpu_profile(profile.gpu_sender());
        }
        let mut last = Instant::now();
        for frame in 0..options.frames.unwrap_or(240) {
            let start = Instant::now();
            let interval = start.duration_since(last).as_secs_f64() * 1000.;
            last = start;
            if options.title {
                // Title previews do not advance or write a world.
            } else if options.pack {
                game.tick_furnace();
            } else {
                game.tick(&Input::default(), STEP);
            }
            if options.tour {
                if frame.is_multiple_of(180) {
                    let stage = (frame / 180) % 10;
                    let (x, z) = match stage {
                        0..=5 => (stage as i32 * 16, 0),
                        6 => (240, 240),
                        7 => (1024, 1024),
                        8 => (1040, 1024),
                        _ => (0, 0),
                    };
                    game.player.pos = [
                        x as f32 + 0.5,
                        game.world.height(x, z) as f32 + 3.,
                        z as f32 + 0.5,
                    ];
                    game.velocity = glam::Vec3::ZERO;
                }
                game.player.yaw += 0.003;
            }
            game.feedback.sounds.clear();
            let simulation_ms = start.elapsed().as_secs_f64() * 1000.;
            let streaming = Instant::now();
            let final_frame = frame + 1 == options.frames.unwrap_or(240);
            // Settle each new view and the final state, independent of GPU speed.
            let settle = frame == 0 || final_frame || (options.tour && frame.is_multiple_of(180));
            loop {
                renderer.update_world(&mut game.world, glam::Vec3::from_array(game.player.pos));
                let [_, queued, active] = game.world.stream_counts();
                if !settle || queued + active == 0 {
                    break;
                }
                if streaming.elapsed() > Duration::from_secs(120) {
                    return Err("Headless terrain streaming timed out after 120 seconds".into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let stream_ms = streaming.elapsed().as_secs_f64() * 1000.;
            renderer.profile_draw = profile.is_some();
            renderer.profile_frame = frame;
            let draw = Instant::now();
            renderer.draw(
                &game,
                options.pack,
                false,
                1. / STEP,
                if final_frame {
                    options.capture.as_deref()
                } else {
                    None
                },
            )?;
            if let Some(profile) = &mut profile {
                profile.record(
                    frame,
                    [
                        interval,
                        simulation_ms,
                        stream_ms,
                        draw.elapsed().as_secs_f64() * 1000.,
                    ],
                    game.world.stream_counts(),
                    true,
                    options.pack,
                    renderer.draw_times,
                )?;
            }
        }
        renderer.drain_gpu_profile()?;
        if let Some(profile) = &mut profile {
            profile.finish()?;
        }
        if !options.title {
            save::write(&game, &options.save)?;
        }
        println!(
            "Headless render completed: {} frames",
            options.frames.unwrap_or(240)
        );
        Ok(())
    })();
    renderer.finish();
    result
}
