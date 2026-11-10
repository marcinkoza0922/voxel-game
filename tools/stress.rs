//! Deterministic CPU workloads; no display, audio, settings or save access.
use crate::{
    render,
    world::{Block, World, chunk},
};
use glam::{IVec3, Vec3};
use std::{
    fs::File,
    hint::black_box,
    io::{BufWriter, Write},
    path::Path,
    time::{Duration, Instant},
};

fn measure(
    samples: &mut Vec<(&'static str, f64, usize)>,
    name: &'static str,
    work: impl FnOnce() -> usize,
) {
    let start = Instant::now();
    let units = black_box(work());
    samples.push((name, start.elapsed().as_secs_f64() * 1000., units));
}

pub fn run(seed: u64, repetitions: u32, output: Option<&Path>) -> Result<(), String> {
    let mut samples = Vec::new();
    // One complete unrecorded warmup, followed by independent measured fixtures.
    for iteration in 0..=repetitions {
        let mut batch = Vec::new();
        let world = World::new(seed);
        let x = (iteration % 4) as i32 * 256 - 256;
        let key = chunk(IVec3::new(x, world.height(x, x), x));
        let mut halo = Vec::new();
        measure(&mut batch, "terrain_halo", || {
            halo = world.halo(key);
            halo.len()
        });
        let mut mesh = Vec::new();
        measure(&mut batch, "surface_mesh", || {
            mesh = render::mesh_halo(seed, key, &halo, &world.torch_mounts);
            mesh.len()
        });
        measure(&mut batch, "surface_sky", || {
            render::apply_sky_access(&world, &mut mesh);
            mesh.len()
        });
        // Checkerboard maximizes isolated faces and stresses meshing/occlusion.
        for y in 0..18 {
            for z in 0..18 {
                for x in 0..18 {
                    halo[x + 18 * (z + 18 * y)] = if (x + y + z) % 2 == 0 {
                        Block::Stone
                    } else {
                        Block::Air
                    };
                }
            }
        }
        measure(&mut batch, "checkerboard_mesh", || {
            black_box(render::mesh_halo(seed, key, &halo, &world.torch_mounts)).len()
        });
        let cave = key - IVec3::Y * 3;
        let cave_halo = world.halo(cave);
        let mut cave_mesh = render::mesh_halo(seed, cave, &cave_halo, &world.torch_mounts);
        measure(&mut batch, "cave_sky", || {
            render::apply_sky_access(&world, &mut cave_mesh);
            cave_mesh.len()
        });
        let mut fluid = World::new(seed);
        let center = IVec3::new(0, 1000, 0);
        // 512 sources with open neighboring cells: a full frontier pulse.
        for x in -8..8 {
            for z in -16..16 {
                fluid.set(center + IVec3::new(x * 2, 0, z * 2), Block::Water);
            }
        }
        measure(&mut batch, "fluid_12_pulses", || {
            for pulse in 0..12 {
                fluid.tick_fluids(center, pulse % 3 == 0);
            }
            fluid.edits.len()
        });
        let mut stream = World::new(seed);
        measure(&mut batch, "stream_relocate", || {
            let start = Instant::now();
            let mut peak = 0;
            for offset in [0, 16, 1024, 0] {
                let pos = Vec3::new(
                    offset as f32,
                    stream.height(offset, offset) as f32 + 3.,
                    offset as f32,
                );
                loop {
                    stream.stream(pos, 1);
                    let [resident, queued, active] = stream.stream_counts();
                    peak = peak.max(resident);
                    if queued + active == 0 {
                        break;
                    }
                    if start.elapsed() > Duration::from_secs(60) {
                        return 0;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            peak
        });
        if batch.last().is_some_and(|s| s.2 == 0) {
            return Err("Stress streaming exceeded 60 seconds".into());
        }
        if iteration > 0 {
            samples.extend(batch);
        }
    }
    if let Some(path) = output {
        let mut writer = BufWriter::new(File::create(path).map_err(|e| e.to_string())?);
        writeln!(writer, "workload,sample,ms,units").map_err(|e| e.to_string())?;
        for (index, (name, ms, units)) in samples.iter().enumerate() {
            writeln!(writer, "{name},{},{ms:.6},{units}", index / 7).map_err(|e| e.to_string())?;
        }
        writer.flush().map_err(|e| e.to_string())?;
    }
    println!(
        "CPU stress: seed={seed}, repetitions={repetitions}, warmup=1, optimized={}",
        !cfg!(debug_assertions)
    );
    for name in [
        "terrain_halo",
        "surface_mesh",
        "surface_sky",
        "checkerboard_mesh",
        "cave_sky",
        "fluid_12_pulses",
        "stream_relocate",
    ] {
        let mut times: Vec<_> = samples
            .iter()
            .filter(|s| s.0 == name)
            .map(|s| s.1)
            .collect();
        times.sort_by(f64::total_cmp);
        let n = times.len();
        println!(
            "{name}: median={:.3} p95={:.3} max={:.3} ms",
            times[(n / 2).min(n - 1)],
            times[(n * 95).div_ceil(100) - 1],
            times[n - 1]
        );
    }
    Ok(())
}
