use crate::{
    game::{Game, Player},
    items::{ITEMS, Inventory, RECIPES},
    world::{Edit, MAX_Y, MIN_Y},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Serialize, Deserialize)]
struct Save {
    version: u32,
    seed: u64,
    player: Player,
    time: f32,
    edits: Vec<Edit>,
    torches: Vec<([i32; 3], f32)>,
}
pub fn write(game: &Game, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let save = Save {
        version: 2,
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
    };
    let data = serde_json::to_vec(&save).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    use std::io::Write;
    let mut f = fs::File::create(&temp).map_err(|e| e.to_string())?;
    f.write_all(&data)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
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
                    .any(|i| old.wear[*i as usize] >= i.durability().max(1))
            {
                return Err("Invalid legacy inventory".into());
            }
            let mut inv = Inventory::empty();
            for item in ITEMS {
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
        Some(2) => (),
        _ => return Err(format!("Unsupported save version {}", value["version"])),
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
    g.time = save.time;
    g.world.edits = save.edits.into_iter().map(|e| (e.pos, e.block)).collect();
    g.torch_life = save.torches.into_iter().collect::<BTreeMap<_, _>>();
    if g.torch_life
        .keys()
        .any(|p| g.world.edits.get(p) != Some(&crate::world::Block::Torch))
    {
        return Err("Torch timers do not match placed torches".into());
    }
    g.say("World loaded. E opens your pack.");
    Ok(Some(g))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_reject_version() {
        let path =
            std::env::temp_dir().join(format!("voxel-save-test-{}.json", std::process::id()));
        let mut g = Game::new(89);
        g.world
            .set(glam::IVec3::new(-16, 100, 16), crate::world::Block::Planks);
        g.player.inventory.add(crate::items::Item::CopperIngot, 3);
        write(&g, &path).unwrap();
        let restored = read(&path).unwrap().unwrap();
        assert_eq!(restored.world.seed, 89);
        assert_eq!(restored.world.edits, g.world.edits);
        assert_eq!(restored.player.inventory, g.player.inventory);
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
        g.player.inventory.furnace[0] = Some(Stack::new(Item::CopperOre, 5));
        write(&g, &path).unwrap();
        let restored = read(&path).unwrap().unwrap();
        assert_eq!(restored.player.inventory, g.player.inventory);
        g.player.inventory.cursor.as_mut().unwrap().count = 65;
        write(&g, &path).unwrap();
        assert!(read(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
