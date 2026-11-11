//! Deterministic integration expedition: controlled resource arena, real game actions.
use crate::{
    ecs::{Mob, MobKind},
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
    g.player.inventory.equip(item, 0);
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
        let origin = g.player.pos;
        g.player.pos = (p.as_vec3() + Vec3::new(0.5, -1., 0.5)).to_array();
        g.collect_drops();
        g.player.pos = origin;
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
        let r = &RECIPES[index];
        if matches!(
            r.station,
            crate::items::Station::Furnace | crate::items::Station::Smelter
        ) {
            for (n, &(item, count)) in r.inputs.iter().enumerate() {
                if item == Item::Coal
                    && (g.furnace().map_or(0, |f| f.furnace_joules) >= crate::items::SMELT_JOULES
                        || g.furnace().is_some_and(|f| f.furnace[1].is_some()))
                {
                    continue;
                }
                require(
                    g.player.inventory.take(item, count),
                    "Missing furnace fixture ingredient",
                )?;
                g.furnaces
                    .entry(g.furnace_position().ok_or("No furnace")?)
                    .or_default()
                    .furnace[n] = Some(crate::items::Stack::new(item, count));
            }
            for _ in 0..crate::items::SMELT_TICKS {
                g.tick_furnace();
            }
            g.craft_grid(2);
        } else {
            let grid = crate::items::recipe_grid(index);
            for (n, s) in grid.into_iter().enumerate() {
                if let Some(s) = s {
                    require(
                        g.player.inventory.take(s.item, 1),
                        "Missing crafting fixture ingredient",
                    )?;
                    if r.station == crate::items::Station::Hand {
                        g.player.inventory.hand[n] = Some(s);
                    } else {
                        g.player.inventory.grid[n] = Some(s);
                    }
                }
            }
            g.craft_grid(if r.station == crate::items::Station::Table {
                1
            } else {
                0
            });
        }
        if let Some(stack) = g.player.inventory.cursor.take() {
            require(
                g.player.inventory.add(stack.item, stack.count),
                "Output did not fit inventory",
            )?;
        }
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
        &format!(
            "Station placement failed: {} at {p:?}: {}",
            item.name(),
            g.message
        ),
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
        kind: MobKind::Husk,
        pos: mob,
        health: 30.,
        cooldown: 0.,
        level: 1,
        heading: 0.,
        stride: 0.,
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
        if let Some(drop) = g.drops.iter().find(|d| d.item == Item::Meat) {
            let origin = g.player.pos;
            g.player.pos = (Vec3::from_array(drop.pos) - Vec3::Y * 0.6).to_array();
            g.collect_drops();
            g.player.pos = origin;
        }
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
    // Extend the same mining/crafting transactions through iron.
    mine(&mut g, Block::Stone, Item::CopperPick, 6)?;
    mine(&mut g, Block::Copper, Item::CopperPick, 2)?;
    craft(&mut g, Item::CopperIngot, 2)?;
    craft(&mut g, Item::Smelter, 1)?;
    place(&mut g, Item::Smelter, IVec3::new(0, 100, -3))?;
    g.active_furnace = Some([0, 100, -3]);
    mine(&mut g, Block::Iron, Item::CopperPick, 8)?;
    mine(&mut g, Block::Coal, Item::CopperPick, 4)?;
    craft(&mut g, Item::IronIngot, 8)?;
    for item in [Item::IronPick, Item::IronAxe, Item::IronSword] {
        craft(&mut g, item, 1)?;
    }
    g.active_furnace = None;
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
                (crate::world::MIN_Y + 4..g.world.height(x, z) - 5)
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
    let home = (-8..=8)
        .find_map(|x| (-8..=8).find_map(|z| g.world.ruin_in_cell(x, z)))
        .ok_or("No polar ruin found")?;
    println!("Polar ruin fixture (seed 314159): {home:?}");
    g.time = 0.75;
    g.player.pos = (home.as_vec3() + Vec3::new(0.5, 1.01, 4.5)).to_array();
    g.velocity = Vec3::ZERO;
    equip(&mut g, Item::IronSword);
    g.tick(&Input::default(), STEP);
    require(
        g.biome().is_snow()
            && g.player
                .discoveries
                .contains(&format!("biome:{}", g.biome().discovery_name())),
        "Polar discovery failed",
    )?;
    require(
        g.entities
            .iter()
            .any(|(_, m)| matches!(m.kind, MobKind::Guardian(_))),
        "Ruin guardian missing",
    )?;
    for _ in 0..360 {
        let point = g
            .entities
            .iter()
            .find(|(_, m)| m.kind == MobKind::Guardian(home.to_array()))
            .map(|(_, m)| m.pos + Vec3::Y);
        let Some(point) = point else { break };
        aim(&mut g, point);
        g.tick(
            &Input {
                attack: true,
                ..Default::default()
            },
            STEP,
        );
    }
    require(
        g.player
            .discoveries
            .contains(&format!("guardian:{:?}", home.to_array())),
        "Guardian combat failed",
    )?;
    equip(&mut g, Item::IronPick);
    let cache = home + IVec3::Y;
    aim(&mut g, cache.as_vec3() + Vec3::splat(0.5));
    for _ in 0..120 {
        g.tick(
            &Input {
                attack: true,
                ..Default::default()
            },
            STEP,
        );
        if g.world.get(cache) == Block::Air {
            break;
        }
    }
    require(g.world.get(cache) == Block::Air, "Ruin cache did not open")?;
    let before = g.player.inventory.count(Item::IronIngot);
    g.player.pos = (cache.as_vec3() + Vec3::new(0.5, 0., 0.5)).to_array();
    g.collect_drops();
    require(
        g.player.inventory.count(Item::IronIngot) == before + 6,
        "Ruin reward missing",
    )?;
    let path = std::env::temp_dir().join(format!("voxel-expedition-{}.json", std::process::id()));
    let result = (|| {
        save::write(&g, &path)?;
        let loaded = save::read(&path)?.ok_or("Missing expedition save")?;
        require(
            serde_json::to_value(&loaded.player).unwrap()
                == serde_json::to_value(&g.player).unwrap()
                && loaded.world.edits == g.world.edits
                && loaded.furnaces == g.furnaces
                && loaded.torch_life == g.torch_life
                && loaded.time == g.time,
            "Expedition reload mismatch",
        )
    })();
    let _ = std::fs::remove_file(path);
    result?;
    println!(
        "Expedition passed: mined wood/stone/copper/coal, placed stations, crafted copper and iron gear, combat/loot, food, sleep, torch, forest/tundra discovery, cave collision, natural ruin guardian and cache, full player/edit/station/torch reload. Resource arena and forest/cave relocations are controlled fixtures."
    );
    Ok(())
}
