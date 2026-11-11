use crate::{
    ecs::{Entities, Mob, MobKind},
    items::{self, Inventory, Item, RECIPES},
    world::{Biome, Block, World},
};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const STEP: f32 = 1. / 60.;
pub const STAT_NAMES: [&str; 7] = [
    "Vigor",
    "Strength",
    "Dexterity",
    "Wisdom",
    "Willpower",
    "Agility",
    "Ingenuity",
];
#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub hunger: f32,
    pub rest: f32,
    pub inventory: Inventory,
    pub xp: u32,
    pub level: u32,
    pub points: u32,
    pub stats: [u32; 7],
    pub crafts: Vec<u32>,
    pub discoveries: BTreeSet<String>,
    pub spawn: [f32; 3],
}
impl Player {
    pub fn new(world: &World) -> Self {
        let mut spawn = Vec3::new(2.5, world.height(2, 2) as f32 + 2., 2.5);
        // Avoid spawning inside a tree or a cave roof.
        while world.get(spawn.floor().as_ivec3()).solid()
            || world.get((spawn + Vec3::Y).floor().as_ivec3()).solid()
        {
            spawn.y += 1.;
        }
        Self {
            pos: spawn.to_array(),
            yaw: 0.5,
            pitch: -0.1,
            health: 100.,
            hunger: 100.,
            rest: 100.,
            inventory: Inventory::default(),
            xp: 0,
            level: 1,
            points: 0,
            stats: [0; 7],
            crafts: vec![0; RECIPES.len()],
            discoveries: BTreeSet::new(),
            spawn: spawn.to_array(),
        }
    }
    pub fn max_health(&self) -> f32 {
        100. + self.stats[0] as f32 * 3.
    }
    pub fn direction(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }
    pub fn eye(&self) -> Vec3 {
        Vec3::from_array(self.pos) + Vec3::Y * 1.62
    }
    pub fn threshold(&self) -> u32 {
        80 + self.level * 40
    }
    pub fn gain(&mut self, xp: u32) {
        self.xp = self.xp.saturating_add(xp);
        while self.xp >= self.threshold() && self.level < 100 {
            self.xp -= self.threshold();
            self.level += 1;
            self.points += 2;
        }
    }
    fn discover(&mut self, key: String, xp: u32) {
        if self.discoveries.insert(key) {
            self.gain(xp);
        }
    }
}
#[derive(Default)]
pub struct Input {
    pub forward: f32,
    pub right: f32,
    pub jump: bool,
    pub sprint: bool,
    pub walk: bool,
    pub attack: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorldDrop {
    #[serde(default)]
    pub velocity: [f32; 3],
    pub pos: [f32; 3],
    pub item: Item,
    pub count: u32,
}
pub struct Game {
    pub god_mode: bool,
    pub drops: Vec<WorldDrop>,
    pub feedback: crate::feedback::Feedback,
    pub world: World,
    pub player: Player,
    pub entities: Entities,
    pub time: f32,
    pub message: String,
    pub message_time: f32,
    pub velocity: Vec3,
    pub grounded: bool,
    pub target: Option<(IVec3, IVec3)>,
    mining: Option<IVec3>,
    pub mining_progress: f32,
    attack_timer: f32,
    spawn_timer: f32,
    grass_timer: f32,
    fluid_timer: f32,
    fluid_pulse: u32,
    pub breath: f32,
    pub furnaces: std::collections::BTreeMap<[i32; 3], items::Furnace>,
    pub active_furnace: Option<[i32; 3]>,
    pub torch_life: std::collections::BTreeMap<[i32; 3], f32>,
}
impl Game {
    pub fn new(seed: u64) -> Self {
        let world = World::new(seed);
        let player = Player::new(&world);
        Self {
            god_mode: false,
            drops: Vec::new(),
            feedback: Default::default(),
            world,
            player,
            entities: Entities::default(),
            time: 0.28,
            message: "Gather logs. E opens your pack and crafting.".into(),
            message_time: 12.,
            velocity: Vec3::ZERO,
            grounded: false,
            target: None,
            mining: None,
            mining_progress: 0.,
            attack_timer: 0.,
            spawn_timer: 4.,
            grass_timer: 5.,
            fluid_timer: 0.2,
            fluid_pulse: 0,
            breath: 15.,
            furnaces: Default::default(),
            active_furnace: None,
            torch_life: Default::default(),
        }
    }

