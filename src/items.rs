use crate::world::Block;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(usize)]
pub enum Item {
    Log,
    Planks,
    Stick,
    Dirt,
    Cobble,
    CopperOre,
    Coal,
    CopperIngot,
    Table,
    Furnace,
    Torch,
    Berries,
    Meat,
    CookedMeat,
    Bed,
    WoodAxe,
    WoodPick,
    StoneAxe,
    StonePick,
    CopperAxe,
    CopperPick,
    Sword,
}
pub const ITEMS: [Item; 22] = [
    Item::Log,
    Item::Planks,
    Item::Stick,
    Item::Dirt,
    Item::Cobble,
    Item::CopperOre,
    Item::Coal,
    Item::CopperIngot,
    Item::Table,
    Item::Furnace,
    Item::Torch,
    Item::Berries,
    Item::Meat,
    Item::CookedMeat,
    Item::Bed,
    Item::WoodAxe,
    Item::WoodPick,
    Item::StoneAxe,
    Item::StonePick,
    Item::CopperAxe,
    Item::CopperPick,
    Item::Sword,
];
impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Self::Log => "Oak log",
            Self::Planks => "Oak planks",
            Self::Stick => "Sticks",
            Self::Dirt => "Dirt",
            Self::Cobble => "Granite cobble",
            Self::CopperOre => "Copper ore",
            Self::Coal => "Coal",
            Self::CopperIngot => "Copper ingot",
            Self::Table => "Crafting table",
            Self::Furnace => "Furnace",
            Self::Torch => "Torch",
            Self::Berries => "Berries",
            Self::Meat => "Raw meat",
            Self::CookedMeat => "Cooked meat",
            Self::Bed => "Bedroll",
            Self::WoodAxe => "Wood axe",
            Self::WoodPick => "Wood pickaxe",
            Self::StoneAxe => "Stone axe",
            Self::StonePick => "Stone pickaxe",
            Self::CopperAxe => "Copper axe",
            Self::CopperPick => "Copper pickaxe",
            Self::Sword => "Copper sword",
        }
    }
    pub fn block(self) -> Option<Block> {
        Some(match self {
            Self::Log => Block::Log,
            Self::Planks => Block::Planks,
            Self::Dirt => Block::Dirt,
            Self::Cobble => Block::Cobble,
            Self::Table => Block::Table,
            Self::Furnace => Block::Furnace,
            Self::Torch => Block::Torch,
            Self::Bed => Block::Bed,
            _ => return None,
        })
    }
    pub fn durability(self) -> u32 {
        match self {
            Self::WoodAxe | Self::WoodPick => 60,
            Self::StoneAxe | Self::StonePick => 160,
            Self::CopperAxe | Self::CopperPick | Self::Sword => 360,
            _ => 0,
        }
    }
    pub fn pick(self) -> u32 {
        match self {
            Self::WoodPick => 1,
            Self::StonePick => 2,
            Self::CopperPick => 3,
            _ => 0,
        }
    }
    pub fn axe(self) -> u32 {
        match self {
            Self::WoodAxe => 1,
            Self::StoneAxe => 2,
            Self::CopperAxe => 3,
            _ => 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stack {
    pub item: Item,
    pub count: u32,
    pub wear: u32,
}
impl Stack {
    pub fn new(item: Item, count: u32) -> Self {
        Self {
            item,
            count,
            wear: 0,
        }
    }
    pub fn limit(item: Item) -> u32 {
        if item.durability() > 0 { 1 } else { 64 }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Furnace {
    pub furnace: [Option<Stack>; 2],
    #[serde(default)]
    pub furnace_joules: u32,
    #[serde(default)]
    pub furnace_output: Option<Stack>,
    #[serde(default)]
    pub furnace_progress: u32,
    #[serde(default)]
    pub furnace_input: Option<Item>,
}
impl Furnace {
    pub fn valid(&self) -> bool {
        self.furnace_joules <= COAL_JOULES
            && self.furnace_progress < SMELT_TICKS
            && (self.furnace_progress == 0
                || matches!(self.furnace_input, Some(Item::CopperOre | Item::Meat)))
            && self
                .furnace_output
                .is_none_or(|s| matches!(s.item, Item::CopperIngot | Item::CookedMeat))
            && self
                .furnace
                .iter()
                .chain(std::iter::once(&self.furnace_output))
                .flatten()
                .all(valid_stack)
    }
}
fn valid_stack(s: &Stack) -> bool {
    s.count > 0 && s.count <= Stack::limit(s.item) && s.wear < s.item.durability().max(1)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<Option<Stack>>,
    pub selected: usize,
    pub grid: [Option<Stack>; 9],
    #[serde(default)]
    pub hand: [Option<Stack>; 9],
    #[serde(flatten)]
    pub processing: Furnace,
    pub cursor: Option<Stack>,
}
impl Default for Inventory {
    fn default() -> Self {
        let mut s = Self::empty();
        s.add(Item::WoodAxe, 1);
        s.add(Item::WoodPick, 1);
        s.add(Item::Berries, 8);
        s
    }
}
impl Inventory {
    pub fn empty() -> Self {
        Self {
            slots: vec![None; 36],
            selected: 0,
            grid: [None; 9],
            hand: [None; 9],
            processing: Furnace::default(),
            cursor: None,
        }
    }
    pub fn count(&self, i: Item) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.item == i)
            .map(|s| s.count)
            .sum()
    }
    pub fn add(&mut self, i: Item, n: u32) -> bool {
        let mut trial = self.clone();
        let mut left = n;
        for s in trial
            .slots
            .iter_mut()
            .flatten()
            .filter(|s| s.item == i && s.wear == 0)
        {
            let amount = left.min(Stack::limit(i) - s.count);
            s.count += amount;
            left -= amount;
        }
        for slot in trial.slots.iter_mut().filter(|s| s.is_none()) {
            if left == 0 {
                break;
            }
            let amount = left.min(Stack::limit(i));
            *slot = Some(Stack::new(i, amount));
            left -= amount;
        }
        if left > 0 {
            return false;
        }
        *self = trial;
        true
    }
    pub fn take(&mut self, i: Item, n: u32) -> bool {
        if self.count(i) < n {
            return false;
        }
        let mut left = n;
        for slot in &mut self.slots {
            if let Some(s) = slot
                && s.item == i
            {
                let amount = left.min(s.count);
                s.count -= amount;
                left -= amount;
                if s.count == 0 {
                    *slot = None;
                }
            }
        }
        true
    }
    pub fn selected(&self) -> Item {
        self.slots[self.selected].map_or(Item::Dirt, |s| s.item)
    }
    pub fn selected_count(&self) -> u32 {
        self.slots[self.selected].map_or(0, |s| s.count)
    }
    pub fn take_selected(&mut self) {
        let slot = &mut self.slots[self.selected];
        if let Some(s) = slot {
            s.count -= 1;
            if s.count == 0 {
                *slot = None;
            }
        }
    }
    pub fn equip(&mut self, i: Item, slot: usize) {
        if let Some(n) = self
            .slots
            .iter()
            .position(|s| s.is_some_and(|s| s.item == i))
        {
            self.slots.swap(n, slot);
        }
    }
    pub fn damage_tool(&mut self, i: Item) {
        if let Some(s) = &mut self.slots[self.selected]
            && s.item == i
            && i.durability() > 0
        {
            s.wear += 1;
            if s.wear >= i.durability() {
                self.slots[self.selected] = None;
            }
        }
    }
    pub fn slot(&self, n: usize) -> Option<Stack> {
        match n {
            0..=35 => self.slots[n],
            36..=44 => self.grid[n - 36],
            45..=46 => self.processing.furnace[n - 45],
            47..=55 => self.hand[n - 47],
            _ => None,
        }
    }
    fn set_slot(&mut self, n: usize, s: Option<Stack>) {
        match n {
            0..=35 => self.slots[n] = s,
            36..=44 => self.grid[n - 36] = s,
            45..=46 => self.processing.furnace[n - 45] = s,
            47..=55 => self.hand[n - 47] = s,
            _ => (),
        }
    }
    pub fn transfer(&mut self, n: usize, right: bool) {
        let mut target = self.slot(n);
        match (&mut self.cursor, &mut target) {
            (None, Some(s)) => {
                let amount = if right { s.count.div_ceil(2) } else { s.count };
                self.cursor = Some(Stack {
                    count: amount,
                    ..*s
                });
                s.count -= amount;
                if s.count == 0 {
                    target = None;
                }
            }
            (Some(c), None) => {
                let amount = if right { 1 } else { c.count };
                target = Some(Stack {
                    count: amount,
                    ..*c
                });
                c.count -= amount;
                if c.count == 0 {
                    self.cursor = None;
                }
            }
            (Some(c), Some(t)) if c.item == t.item && c.wear == t.wear => {
                let amount = (Stack::limit(t.item) - t.count).min(if right { 1 } else { c.count });
                t.count += amount;
                c.count -= amount;
                if c.count == 0 {
                    self.cursor = None;
                }
            }
            (Some(_), Some(_)) if !right => std::mem::swap(&mut self.cursor, &mut target),
            _ => (),
        }
        self.set_slot(n, target);
    }
    /// Move as much as fits, preserving wear and leaving the carried stack alone.
    fn move_stack(&mut self, mut stack: Stack, destinations: &[usize]) -> Option<Stack> {
        for empty in [false, true] {
            for &n in destinations {
                let target = self.slot(n);
                let capacity = match target {
                    Some(t) if !empty && t.item == stack.item && t.wear == stack.wear => {
                        Stack::limit(t.item) - t.count
                    }
                    None if empty => Stack::limit(stack.item),
                    _ => 0,
                };
                let amount = capacity.min(stack.count);
                if amount > 0 {
                    self.set_slot(
                        n,
                        Some(Stack {
                            count: target.map_or(0, |s| s.count) + amount,
                            ..stack
                        }),
                    );
                    stack.count -= amount;
                    if stack.count == 0 {
                        return None;
                    }
                }
            }
        }
        Some(stack)
    }
    pub fn quick_transfer(&mut self, n: usize, mode: u8) {
        let Some(stack) = self.slot(n) else {
            return;
        };
        let destinations: Vec<usize> = if n < 36 {
            match mode {
                2 => match stack.item {
                    Item::Coal => vec![46],
                    Item::CopperOre | Item::Meat => vec![45],
                    _ => vec![],
                },
                1 => (36..45).collect(),
                _ => vec![47, 48, 50, 51],
            }
        } else {
            (0..36).collect()
        };
        let remaining = self.move_stack(stack, &destinations);
        self.set_slot(n, remaining);
    }
    /// Take one crafting result (or available furnace output) directly into storage.
    pub fn quick_output(&mut self, mode: u8) -> Option<usize> {
        let mut trial = self.clone();
        trial.cursor = None;
        let index = take_output(&mut trial, mode)?;
        let stack = trial.cursor.take()?;
        if trial
            .move_stack(stack, &(0..36).collect::<Vec<_>>())
            .is_some()
        {
            return None;
        }
        trial.cursor = self.cursor;
        *self = trial;
        Some(index)
    }
    pub fn valid(&self) -> bool {
        self.processing.valid()
            && self.selected < 9
            && self.slots.len() == 36
            && [2, 5, 6, 7, 8].into_iter().all(|n| self.hand[n].is_none())
            && self
                .slots
                .iter()
                .chain(self.grid.iter())
                .chain(self.hand.iter())
                .chain(std::iter::once(&self.cursor))
                .flatten()
                .all(valid_stack)
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Station {
    Hand,
    Table,
    Furnace,
}
pub struct Recipe {
    pub output: Item,
    pub amount: u32,
    pub inputs: &'static [(Item, u32)],
    pub station: Station,
    pub xp: u32,
}
use Item::*;
pub const RECIPES: &[Recipe] = &[
    Recipe {
        output: Planks,
        amount: 4,
        inputs: &[(Log, 1)],
        station: Station::Hand,
        xp: 12,
    },
    Recipe {
        output: Stick,
        amount: 4,
        inputs: &[(Planks, 2)],
        station: Station::Hand,
        xp: 8,
    },
    Recipe {
        output: Table,
        amount: 1,
        inputs: &[(Planks, 4)],
        station: Station::Hand,
        xp: 30,
    },
    Recipe {
        output: WoodAxe,
        amount: 1,
        inputs: &[(Planks, 3), (Stick, 2)],
        station: Station::Table,
        xp: 12,
    },
    Recipe {
        output: WoodPick,
        amount: 1,
        inputs: &[(Planks, 3), (Stick, 2)],
        station: Station::Table,
        xp: 12,
    },
    Recipe {
        output: Torch,
        amount: 4,
        inputs: &[(Coal, 1), (Stick, 1)],
        station: Station::Hand,
        xp: 12,
    },
    Recipe {
        output: Bed,
        amount: 1,
        inputs: &[(Planks, 6), (Log, 2)],
        station: Station::Table,
        xp: 40,
    },
    Recipe {
        output: StoneAxe,
        amount: 1,
        inputs: &[(Cobble, 3), (Stick, 2)],
        station: Station::Table,
        xp: 25,
    },
    Recipe {
        output: StonePick,
        amount: 1,
        inputs: &[(Cobble, 3), (Stick, 2)],
        station: Station::Table,
        xp: 25,
    },
    Recipe {
        output: Furnace,
        amount: 1,
        inputs: &[(Cobble, 8)],
        station: Station::Table,
        xp: 45,
    },
    Recipe {
        output: CopperIngot,
        amount: 1,
        inputs: &[(CopperOre, 1), (Coal, 1)],
        station: Station::Furnace,
        xp: 24,
    },
    Recipe {
        output: CookedMeat,
        amount: 1,
        inputs: &[(Meat, 1), (Coal, 1)],
        station: Station::Furnace,
        xp: 10,
    },
    Recipe {
        output: CopperAxe,
        amount: 1,
        inputs: &[(CopperIngot, 3), (Stick, 2)],
        station: Station::Table,
        xp: 50,
    },
    Recipe {
        output: CopperPick,
        amount: 1,
        inputs: &[(CopperIngot, 3), (Stick, 2)],
        station: Station::Table,
        xp: 50,
    },
    Recipe {
        output: Sword,
        amount: 1,
        inputs: &[(CopperIngot, 2), (Stick, 1)],
        station: Station::Table,
        xp: 50,
    },
];
pub fn craft(
    inv: &mut Inventory,
    index: usize,
    table: bool,
    furnace: bool,
) -> Result<(), &'static str> {
    let r = RECIPES.get(index).ok_or("Unknown recipe")?;
    if (r.station == Station::Table && !table) || (r.station == Station::Furnace && !furnace) {
        return Err("Place the required station nearby");
    }
    if r.station == Station::Furnace {
        return Err("Put input and fuel in the furnace to smelt");
    }
    if r.inputs.iter().any(|(i, n)| inv.count(*i) < *n) {
        return Err("Missing ingredients");
    }
    let mut trial = inv.clone();
    for (i, n) in r.inputs {
        trial.take(*i, *n);
    }
    if !trial.add(r.output, r.amount) {
        return Err("Inventory full");
    }
    *inv = trial;
    Ok(())
}
pub fn craft_xp(base: u32, count: u32) -> u32 {
    if count >= 16 { 0 } else { base >> (count * 2) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_craft_is_atomic() {
        let mut i = Inventory::default();
        i.add(Cobble, 3);
        let before = i.clone();
        assert!(craft(&mut i, 8, false, false).is_err());
        assert_eq!(i, before);
        assert!(craft(&mut i, 8, true, false).is_err());
        assert_eq!(i, before);
    }
    #[test]
    fn progression_recipe() {
        let mut i = Inventory::default();
        i.add(Log, 1);
        craft(&mut i, 0, false, false).unwrap();
        assert_eq!(i.count(Planks), 4);
        assert_eq!(i.count(Log), 0);
    }
    #[test]
    fn quarter_xp_reaches_zero() {
        assert_eq!(
            (0..5).map(|n| craft_xp(40, n)).collect::<Vec<_>>(),
            vec![40, 10, 2, 0, 0]
        );
        assert_eq!(craft_xp(40, u32::MAX), 0);
    }
    #[test]
    fn tool_breaks() {
        let mut i = Inventory::default();
        i.selected = 1;
        for _ in 0..60 {
            i.damage_tool(WoodPick);
        }
        assert_eq!(i.count(WoodPick), 0);
    }
}

/// Compact row-major patterns; spaces are required empty cells.
pub fn pattern(index: usize) -> (&'static str, usize) {
    match index {
        0 => ("L", 1),
        1 => ("PP", 1),
        2 => ("PPPP", 2),
        3 | 7 | 12 => ("MM MS  S ", 3),
        4 | 8 | 13 => ("MMM S  S ", 3),
        5 => ("CS", 1),
        6 => ("PPPPPPL L", 3),
        9 => ("MMMM MMMM", 3),
        14 => ("MMS", 1),
        _ => ("", 1),
    }
}
fn ingredient(index: usize, c: char) -> Option<Item> {
    match c {
        'L' => Some(Item::Log),
        'P' => Some(Item::Planks),
        'S' => Some(Item::Stick),
        'C' => Some(Item::Coal),
        'M' => Some(match index {
            3 | 4 => Item::Planks,
            7..=9 => Item::Cobble,
            _ => Item::CopperIngot,
        }),
        _ => None,
    }
}
pub fn grid_recipe(grid: &[Option<Stack>; 9], size: usize) -> Option<usize> {
    (0..RECIPES.len()).find(|&index| {
        let (p, w) = pattern(index);
        if p.is_empty() {
            return false;
        }
        let h = p.len() / w;
        if w > size || h > size {
            return false;
        }
        (0..=size - w).any(|ox| {
            (0..=size - h).any(|oy| {
                (0..2).any(|mirror| {
                    (0..9).all(|n| {
                        let x = n % 3;
                        let y = n / 3;
                        let expected = if x >= ox && x < ox + w && y >= oy && y < oy + h {
                            let px = if mirror == 1 {
                                w - 1 - (x - ox)
                            } else {
                                x - ox
                            };
                            ingredient(index, p.as_bytes()[(y - oy) * w + px] as char)
                        } else {
                            None
                        };
                        grid[n].map(|s| s.item) == expected
                    })
                })
            })
        })
    })
}
pub const COAL_JOULES: u32 = 4_000;
pub const SMELT_JOULES: u32 = 1_000;
pub const SMELT_TICKS: u32 = 5 * 60;

pub fn furnace_recipe(inv: &Furnace) -> Option<usize> {
    match inv.furnace[0]?.item {
        Item::CopperOre => Some(10),
        Item::Meat => Some(11),
        _ => None,
    }
}
/// Advance one 60 Hz simulation step. Energy drains only while work advances.
pub fn tick_furnace(inv: &mut Furnace) -> Option<usize> {
    let input = inv.furnace[0].map(|s| s.item);
    if inv.furnace_input != input {
        inv.furnace_progress = 0;
        inv.furnace_input = input;
    }
    let index = furnace_recipe(inv)?;
    let r = &RECIPES[index];
    if inv
        .furnace_output
        .is_some_and(|s| s.item != r.output || s.count + r.amount > Stack::limit(s.item))
    {
        return None;
    }
    let next = inv.furnace_progress + 1;
    let cost =
        next * SMELT_JOULES / SMELT_TICKS - inv.furnace_progress * SMELT_JOULES / SMELT_TICKS;
    if inv.furnace_joules < cost {
        let fuel = inv.furnace[1].as_mut()?;
        if fuel.item != Item::Coal {
            return None;
        }
        fuel.count -= 1;
        if fuel.count == 0 {
            inv.furnace[1] = None;
        }
        inv.furnace_joules += COAL_JOULES;
    }
    inv.furnace_joules -= cost;
    inv.furnace_progress = next;
    if next < SMELT_TICKS {
        return None;
    }
    let input = inv.furnace[0].as_mut().expect("valid recipe has input");
    input.count -= 1;
    if input.count == 0 {
        inv.furnace[0] = None;
    }
    if let Some(output) = &mut inv.furnace_output {
        output.count += r.amount;
    } else {
        inv.furnace_output = Some(Stack::new(r.output, r.amount));
    }
    inv.furnace_progress = 0;
    Some(index)
}
pub fn take_output(inv: &mut Inventory, mode: u8) -> Option<usize> {
    let index = if mode == 2 {
        match inv.processing.furnace_output?.item {
            Item::CopperIngot => 10,
            Item::CookedMeat => 11,
            _ => return None,
        }
    } else {
        grid_recipe(
            if mode == 0 { &inv.hand } else { &inv.grid },
            if mode == 1 { 3 } else { 2 },
        )?
    };
    let r = &RECIPES[index];
    let amount = if mode == 2 {
        inv.processing.furnace_output?.count
    } else {
        r.amount
    };
    if let Some(c) = inv.cursor
        && (c.item != r.output || c.wear != 0 || c.count + amount > Stack::limit(c.item))
    {
        return None;
    }
    if let Some(c) = &mut inv.cursor {
        c.count += amount;
    } else {
        inv.cursor = Some(Stack::new(r.output, amount));
    }
    if mode == 2 {
        inv.processing.furnace_output = None;
        return Some(index);
    }
    let slots: &mut [Option<Stack>] = if mode == 0 {
        &mut inv.hand
    } else {
        &mut inv.grid
    };
    for slot in slots {
        if let Some(s) = slot {
            s.count -= 1;
            if s.count == 0 {
                *slot = None;
            }
        }
    }
    Some(index)
}
pub fn recipe_grid(index: usize) -> [Option<Stack>; 9] {
    let (p, w) = pattern(index);
    let mut grid = [None; 9];
    for (n, c) in p.chars().enumerate() {
        grid[n / w * 3 + n % w] = ingredient(index, c).map(|i| Stack::new(i, 1));
    }
    grid
}
#[cfg(test)]
mod slot_tests {
    use super::*;
    #[test]
    fn quick_moves_respect_capacity_routes_and_wear() {
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(Stack::new(Item::Coal, 10));
        inv.processing.furnace[1] = Some(Stack::new(Item::Coal, 60));
        inv.quick_transfer(0, 2);
        assert_eq!(inv.slots[0].unwrap().count, 6);
        assert_eq!(inv.processing.furnace[1].unwrap().count, 64);
        inv.slots[1] = Some(Stack::new(Item::CopperOre, 8));
        inv.quick_transfer(1, 2);
        assert!(inv.slots[1].is_none());
        assert_eq!(inv.processing.furnace[0].unwrap().count, 8);
        inv.slots[2] = Some(Stack::new(Item::CopperIngot, 3));
        inv.quick_transfer(2, 2);
        assert_eq!(inv.slots[2].unwrap().count, 3);
        inv.slots[3] = Some(Stack {
            wear: 2,
            ..Stack::new(Item::WoodPick, 1)
        });
        inv.quick_transfer(3, 0);
        assert_eq!(inv.hand[0].unwrap().wear, 2);
        inv.quick_transfer(47, 0);
        assert_eq!(inv.slots[1].unwrap().wear, 2);
        assert!(inv.valid());
    }
    #[test]
    fn quick_output_is_atomic_and_preserves_cursor() {
        let mut inv = Inventory::empty();
        inv.processing.furnace_output = Some(Stack::new(Item::CopperIngot, 8));
        inv.cursor = Some(Stack::new(Item::Coal, 2));
        inv.slots.fill(Some(Stack::new(Item::Coal, 64)));
        let before = inv.clone();
        assert!(inv.quick_output(2).is_none());
        assert_eq!(inv, before);
        inv.slots[4] = None;
        assert!(inv.quick_output(2).is_some());
        assert_eq!(inv.slots[4], Some(Stack::new(Item::CopperIngot, 8)));
        assert_eq!(inv.cursor, before.cursor);
        assert!(inv.processing.furnace_output.is_none());
        inv.hand = recipe_grid(0);
        inv.slots[5] = None;
        assert!(inv.quick_output(0).is_some());
        assert!(inv.hand.iter().all(Option::is_none));
    }
    #[test]
    fn patterns_match_declared_ingredients_and_outputs() {
        for (index, r) in RECIPES
            .iter()
            .enumerate()
            .filter(|(_, r)| r.station != Station::Furnace)
        {
            let grid = recipe_grid(index);
            for &(item, count) in r.inputs {
                assert_eq!(
                    grid.iter()
                        .flatten()
                        .filter(|s| s.item == item)
                        .map(|s| s.count)
                        .sum::<u32>(),
                    count,
                    "recipe {index}"
                );
            }
            assert_eq!(grid_recipe(&grid, 3), Some(index));
            let mut inv = Inventory::empty();
            inv.grid = grid;
            assert_eq!(take_output(&mut inv, 1), Some(index));
            assert_eq!(inv.cursor, Some(Stack::new(r.output, r.amount)));
            assert!(inv.grid.iter().all(Option::is_none));
        }
    }
    #[test]
    fn matching_supports_offsets_and_mirrored_axes_but_rejects_extra_items() {
        let mut g = [None; 9];
        g[4] = Some(Stack::new(Item::Log, 3));
        assert_eq!(grid_recipe(&g, 2), Some(0));
        g[8] = Some(Stack::new(Item::Coal, 1));
        assert_eq!(grid_recipe(&g, 2), None);
        let original = recipe_grid(7);
        let mut mirrored = [None; 9];
        for n in 0..9 {
            mirrored[n / 3 * 3 + 2 - n % 3] = original[n];
        }
        assert_eq!(grid_recipe(&mirrored, 3), Some(7));
        assert_eq!(grid_recipe(&mirrored, 2), None);
    }
    #[test]
    fn split_merge_swap_preserve_counts_and_tool_wear() {
        let mut inv = Inventory::empty();
        inv.slots[9] = Some(Stack::new(Item::Log, 9));
        inv.transfer(9, true);
        assert_eq!(inv.cursor.unwrap().count, 5);
        assert_eq!(inv.slots[9].unwrap().count, 4);
        inv.transfer(36, true);
        assert_eq!(inv.grid[0].unwrap().count, 1);
        inv.transfer(9, false);
        assert_eq!(inv.slots[9].unwrap().count, 8);
        assert!(inv.cursor.is_none());
        inv.slots[10] = Some(Stack {
            item: Item::StonePick,
            count: 1,
            wear: 37,
        });
        inv.transfer(10, false);
        inv.transfer(9, false);
        assert_eq!(inv.slots[9].unwrap().wear, 37);
        assert_eq!(inv.cursor.unwrap().item, Item::Log);
        assert!(inv.valid());
    }
    #[test]
    fn full_inventory_and_output_are_atomic() {
        let mut inv = Inventory::empty();
        inv.slots.fill(Some(Stack::new(Item::Cobble, 64)));
        let before = inv.clone();
        assert!(!inv.add(Item::Log, 1));
        assert_eq!(inv, before);
        inv.hand = recipe_grid(0);
        inv.cursor = Some(Stack::new(Item::Planks, 64));
        let before = inv.clone();
        assert_eq!(take_output(&mut inv, 0), None);
        assert_eq!(inv, before);
        inv.cursor = Some(Stack::new(Item::Coal, 1));
        let before = inv.clone();
        assert_eq!(take_output(&mut inv, 0), None);
        assert_eq!(inv, before);
    }
    #[test]
    fn timed_furnace_uses_one_coal_for_four_ingots() {
        let mut inv = Inventory::empty();
        inv.processing.furnace = [
            Some(Stack::new(Item::CopperOre, 5)),
            Some(Stack::new(Item::Coal, 1)),
        ];
        assert_eq!(take_output(&mut inv, 2), None);
        assert_eq!(inv.processing.furnace[1].unwrap().count, 1);
        for count in 1..=4 {
            for _ in 0..SMELT_TICKS - 1 {
                assert_eq!(tick_furnace(&mut inv.processing), None);
            }
            assert_eq!(
                inv.processing.furnace_output.map(|s| s.count).unwrap_or(0),
                count - 1
            );
            assert_eq!(tick_furnace(&mut inv.processing), Some(10));
            assert_eq!(inv.processing.furnace_joules, (4 - count) * SMELT_JOULES);
        }
        assert_eq!(
            inv.processing.furnace_output,
            Some(Stack::new(Item::CopperIngot, 4))
        );
        let before = inv.clone();
        for _ in 0..SMELT_TICKS {
            assert_eq!(tick_furnace(&mut inv.processing), None);
        }
        assert_eq!(inv, before);
        assert_eq!(take_output(&mut inv, 2), Some(10));
        assert_eq!(inv.cursor, Some(Stack::new(Item::CopperIngot, 4)));
        assert_eq!(inv.processing.furnace_output, None);
    }
    #[test]
    fn furnace_pauses_for_full_output_and_preserves_collection_failures() {
        let mut inv = Inventory::empty();
        inv.processing.furnace = [
            Some(Stack::new(Item::CopperOre, 2)),
            Some(Stack::new(Item::Coal, 1)),
        ];
        for _ in 0..SMELT_TICKS / 2 {
            tick_furnace(&mut inv.processing);
        }
        assert!(inv.processing.furnace_joules > 3_000 && inv.processing.furnace_joules < 4_000);
        inv.processing.furnace_output = Some(Stack::new(Item::CopperIngot, 64));
        let before = inv.clone();
        for _ in 0..SMELT_TICKS {
            tick_furnace(&mut inv.processing);
        }
        assert_eq!(inv, before);
        inv.cursor = Some(Stack::new(Item::Log, 1));
        assert_eq!(take_output(&mut inv, 2), None);
        inv.cursor = None;
        assert_eq!(take_output(&mut inv, 2), Some(10));
        for _ in 0..SMELT_TICKS / 2 {
            tick_furnace(&mut inv.processing);
        }
        assert_eq!(
            inv.processing.furnace_output,
            Some(Stack::new(Item::CopperIngot, 1))
        );
        assert_eq!(inv.processing.furnace_joules, 3_000);
    }
    #[test]
    fn idle_furnace_and_changed_inputs_do_not_waste_fuel_or_transfer_progress() {
        let mut inv = Inventory::empty();
        inv.processing.furnace[1] = Some(Stack::new(Item::Coal, 1));
        let before = inv.clone();
        for _ in 0..SMELT_TICKS {
            tick_furnace(&mut inv.processing);
        }
        assert_eq!(inv, before);
        inv.processing.furnace[0] = Some(Stack::new(Item::CopperOre, 1));
        for _ in 0..SMELT_TICKS / 2 {
            tick_furnace(&mut inv.processing);
        }
        let energy = inv.processing.furnace_joules;
        inv.processing.furnace[0] = Some(Stack::new(Item::Meat, 1));
        tick_furnace(&mut inv.processing);
        assert_eq!(inv.processing.furnace_progress, 1);
        assert!(inv.processing.furnace_joules <= energy);
        for _ in 1..SMELT_TICKS {
            tick_furnace(&mut inv.processing);
        }
        assert_eq!(
            inv.processing.furnace_output,
            Some(Stack::new(Item::CookedMeat, 1))
        );
        let energy = inv.processing.furnace_joules;
        for _ in 0..SMELT_TICKS {
            tick_furnace(&mut inv.processing);
        }
        assert_eq!(inv.processing.furnace_joules, energy);
    }
    #[test]
    fn personal_and_table_inputs_are_independent() {
        let mut inv = Inventory::empty();
        inv.hand = recipe_grid(0);
        inv.grid = recipe_grid(8);
        let table = inv.grid;
        assert_eq!(take_output(&mut inv, 0), Some(0));
        assert_eq!(inv.grid, table);
        assert!(inv.hand.iter().all(Option::is_none));
        inv.cursor = None;
        assert_eq!(take_output(&mut inv, 1), Some(8));
        assert_eq!(inv.cursor.unwrap().item, Item::StonePick);
        inv.hand[2] = Some(Stack::new(Item::Log, 1));
        assert!(!inv.valid());
    }
    #[test]
    fn tools_have_independent_wear() {
        let mut inv = Inventory::empty();
        assert!(inv.add(Item::WoodPick, 2));
        inv.damage_tool(Item::WoodPick);
        assert_eq!(inv.slots[0].unwrap().wear, 1);
        assert_eq!(inv.slots[1].unwrap().wear, 0);
    }
}
