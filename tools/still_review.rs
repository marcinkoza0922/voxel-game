//! Frozen camera/simulation captures for temporal rendering diagnostics; saves are read only.
use crate::{Options, render::Renderer, save};
use std::time::{Duration, Instant};

pub fn run(options: &Options) -> Result<(), String> {
    let mut game = save::read(&options.save)?.ok_or("Still review needs an existing --save")?;
    let mut renderer = pollster::block_on(Renderer::headless(1280, 800))?;
    let result = (|| {
        renderer.apply_settings(&crate::settings::Settings::read(&options.settings_path)?);
        let start = Instant::now();
        loop {
            renderer.update_world(&mut game.world, game.player.pos.into());
            let [_, queued, active] = game.world.stream_counts();
            if queued + active == 0 {
                break;
            }
            if start.elapsed() > Duration::from_secs(120) {
                return Err("Still review streaming timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let directory = options
            .capture
            .clone()
            .unwrap_or_else(|| "target/still-review".into());
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let frames = options.frames.unwrap_or(64).max(16);
        for frame in 0..frames {
            let path = directory.join(format!("{frame:03}.ppm"));
            renderer.draw(
                &game,
                false,
                false,
                60.,
                (frame >= frames - 16).then_some(path.as_path()),
            )?;
        }
        println!("Frozen captures: {} (save unchanged)", directory.display());
        Ok(())
    })();
    renderer.finish();
    result
}