    pub fn new_in_biome(seed: u64, biome: Biome) -> Result<Self, String> {
        let mut game = Self::new(seed);
        const GRID_STEP: i32 = 24;
        const SEARCH_RADIUS: i32 = 16_384;
        let candidate = |x, z| {
            if game.world.biome(x, z) != biome {
                return None;
            }
            let y = game.world.height(x, z);
            if y <= crate::world::SEA_LEVEL
                || [(8, 0), (-8, 0), (0, 8), (0, -8)]
                    .into_iter()
                    .any(|(dx, dz)| game.world.biome(x + dx, z + dz) != biome)
                || [(2, 0), (-2, 0), (0, 2), (0, -2)]
                    .into_iter()
                    .any(|(dx, dz)| (game.world.height(x + dx, z + dz) - y).abs() > 3)
            {
                return None;
            }
            let ground = IVec3::new(x, y, z);
            if !matches!(
                game.world.get(ground),
                Block::Grass | Block::Snow | Block::Stone | Block::Dirt | Block::Ice
            ) || !(1..=2).all(|dy| {
                let block = game.world.get(ground + IVec3::Y * dy);
                !block.solid() && block.fluid().is_none()
            }) {
                return None;
            }
            Some(ground.as_vec3() + Vec3::new(0.5, 1.01, 0.5))
        };
        let spawn = candidate(2, 2).or_else(|| {
            (GRID_STEP..=SEARCH_RADIUS)
                .step_by(GRID_STEP as usize)
                .find_map(|r| {
                    (-r..=r).step_by(GRID_STEP as usize).find_map(|s| {
                        candidate(s, -r).or_else(|| candidate(s, r)).or_else(|| {
                            if s.abs() < r {
                                candidate(-r, s).or_else(|| candidate(r, s))
                            } else {
                                None
                            }
                        })
                    })
                })
        });
        let Some(spawn) = spawn else {
            return Err(format!(
                "No dry {} spawn found within {SEARCH_RADIUS} blocks for seed {seed}",
                biome.name()
            ));
        };
        game.player.pos = spawn.to_array();
        game.player.spawn = game.player.pos;
        Ok(game)
    }
    pub fn toggle_god_mode(&mut self) {
        self.god_mode = !self.god_mode;
        self.velocity = Vec3::ZERO;
        self.grounded = false;
        self.mining = None;
        self.mining_progress = 0.;
        self.say(if self.god_mode {
            "God mode ON: WASD fly, Space up, Shift down, LMB instant mine"
        } else {
            "God mode OFF"
        });
    }
    pub fn say(&mut self, s: impl Into<String>) {
        self.message = s.into();
        self.message_time = 4.;
    }
    pub fn target(&mut self) {
        self.target = self.world.raycast_fluid(
            self.player.eye(),
            self.player.direction(),
            5.,
            self.player.inventory.selected_count() > 0 && self.player.inventory.selected().bucket(),
        );
    }
    pub fn stations(&self) -> (bool, bool) {
        let p = Vec3::from_array(self.player.pos).floor().as_ivec3();
        let mut table = false;
        for x in -3..=3 {
            for y in -2..=2 {
                for z in -3..=3 {
                    if self.world.get(p + IVec3::new(x, y, z)) == Block::Table {
                        table = true;
                    }
                }
            }
        }
        (table, self.furnace_position().is_some())
    }
    pub fn furnace_position(&self) -> Option<[i32; 3]> {
        let p = Vec3::from_array(self.player.pos).floor().as_ivec3();
        let reachable = |q: [i32; 3]| {
            let d = IVec3::from_array(q) - p;
            d.x.abs() <= 3
                && d.y.abs() <= 2
                && d.z.abs() <= 3
                && self.world.get(IVec3::from_array(q)).processor()
        };
        if let Some(q) = self.active_furnace {
            return reachable(q).then_some(q);
        }
        (-3..=3)
            .flat_map(|x| {
                (-2..=2)
                    .flat_map(move |y| (-3..=3).map(move |z| (p + IVec3::new(x, y, z)).to_array()))
            })
            .filter(|q| reachable(*q))
            .min_by_key(|q| ((IVec3::from_array(*q) - p).length_squared(), *q))
    }
    pub fn furnace(&self) -> Option<&items::Furnace> {
        self.furnaces.get(&self.furnace_position()?)
    }
    // Reuse inventory transactions with a temporary station state; the player
    // never retains a copy of a placed furnace's contents.
    fn furnace_transaction(&mut self, action: impl FnOnce(&mut Inventory)) {
        if let Some(p) = self.furnace_position() {
            let state = self.furnaces.entry(p).or_default();
            std::mem::swap(&mut self.player.inventory.processing, state);
            action(&mut self.player.inventory);
            std::mem::swap(&mut self.player.inventory.processing, state);
        }
    }
    pub fn transfer(&mut self, n: usize, right: bool) {
        if (45..=46).contains(&n) {
            self.furnace_transaction(|inv| inv.transfer(n, right));
        } else {
            self.player.inventory.transfer(n, right);
        }
    }
    pub fn quick_transfer(&mut self, n: usize, mode: u8) {
        let (table, furnace) = self.stations();
        if mode == 1 && !table || mode == 2 && !furnace {
            return;
        }
        if mode == 2 {
            self.furnace_transaction(|inv| inv.quick_transfer(n, mode));
        } else {
            self.player.inventory.quick_transfer(n, mode);
        }
    }
    pub fn quick_output(&mut self, mode: u8) {
        let (table, furnace) = self.stations();
        if mode == 1 && !table || mode == 2 && !furnace {
            return;
        }
        if mode == 2 {
            self.furnace_transaction(|inv| {
                inv.quick_output(mode);
            });
        } else if let Some(index) = self.player.inventory.quick_output(mode) {
            self.reward_craft(index);
        }
    }
    pub fn craft(&mut self, index: usize) {
        let (table, furnace) = self.stations();
        match items::craft(&mut self.player.inventory, index, table, furnace) {
            Ok(()) => {
                self.reward_craft(index);
            }
            Err(e) => self.say(e),
        }
    }
    pub fn craft_grid(&mut self, mode: u8) {
        let (table, furnace) = self.stations();
        if mode == 1 && !table || mode == 2 && !furnace {
            self.say("Required station is out of reach");
            return;
        }
        if mode == 2 {
            self.furnace_transaction(|inv| {
                items::take_output(inv, 2);
            });
            return;
        }
        if let Some(index) = items::take_output(&mut self.player.inventory, mode) {
            self.reward_craft(index);
        }
    }
    fn reward_craft(&mut self, index: usize) {
        let r = &RECIPES[index];
        let count = self.player.crafts[index];
        let xp = items::craft_xp(r.xp, count);
        self.player.crafts[index] = count.saturating_add(1);
        let ingenuity = self.player.stats[6];
        if ingenuity > 0 && count.saturating_add(1).is_multiple_of(24 - ingenuity) {
            self.player.inventory.add(r.output, 1);
        }
        self.player.gain(xp);
        self.say(format!(
            "Crafted {} x{} (+{} XP)",
            r.output.name(),
            r.amount,
            xp
        ));
    }
    pub fn allocate(&mut self, index: usize) {
        if index < 7
            && index != 3
            && index != 4
            && self.player.points > 0
            && self.player.stats[index] < 20
        {
            self.player.points -= 1;
            self.player.stats[index] += 1;
            self.say(format!("{} increased", STAT_NAMES[index]));
        }
    }
    pub fn use_selected(&mut self) {
        let i = self.player.inventory.selected();
        if self.player.inventory.selected_count() == 0 {
            self.say("That slot is empty");
            return;
        }
        if matches!(i, Item::Berries | Item::CookedMeat | Item::Meat) {
            self.player.inventory.take_selected();
            let food = if i == Item::CookedMeat {
                40.
            } else if i == Item::Berries {
                15.
            } else {
                7.
            };
            self.player.hunger = (self.player.hunger + food).min(100.);
            self.say("You eat and recover some hunger");
            return;
        }
        if i.bucket() {
            self.use_bucket(i);
            return;
        }
        self.target();
        if let Some((hit, adjacent)) = self.target {
            if self.world.get(hit) == Block::Bed {
                self.sleep();
                return;
            }
            if let Some(block) = i.block() {
                if adjacent == hit || !self.world.get(adjacent).replaceable() {
                    return;
                }
                if block.plant() && !self.world.get(adjacent - IVec3::Y).dirt() {
                    self.say("Plants need dirt beneath them");
                    return;
                }
                let pos = Vec3::from_array(self.player.pos);
                if block.solid() && overlaps(pos, adjacent) {
                    self.say("Cannot place a block inside yourself");
                    return;
                }
                if self.entities.iter().any(|(_, m)| overlaps(m.pos, adjacent)) {
                    self.say("A creature is in the way");
                    return;
                }
                if block == Block::Torch && (!self.world.get(hit).solid() || adjacent.y < hit.y) {
                    self.say("Torches need a solid wall or floor");
                    return;
                }
                if block == Block::Torch && self.torch_life.len() >= 256 {
                    self.say("Torch limit reached (256)");
                    return;
                }
                if self.world.set(adjacent, block) {
                    self.torch_life.remove(&adjacent.to_array());
                    self.player.inventory.take_selected();
                    if block.processor() {
                        self.furnaces.insert(
                            adjacent.to_array(),
                            std::mem::take(&mut self.player.inventory.processing),
                        );
                    }
                    if block == Block::Torch {
                        self.torch_life.insert(adjacent.to_array(), 600.);
                        self.world
                            .torch_mounts
                            .insert(adjacent.to_array(), (hit - adjacent).to_array());
                    }
                }
            }
        }
    }
    fn use_bucket(&mut self, item: Item) {
        let Some((hit, adjacent)) =
            self.world
                .raycast_fluid(self.player.eye(), self.player.direction(), 5., true)
        else {
            return;
        };
        let block = self.world.get(hit);
        let (pos, replacement, bucket) = if let Some(fluid) = item.bucket_contents() {
            let pos = adjacent;
            if pos == hit
                || !self.world.get(pos).replaceable()
                || self.world.get(pos).fluid().is_some()
            {
                return;
            }
            (pos, fluid, item.empty_bucket())
        } else {
            let Some(filled) = item.fill_bucket(block) else {
                self.say(if block == Block::Lava {
                    "Lava needs an iron or stronger bucket"
                } else {
                    "Scoop a water or lava source"
                });
                return;
            };
            (hit, Block::Air, filled)
        };
        if self.world.set(pos, replacement) {
            self.torch_life.remove(&pos.to_array());
            let selected = self.player.inventory.selected;
            self.player.inventory.slots[selected] = Some(items::Stack::new(bucket, 1));
            self.feedback.swing = 0.32;
        }
    }
    pub fn sleep(&mut self) {
        let p = Vec3::from_array(self.player.pos).floor().as_ivec3();
        if self.entities.iter().any(|(_, m)| {
            m.kind.hostile() && m.pos.distance(Vec3::from_array(self.player.pos)) < 12.
        }) {
            self.say("A hostile creature is too close to sleep");
            return;
        }
        let mut bed = false;
        for x in -3..=3 {
            for y in -2..=2 {
                for z in -3..=3 {
                    bed |= self.world.get(p + IVec3::new(x, y, z)) == Block::Bed;
                }
            }
        }
        if bed {
            self.time = self.time.floor() + 1.28;
            self.player.rest = 100.;
            self.player.hunger = (self.player.hunger - 12.).max(0.);
            self.player.health = (self.player.health + 25.).min(self.player.max_health());
            self.say("You wake at dawn, rested");
        } else {
            self.say("Place a bedroll nearby to sleep");
        }
    }
    pub fn tick_furnace(&mut self) {
        let mut rewards = Vec::new();
        for (pos, state) in &mut self.furnaces {
            if let Some(index) = if self.world.get(IVec3::from_array(*pos)) == Block::Smelter {
                items::tick_processor(state, true)
            } else {
                items::tick_furnace(state)
            } {
                rewards.push(index);
            }
        }
        for index in rewards {
            self.reward_craft(index);
        }
    }

    fn tick_drops(&mut self, dt: f32) {
        let player = Vec3::from_array(self.player.pos);
        // Short sweeps keep fast falls from crossing an entire voxel.
        for drop in &mut self.drops {
            let mut pos = Vec3::from_array(drop.pos);
            let mut velocity = Vec3::from_array(drop.velocity);
            let nearest = Vec3::new(player.x, pos.y.clamp(player.y, player.y + 1.8), player.z);
            let target = Vec3::new(
                player.x,
                pos.y.clamp(player.y + 0.45, player.y + 1.35),
                player.z,
            );
            let mut capacity = self.player.inventory.clone();
            if pos.distance_squared(nearest) <= 1.4 * 1.4 && capacity.add(drop.item, drop.count) {
                // Steer toward the body quickly, with a capped final step so
                // the drop cannot overshoot the pickup region.
                let offset = target - pos;
                let desired = offset.normalize_or_zero() * 7.;
                velocity = velocity.lerp(desired, (dt * 18.).min(1.));
                velocity = velocity.clamp_length_max(offset.length() / dt.max(0.0001));
            } else {
                velocity.y = (velocity.y - 18. * dt).max(-24.);
            }
            let steps = ((velocity.length() * dt / 0.1).ceil() as usize).max(1);
            for _ in 0..steps {
                for axis in [0, 2, 1] {
                    let delta = velocity[axis] * dt / steps as f32;
                    let mut next = pos;
                    next[axis] += delta;
                    if drop_collides(&self.world, next) {
                        // Resolve to the surface so resting drops do not hover
                        // at a distance determined by their last fall speed.
                        let mut low = 0.;
                        let mut high = 1.;
                        for _ in 0..12 {
                            let fraction = (low + high) * 0.5;
                            let mut probe = pos;
                            probe[axis] += delta * fraction;
                            if drop_collides(&self.world, probe) {
                                high = fraction;
                            } else {
                                low = fraction;
                            }
                        }
                        pos[axis] += delta * low;
                        if axis == 1 && velocity.y < -1.5 {
                            velocity.y *= -0.28;
                            velocity.x *= 0.55;
                            velocity.z *= 0.55;
                        } else {
                            velocity[axis] = 0.;
                            if axis == 1 {
                                velocity.x *= 0.8;
                                velocity.z *= 0.8;
                            }
                        }
                    } else {
                        pos = next;
                    }
                }
            }
            drop.pos = pos.to_array();
            drop.velocity = velocity.to_array();
        }
    }

    pub fn collect_drops(&mut self) {
        let player = Vec3::from_array(self.player.pos);
        let mut collected = false;
        self.drops.retain(|drop| {
            let p = Vec3::from_array(drop.pos);
            let nearest = Vec3::new(player.x, p.y.clamp(player.y, player.y + 1.8), player.z);
            if p.distance_squared(nearest) <= 0.35 * 0.35
                && self.player.inventory.add(drop.item, drop.count)
            {
                collected = true;
                self.player
                    .discover(format!("resource:{}", drop.item.name()), 15);
                false
            } else {
                true
            }
        });
        if collected {
            self.feedback.sound(crate::feedback::Cue::Pickup);
        }
    }

