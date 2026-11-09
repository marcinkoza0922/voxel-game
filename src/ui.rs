use crate::{
    game::{Game, STAT_NAMES},
    items::{ITEMS, RECIPES},
};
use bytemuck::{Pod, Zeroable};
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UiVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}
#[derive(Clone, Copy)]
pub enum Action {
    Craft(usize),
    Equip(usize),
    Stat(usize),
    Resume,
    Save,
    Quit,
}
pub struct Ui {
    pub vertices: Vec<UiVertex>,
    pub hits: Vec<([f32; 4], Action)>,
    pub scale: f32,
    pub width: f32,
    pub height: f32,
}
impl Ui {
    pub fn new() -> Self {
        Self {
            vertices: vec![],
            hits: vec![],
            scale: 1.,
            width: 1100.,
            height: 720.,
        }
    }
    pub fn begin(&mut self, width: u32, height: u32) {
        self.vertices.clear();
        self.hits.clear();
        self.scale = (width as f32 / 1100.).min(height as f32 / 720.);
        self.width = width as f32 / self.scale;
        self.height = height as f32 / self.scale;
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let p = |a: f32, b: f32| [a / self.width * 2. - 1., 1. - b / self.height * 2.];
        for pos in [
            p(x, y),
            p(x + w, y),
            p(x + w, y + h),
            p(x, y),
            p(x + w, y + h),
            p(x, y + h),
        ] {
            self.vertices.push(UiVertex { pos, color });
        }
    }
    pub fn text(&mut self, x: f32, y: f32, text: &str, size: f32, color: [f32; 4]) {
        for (n, ch) in text.to_uppercase().chars().enumerate() {
            for (row, bits) in glyph(ch).iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        self.rect(
                            x + n as f32 * 6. * size + col as f32 * size,
                            y + row as f32 * size,
                            size,
                            size,
                            color,
                        );
                    }
                }
            }
        }
    }
    pub fn button(&mut self, r: [f32; 4], label: &str, action: Action, enabled: bool) {
        self.rect(
            r[0],
            r[1],
            r[2],
            r[3],
            if enabled {
                [0.17, 0.25, 0.24, 0.98]
            } else {
                [0.12, 0.14, 0.16, 0.98]
            },
        );
        self.text(
            r[0] + 8.,
            r[1] + 9.,
            label,
            1.4,
            if enabled { INK } else { MUTED },
        );
        self.hits.push((r, action));
    }
    pub fn click(&self, x: f32, y: f32) -> Option<Action> {
        let x = x / self.scale;
        let y = y / self.scale;
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3])
            .map(|(_, a)| *a)
    }
    pub fn build(&mut self, g: &Game, pack: bool, paused: bool, width: u32, height: u32, fps: f32) {
        self.begin(width, height);
        let w = self.width;
        let h = self.height;
        if pack {
            self.pack(g);
            return;
        }
        if paused {
            self.rect(0., 0., w, h, [0.015, 0.025, 0.035, 0.82]);
            let x = w / 2. - 160.;
            let y = h / 2. - 130.;
            self.text(x, y, "VOXEL ADVENTURE", 2.7, INK);
            self.button([x, y + 52., 320., 42.], "Resume", Action::Resume, true);
            self.button([x, y + 104., 320., 42.], "Save world", Action::Save, true);
            self.button(
                [x, y + 156., 320., 42.],
                "Save and quit",
                Action::Quit,
                true,
            );
            self.text(x, y + 220., "Esc returns to the world", 1.4, MUTED);
            if g.message_time > 0. {
                self.text(24., h - 40., &g.message, 1.3, INK);
            }
            return;
        }
        self.rect(20., 20., 260., 86., PANEL);
        self.text(32., 31., g.biome().name(), 1.8, INK);
        self.text(
            32.,
            54.,
            &format!(
                "Day {}   Level {}  XP {}/{}",
                g.time.floor() as u32 + 1,
                g.player.level,
                g.player.xp,
                g.player.threshold()
            ),
            1.2,
            MUTED,
        );
        self.text(
            32.,
            75.,
            &format!(
                "Y {:.0}  {} chunks  {:.0} FPS",
                g.player.pos[1],
                g.world.chunk_count(),
                fps
            ),
            1.2,
            MUTED,
        );
        let bx = 24.;
        let by = h - 170.;
        for (i, (label, value, color)) in [
            (
                "HEALTH",
                g.player.health / g.player.max_health(),
                [0.72, 0.27, 0.25, 1.],
            ),
            ("HUNGER", g.player.hunger / 100., [0.77, 0.57, 0.27, 1.]),
            ("REST", g.player.rest / 100., [0.33, 0.53, 0.69, 1.]),
        ]
        .iter()
        .enumerate()
        {
            let y = by + i as f32 * 25.;
            self.text(bx, y, label, 1.3, INK);
            self.rect(bx + 65., y, 150., 10., [0.03, 0.04, 0.05, 0.8]);
            self.rect(bx + 65., y, 150. * value.clamp(0., 1.), 10., *color);
        }
        let start = w / 2. - 9. * 53. / 2.;
        for (n, item) in g.player.inventory.hotbar.iter().enumerate() {
            let x = start + n as f32 * 53.;
            let selected = n == g.player.inventory.selected;
            self.rect(
                x,
                h - 70.,
                49.,
                49.,
                if selected {
                    [0.55, 0.62, 0.44, 0.98]
                } else {
                    PANEL
                },
            );
            self.text(x + 5., h - 65., &format!("{}", n + 1), 1.1, MUTED);
            self.text(x + 7., h - 48., abbrev(*item), 1.2, INK);
            self.text(
                x + 7.,
                h - 32.,
                &g.player.inventory.count(*item).to_string(),
                1.1,
                INK,
            );
        }
        let selected = g.player.inventory.selected();
        self.text(w / 2. - 150., h - 91., selected.name(), 1.6, INK);
        if selected.durability() > 0 {
            self.text(
                w / 2. + 65.,
                h - 91.,
                &format!(
                    "{}/{}",
                    selected
                        .durability()
                        .saturating_sub(g.player.inventory.wear[selected as usize]),
                    selected.durability()
                ),
                1.3,
                MUTED,
            );
        }
        self.rect(w / 2. - 6., h / 2. - 1., 12., 2., INK);
        self.rect(w / 2. - 1., h / 2. - 6., 2., 12., INK);
        if let Some((p, _)) = g.target {
            self.text(w / 2. - 120., h / 2. + 22., g.world.get(p).name(), 1.5, INK);
            if g.mining_progress > 0. {
                self.rect(w / 2. - 60., h / 2. + 40., 120., 5., PANEL);
                self.rect(
                    w / 2. - 60.,
                    h / 2. + 40.,
                    120. * g.mining_progress.min(1.),
                    5.,
                    [0.8, 0.75, 0.46, 1.],
                );
            }
        }
        if let Some((_, m)) = g.entities.iter().find(|(_, m)| {
            let d = m.pos + glam::Vec3::Y - g.player.eye();
            d.length() < 8. && d.normalize_or_zero().dot(g.player.direction()) > 0.96
        }) {
            self.text(
                w / 2. - 90.,
                h / 2. - 40.,
                &format!("HUSK  LV {}  HP {:.0}", m.level, m.health),
                1.5,
                [0.94, 0.5, 0.4, 1.],
            );
        }
        self.text(
            24.,
            h - 26.,
            "WASD move  Space jump  LMB mine/attack  RMB use  E pack  R sleep  Esc menu",
            1.2,
            MUTED,
        );
        if g.message_time > 0. {
            self.rect(w / 2. - 330., 118., 660., 30., PANEL);
            self.text(w / 2. - 317., 128., &g.message, 1.25, INK);
        }
    }
    fn pack(&mut self, g: &Game) {
        let x = (self.width - 1060.) / 2.;
        let y = (self.height - 650.) / 2.;
        self.rect(0., 0., self.width, self.height, [0.015, 0.025, 0.035, 0.78]);
        self.rect(x, y, 1060., 650., PANEL);
        self.text(x + 24., y + 22., "FIELD PACK", 2.8, INK);
        self.text(
            x + 330.,
            y + 29.,
            "E closes | Click an item to equip in active slot",
            1.4,
            MUTED,
        );
        self.text(x + 24., y + 68., "INVENTORY", 1.8, INK);
        for (n, item) in ITEMS.iter().enumerate() {
            let row = n / 2;
            let col = n % 2;
            let r = [
                x + 24. + col as f32 * 156.,
                y + 99. + row as f32 * 37.,
                149.,
                32.,
            ];
            self.button(
                r,
                &format!("{} {}", abbrev(*item), g.player.inventory.count(*item)),
                Action::Equip(n),
                g.player.inventory.count(*item) > 0,
            );
        }
        self.text(
            x + 24.,
            y + 535.,
            &format!("Active slot: {}", g.player.inventory.selected + 1),
            1.5,
            INK,
        );
        self.text(x + 24., y + 560., "1-9 selects a hotbar slot", 1.2, MUTED);
        self.text(x + 350., y + 68., "CRAFTING", 1.8, INK);
        let (table, furnace) = g.stations();
        for (n, r) in RECIPES.iter().enumerate() {
            let ready = r
                .inputs
                .iter()
                .all(|(i, c)| g.player.inventory.count(*i) >= *c)
                && match r.station {
                    crate::items::Station::Hand => true,
                    crate::items::Station::Table => table,
                    crate::items::Station::Furnace => furnace,
                };
            let yy = y + 99. + n as f32 * 32.;
            self.button(
                [x + 350., yy, 414., 29.],
                &format!("{} x{}", r.output.name(), r.amount),
                Action::Craft(n),
                ready,
            );
            let ingredients = r
                .inputs
                .iter()
                .map(|(i, n)| format!("{} {}", n, abbrev(*i)))
                .collect::<Vec<_>>()
                .join(" + ");
            self.text(
                x + 563.,
                yy + 10.,
                &format!("[{}] {}", r.station.name(), ingredients),
                0.9,
                MUTED,
            );
        }
        self.text(
            x + 350.,
            y + 593.,
            "Place stations within 3 blocks to use them.",
            1.2,
            MUTED,
        );
        self.text(x + 790., y + 68., "ATTRIBUTES", 1.8, INK);
        self.text(
            x + 790.,
            y + 100.,
            &format!("Level {} | {} points", g.player.level, g.player.points),
            1.4,
            MUTED,
        );
        for (n, name) in STAT_NAMES.iter().enumerate() {
            self.button(
                [x + 790., y + 140. + n as f32 * 44., 246., 35.],
                &if n == 3 || n == 4 {
                    format!("{} (future)", name)
                } else {
                    format!("{} {}  +", name, g.player.stats[n])
                },
                Action::Stat(n),
                n != 3 && n != 4 && g.player.points > 0 && g.player.stats[n] < 20,
            );
        }
        self.text(x + 790., y + 475., "Each stat is capped at 20.", 1.2, MUTED);
        self.text(x + 790., y + 500., "XP: discoveries, crafting,", 1.2, MUTED);
        self.text(x + 790., y + 518., "and challenging enemies.", 1.2, MUTED);
        for (row, line) in [
            "VIG: health | STR: damage",
            "DEX: attack speed",
            "AGI: speed, jump, falls",
            "ING: bonus crafting yield",
        ]
        .iter()
        .enumerate()
        {
            self.text(x + 790., y + 550. + row as f32 * 15., line, 1.1, MUTED);
        }
        self.text(
            x + 24.,
            y + 620.,
            if g.message_time > 0. {
                &g.message
            } else {
                "Tip: logs > planks > table > stone tools > furnace > copper"
            },
            1.3,
            INK,
        );
    }
}
const PANEL: [f32; 4] = [0.045, 0.067, 0.075, 0.96];
const INK: [f32; 4] = [0.90, 0.91, 0.81, 1.];
const MUTED: [f32; 4] = [0.58, 0.66, 0.64, 1.];
fn abbrev(i: crate::items::Item) -> &'static str {
    use crate::items::Item::*;
    match i {
        Log => "LOG",
        Planks => "PLANK",
        Stick => "STICK",
        Dirt => "DIRT",
        Cobble => "STONE",
        CopperOre => "C.ORE",
        Coal => "COAL",
        CopperIngot => "COPPER",
        Table => "TABLE",
        Furnace => "FURN.",
        Torch => "TORCH",
        Berries => "BERRY",
        Meat => "MEAT",
        CookedMeat => "COOKED",
        Bed => "BED",
        WoodAxe => "W.AXE",
        WoodPick => "W.PICK",
        StoneAxe => "S.AXE",
        StonePick => "S.PICK",
        CopperAxe => "C.AXE",
        CopperPick => "C.PICK",
        Sword => "SWORD",
    }
}
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '.' => [0, 0, 0, 0, 0, 6, 6],
        ',' => [0, 0, 0, 0, 6, 6, 4],
        ':' => [0, 6, 6, 0, 6, 6, 0],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '[' => [14, 8, 8, 8, 8, 8, 14],
        ']' => [14, 2, 2, 2, 2, 2, 14],
        '(' => [2, 4, 8, 8, 8, 4, 2],
        ')' => [8, 4, 2, 2, 2, 4, 8],
        '>' => [16, 8, 4, 2, 4, 8, 16],
        '|' => [4; 7],
        '!' => [4, 4, 4, 4, 4, 0, 4],
        _ => [0; 7],
    }
}
