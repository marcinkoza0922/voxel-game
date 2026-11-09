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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    pub counts: [u32; 22],
    pub wear: [u32; 22],
    pub hotbar: [Item; 9],
    pub selected: usize,
}
impl Default for Inventory {
    fn default() -> Self {
        let mut s = Self {
            counts: [0; 22],
            wear: [0; 22],
            hotbar: [
                Item::WoodAxe,
                Item::WoodPick,
                Item::Dirt,
                Item::Planks,
                Item::Torch,
                Item::Berries,
                Item::Table,
                Item::Furnace,
                Item::Bed,
            ],
            selected: 0,
        };
        s.add(Item::WoodAxe, 1);
        s.add(Item::WoodPick, 1);
        s.add(Item::Berries, 8);
        s
    }
}
impl Inventory {
    pub fn count(&self, i: Item) -> u32 {
        self.counts[i as usize]
    }
    pub fn add(&mut self, i: Item, n: u32) {
        self.counts[i as usize] = self.count(i).saturating_add(n).min(9999);
    }
    pub fn take(&mut self, i: Item, n: u32) -> bool {
        if self.count(i) < n {
            false
        } else {
            self.counts[i as usize] -= n;
            true
        }
    }
    pub fn selected(&self) -> Item {
        self.hotbar[self.selected.min(8)]
    }
    pub fn damage_tool(&mut self, i: Item) {
        if i.durability() > 0 && self.count(i) > 0 {
            self.wear[i as usize] += 1;
            if self.wear[i as usize] >= i.durability() {
                self.take(i, 1);
                self.wear[i as usize] = 0;
            }
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Station {
    Hand,
    Table,
    Furnace,
}
impl Station {
    pub fn name(self) -> &'static str {
        match self {
            Self::Hand => "Hand",
            Self::Table => "Table",
            Self::Furnace => "Furnace",
        }
    }
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
        station: Station::Hand,
        xp: 12,
    },
    Recipe {
        output: WoodPick,
        amount: 1,
        inputs: &[(Planks, 3), (Stick, 2)],
        station: Station::Hand,
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
    if r.inputs.iter().any(|(i, n)| inv.count(*i) < *n) {
        return Err("Missing ingredients");
    }
    if inv.count(r.output) > 9999 - r.amount {
        return Err("Output stack is full");
    }
    for (i, n) in r.inputs {
        inv.take(*i, *n);
    }
    inv.add(r.output, r.amount);
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
        let before = i.counts;
        assert!(craft(&mut i, 8, false, false).is_err());
        assert_eq!(i.counts, before);
        assert!(craft(&mut i, 8, true, false).is_err());
        assert_eq!(i.counts, before);
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
        for _ in 0..60 {
            i.damage_tool(WoodPick);
        }
        assert_eq!(i.count(WoodPick), 0);
    }
}
