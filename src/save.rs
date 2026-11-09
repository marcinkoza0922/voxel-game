use crate::{
    game::{Game, Player},
    items::{ITEMS, Inventory, RECIPES},
    world::{Edit, MAX_Y, MIN_Y},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
struct Save {
    version: u32,
    seed: u64,
    player: Player,
    time: f32,
    edits: Vec<Edit>,
    #[serde(default)]
    drops: Vec<crate::game::WorldDrop>,
    torches: Vec<([i32; 3], f32)>,
    #[serde(default)]
    torch_mounts: Vec<([i32; 3], [i32; 3])>,
    #[serde(default)]
    furnaces: Vec<([i32; 3], crate::items::Furnace)>,
}
pub fn write(game: &Game, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = encode(game)?;
    let temp = path.with_extension("json.tmp");
    use std::io::Write;
    let mut f = fs::File::create(&temp).map_err(|e| e.to_string())?;
    f.write_all(&data)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}

/// Creates a separate save beside the current one, never replacing an existing file.
pub fn create(game: &Game, beside: &Path) -> Result<PathBuf, String> {
    use std::io::Write;
    let directory = beside
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let data = encode(game)?;
    for index in 1u64.. {
        let path = directory.join(format!("world-{index}.json"));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        };
        if let Err(e) = file.write_all(&data).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(e.to_string());
        }
        return Ok(path);
    }
    Err("No available world filename".into())
}

fn encode(game: &Game) -> Result<Vec<u8>, String> {
    let save = Save {
        version: 3,
        drops: game.drops.clone(),
        seed: game.world.seed,
        player: game.player.clone(),
        time: game.time,
        edits: game
            .world
            .edits
            .iter()
            .map(|(pos, block)| Edit {
                pos: *pos,
                block: *block,
            })
            .collect(),
        torches: game.torch_life.iter().map(|(p, t)| (*p, *t)).collect(),
        torch_mounts: game
            .world
            .torch_mounts
            .iter()
            .map(|(p, n)| (*p, *n))
            .collect(),
        furnaces: game.furnaces.iter().map(|(p, f)| (*p, f.clone())).collect(),
    };
    serde_json::to_vec(&save).map_err(|e| e.to_string())
}

