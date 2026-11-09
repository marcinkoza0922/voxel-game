//! Deterministic integration expedition: controlled resource arena, real game actions.
use crate::{
    ecs::Mob,
    game::{Game, Input, STEP},
    items::{Item, RECIPES},
    save,
    world::{Biome, Block},
};
use glam::{IVec3, Vec3};

fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn aim(g: &mut Game, point: Vec3) {
    let d = (point - g.player.eye()).normalize();
    g.player.yaw = d.x.atan2(-d.z);
    g.player.pitch = d.y.asin();
}
fn equip(g: &mut Game, item: Item) {
    g.player.inventory.hotbar[0] = item;
    g.player.inventory.selected = 0;
}
fn mine(g: &mut Game, block: Block, tool: Item, count: usize) -> Result<(), String> {
    equip(g, tool);
    for _ in 0..count {
        let p = IVec3::new(0, 101, -2);
        g.world.set(p, block);
        aim(g, p.as_vec3() + Vec3::splat(0.5));
        for _ in 0..180 {
            g.tick(
                &Input {
                    attack: true,
                    ..Default::default()
                },
                STEP,
            );
            if g.world.get(p) == Block::Air {
                break;
            }
        }
        require(
            g.world.get(p) == Block::Air,
            "Could not mine fixture resource",
        )?;
    }
    Ok(())
}
fn craft(g: &mut Game, output: Item, count: usize) -> Result<(), String> {
    let index = RECIPES
        .iter()
        .position(|r| r.output == output)
        .ok_or("Missing recipe")?;
    for _ in 0..count {
        let before = g.player.inventory.count(output);
        g.craft(index);
        require(
            g.player.inventory.count(output) > before,
            &format!("Craft failed: {}: {}", output.name(), g.message),
        )?;
    }
    Ok(())
}
fn place(g: &mut Game, item: Item, p: IVec3) -> Result<(), String> {
    equip(g, item);
    // Aim at the top of the supporting block, exercising reach and placement checks.
    aim(g, p.as_vec3() + Vec3::new(0.5, -0.01, 0.5));
    g.use_selected();
    require(
        Some(g.world.get(p)) == item.block(),
        "Station placement failed",
    )
}
pub fn run() -> Result<(), String> {
    let mut g = Game::new(314159);
    // Controlled arena isolates progression from random travel/resource search.
    g.player.pos = [0.5, 100.01, 0.5];
    for x in -5..=5 {
        for z in -5..=5 {
            g.world.set(IVec3::new(x, 99, z), Block::Stone);
        }
    }
    for _ in 0..120 {
        g.tick(&Input::default(), STEP);
    }
    require(g.grounded, "Landing failed")?;
    mine(&mut g, Block::Log, Item::WoodAxe, 12)?;
    craft(&mut g, Item::Planks, 10)?;
    craft(&mut g, Item::Stick, 4)?;
    craft(&mut g, Item::Table, 1)?;
    place(&mut g, Item::Table, IVec3::new(2, 100, 0))?;
    mine(&mut g, Block::Stone, Item::WoodPick, 14)?;
    craft(&mut g, Item::StonePick, 1)?;
    craft(&mut g, Item::StoneAxe, 1)?;
    craft(&mut g, Item::Furnace, 1)?;
    place(&mut g, Item::Furnace, IVec3::new(-2, 100, 0))?;
    mine(&mut g, Block::Copper, Item::StonePick, 8)?;
    mine(&mut g, Block::Coal, Item::StonePick, 10)?;
    craft(&mut g, Item::CopperIngot, 8)?;
    for item in [Item::CopperPick, Item::CopperAxe, Item::Sword, Item::Bed] {
        craft(&mut g, item, 1)?;
    }
    place(&mut g, Item::Bed, IVec3::new(0, 100, 2))?;
    equip(&mut g, Item::Sword);
    let mob = Vec3::new(0.5, 100.01, -1.5);
    g.entities.spawn(Mob {
        pos: mob,
        health: 30.,
        cooldown: 0.,
        level: 1,
    });
    g.player.rest = 20.;
    g.sleep();
    require(g.player.rest == 20., "Nearby enemy did not block sleep")?;
    let before = g.player.inventory.count(Item::Meat);
    for _ in 0..240 {
        let point = g
            .entities
            .iter()
            .find(|(_, m)| m.pos.y > 90.)
            .map(|(_, m)| m.pos + Vec3::Y);
        if let Some(point) = point {
            aim(&mut g, point);
        }
        g.tick(
            &Input {
                attack: true,
                ..Default::default()
            },
            STEP,
        );
        if g.player.inventory.count(Item::Meat) > before {
            break;
        }
    }
    require(
        g.player.inventory.count(Item::Meat) > before,
        "Combat did not produce loot",
    )?;
    craft(&mut g, Item::CookedMeat, 1)?;
    g.player.hunger = 30.;
    equip(&mut g, Item::CookedMeat);
    g.use_selected();
    require(g.player.hunger == 70., "Eating did not restore hunger")?;
    g.sleep();
    require(g.player.rest == 100., "Sleep failed after combat")?;
    craft(&mut g, Item::Torch, 1)?;
    place(&mut g, Item::Torch, IVec3::new(2, 100, 2))?;
    // Sample natural forest and cave locations; relocation is diagnostic, not walked travel.
    let forest = (-300..=300)
        .step_by(10)
        .find_map(|x| {
            (-300..=300)
                .step_by(10)
                .find(|&z| g.world.biome(x, z) == Biome::Forest)
                .map(|z| (x, z))
        })
        .ok_or("No forest found")?;
    g.player.pos = [
        forest.0 as f32 + 0.5,
        g.world.height(forest.0, forest.1) as f32 + 8.,
        forest.1 as f32 + 0.5,
    ];
    g.tick(&Input::default(), STEP);
    require(
        g.player.discoveries.contains("biome:Oak forest"),
        "Forest discovery failed",
    )?;
    let cave = (-100..=100)
        .step_by(4)
        .find_map(|x| {
            (-100..=100).step_by(4).find_map(|z| {
                (0..g.world.height(x, z) - 5)
                    .rev()
                    .find(|&y| {
                        let p = Vec3::new(x as f32 + 0.5, y as f32 + 0.01, z as f32 + 0.5);
                        !crate::game::collides(&g.world, p)
                            && crate::game::collides(&g.world, p - Vec3::Y * 0.1)
                    })
                    .map(|y| [x as f32 + 0.5, y as f32 + 0.01, z as f32 + 0.5])
            })
        })
        .ok_or("No cave found")?;
    g.player.pos = cave;
    g.velocity = Vec3::ZERO;
    g.tick(&Input::default(), STEP);
    require(
        !crate::game::collides(&g.world, Vec3::from_array(g.player.pos)),
        "Cave collision failed",
    )?;
    let path = std::env::temp_dir().join(format!("voxel-expedition-{}.json", std::process::id()));
    let result = (|| {
        save::write(&g, &path)?;
        let loaded = save::read(&path)?.ok_or("Missing expedition save")?;
        require(
            serde_json::to_value(&loaded.player).unwrap()
                == serde_json::to_value(&g.player).unwrap()
                && loaded.world.edits == g.world.edits
                && loaded.torch_life == g.torch_life
                && loaded.time == g.time,
            "Expedition reload mismatch",
        )
    })();
    let _ = std::fs::remove_file(path);
    result?;
    println!(
        "Expedition passed: mined wood/stone/copper/coal, placed stations, crafted copper gear, combat/loot, food, sleep, torch, forest discovery, cave collision, full player/edit/torch reload. Resource arena and forest/cave relocations are controlled fixtures."
    );
    Ok(())
}
