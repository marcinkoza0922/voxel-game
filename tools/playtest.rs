//! Headless mechanics course. Fixtures arrange conditions; fixed-step game input
//! drives behavior, and each checkpoint fails with its gameplay contract.
use crate::{
    game::{Game, Input, STEP, collides},
    world::Block,
};
use glam::{IVec3, Vec3};

fn check(ok: bool, checkpoint: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("Mechanics playtest: {checkpoint}"))
    }
}
fn advance(g: &mut Game, input: Input, frames: usize) {
    for _ in 0..frames {
        g.tick(&input, STEP);
    }
}
fn reset_position(g: &mut Game) {
    g.player.pos = [0.5, 300.01, 0.5];
    g.velocity = Vec3::ZERO;
    g.player.yaw = 0.;
    g.player.pitch = 0.;
}

pub fn run() -> Result<(), String> {
    let mut g = Game::new(314159);
    // Explicitly clear headroom; generation changes cannot obstruct this course.
    for x in -8..=8 {
        for z in -8..=8 {
            for y in 299..=322 {
                g.world.set(
                    IVec3::new(x, y, z),
                    if y == 299 { Block::Stone } else { Block::Air },
                );
            }
        }
    }
    reset_position(&mut g);
    g.player.spawn = g.player.pos;
    advance(&mut g, Input::default(), 60);
    check(g.grounded, "gravity must land the player on the course")?;
    let hunger = g.player.hunger;
    let rest = g.player.rest;
    advance(
        &mut g,
        Input {
            forward: 1.,
            ..Default::default()
        },
        30,
    );
    let walked = 0.5 - g.player.pos[2];
    check(
        (2.1..2.4).contains(&walked),
        "walking must travel at the expected speed",
    )?;
    check(
        g.player.hunger < hunger && g.player.rest < rest,
        "simulation must drain hunger and rest",
    )?;
    reset_position(&mut g);
    advance(
        &mut g,
        Input {
            forward: 1.,
            sprint: true,
            ..Default::default()
        },
        30,
    );
    let sprinted = 0.5 - g.player.pos[2];
    check(
        sprinted > walked * 1.4 && sprinted < walked * 1.5,
        "fed sprint must increase movement speed",
    )?;
    reset_position(&mut g);
    g.player.hunger = 5.;
    advance(
        &mut g,
        Input {
            forward: 1.,
            sprint: true,
            ..Default::default()
        },
        30,
    );
    check(
        (0.5 - g.player.pos[2] - walked).abs() < 0.02,
        "low hunger must disable sprint bonus",
    )?;
    g.player.hunger = 50.;
    reset_position(&mut g);
    advance(&mut g, Input::default(), 2);
    advance(
        &mut g,
        Input {
            jump: true,
            ..Default::default()
        },
        1,
    );
    check(
        g.velocity.y > 0. && !g.grounded,
        "grounded jump must lift the player",
    )?;
    advance(&mut g, Input::default(), 90);
    check(
        g.grounded && (g.player.pos[1] - 300.).abs() < 0.1,
        "jump must land back on the floor",
    )?;
    for x in -8..=8 {
        for y in 300..=303 {
            g.world.set(IVec3::new(x, y, -2), Block::Stone);
        }
    }
    advance(
        &mut g,
        Input {
            forward: 1.,
            sprint: true,
            ..Default::default()
        },
        120,
    );
    check(
        g.player.pos[2] >= -0.71 && !collides(&g.world, Vec3::from_array(g.player.pos)),
        "walking must stop outside a solid wall",
    )?;
    reset_position(&mut g);
    g.player.pos[1] = 310.;
    g.player.health = 100.;
    advance(&mut g, Input::default(), 90);
    check(
        g.grounded && g.player.health < 90.,
        "a high fall must damage the player",
    )?;
    g.player.hunger = 0.;
    let health = g.player.health;
    advance(&mut g, Input::default(), 60);
    check(
        g.player.health < health - 1.,
        "starvation must damage the player",
    )?;
    let inventory = g.player.inventory.clone();
    g.player.health = 0.;
    advance(&mut g, Input::default(), 1);
    check(
        g.player.pos == g.player.spawn
            && g.player.health == g.player.max_health()
            && g.player.hunger == 75.
            && g.player.rest == 75.
            && g.player.inventory == inventory,
        "death must respawn with restored vitals and retained inventory",
    )?;
    let torch = IVec3::new(2, 300, 0);
    g.world.set(torch, Block::Torch);
    g.torch_life.insert(torch.to_array(), STEP * 2.5);
    advance(&mut g, Input::default(), 2);
    check(
        g.world.get(torch) == Block::Torch,
        "torch must remain until its timer expires",
    )?;
    advance(&mut g, Input::default(), 1);
    check(
        g.world.get(torch) == Block::Air && !g.torch_life.contains_key(&torch.to_array()),
        "expired torch must remove its block and timer",
    )?;
    println!(
        "Mechanics playtest passed: walking, sprint/hunger gate, jump/landing, wall collision, fall damage, survival drain, starvation, respawn, torch expiry."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn mechanics_course() {
        super::run().unwrap();
    }

    #[test]
    fn progression_expedition() {
        crate::expedition::run().unwrap();
    }
}