    pub fn tick(&mut self, input: &Input, dt: f32) {
        let previous_health = self.player.health;
        self.feedback.tick(dt);
        self.tick_furnace();
        self.fluid_timer -= dt;
        if self.fluid_timer <= 0. {
            self.fluid_timer += 0.2;
            self.fluid_pulse = self.fluid_pulse.wrapping_add(1);
            self.world.tick_block_physics(
                Vec3::from_array(self.player.pos).floor().as_ivec3(),
                self.fluid_pulse,
            );
            self.world.tick_fluids(
                Vec3::from_array(self.player.pos).floor().as_ivec3(),
                self.fluid_pulse.is_multiple_of(3),
            );
        }
        self.time += dt / 900.;
        self.grass_timer -= dt;
        if self.grass_timer <= 0. {
            self.grass_timer = 5.;
            self.world
                .spread_grass(Vec3::from_array(self.player.pos).floor().as_ivec3());
        }
        self.message_time = (self.message_time - dt).max(0.);
        self.attack_timer = (self.attack_timer - dt).max(0.);
        let mut expired = vec![];
        for (p, t) in &mut self.torch_life {
            *t -= dt;
            let unsupported = self.world.torch_mounts.get(p).is_some_and(|n| {
                !self
                    .world
                    .get(IVec3::from_array(*p) + IVec3::from_array(*n))
                    .solid()
            });
            if *t <= 0. || unsupported || self.world.get(IVec3::from_array(*p)) != Block::Torch {
                expired.push(*p);
            }
        }
        for p in expired {
            self.torch_life.remove(&p);
            if self.world.get(IVec3::from_array(p)) == Block::Torch {
                self.world.set(IVec3::from_array(p), Block::Air);
            }
        }
        let pos = if self.god_mode {
            let right = Vec3::new(self.player.yaw.cos(), 0., self.player.yaw.sin());
            let vertical = input.jump as i32 as f32 - input.walk as i32 as f32;
            let direction = (self.player.direction() * input.forward
                + right * input.right
                + Vec3::Y * vertical)
                .normalize_or_zero();
            self.velocity = direction * if input.sprint { 30. } else { 12. };
            self.grounded = false;
            let pos = Vec3::from_array(self.player.pos) + self.velocity * dt;
            self.player.pos = pos.to_array();
            self.breath = 15.;
            pos
        } else {
            let sprint = input.sprint && !input.walk && self.player.hunger > 10.;
            let body_fluid = [0.1, 0.9].into_iter().find_map(|y| {
                self.world
                    .fluid_at(Vec3::from_array(self.player.pos) + Vec3::Y * y)
            });
            let swimming = body_fluid.is_some();
            let speed = (if swimming { 2.2 } else { 4.5 })
                * (1. + (self.player.stats[5] as f32 * 0.012).min(0.24))
                * if input.walk {
                    0.45
                } else if sprint {
                    1.45
                } else {
                    1.
                };
            let forward = Vec3::new(self.player.yaw.sin(), 0., -self.player.yaw.cos());
            let right = Vec3::new(self.player.yaw.cos(), 0., self.player.yaw.sin());
            let walk = (forward * input.forward + right * input.right).normalize_or_zero() * speed;
            self.velocity.x = walk.x;
            self.velocity.z = walk.z;
            let position = Vec3::from_array(self.player.pos);
            let ahead = position + walk.normalize_or_zero() * 0.45;
            let auto_jump = !input.walk
                && walk.length_squared() > 0.
                && collides(&self.world, ahead)
                && !collides(&self.world, ahead + Vec3::Y)
                && !collides(&self.world, position + Vec3::Y);
            if !swimming && (input.jump || auto_jump) && self.grounded {
                self.velocity.y = 7.2 * (1. + self.player.stats[5] as f32 * 0.008);
                self.grounded = false;
            }
            if swimming {
                self.velocity.y = (self.velocity.y * (1. - 4. * dt).max(0.)
                    + if input.jump { 12. * dt } else { -3. * dt })
                .clamp(-2., 3.);
                // A clear bank needs a jump impulse: swim drag alone stalls at
                // the surface before the player's feet can clear the ledge.
                if auto_jump && self.world.fluid_at(self.player.eye()).is_none() {
                    self.velocity.y = 7.2;
                }
            } else {
                self.velocity.y = (self.velocity.y - 22. * dt).max(-45.);
            }
            let mut pos = Vec3::from_array(self.player.pos);
            let was_falling = self.velocity.y;
            let edge_guard = input.walk && self.grounded && !swimming;
            self.grounded = false;
            for axis in [0, 2, 1] {
                let delta = self.velocity[axis] * dt;
                let steps = (delta.abs() / 0.2).ceil().max(1.) as u32;
                for _ in 0..steps {
                    let mut next = pos;
                    next[axis] += delta / steps as f32;
                    if axis != 1 && edge_guard && !collides(&self.world, next - Vec3::Y * 0.06) {
                        break;
                    }
                    if collides(&self.world, next) {
                        if axis == 1 {
                            if self.velocity.y < 0. {
                                self.grounded = true;
                                if was_falling < -12. && !swimming {
                                    let damage = (-was_falling - 12.)
                                        * 3.
                                        * (1. - self.player.stats[5] as f32 * 0.02);
                                    self.player.health -= damage.max(0.);
                                }
                            }
                            self.velocity.y = 0.;
                        }
                        break;
                    }
                    pos = next;
                }
            }
            self.player.pos = pos.to_array();
            if body_fluid.is_some_and(|(lava, _)| lava) {
                self.player.health -= dt * 25.;
            }
            if self
                .world
                .fluid_at(self.player.eye())
                .is_some_and(|(lava, _)| !lava)
            {
                self.breath = (self.breath - dt).max(0.);
                if self.breath == 0. {
                    self.player.health -= dt * 8.;
                }
            } else {
                self.breath = (self.breath + dt * 5.).min(15.);
            }
            self.player.hunger =
                (self.player.hunger - dt * if sprint { 0.12 } else { 0.055 }).max(0.);
            self.player.rest = (self.player.rest - dt * 0.035).max(0.);
            if self.player.hunger == 0. || self.player.rest == 0. {
                self.player.health -= dt * 1.5;
            } else if self.player.hunger > 60. && self.player.rest > 25. {
                self.player.health = (self.player.health + dt * 0.6).min(self.player.max_health());
            }
            pos
        };
        self.tick_drops(dt);
        self.collect_drops();
        let biome = self.world.biome(pos.x.floor() as i32, pos.z.floor() as i32);
        self.player
            .discover(format!("biome:{}", biome.discovery_name()), 30);
        self.target();
        if input.attack {
            if self.feedback.swing == 0. {
                self.feedback.swing = 0.32;
                self.feedback.sound(crate::feedback::Cue::Swing);
            }
            self.attack(dt);
        } else {
            self.mining = None;
            self.mining_progress = 0.;
        }
        self.update_mobs(dt);
        if self.player.health < previous_health && self.feedback.hurt == 0. {
            self.feedback.hurt = 0.3;
            self.feedback.sound(crate::feedback::Cue::Hurt);
        }
        if !self.god_mode && (self.player.health <= 0. || pos.y < crate::world::MIN_Y as f32) {
            self.player.pos = self.player.spawn;
            self.velocity = Vec3::ZERO;
            self.player.health = self.player.max_health();
            self.player.hunger = 75.;
            self.player.rest = 75.;
            self.breath = 15.;
            self.say("You awaken at your starting place. Inventory retained.");
        }
    }
    fn attack(&mut self, dt: f32) {
        let eye = self.player.eye();
        let dir = self.player.direction();
        let block_distance = self
            .target
            .map(|(p, _)| (p.as_vec3() + Vec3::splat(0.5)).distance(eye))
            .unwrap_or(5.);
        let victim = self
            .entities
            .iter()
            .filter_map(|(id, m)| {
                let d = m.pos + Vec3::Y * m.kind.target_height() - eye;
                let along = d.dot(dir);
                if along > 0.
                    && along < 3.2
                    && along < block_distance
                    && (d - dir * along).length() < 0.8
                {
                    Some((id, along))
                } else {
                    None
                }
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id);
        if let Some(id) = victim {
            self.mining_progress = 0.;
            if self.attack_timer > 0. {
                return;
            }
            self.attack_timer = 0.5 / (1. + self.player.stats[2] as f32 * 0.015);
            let selected = self.player.inventory.selected();
            let equipped = self.player.inventory.selected_count() > 0;
            let damage = (if equipped && matches!(selected, Item::Sword | Item::IronSword) {
                if selected == Item::IronSword {
                    19.
                } else {
                    14.
                }
            } else if equipped && selected.axe() > 0 {
                7.
            } else {
                3.
            }) * (1. + self.player.stats[1] as f32 * 0.025);
            let mut killed = None;
            if let Some(m) = self.entities.get_mut(id) {
                m.health -= damage;
                if let MobKind::PolarBear { provoked, .. } = &mut m.kind {
                    *provoked = true;
                }
                let pushed = m.pos + Vec3::new(dir.x, 0., dir.z) * 0.4;
                if !matches!(m.kind, MobKind::Penguin { .. } | MobKind::PolarBear { .. })
                    || !mob_blocked(&self.world, pushed, m.kind)
                {
                    m.pos = pushed;
                }
                if m.health <= 0. {
                    killed = Some((m.level, m.pos, m.kind));
                }
            }
            self.feedback.hit = 0.18;
            self.feedback.sound(crate::feedback::Cue::Hit);
            self.feedback.burst(eye + dir * 1.8, [0.55, 0.63, 0.32], 12);
            if equipped {
                self.player.inventory.damage_tool(selected);
            }
            if let Some((level, pos, kind)) = killed {
                self.entities.despawn(id);
                let xp = if self.player.level > level + 5 {
                    0
                } else {
                    25 / (1 + self.player.level.saturating_sub(level))
                };
                self.player.gain(xp);
                self.drops.push(WorldDrop {
                    velocity: [0.6, 2.5, -0.4],
                    pos: (pos + Vec3::Y * 0.6).to_array(),
                    item: if matches!(kind, MobKind::Guardian(_)) {
                        Item::Coal
                    } else {
                        Item::Meat
                    },
                    count: 2,
                });
                if let MobKind::Guardian(home) = kind {
                    self.player.discover(format!("guardian:{home:?}"), 60);
                }
                self.say(format!("{} defeated (+{xp} XP)", kind.name()));
            }
            return;
        }
        let Some((p, _)) = self.target else {
            self.mining = None;
            self.mining_progress = 0.;
            return;
        };
        let b = self.world.get(p);
        if b.fluid().is_some() {
            return;
        }
        let tool = self.player.inventory.selected();
        let has = self.player.inventory.selected_count() > 0;
        let pick = if has { tool.pick() } else { 0 };
        let axe = if has { tool.axe() } else { 0 };
        let hard = matches!(
            b,
            Block::Stone
                | Block::Copper
                | Block::Coal
                | Block::Cobble
                | Block::Furnace
                | Block::Iron
                | Block::Smelter
                | Block::RuinStone
                | Block::Reliquary
                | Block::Obsidian
        );
        if !self.god_mode
            && ((hard && pick == 0)
                || (b == Block::Copper && pick < 2)
                || (b == Block::Iron && pick < 3)
                || (b == Block::Obsidian && pick < 4)
                || (b == Block::Log && axe == 0))
        {
            if self.message_time < 1. {
                self.say(if b == Block::Obsidian {
                    "Obsidian needs an iron pickaxe"
                } else if b == Block::Iron {
                    "Iron ore needs a copper pickaxe"
                } else if b == Block::Copper {
                    "Copper ore needs a stone pickaxe"
                } else if b == Block::Log {
                    "Use an axe to harvest logs"
                } else {
                    "Use a pickaxe to harvest stone"
                });
            }
            return;
        }
        if !self.god_mode
            && b == Block::Reliquary
            && self.world.nearby_ruin(p.x, p.z).is_some_and(|home| {
                !self
                    .player
                    .discoveries
                    .contains(&format!("guardian:{:?}", home.to_array()))
            })
        {
            self.say("Defeat the ruin guardian to open this cache");
            return;
        }
        if self.mining != Some(p) {
            self.mining = Some(p);
            self.mining_progress = 0.;
        }
        let seconds = if b.plant() {
            0.12
        } else if hard {
            1.8 / (pick as f32).max(1.)
        } else if b == Block::Log {
            1.2 / (axe as f32).max(1.)
        } else {
            0.35
        };
        let previous_progress = self.mining_progress;
        self.mining_progress += if self.god_mode { 1. } else { dt / seconds };
        if previous_progress == 0.
            || (previous_progress * seconds / 0.32).floor()
                < (self.mining_progress * seconds / 0.32).floor()
        {
            let cue = if hard {
                crate::feedback::Cue::Stone
            } else if matches!(b, Block::Log | Block::Planks | Block::Table) {
                crate::feedback::Cue::Wood
            } else {
                crate::feedback::Cue::Earth
            };
            self.feedback.sound(cue);
            let (_, adjacent) = self.target.unwrap();
            let face = (adjacent - p).as_vec3();
            self.feedback
                .burst(p.as_vec3() + Vec3::splat(0.5) + face * 0.53, b.color(), 4);
        }
        if self.mining_progress >= 1. && p.y > crate::world::MIN_Y && p.y <= crate::world::MAX_Y {
            if b.processor()
                && self.furnaces.get(&p.to_array()).is_some_and(|f| {
                    f.furnace.iter().any(Option::is_some) || f.furnace_output.is_some()
                })
            {
                self.say("Empty the furnace before mining it");
                return;
            }
            let drop = if let Some(index) = b.flower_index() {
                Some(crate::items::FLOWER_ITEMS[index])
            } else {
                match b {
                    Block::TallGrass
                        if crate::world::hash(self.world.seed ^ 0x5eed, p.x, p.y, p.z)
                            .is_multiple_of(8) =>
                    {
                        Some(Item::WheatSeeds)
                    }
                    Block::Obsidian => Some(Item::Obsidian),
                    Block::Log => Some(Item::Log),
                    Block::Stone | Block::Cobble | Block::RuinStone => Some(Item::Cobble),
                    Block::Iron => Some(Item::IronOre),
                    Block::Reliquary => Some(Item::IronIngot),
                    Block::Smelter => Some(Item::Smelter),
                    Block::Copper => Some(Item::CopperOre),
                    Block::Coal => Some(Item::Coal),
                    Block::Dirt | Block::Grass | Block::Snow => Some(Item::Dirt),
                    Block::Planks => Some(Item::Planks),
                    Block::Table => Some(Item::Table),
                    Block::Furnace => Some(Item::Furnace),
                    Block::Bed => Some(Item::Bed),
                    Block::Torch => Some(Item::Torch),
                    Block::Sand => Some(Item::Sand),
                    Block::Gravel => Some(Item::Gravel),
                    Block::Leaves => Some(Item::Berries),
                    _ => None,
                }
            };
            if let Some(i) = drop {
                self.drops.push(WorldDrop {
                    velocity: [0.6, 2.5, -0.4],
                    pos: (p.as_vec3() + Vec3::splat(0.5)).to_array(),
                    item: i,
                    count: if b == Block::Reliquary { 6 } else { 1 },
                });
            }
            self.feedback.sound(crate::feedback::Cue::Break);
            self.feedback
                .burst(p.as_vec3() + Vec3::splat(0.5), b.color(), 20);
            self.world.set(p, Block::Air);
            self.torch_life.remove(&p.to_array());
            self.furnaces.remove(&p.to_array());
            if !self.god_mode && !b.plant() {
                self.player.inventory.damage_tool(tool);
            }
            self.mining_progress = 0.;
            self.mining = None;
        }
    }
    fn spawn_is_dark(&self, pos: Vec3) -> bool {
        let sample = pos + Vec3::Y;
        if crate::render::spawn_sky_light(&self.world, self.time, sample) > 0.1 {
            return false;
        }
        let torch_lit = |light: Vec3| {
            let falloff = (1. - sample.distance(light) / 9.).max(0.);
            0.8 * falloff * falloff > 0.1
        };
        !self
            .torch_life
            .keys()
            .any(|p| torch_lit(IVec3::from_array(*p).as_vec3() + Vec3::new(0.5, 0.77, 0.5)))
            && !self
                .world
                .lava_lights(sample.floor().as_ivec3())
                .into_iter()
                .any(|light| {
                    let falloff = (1. - sample.distance(light) / 9.).max(0.);
                    0.22 * falloff * falloff > 0.1
                })
            && !(self.player.inventory.selected() == Item::Torch
                && self.player.inventory.selected_count() > 0
                && torch_lit(Vec3::from_array(self.player.pos) + Vec3::Y * 1.6))
    }

    fn polar_spawn_site(&self, x: i32, z: i32, kind: MobKind) -> Option<Vec3> {
        if !self.world.biome(x, z).is_snow() {
            return None;
        }
        let height = self.world.height(x, z);
        for y in (height - 4..=height + 3).rev() {
            let ground = IVec3::new(x, y, z);
            let block = self.world.get(ground);
            if !block.solid() {
                continue;
            }
            let suitable = match kind {
                MobKind::Penguin { .. } => {
                    matches!(
                        block,
                        Block::Snow | Block::Sand | Block::Gravel | Block::Dirt
                    )
                }
                _ => matches!(block, Block::Snow | Block::Sand | Block::Gravel),
            };
            let pos = ground.as_vec3() + Vec3::new(0.5, 1.01, 0.5);
            return (suitable
                && !mob_blocked(&self.world, pos, kind)
                && self
                    .entities
                    .iter()
                    .all(|(_, mob)| mob.pos.distance(pos) > 1.5))
            .then_some(pos);
        }
        None
    }

    fn penguin_shore(&self, pos: Vec3) -> bool {
        let p = pos.floor().as_ivec3();
        [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
            .into_iter()
            .any(|side| {
                (p.y - 4..p.y).any(|y| {
                    [4, 8].into_iter().all(|distance| {
                        let q = p.with_y(y) + side * distance;
                        let block = self.world.get(q);
                        block == Block::Water
                            || (block == Block::Ice && self.world.get(q - IVec3::Y) == Block::Water)
                    })
                })
            })
    }

    fn spawn_polar_wildlife(&mut self, player: Vec3) {
        let penguins = self
            .entities
            .iter()
            .any(|(_, m)| matches!(m.kind, MobKind::Penguin { .. }));
        let bear = self
            .entities
            .iter()
            .any(|(_, m)| matches!(m.kind, MobKind::PolarBear { .. }));
        if penguins && bear {
            return;
        }
        let h = crate::world::hash(
            self.world.seed ^ 0x706f6c6172,
            (self.time * 10000.) as i32,
            0,
            0,
        );
        let angle = (h % 628) as f32 / 100.;
        let candidates = || {
            (0..16).map(|index| {
                let heading = angle + index as f32 * 2.39996;
                let radius = 18. + (index % 4) as f32 * 7.;
                let x = (player.x + heading.sin() * radius).floor() as i32;
                let z = (player.z + heading.cos() * radius).floor() as i32;
                (x, z, heading)
            })
        };
        if !penguins {
            let kind = MobKind::Penguin { male: false };
            for (x, z, heading) in candidates() {
                let Some(site) = self.polar_spawn_site(x, z, kind) else {
                    continue;
                };
                if (site.y - player.y).abs() > 20.
                    || self.entities.iter().any(|(_, m)| {
                        matches!(m.kind, MobKind::PolarBear { .. }) && m.pos.distance(site) <= 36.
                    })
                    || !self.penguin_shore(site)
                {
                    continue;
                }
                let mut colony = vec![site];
                for (dx, dz) in [(2, 1), (-2, 1), (0, -2), (3, -2), (-3, -2)] {
                    if let Some(pos) = self.polar_spawn_site(x + dx, z + dz, kind)
                        && (pos.y - site.y).abs() <= 1.1
                    {
                        colony.push(pos);
                    }
                }
                if colony.len() < 3 {
                    continue;
                }
                for (member, pos) in colony.into_iter().take(5).enumerate() {
                    self.entities.spawn(Mob {
                        kind: MobKind::Penguin {
                            male: (member as u64 + (h >> 8)) & 1 == 0,
                        },
                        pos,
                        health: 12.,
                        cooldown: 0.,
                        level: 1,
                        heading,
                        stride: 0.,
                    });
                }
                break;
            }
        }
        if !bear {
            let kind = MobKind::PolarBear {
                male: (h >> 9) & 1 == 0,
                provoked: false,
            };
            for (x, z, heading) in candidates() {
                if let Some(pos) = self.polar_spawn_site(x, z, kind)
                    && (pos.y - player.y).abs() <= 20.
                    && self.entities.iter().all(|(_, m)| {
                        !matches!(m.kind, MobKind::Penguin { .. }) || m.pos.distance(pos) > 32.
                    })
                {
                    self.entities.spawn(Mob {
                        kind,
                        pos,
                        health: 60.,
                        cooldown: 0.,
                        level: 3,
                        heading,
                        stride: 0.,
                    });
                    break;
                }
            }
        }
    }

    fn update_mobs(&mut self, dt: f32) {
        let player = Vec3::from_array(self.player.pos);
        if let Some(home) = self.world.nearby_ruin(player.x as i32, player.z as i32) {
            let key = format!("guardian:{:?}", home.to_array());
            if self.spawn_is_dark(home.as_vec3() + Vec3::new(0.5, 1.01, 3.5))
                && player.distance(home.as_vec3()) < 28.
                && self.world.get(home + IVec3::Y) == Block::Reliquary
                && !self.player.discoveries.contains(&key)
                && !self
                    .entities
                    .iter()
                    .any(|(_, m)| m.kind == MobKind::Guardian(home.to_array()))
            {
                self.entities.spawn(Mob {
                    kind: MobKind::Guardian(home.to_array()),
                    pos: home.as_vec3() + Vec3::new(0.5, 1.01, 3.5),
                    health: 65.,
                    cooldown: 0.8,
                    level: 4,
                    heading: 0.,
                    stride: 0.,
                });
                self.player
                    .discover(format!("ruin:{:?}", home.to_array()), 50);
                self.say("Ancient ruin: defeat its guardian to unseal the iron cache");
            }
        }
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0. {
            self.spawn_timer = 18.;
            if self
                .entities
                .iter()
                .filter(|(_, m)| matches!(m.kind, MobKind::Sheep { .. }))
                .count()
                < 6
            {
                let h = crate::world::hash(
                    self.world.seed ^ 0x7368656570,
                    (self.time * 10000.) as i32,
                    0,
                    0,
                );
                let angle = (h % 628) as f32 / 100.;
                let remaining = 6 - self
                    .entities
                    .iter()
                    .filter(|(_, m)| matches!(m.kind, MobKind::Sheep { .. }))
                    .count();
                for member in 0..remaining.min(3) as i32 {
                    let x = (player.x + angle.sin() * 26.) as i32 + member * 2;
                    let z = (player.z + angle.cos() * 26.) as i32;
                    let y = self.world.height(x, z) + 1;
                    let pos = Vec3::new(x as f32 + 0.5, y as f32 + 0.01, z as f32 + 0.5);
                    if self.world.biome(x, z).is_grassland()
                        && self.world.get(IVec3::new(x, y - 1, z)) == Block::Grass
                        && !collides(&self.world, pos)
                    {
                        self.entities.spawn(Mob {
                            kind: MobKind::Sheep {
                                male: member % 2 == 0,
                            },
                            pos,
                            health: 16.,
                            cooldown: 0.,
                            level: 1,
                            heading: 0.,
                            stride: 0.,
                        });
                    }
                }
            }

            self.spawn_polar_wildlife(player);
            if self
                .entities
                .iter()
                .filter(|(_, m)| matches!(m.kind, MobKind::Husk | MobKind::Guardian(_)))
                .count()
                < 5
            {
                let n = (self.time * 10000.) as i32;
                let h = crate::world::hash(self.world.seed, n, 0, 0);
                let angle = (h % 628) as f32 / 100.;
                let x = player.x + angle.sin() * 20.;
                let z = player.z + angle.cos() * 20.;
                let surface = self.world.height(x.floor() as i32, z.floor() as i32) + 1;
                let base = player.y.floor() as i32;
                let candidate = (base - 8..=base + 8)
                    .chain(std::iter::once(surface))
                    .map(|y| Vec3::new(x, y as f32 + 0.01, z))
                    .find(|p| {
                        !collides(&self.world, *p)
                            && collides(&self.world, *p - Vec3::Y * 0.06)
                            && self.spawn_is_dark(*p)
                    });
                if let Some(pos) = candidate {
                    self.entities.spawn(Mob {
                        kind: MobKind::Husk,
                        pos,
                        health: 30.,
                        cooldown: 0.,
                        level: 1,
                        heading: 0.,
                        stride: 0.,
                    });
                }
            }
        }
        let remove: Vec<_> = self
            .entities
            .iter()
            .filter(|(_, m)| m.pos.distance(player) > 90.)
            .map(|(id, _)| id)
            .collect();
        for id in remove {
            self.entities.despawn(id);
        }
        for mob in self.entities.iter_mut() {
            mob.cooldown = (mob.cooldown - dt).max(0.);
            let diff = player - mob.pos;
            let dist = diff.length();
            if dist > 22.
                && let MobKind::PolarBear { provoked, .. } = &mut mob.kind
            {
                *provoked = false;
            }
            let passive = !mob.kind.hostile();
            let penguin = matches!(mob.kind, MobKind::Penguin { .. });
            let bear = matches!(mob.kind, MobKind::PolarBear { .. });
            let flee = !bear && dist < if penguin { 5. } else { 3. };
            if bear && !passive && dist < 18. {
                mob.heading = (-diff.x).atan2(-diff.z);
            }
            let phase = self.time * 1200. + mob.pos.x * 0.07 + mob.pos.z * 0.05;
            let direction = if passive {
                if flee {
                    -diff
                } else {
                    Vec3::new(phase.sin(), 0., phase.cos())
                }
            } else {
                diff
            };
            if (passive && (flee || phase.sin() > 0.35)) || (!passive && dist < 18. && dist > 1.4) {
                let speed = if bear {
                    if passive { 0.85 } else { 2.65 }
                } else if penguin {
                    if flee { 1.65 } else { 0.9 }
                } else if passive {
                    0.65
                } else if matches!(mob.kind, MobKind::Guardian(_)) {
                    2.2
                } else {
                    1.7
                };
                let movement =
                    Vec3::new(direction.x, 0., direction.z).normalize_or_zero() * dt * speed;
                let before = mob.pos;
                for axis in [0, 2] {
                    let mut p = mob.pos;
                    p[axis] += movement[axis];
                    if !mob_blocked(&self.world, p, mob.kind) {
                        mob.pos = p;
                    } else if !mob_blocked(&self.world, p + Vec3::Y, mob.kind)
                        && body_collides(&self.world, p - Vec3::Y * 0.1, mob.kind.body())
                    {
                        mob.pos = p + Vec3::Y;
                    }
                }
                if (penguin || bear) && mob.pos != before {
                    mob.heading = (-movement.x).atan2(-movement.z);
                    mob.stride += before.distance(mob.pos);
                }
            }
            let down = mob.pos - Vec3::Y * dt * 6.;
            if !mob_blocked(&self.world, down, mob.kind) {
                mob.pos = down;
            }
            if mob.kind.hostile()
                && dist < 1.8
                && mob.cooldown == 0.
                && self
                    .world
                    .raycast(
                        mob.pos + Vec3::Y,
                        (player + Vec3::Y) - (mob.pos + Vec3::Y),
                        dist,
                    )
                    .is_none()
            {
                if !self.god_mode {
                    self.player.health -= match mob.kind {
                        MobKind::PolarBear { .. } => 15.,
                        MobKind::Guardian(_) => 12.,
                        _ => 7.,
                    };
                }
                mob.cooldown = 1.2;
            }
        }
    }
    pub fn biome(&self) -> Biome {
        let p = Vec3::from_array(self.player.pos);
        self.world.biome(p.x.floor() as i32, p.z.floor() as i32)
    }
}
fn drop_collides(world: &World, pos: Vec3) -> bool {
    let min = (pos - Vec3::splat(0.2)).floor().as_ivec3();
    let max = (pos + Vec3::splat(0.2)).floor().as_ivec3();
    for x in min.x..=max.x {
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                if world.get(IVec3::new(x, y, z)).solid() {
                    return true;
                }
            }
        }
    }
    false
}

pub fn overlaps(pos: Vec3, b: IVec3) -> bool {
    pos.x + 0.29 > b.x as f32
        && pos.x - 0.29 < (b.x + 1) as f32
        && pos.y + 1.8 > b.y as f32
        && pos.y < (b.y + 1) as f32
        && pos.z + 0.29 > b.z as f32
        && pos.z - 0.29 < (b.z + 1) as f32
}
pub fn collides(world: &World, pos: Vec3) -> bool {
    body_collides(world, pos, (0.29, 1.79))
}

fn mob_blocked(world: &World, pos: Vec3, kind: MobKind) -> bool {
    body_collides(world, pos, kind.body())
        || (matches!(kind, MobKind::Penguin { .. } | MobKind::PolarBear { .. })
            && world.get(pos.floor().as_ivec3()).fluid().is_some())
}

fn body_collides(world: &World, pos: Vec3, (radius, height): (f32, f32)) -> bool {
    let min = (pos - Vec3::new(radius, 0., radius)).floor().as_ivec3();
    let max = (pos + Vec3::new(radius, height, radius)).floor().as_ivec3();
    for x in min.x..=max.x {
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                if world.get(IVec3::new(x, y, z)).solid() {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biome_test_starts_on_dry_ground_in_each_selectable_biome() {
        for name in Biome::SPAWN_NAMES.split(", ") {
            let biome = Biome::from_spawn_name(name).unwrap();
            let game = Game::new_in_biome(314159, biome).unwrap();
            assert_eq!(game.biome(), biome, "{name}");
            assert_eq!(game.player.pos, game.player.spawn);
            let pos = Vec3::from_array(game.player.pos);
            assert!(!collides(&game.world, pos));
            assert!(collides(&game.world, pos - Vec3::Y * 0.06));
            assert!(game.world.get(pos.floor().as_ivec3()).fluid().is_none());
        }
    }
    #[test]
    fn god_mode_flies_through_blocks_and_preserves_survival() {
        let mut g = Game::new(42);
        g.player.pos = [0.5, 1000., 0.5];
        g.world.set(IVec3::new(0, 1000, 0), Block::Stone);
        g.player.hunger = 0.;
        g.player.rest = 0.;
        g.toggle_god_mode();
        g.tick(
            &Input {
                jump: true,
                ..Default::default()
            },
            0.1,
        );
        assert!((g.player.pos[1] - 1001.2).abs() < 0.001);
        assert_eq!(g.player.health, 100.);
        assert_eq!(g.player.hunger, 0.);
        assert_eq!(g.player.rest, 0.);
        g.tick(&Input::default(), 0.1);
        assert!((g.player.pos[1] - 1001.2).abs() < 0.001);
        g.toggle_god_mode();
        assert_eq!(g.velocity, Vec3::ZERO);
        g.tick(&Input::default(), 0.1);
        assert!(g.player.pos[1] < 1001.2);
    }

    #[test]
    fn god_mode_instantly_mines_obsidian_without_tools() {
        let mut g = Game::new(42);
        let p = IVec3::new(0, 1000, -2);
        g.player.pos = [0.5, 999., 0.5];
        g.world.set(p, Block::Obsidian);
        g.target = Some((p, p + IVec3::Z));
        g.attack(1. / 60.);
        assert_eq!(g.world.get(p), Block::Obsidian);
        g.toggle_god_mode();
        let inventory = g.player.inventory.clone();
        g.attack(1. / 60.);
        assert_eq!(g.world.get(p), Block::Air);
        assert_eq!(g.player.inventory, inventory);
    }

    #[test]
    fn sheep_are_passive_and_daylight_spawning_is_bounded() {
        let mut g = Game::new(314159);
        let pos = Vec3::from_array(g.player.pos);
        g.entities.spawn(Mob {
            kind: MobKind::Sheep { male: false },
            pos: pos + Vec3::X,
            health: 16.,
            cooldown: 0.,
            level: 1,
            heading: 0.,
            stride: 0.,
        });
        let health = g.player.health;
        g.spawn_timer = 100.;
        g.update_mobs(0.1);
        assert_eq!(g.player.health, health);
        assert!(g.entities.iter().all(|(_, m)| !m.kind.hostile()));
        for _ in 0..20 {
            g.spawn_timer = 0.;
            g.update_mobs(0.);
        }
        assert!(g.entities.iter().filter(|(_, m)| !m.kind.hostile()).count() <= 6);
    }

    #[test]
    fn polar_wildlife_spawns_in_coastal_colonies_and_solitary_bears() {
        let mut game = Game::new(314159);
        let world = &game.world;
        let coast = (-1500..1500)
            .step_by(12)
            .flat_map(|x| (-1500..1500).step_by(12).map(move |z| (x, z)))
            .find_map(|(x, z)| {
                let y = world.height(x, z);
                if y != crate::world::SEA_LEVEL + 1 || !world.biome(x, z).is_snow() {
                    return None;
                }
                let pos = Vec3::new(x as f32 + 0.5, y as f32 + 1.01, z as f32 + 0.5);
                game.penguin_shore(pos).then_some(pos)
            })
            .expect("seed has a polar coast");
        for attempt in 0..24 {
            game.time = attempt as f32 / 100.;
            game.spawn_polar_wildlife(coast);
            let penguins: Vec<_> = game
                .entities
                .iter()
                .filter(|(_, m)| matches!(m.kind, MobKind::Penguin { .. }))
                .collect();
            let bears: Vec<_> = game
                .entities
                .iter()
                .filter(|(_, m)| matches!(m.kind, MobKind::PolarBear { .. }))
                .collect();
            if !penguins.is_empty() && !bears.is_empty() {
                assert!((3..=5).contains(&penguins.len()));
                assert_eq!(bears.len(), 1);
                assert!(
                    penguins
                        .iter()
                        .all(|(_, m)| m.pos.distance(bears[0].1.pos) > 32.)
                );
                return;
            }
        }
        panic!("expected both polar species near the seeded coastline");
    }

    #[test]
    fn polar_bear_retaliates_after_a_player_hit() {
        let mut g = sky_fixture();
        g.spawn_timer = 100.;
        g.player.pitch = -0.45;
        g.entities.spawn(Mob {
            kind: MobKind::PolarBear {
                male: false,
                provoked: false,
            },
            pos: Vec3::new(0.5, 100.01, -1.0),
            health: 60.,
            cooldown: 0.,
            level: 3,
            heading: 0.,
            stride: 0.,
        });
        let health = g.player.health;
        g.update_mobs(0.);
        assert_eq!(g.player.health, health);
        g.attack(STEP);
        assert!(g.entities.iter().any(|(_, m)| m.kind.hostile()));
        for _ in 0..60 {
            g.update_mobs(STEP);
        }
        assert!(g.player.health < health);
    }
    #[test]
    fn spawning_requires_darkness_and_respects_torches() {
        let mut g = sky_fixture();
        let p = Vec3::new(0.5, 100.01, 0.5);
        g.time = 0.25;
        assert!(!g.spawn_is_dark(p));
        g.time = 0.75;
        assert!(g.spawn_is_dark(p));
        g.torch_life.insert([1, 100, 0], 600.);
        assert!(!g.spawn_is_dark(p));
        g.torch_life.clear();
        g.time = 0.25;
        for x in -12..=12 {
            for z in -12..=12 {
                g.world.set(IVec3::new(x, 103, z), Block::Stone);
            }
        }
        assert!(g.spawn_is_dark(p));
    }

    #[test]
    fn auto_jump_climbs_one_block_but_walking_stays_below() {
        for walking in [false, true] {
            let mut g = sky_fixture();
            for x in -2..=2 {
                g.world.set(IVec3::new(x, 100, -1), Block::Stone);
            }
            for _ in 0..120 {
                g.tick(
                    &Input {
                        forward: 1.,
                        walk: walking,
                        ..Default::default()
                    },
                    STEP,
                );
            }
            if walking {
                assert!(g.player.pos[2] >= 0.29);
                assert!(g.player.pos[1] < 100.1);
            } else {
                assert!(g.player.pos[2] < -1.);
            }
        }
    }

    #[test]
    fn walking_guards_edges_in_both_axes_and_overrides_sprint() {
        let mut g = sky_fixture();
        for _ in 0..600 {
            g.tick(
                &Input {
                    forward: 1.,
                    right: 1.,
                    walk: true,
                    sprint: true,
                    ..Default::default()
                },
                STEP,
            );
        }
        assert!(g.player.pos[1] > 99.9);
        assert!(g.player.pos[0] < 6.29);
        assert!(g.player.pos[2] > -5.29);
        assert!(g.velocity.x.abs() < 2.);
    }

    #[test]
    fn fixed_step_landing() {
        let mut g = Game::new(7);
        for _ in 0..180 {
            g.tick(&Input::default(), STEP);
        }
        assert!(g.grounded);
        assert!(!collides(&g.world, Vec3::from_array(g.player.pos)));
        assert!(g.player.health > 0.);
    }
    #[test]
    fn grounded_player_can_jump() {
        let mut g = Game::new(7);
        for _ in 0..120 {
            g.tick(&Input::default(), STEP);
        }
        let y = g.player.pos[1];
        g.tick(
            &Input {
                jump: true,
                ..Default::default()
            },
            STEP,
        );
        assert!(g.player.pos[1] > y);
    }
    #[test]
    fn stats_are_bounded() {
        let mut g = Game::new(7);
        g.player.points = 40;
        for _ in 0..30 {
            g.allocate(0);
        }
        assert_eq!(g.player.stats[0], 20);
        assert_eq!(g.player.points, 20);
    }
    fn sky_fixture() -> Game {
        let mut g = Game::new(7);
        g.player.pos = [0.5, 100.01, 0.5];
        g.player.yaw = 0.;
        g.player.pitch = 0.;
        for x in -5..=5 {
            for z in -5..=5 {
                g.world.set(IVec3::new(x, 99, z), Block::Stone);
            }
        }
        g
    }
    #[test]
    fn dirt_gains_cover_after_simulation_time() {
        let mut g = sky_fixture();
        g.spawn_timer = 100.;
        let p = IVec3::new(2, 100, 0);
        g.world.set(p, Block::Dirt);
        g.world.set(p + IVec3::X, Block::Grass);
        g.grass_timer = 0.05;
        g.tick(&Input::default(), 0.04);
        assert_eq!(g.world.get(p), Block::Dirt);
        g.tick(&Input::default(), 0.02);
        assert_eq!(g.world.get(p), Block::Grass);
        assert_eq!(g.world.edits.get(&p.to_array()), Some(&Block::Grass));
    }
    #[test]
    fn harvesting_plants_drops_flowers_and_occasional_seeds_without_tool_wear() {
        let mut g = sky_fixture();
        let before = g.player.inventory.clone();
        for x in 0..128 {
            let p = IVec3::new(x, 101, -2);
            g.world.set(p - IVec3::Y, Block::Grass);
            g.world.set(p, Block::TallGrass);
            g.target = Some((p, p + IVec3::Z));
            g.attack(0.13);
            assert_eq!(g.world.get(p), Block::Air);
        }
        assert!(!g.drops.is_empty() && g.drops.len() < 40);
        assert!(g.drops.iter().all(|d| d.item == Item::WheatSeeds));
        assert_eq!(g.player.inventory, before);
        g.drops.clear();
        for (index, block) in crate::world::FLOWERS.into_iter().enumerate() {
            let p = IVec3::new(index as i32, 101, -2);
            g.world.set(p, block);
            g.target = Some((p, p + IVec3::Z));
            g.attack(0.13);
            assert_eq!(g.drops[index].item, crate::items::FLOWER_ITEMS[index]);
            assert_eq!(g.drops[index].count, 1);
            assert_eq!(g.world.get(p), Block::Air);
        }
    }
    #[test]
    fn wall_torches_attach_reject_stacking_and_lose_support() {
        let mut g = sky_fixture();
        let support = IVec3::new(0, 101, -2);
        let torch = IVec3::new(0, 101, -1);
        g.world.set(support, Block::Stone);
        g.player.inventory.add(Item::Torch, 4);
        g.player.inventory.equip(Item::Torch, 0);
        g.player.inventory.selected = 0;
        g.use_selected();
        assert_eq!(g.world.get(torch), Block::Torch);
        assert_eq!(g.world.torch_mounts[&torch.to_array()], [0, 0, -1]);
        let count = g.player.inventory.count(Item::Torch);
        g.use_selected();
        assert_eq!(g.player.inventory.count(Item::Torch), count);
        assert_eq!(g.torch_life.len(), 1);
        g.world.set(support, Block::Air);
        g.tick(&Input::default(), 0.01);
        assert_eq!(g.world.get(torch), Block::Air);
        assert!(g.world.torch_mounts.is_empty());
    }

    #[test]
    fn walking_cannot_tunnel_through_wall() {
        let mut g = sky_fixture();
        for x in -3..=3 {
            for y in 100..=104 {
                g.world.set(IVec3::new(x, y, -2), Block::Stone);
            }
        }
        for _ in 0..180 {
            g.tick(
                &Input {
                    forward: 1.,
                    sprint: true,
                    ..Default::default()
                },
                STEP,
            );
        }
        assert!(g.player.pos[2] >= -0.72);
        assert!(!collides(&g.world, Vec3::from_array(g.player.pos)));
    }
    #[test]
    fn drops_fall_bounce_settle_and_land_again_when_support_is_removed() {
        let mut g = sky_fixture();
        g.player.pos = [20., 100., 20.];
        for x in -3..=3 {
            for z in -3..=3 {
                g.world.set(IVec3::new(x, 100, z), Block::Stone);
            }
        }
        g.drops.push(WorldDrop {
            pos: [0.5, 104., 0.5],
            velocity: [0., 0., 0.],
            item: Item::Log,
            count: 1,
        });
        let mut bounced = false;
        for _ in 0..240 {
            g.tick_drops(STEP);
            bounced |= g.drops[0].velocity[1] > 0.;
            assert!(!drop_collides(&g.world, Vec3::from_array(g.drops[0].pos)));
        }
        assert!(bounced);
        assert!((g.drops[0].pos[1] - 101.2).abs() < 0.002);
        assert_eq!(g.drops[0].velocity[1], 0.);
        g.world.set(IVec3::new(0, 100, 0), Block::Air);
        for _ in 0..20 {
            g.tick_drops(STEP);
        }
        assert!(g.drops[0].pos[1] < 101.);
    }

    #[test]
    fn fast_drops_collide_with_walls_without_tunneling() {
        let mut g = sky_fixture();
        g.world.set(IVec3::new(1, 103, 0), Block::Stone);
        g.drops.push(WorldDrop {
            pos: [0.5, 103.5, 0.5],
            velocity: [24., 0., 0.],
            item: Item::Log,
            count: 1,
        });
        g.tick_drops(0.1);
        assert!(g.drops[0].pos[0] <= 0.8);
        assert_eq!(g.drops[0].velocity[0], 0.);
    }

    #[test]
    fn nearby_drops_pull_in_before_collection_and_sound() {
        let mut g = sky_fixture();
        g.player.pos = [0.5, 100., 0.5];
        g.drops.push(WorldDrop {
            pos: [1.7, 101., 0.5],
            velocity: [0.; 3],
            item: Item::Log,
            count: 1,
        });
        g.collect_drops();
        assert_eq!(g.drops.len(), 1);
        assert!(g.feedback.sounds.is_empty());
        g.tick_drops(STEP);
        assert!(g.drops[0].pos[0] < 1.7);
        assert!(g.drops[0].pos[0] > 0.85);
        for _ in 0..60 {
            g.tick_drops(STEP);
            g.collect_drops();
            if g.drops.is_empty() {
                break;
            }
        }
        assert!(g.drops.is_empty());
        assert_eq!(g.player.inventory.count(Item::Log), 1);
        assert_eq!(
            g.feedback
                .sounds
                .iter()
                .filter(|c| matches!(c, crate::feedback::Cue::Pickup))
                .count(),
            1
        );
    }

    #[test]
    fn full_inventory_disables_drop_attraction() {
        let mut g = sky_fixture();
        g.player.pos = [0.5, 100., 0.5];
        g.player
            .inventory
            .slots
            .fill(Some(items::Stack::new(Item::Dirt, 64)));
        g.drops.push(WorldDrop {
            pos: [1.7, 101., 0.5],
            velocity: [0.; 3],
            item: Item::Log,
            count: 1,
        });
        g.tick_drops(STEP);
        g.collect_drops();
        assert_eq!(g.drops[0].pos[0], 1.7);
        assert!(g.drops[0].velocity[1] < 0.);
        assert!(g.feedback.sounds.is_empty());
    }

    #[test]
    fn drops_wait_for_proximity_and_inventory_space() {
        let mut g = sky_fixture();
        let p = IVec3::new(0, 101, -2);
        g.world.set(p, Block::Dirt);
        for slot in &mut g.player.inventory.slots {
            *slot = Some(items::Stack::new(Item::Dirt, 64));
        }
        g.target = Some((p, p + IVec3::Z));
        g.attack(5.);
        assert_eq!(g.world.get(p), Block::Air);
        assert_eq!(g.drops.len(), 1);
        g.collect_drops();
        assert_eq!(g.drops.len(), 1);
        g.player.inventory.slots[0] = None;
        g.collect_drops();
        assert_eq!(g.drops.len(), 1);
        g.player.pos = [0.5, 100., -1.5];
        g.collect_drops();
        assert!(g.drops.is_empty());
        assert_eq!(g.player.inventory.count(Item::Dirt), 35 * 64 + 1);
        assert!(
            g.player
                .discoveries
                .contains(&format!("resource:{}", Item::Dirt.name()))
        );
    }

    #[test]
    fn copper_requires_stone_pick_and_yields_ore() {
        let mut g = sky_fixture();
        let p = IVec3::new(0, 101, -2);
        g.world.set(p, Block::Copper);
        g.player.inventory.selected = 1;
        g.target();
        g.attack(5.);
        assert_eq!(g.world.get(p), Block::Copper);
        assert_eq!(g.player.inventory.count(Item::CopperOre), 0);
        assert!(g.feedback.particles.is_empty());
        assert!(g.feedback.sounds.is_empty());
        g.player.inventory.add(Item::StonePick, 1);
        g.player.inventory.equip(Item::StonePick, 1);
        g.attack(5.);
        assert_eq!(g.world.get(p), Block::Air);
        assert_eq!(g.player.inventory.count(Item::CopperOre), 0);
        assert_eq!(g.drops[0].item, Item::CopperOre);
        assert!(!g.feedback.particles.is_empty());
        assert!(
            g.feedback
                .sounds
                .iter()
                .any(|cue| matches!(cue, crate::feedback::Cue::Break))
        );
    }
    #[test]
    fn stations_gate_complete_copper_progression() {
        let mut g = sky_fixture();
        g.player.inventory.add(Item::Log, 10);
        for _ in 0..10 {
            g.craft(0);
        }
        g.craft(1);
        g.craft(1);
        g.craft(2);
        g.player.inventory.add(Item::Cobble, 14);
        g.craft(8);
        assert_eq!(g.player.inventory.count(Item::StonePick), 0);
        g.world.set(IVec3::new(2, 100, 0), Block::Table);
        g.player.inventory.take(Item::Table, 1);
        g.craft(8);
        g.craft(7);
        g.craft(9);
        assert_eq!(g.player.inventory.count(Item::StonePick), 1);
        assert_eq!(g.player.inventory.count(Item::Furnace), 1);
        g.player.inventory.add(Item::CopperOre, 3);
        g.player.inventory.add(Item::Coal, 3);
        g.craft(10);
        assert_eq!(g.player.inventory.count(Item::CopperIngot), 0);
        g.world.set(IVec3::new(-2, 100, 0), Block::Furnace);
        g.player.inventory.take(Item::Furnace, 1);
        g.furnaces.entry([-2, 100, 0]).or_default().furnace = [
            Some(items::Stack::new(Item::CopperOre, 3)),
            Some(items::Stack::new(Item::Coal, 1)),
        ];
        g.player.inventory.take(Item::CopperOre, 3);
        g.player.inventory.take(Item::Coal, 1);
        for _ in 0..3 * items::SMELT_TICKS {
            g.tick_furnace();
        }
        g.craft_grid(2);
        let output = g.player.inventory.cursor.take().unwrap();
        g.player.inventory.add(output.item, output.count);
        g.craft(13);
        assert_eq!(g.player.inventory.count(Item::CopperPick), 1);
        assert_eq!(g.player.inventory.count(Item::CopperOre), 0);
        assert!(g.player.level > 1);
    }
    #[test]
    fn torches_expire_and_sleep_restores_rest() {
        let mut g = sky_fixture();
        let p = IVec3::new(2, 100, 0);
        g.world.set(p, Block::Torch);
        g.torch_life.insert(p.to_array(), 0.01);
        g.tick(&Input::default(), STEP);
        assert_eq!(g.world.get(p), Block::Air);
        assert!(g.torch_life.is_empty());
        g.world.set(p, Block::Bed);
        g.player.rest = 1.;
        g.sleep();
        assert_eq!(g.player.rest, 100.);
    }
    #[test]
    fn magic_stats_do_not_consume_points() {
        let mut g = sky_fixture();
        g.player.points = 2;
        g.allocate(3);
        g.allocate(4);
        assert_eq!(g.player.points, 2);
    }
}

#[cfg(test)]
mod placed_furnace_tests {
    use super::*;
    use items::{Furnace, SMELT_TICKS, Stack};

    #[test]
    fn independent_furnaces_process_away_from_player_and_route_transactions() {
        let mut g = Game::new(9);
        g.player.pos = [0.5, 100., 0.5];
        let a = [-1, 100, 0];
        let b = [1, 100, 0];
        for (p, input) in [(a, Item::CopperOre), (b, Item::Meat)] {
            g.world.set(IVec3::from_array(p), Block::Furnace);
            g.active_furnace = Some(p);
            g.player.inventory.cursor = Some(Stack::new(input, 2));
            g.transfer(45, false);
            g.player.inventory.cursor = Some(Stack::new(Item::Coal, 1));
            g.transfer(46, false);
        }
        assert_eq!(g.player.inventory.processing, Furnace::default());
        g.furnaces.get_mut(&a).unwrap().furnace_output = Some(Stack::new(Item::CopperIngot, 64));
        g.player.pos = [1000., 100., 1000.];
        for _ in 0..SMELT_TICKS {
            g.tick_furnace();
        }
        assert_eq!(g.furnaces[&a].furnace_progress, 0);
        assert_eq!(g.furnaces[&a].furnace[1].unwrap().count, 1);
        assert_eq!(
            g.furnaces[&b].furnace_output,
            Some(Stack::new(Item::CookedMeat, 1))
        );
        assert_eq!(g.player.crafts[10], 0);
        assert_eq!(g.player.crafts[11], 1);
        // A stale station must not silently redirect an interaction elsewhere.
        g.player.inventory.cursor = Some(Stack::new(Item::Coal, 2));
        g.transfer(46, false);
        assert_eq!(g.player.inventory.cursor.unwrap().count, 2);
        g.player.inventory.cursor = None;
        g.player.pos = [0.5, 100., 0.5];
        g.active_furnace = Some(a);
        g.craft_grid(2);
        assert_eq!(
            g.player.inventory.cursor.take(),
            Some(Stack::new(Item::CopperIngot, 64))
        );
        assert!(g.furnaces[&a].furnace_output.is_none());
        g.active_furnace = Some(b);
        g.craft_grid(2);
        assert_eq!(
            g.player.inventory.cursor.take(),
            Some(Stack::new(Item::CookedMeat, 1))
        );
        assert_eq!(g.player.crafts[11], 1); // Collection awards no extra XP.
        for _ in 0..SMELT_TICKS {
            g.tick_furnace();
        }
        assert_eq!(
            g.furnaces[&a].furnace_output,
            Some(Stack::new(Item::CopperIngot, 1))
        );
        assert_eq!(
            g.furnaces[&b].furnace_output,
            Some(Stack::new(Item::CookedMeat, 1))
        );
    }

    #[test]
    fn mining_requires_empty_slots_and_discards_heat_without_duplicating_items() {
        let mut g = Game::new(9);
        g.player.pos = [0.5, 100., 0.5];
        g.player.inventory.equip(Item::WoodPick, 0);
        g.player.inventory.selected = 0;
        let p = IVec3::new(0, 101, -2);
        g.world.set(p, Block::Furnace);
        g.target = Some((p, p + IVec3::Z));
        for slot in 0..3 {
            let f = g.furnaces.entry(p.to_array()).or_default();
            *f = Furnace::default();
            if slot < 2 {
                f.furnace[slot] = Some(Stack::new(Item::Coal, 1));
            } else {
                f.furnace_output = Some(Stack::new(Item::CopperIngot, 1));
            }
            g.attack(10.);
            assert_eq!(g.world.get(p), Block::Furnace);
            assert_eq!(g.player.inventory.count(Item::Furnace), 0);
        }
        let f = g.furnaces.get_mut(&p.to_array()).unwrap();
        *f = Furnace::default();
        f.furnace_joules = 3000;
        g.attack(10.);
        assert_eq!(g.world.get(p), Block::Air);
        assert!(!g.furnaces.contains_key(&p.to_array()));
        assert_eq!(g.player.inventory.count(Item::Furnace), 0);
        assert_eq!(g.drops[0].item, Item::Furnace);
    }

    #[test]
    fn legacy_contents_attach_once_when_placing_a_furnace() {
        let mut g = Game::new(9);
        g.player.pos = [0.5, 100., 0.5];
        for y in 100..=102 {
            for z in -3..=0 {
                g.world.set(IVec3::new(0, y, z), Block::Air);
            }
        }
        let support = IVec3::new(0, 101, -3);
        g.world.set(support, Block::Stone);
        g.player.yaw = 0.;
        g.player.pitch = 0.;
        g.player.inventory.add(Item::Furnace, 2);
        g.player.inventory.equip(Item::Furnace, 0);
        g.player.inventory.selected = 0;
        g.player.inventory.processing.furnace[0] = Some(Stack::new(Item::CopperOre, 3));
        g.player.inventory.processing.furnace_joules = 1234;
        let legacy = g.player.inventory.processing.clone();
        g.use_selected();
        let p = [0, 101, -2];
        assert_eq!(g.world.get(IVec3::from_array(p)), Block::Furnace);
        assert_eq!(g.furnaces[&p], legacy);
        assert_eq!(g.player.inventory.processing, Furnace::default());
        g.use_selected();
        assert_eq!(g.furnaces[&[0, 101, -1]], Furnace::default());
    }
}

#[cfg(test)]
mod expansion_tests {
    use super::*;
    #[test]
    fn iron_rejects_stone_pick_and_cache_requires_guardian_victory() {
        let mut g = Game::new(314159);
        let p = IVec3::new(0, 101, -2);
        g.world.set(p, Block::Iron);
        g.player.inventory.add(Item::StonePick, 1);
        g.player.inventory.equip(Item::StonePick, 0);
        g.player.inventory.selected = 0;
        g.target = Some((p, p + IVec3::Z));
        g.attack(10.);
        assert_eq!(g.world.get(p), Block::Iron);
        assert!(g.drops.is_empty());
        g.player.inventory.add(Item::CopperPick, 1);
        g.player.inventory.equip(Item::CopperPick, 0);
        g.attack(10.);
        assert_eq!(g.world.get(p), Block::Air);
        assert_eq!(g.drops[0].item, Item::IronOre);

        let home = (-8..=8)
            .find_map(|x| (-8..=8).find_map(|z| g.world.ruin_in_cell(x, z)))
            .unwrap();
        g.player.pos = (home.as_vec3() + Vec3::new(0.5, 1.01, 4.5)).to_array();
        g.time = 0.75;
        g.update_mobs(0.);
        g.update_mobs(0.);
        assert_eq!(
            g.entities
                .iter()
                .filter(|(_, m)| matches!(m.kind, MobKind::Guardian(_)))
                .count(),
            1
        );
        g.target = Some((home + IVec3::Y, home + IVec3::Y + IVec3::Z));
        // Look away from the guardian so the test targets mining only.
        g.player.yaw = std::f32::consts::PI;
        g.attack(10.);
        assert_eq!(g.world.get(home + IVec3::Y), Block::Reliquary);
        g.player
            .discoveries
            .insert(format!("guardian:{:?}", home.to_array()));
        g.attack(10.);
        assert_eq!(g.world.get(home + IVec3::Y), Block::Air);
        assert!(
            g.drops
                .iter()
                .any(|d| d.item == Item::IronIngot && d.count == 6)
        );
        let ids: Vec<_> = g.entities.iter().map(|(id, _)| id).collect();
        for id in ids {
            g.entities.despawn(id);
        }
        g.update_mobs(0.);
        assert!(
            !g.entities
                .iter()
                .any(|(_, m)| matches!(m.kind, MobKind::Guardian(_)))
        );
    }
}

#[cfg(test)]
mod fluid_tests {
    use super::*;
    #[test]
    fn all_bucket_materials_scoop_and_pour_water_but_only_iron_carries_lava() {
        for bucket in [Item::WoodBucket, Item::CopperBucket, Item::IronBucket] {
            let mut g = Game::new(41);
            let p = IVec3::new(0, 200, 0);
            g.world.set(p - IVec3::Y, Block::Stone);
            g.world.set(p, Block::Water);
            g.player.pos = [0.5, 201., 0.5];
            g.player.pitch = -std::f32::consts::FRAC_PI_2;
            g.player.inventory = Inventory::empty();
            g.player.inventory.add(bucket, 1);
            g.use_selected();
            assert_eq!(g.world.get(p), Block::Air);
            assert_eq!(
                g.player.inventory.selected().bucket_contents(),
                Some(Block::Water)
            );
            g.use_selected();
            assert_eq!(g.world.get(p), Block::Water);
            assert_eq!(g.player.inventory.selected(), bucket);
            g.world.set(p, Block::Lava);
            g.use_selected();
            if bucket == Item::IronBucket {
                assert_eq!(g.player.inventory.selected(), Item::IronLavaBucket);
                g.use_selected();
                assert_eq!(g.world.get(p), Block::Lava);
                assert_eq!(g.player.inventory.selected(), bucket);
            } else {
                assert_eq!(g.world.get(p), Block::Lava);
                assert_eq!(g.player.inventory.selected(), bucket);
            }
            g.world.set(p, Block::FlowingWater(3));
            g.use_selected();
            assert_eq!(g.player.inventory.selected(), bucket);
            assert_eq!(g.world.get(p), Block::FlowingWater(3));
        }
    }
    #[test]
    fn swimming_climbs_a_clear_bank() {
        let mut g = Game::new(41);
        for x in -2..=2 {
            for z in -3..=3 {
                g.world.set(IVec3::new(x, 198, z), Block::Stone);
                for y in 199..=202 {
                    g.world.set(
                        IVec3::new(x, y, z),
                        if y == 199 {
                            if z < 0 { Block::Stone } else { Block::Water }
                        } else {
                            Block::Air
                        },
                    );
                }
            }
        }
        g.player.pos = [0.5, 199.1, 0.5];
        g.player.yaw = 0.;
        for _ in 0..60 {
            g.tick(
                &Input {
                    forward: 1.,
                    jump: true,
                    ..Default::default()
                },
                STEP,
            );
        }
        assert!(g.player.pos[2] < -0.5, "{:?}", g.player.pos);
        assert!(g.player.pos[1] >= 200., "{:?}", g.player.pos);
        assert!(!collides(&g.world, Vec3::from_array(g.player.pos)));
    }

    #[test]
    fn water_slows_falls_allows_swimming_and_consumes_air_lava_hurts() {
        let mut g = Game::new(41);
        g.player.pos = [0.5, 200.1, 0.5];
        g.world.set(IVec3::new(0, 200, 0), Block::Water);
        g.world.set(IVec3::new(0, 201, 0), Block::Water);
        g.tick(
            &Input {
                jump: true,
                ..Default::default()
            },
            STEP,
        );
        assert!(g.velocity.y > 0.);
        assert!(g.breath < 15.);
        g.breath = 0.;
        g.tick(&Input::default(), STEP);
        assert!(g.player.health < 100.);
        g.world.set(IVec3::new(0, 200, 0), Block::Lava);
        let health = g.player.health;
        g.tick(&Input::default(), STEP);
        assert!(g.player.health < health);
    }
}