pub fn read(path: &Path) -> Result<Option<Game>, String> {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut value: serde_json::Value =
        serde_json::from_slice(&data).map_err(|e| format!("Invalid save: {e}"))?;
    match value["version"].as_u64() {
        Some(1) => {
            #[derive(Deserialize)]
            struct Legacy {
                counts: [u32; 22],
                wear: [u32; 22],
                hotbar: [crate::items::Item; 9],
                selected: usize,
            }
            let old: Legacy = serde_json::from_value(value["player"]["inventory"].clone())
                .map_err(|e| format!("Invalid legacy inventory: {e}"))?;
            if old.selected >= 9
                || old.counts.iter().any(|n| *n > 9999)
                || ITEMS
                    .iter()
                    .take(22)
                    .any(|i| old.wear[*i as usize] >= i.durability().max(1))
            {
                return Err("Invalid legacy inventory".into());
            }
            let mut inv = Inventory::empty();
            for item in ITEMS.into_iter().take(22) {
                if !inv.add(item, old.counts[item as usize]) {
                    return Err("Legacy inventory exceeds 36 slots; original save preserved".into());
                }
                if let Some(s) = inv.slots.iter_mut().flatten().find(|s| s.item == item) {
                    s.wear = old.wear[item as usize];
                }
            }
            for (n, item) in old.hotbar.into_iter().enumerate() {
                inv.equip(item, n);
            }
            inv.selected = old.selected;
            value["player"]["inventory"] = serde_json::to_value(inv).map_err(|e| e.to_string())?;
            value["version"] = 2.into();
        }
        Some(2 | 3) => (),
        _ => return Err(format!("Unsupported save version {}", value["version"])),
    }
    if let Some(crafts) = value["player"]["crafts"].as_array_mut()
        && crafts.len() == 15
    {
        crafts.resize(RECIPES.len(), 0.into());
    }
    let save: Save = serde_json::from_value(value).map_err(|e| format!("Invalid save: {e}"))?;
    let p = &save.player;
    let valid_pos = |v: &[f32; 3]| {
        v.iter().all(|x| x.is_finite() && x.abs() < 10_000_000.)
            && v[1] >= MIN_Y as f32
            && v[1] <= (MAX_Y + 2) as f32
    };
    if !valid_pos(&p.pos)
        || !valid_pos(&p.spawn)
        || !p.yaw.is_finite()
        || !p.pitch.is_finite()
        || p.pitch.abs() > 1.56
        || !p.health.is_finite()
        || !(0.0..=160.).contains(&p.health)
        || !p.hunger.is_finite()
        || !(0.0..=100.).contains(&p.hunger)
        || !p.rest.is_finite()
        || !(0.0..=100.).contains(&p.rest)
        || !p.inventory.valid()
        || p.crafts.len() != RECIPES.len()
        || !(1..=100).contains(&p.level)
        || p.stats.iter().any(|n| *n > 20)
        || p.points > 200
        || !save.time.is_finite()
        || save.time < 0.
    {
        return Err("Save contains invalid player state".into());
    }
    if save.edits.iter().any(|e| {
        !(MIN_Y + 1..=MAX_Y).contains(&e.pos[1])
            || e.pos[0].unsigned_abs() > 10_000_000
            || e.pos[2].unsigned_abs() > 10_000_000
    }) {
        return Err("Save contains out-of-bounds edits".into());
    }
    if save.torches.len() > 256
        || save
            .torches
            .iter()
            .any(|(_, t)| !t.is_finite() || *t <= 0. || *t > 600.)
    {
        return Err("Save contains invalid torch timers".into());
    }
    let mut g = Game::new(save.seed);
    g.player = save.player;
    if save.drops.iter().any(|d| {
        !valid_pos(&d.pos)
            || d.velocity.iter().any(|v| !v.is_finite() || v.abs() > 100.)
            || d.count == 0
            || d.count > crate::items::Stack::limit(d.item)
    }) {
        return Err("Invalid world drop".into());
    }
    g.drops = save.drops;
    g.time = save.time;
    g.world.edits = save.edits.into_iter().map(|e| (e.pos, e.block)).collect();
    if save.version < 3 && !save.furnaces.is_empty() {
        return Err("Legacy save contains unsupported placed furnace state".into());
    }
    for (pos, state) in save.furnaces {
        if !state.valid()
            || !g.world.edits.get(&pos).is_some_and(|b| b.processor())
            || g.furnaces.insert(pos, state).is_some()
        {
            return Err("Invalid or duplicate placed furnace state".into());
        }
    }
    if save.version < 3 {
        let p = glam::Vec3::from_array(g.player.pos);
        let nearest = g
            .world
            .edits
            .iter()
            .filter(|(_, b)| b.processor())
            .map(|(q, _)| *q)
            .min_by(|a, b| {
                let dist =
                    |q: &[i32; 3]| glam::Vec3::from_array(q.map(|n| n as f32)).distance_squared(p);
                dist(a).total_cmp(&dist(b)).then_with(|| a.cmp(b))
            });
        if let Some(pos) = nearest {
            g.furnaces
                .insert(pos, std::mem::take(&mut g.player.inventory.processing));
        }
    }
    g.torch_life = save.torches.into_iter().collect::<BTreeMap<_, _>>();
    if g.torch_life
        .keys()
        .any(|p| g.world.edits.get(p) != Some(&crate::world::Block::Torch))
    {
        return Err("Torch timers do not match placed torches".into());
    }
    for (p, n) in save.torch_mounts {
        if !g.torch_life.contains_key(&p)
            || ![[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, -1, 0]].contains(&n)
            || g.world.torch_mounts.insert(p, n).is_some()
        {
            return Err("Invalid torch attachment".into());
        }
    }
    g.say("World loaded. E opens your pack.");
    Ok(Some(g))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_saves_preserve_existing_worlds() {
        let directory = std::env::temp_dir().join(format!("voxel-new-save-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let original = directory.join("world.json");
        fs::write(&original, b"existing world").unwrap();
        fs::write(directory.join("world-1.json"), b"occupied").unwrap();
        let first = create(&Game::new(42), &original).unwrap();
        let second = create(&Game::new(99), &original).unwrap();
        assert_ne!(first, second);
        assert_eq!(read(&first).unwrap().unwrap().world.seed, 42);
        assert_eq!(read(&second).unwrap().unwrap().world.seed, 99);
        assert_eq!(fs::read(&original).unwrap(), b"existing world");
        assert_eq!(
            fs::read(directory.join("world-1.json")).unwrap(),
            b"occupied"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn roundtrip_and_reject_version() {
        let path =
            std::env::temp_dir().join(format!("voxel-save-test-{}.json", std::process::id()));
        let mut g = Game::new(89);
        g.world
            .set(glam::IVec3::new(-16, 100, 16), crate::world::Block::Planks);
        g.player.inventory.add(crate::items::Item::CopperIngot, 3);
        g.drops.push(crate::game::WorldDrop {
            velocity: [0.6, 2.5, -0.4],
            pos: [0.5, 100.5, 0.5],
            item: crate::items::Item::Log,
            count: 1,
        });
        let torch = glam::IVec3::new(-15, 100, 16);
        g.world.set(torch, crate::world::Block::Torch);
        g.torch_life.insert(torch.to_array(), 600.);
        g.world.torch_mounts.insert(torch.to_array(), [-1, 0, 0]);
        write(&g, &path).unwrap();
        let restored = read(&path).unwrap().unwrap();
        assert_eq!(restored.world.seed, 89);
        assert_eq!(restored.world.torch_mounts, g.world.torch_mounts);
        assert_eq!(restored.drops, g.drops);
        assert_eq!(restored.world.edits, g.world.edits);
        assert_eq!(restored.player.inventory, g.player.inventory);
        assert_eq!(restored.furnaces, g.furnaces);
        let mut data: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        data["version"] = 99.into();
        fs::write(&path, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(read(&path).is_err());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn malformed_is_error() {
        let path = std::env::temp_dir().join(format!("voxel-bad-test-{}.json", std::process::id()));
        fs::write(&path, b"bad").unwrap();
        assert!(read(&path).is_err());
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn invalid_coordinates_are_rejected_without_panicking() {
        let path =
            std::env::temp_dir().join(format!("voxel-invalid-coord-{}.json", std::process::id()));
        let mut g = Game::new(1);
        g.world
            .edits
            .insert([i32::MIN, 100, 0], crate::world::Block::Stone);
        write(&g, &path).unwrap();
        assert!(read(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
#[cfg(test)]
mod inventory_migration_tests {
    use super::*;
    use crate::items::{Item, Stack};
    #[test]
    fn migrate_v1_preserves_counts_tool_wear_and_world() {
        let path =
            std::env::temp_dir().join(format!("voxel-v1-migration-{}.json", std::process::id()));
        let g = Game::new(234);
        write(&g, &path).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let mut counts = [0_u32; 22];
        let mut wear = [0_u32; 22];
        counts[Item::WoodPick as usize] = 2;
        wear[Item::WoodPick as usize] = 17;
        counts[Item::Log as usize] = 130;
        value["version"] = 1.into();
        value["player"]["inventory"] = serde_json::json!({"counts":counts,"wear":wear,"hotbar":["WoodPick","Log","Dirt","Planks","Torch","Berries","Table","Furnace","Bed"],"selected":0});
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let loaded = read(&path).unwrap().unwrap();
        assert_eq!(loaded.player.inventory.count(Item::Log), 130);
        assert_eq!(loaded.player.inventory.count(Item::WoodPick), 2);
        assert_eq!(loaded.player.inventory.slots[0].unwrap().wear, 17);
        assert_eq!(loaded.world.seed, 234);
        // Reject overflowing old totals without rewriting the file.
        value["player"]["inventory"]["counts"][Item::Log as usize] = 9999.into();
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(read(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn held_grid_and_furnace_stacks_roundtrip_and_invalid_stacks_rejected() {
        let path =
            std::env::temp_dir().join(format!("voxel-slots-save-{}.json", std::process::id()));
        let mut g = Game::new(235);
        g.player.inventory.cursor = Some(Stack::new(Item::Planks, 3));
        g.player.inventory.grid[8] = Some(Stack::new(Item::Stick, 4));
        g.player.inventory.hand[3] = Some(Stack::new(Item::Coal, 2));
        let pos = [2, 100, 2];
        g.world
            .set(glam::IVec3::from_array(pos), crate::world::Block::Furnace);
        let f = g.furnaces.entry(pos).or_default();
        f.furnace[0] = Some(Stack::new(Item::CopperOre, 5));
        f.furnace_joules = 3_000;
        for _ in 0..crate::items::SMELT_TICKS / 2 {
            g.tick_furnace();
        }
        g.furnaces.get_mut(&pos).unwrap().furnace_output = Some(Stack::new(Item::CopperIngot, 2));
        write(&g, &path).unwrap();
        let restored = read(&path).unwrap().unwrap();
        assert_eq!(restored.player.inventory, g.player.inventory);
        assert_eq!(restored.furnaces, g.furnaces);
        let mut resumed = restored;
        for _ in 0..crate::items::SMELT_TICKS / 2 {
            resumed.tick_furnace();
        }
        assert_eq!(resumed.furnaces[&pos].furnace_output.unwrap().count, 3);
        assert_eq!(resumed.furnaces[&pos].furnace_joules, 2_000);
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for field in [
            "furnace_joules",
            "furnace_progress",
            "furnace_input",
            "furnace_output",
        ] {
            legacy["player"]["inventory"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(
            read(&path)
                .unwrap()
                .unwrap()
                .player
                .inventory
                .processing
                .furnace_joules,
            0
        );
        let legacy_inventory = read(&path).unwrap().unwrap().player.inventory;
        assert_eq!(legacy_inventory.processing.furnace_progress, 0);
        assert_eq!(legacy_inventory.processing.furnace_output, None);
        g.player.inventory.processing.furnace_joules = crate::items::COAL_JOULES + 1;
        write(&g, &path).unwrap();
        assert!(read(&path).is_err());
        g.player.inventory.processing.furnace_joules = 3_000;
        g.player.inventory.cursor.as_mut().unwrap().count = 65;
        write(&g, &path).unwrap();
        assert!(read(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod placed_furnace_save_tests {
    use super::*;
    use crate::{
        items::{Furnace, Item, SMELT_TICKS, Stack},
        world::Block,
    };
    use glam::IVec3;

    #[test]
    fn multiple_partial_furnaces_resume_and_invalid_states_are_rejected() {
        let path =
            std::env::temp_dir().join(format!("voxel-placed-furnaces-{}.json", std::process::id()));
        let mut g = Game::new(9);
        for (pos, input) in [([-2, 100, 0], Item::CopperOre), ([2, 100, 0], Item::Meat)] {
            g.world.set(IVec3::from_array(pos), Block::Furnace);
            let mut f = Furnace::default();
            f.furnace = [Some(Stack::new(input, 2)), Some(Stack::new(Item::Coal, 1))];
            g.furnaces.insert(pos, f);
        }
        for _ in 0..SMELT_TICKS / 2 {
            g.tick_furnace();
        }
        write(&g, &path).unwrap();
        let good = fs::read(&path).unwrap();
        let mut restored = read(&path).unwrap().unwrap();
        assert_eq!(restored.furnaces, g.furnaces);
        for _ in 0..SMELT_TICKS / 2 {
            restored.tick_furnace();
        }
        for (pos, output) in [
            ([-2, 100, 0], Item::CopperIngot),
            ([2, 100, 0], Item::CookedMeat),
        ] {
            assert_eq!(
                restored.furnaces[&pos].furnace_output,
                Some(Stack::new(output, 1))
            );
            assert_eq!(restored.furnaces[&pos].furnace_joules, 3000);
        }
        let original: serde_json::Value = serde_json::from_slice(&good).unwrap();
        for (field, invalid) in [("furnace_joules", 4001), ("furnace_progress", SMELT_TICKS)] {
            let mut bad = original.clone();
            bad["furnaces"][0][1][field] = invalid.into();
            fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            assert!(read(&path).is_err());
        }
        let mut bad = original.clone();
        bad["furnaces"][0][1]["furnace"][0]["count"] = 65.into();
        fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(read(&path).is_err());
        let mut bad = original.clone();
        let duplicate = bad["furnaces"][0].clone();
        bad["furnaces"].as_array_mut().unwrap().push(duplicate);
        fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(read(&path).is_err());
        let mut bad = original;
        bad["furnaces"][0][0] = serde_json::json!([50, 100, 50]);
        fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(read(&path).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn v2_shared_contents_migrate_once_to_nearest_or_survive_without_a_station() {
        let path = std::env::temp_dir().join(format!(
            "voxel-furnace-migration-{}.json",
            std::process::id()
        ));
        let mut g = Game::new(9);
        g.player.pos = [0.5, 100., 0.5];
        g.player.inventory.processing.furnace = [
            Some(Stack::new(Item::CopperOre, 3)),
            Some(Stack::new(Item::Coal, 2)),
        ];
        g.player.inventory.processing.furnace_joules = 2500;
        for _ in 0..100 {
            crate::items::tick_furnace(&mut g.player.inventory.processing);
        }
        let legacy = g.player.inventory.processing.clone();
        // With no placed station, retain all old contents across new saves.
        write(&g, &path).unwrap();
        let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["version"] = 2.into();
        v.as_object_mut().unwrap().remove("furnaces");
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let retained = read(&path).unwrap().unwrap();
        assert_eq!(retained.player.inventory.processing, legacy);
        write(&retained, &path).unwrap();
        assert_eq!(
            read(&path).unwrap().unwrap().player.inventory.processing,
            legacy
        );
        for p in [[-20, 100, 0], [1, 100, 0]] {
            g.world.set(IVec3::from_array(p), Block::Furnace);
        }
        write(&g, &path).unwrap();
        let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["version"] = 2.into();
        v.as_object_mut().unwrap().remove("furnaces");
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let migrated = read(&path).unwrap().unwrap();
        assert_eq!(migrated.furnaces[&[1, 100, 0]], legacy);
        assert_eq!(migrated.furnaces.len(), 1);
        assert_eq!(migrated.player.inventory.processing, Furnace::default());
        write(&migrated, &path).unwrap();
        assert_eq!(read(&path).unwrap().unwrap().furnaces, migrated.furnaces);
        fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod expansion_tests {
    use super::*;
    use crate::{
        game::STEP,
        items::{Item, Stack},
        world::Block,
    };
    use glam::IVec3;

    #[test]
    fn old_recipe_counts_expand_and_smelter_ruin_state_roundtrips() {
        let mut g = Game::new(314159);
        g.player.crafts[0] = 3;
        let path =
            std::env::temp_dir().join(format!("voxel-expansion-save-{}.json", std::process::id()));
        let result = (|| -> Result<(), String> {
            let mut old: serde_json::Value = serde_json::from_slice(&encode(&g)?).unwrap();
            old["player"]["crafts"].as_array_mut().unwrap().truncate(15);
            fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
            let loaded = read(&path)?.unwrap();
            assert_eq!(loaded.player.crafts.len(), RECIPES.len());
            assert_eq!(loaded.player.crafts[0], 3);
            assert!(loaded.player.crafts[15..].iter().all(|n| *n == 0));
            let p = IVec3::new(2, 100, 0);
            g.world.set(p, Block::Smelter);
            let f = g.furnaces.entry(p.to_array()).or_default();
            f.furnace = [
                Some(Stack::new(Item::IronOre, 2)),
                Some(Stack::new(Item::Coal, 1)),
            ];
            for _ in 0..130 {
                g.tick_furnace();
            }
            let home = (-8..=8)
                .find_map(|x| (-8..=8).find_map(|z| g.world.ruin_in_cell(x, z)))
                .unwrap();
            g.world.set(home + IVec3::Y, Block::Air);
            g.player
                .discoveries
                .insert(format!("guardian:{:?}", home.to_array()));
            g.player.inventory.add(Item::IronPick, 1);
            write(&g, &path)?;
            let mut loaded = read(&path)?.unwrap();
            assert_eq!(loaded.furnaces, g.furnaces);
            assert_eq!(loaded.player.discoveries, g.player.discoveries);
            assert_eq!(loaded.world.get(home + IVec3::Y), Block::Air);
            assert_eq!(loaded.player.inventory.count(Item::IronPick), 1);
            for _ in 130..crate::items::SMELT_TICKS {
                loaded.tick_furnace();
            }
            assert_eq!(
                loaded.furnaces[&p.to_array()].furnace_output,
                Some(Stack::new(Item::IronIngot, 1))
            );
            // Returning after reload does not summon a defeated guardian.
            loaded.player.pos = (home.as_vec3() + glam::Vec3::new(0.5, 1.01, 4.5)).to_array();
            loaded.tick(&crate::game::Input::default(), STEP);
            assert!(
                !loaded
                    .entities
                    .iter()
                    .any(|(_, m)| matches!(m.kind, crate::ecs::MobKind::Guardian(_)))
            );
            Ok(())
        })();
        let _ = fs::remove_file(path);
        result.unwrap();
    }
}
