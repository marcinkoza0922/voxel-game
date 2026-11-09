use crate::{
    game::{Game, STAT_NAMES},
    items::{Item, RECIPES, Stack, grid_recipe},
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
    Output,
    Recipe(i32),
    Slot(usize),
    Tab(u8),
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
    pub mode: u8,
    pub recipe: usize,
    pub pointer: (f32, f32),
}
impl Ui {
    pub fn new() -> Self {
        Self {
            vertices: vec![],
            hits: vec![],
            scale: 1.,
            width: 1100.,
            height: 720.,
            mode: 0,
            recipe: 0,
            pointer: (0., 0.),
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
        for n in 0..9 {
            self.stack_slot(
                [start + n as f32 * 53., h - 70., 49., 49.],
                g.player.inventory.slots[n],
                None,
                n == g.player.inventory.selected,
            );
        }
        let selected = g.player.inventory.selected();
        if g.player.inventory.selected_count() > 0 {
            self.text(w / 2. - 110., h - 92., selected.name(), 1.5, INK);
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
    pub fn icon(&mut self, x: f32, y: f32, item: Item, scale: f32) {
        use Item::*;
        let block = matches!(
            item,
            Log | Planks | Dirt | Cobble | CopperOre | Table | Furnace | Bed
        );
        let color: [f32; 4] = match item {
            Log => [0.43, 0.27, 0.12, 1.],
            Planks | Table => [0.73, 0.51, 0.27, 1.],
            Dirt => [0.48, 0.30, 0.19, 1.],
            Cobble | CopperOre | Furnace | StoneAxe | StonePick => [0.57, 0.61, 0.62, 1.],
            CopperIngot | CopperAxe | CopperPick | Sword => [0.84, 0.45, 0.25, 1.],
            Coal => [0.18, 0.20, 0.24, 1.],
            Berries => [0.85, 0.22, 0.34, 1.],
            Meat => [0.84, 0.41, 0.37, 1.],
            CookedMeat => [0.51, 0.25, 0.13, 1.],
            Bed => [0.63, 0.29, 0.24, 1.],
            _ => [0.73, 0.53, 0.29, 1.],
        };
        let tool = item.durability() > 0;
        for py in 0..16 {
            for px in 0_i32..16 {
                let head = if matches!(item, WoodPick | StonePick | CopperPick) {
                    (2..14).contains(&px) && (2..5).contains(&py)
                } else if item == Sword {
                    (7..10).contains(&px) && (1..11).contains(&py)
                } else {
                    (3..10).contains(&px) && (2..7).contains(&py)
                };
                let handle = tool && (7..9).contains(&px) && (5..15).contains(&py);
                let filled = if block {
                    (1..15).contains(&px)
                        && py >= 2 + (px - 8).abs() / 2
                        && py <= 14 - (px - 8).abs() / 2
                } else if tool {
                    head || handle
                } else if matches!(item, Stick | Torch) {
                    (7..10).contains(&px) && (2..15).contains(&py)
                } else if item == CopperIngot {
                    (3..14).contains(&px) && (5..12).contains(&py) && px + py > 9 && px + py < 23
                } else {
                    let dx = px - 8;
                    let dy = py - 8;
                    dx * dx + dy * dy < 39
                };
                if !filled {
                    continue;
                }
                let mut c = if handle {
                    [0.47, 0.30, 0.15, 1.]
                } else {
                    color
                };
                if item == Torch && py < 6 {
                    c = [1., 0.73, 0.20, 1.];
                }
                if matches!(item, Table | Furnace) && (5..11).contains(&px) && (6..11).contains(&py)
                {
                    c = [0.20, 0.19, 0.17, 1.];
                }
                if item == CopperOre && (px + py * 3) % 7 < 2 {
                    c = [0.90, 0.49, 0.26, 1.];
                }
                if item == Planks && py % 4 == 0 {
                    for v in &mut c[..3] {
                        *v *= 0.65;
                    }
                }
                if item == Log && px % 3 == 0 && py > 6 {
                    for v in &mut c[..3] {
                        *v *= 0.65;
                    }
                }
                if item == Berries && py < 5 {
                    c = [0.25, 0.57, 0.24, 1.];
                }
                let shade = if block {
                    if py <= 6 - (px - 8).abs() / 2 {
                        1.20
                    } else if px < 8 {
                        0.85
                    } else {
                        0.65
                    }
                } else if px < 4 || py < 4 {
                    1.16
                } else if px > 11 || py > 11 {
                    0.70
                } else if (px + py * 3) % 5 == 0 {
                    0.87
                } else {
                    1.
                };
                for v in &mut c[..3] {
                    *v = (*v * shade).min(1.);
                }
                self.rect(
                    x + px as f32 * scale,
                    y + py as f32 * scale,
                    scale,
                    scale,
                    c,
                );
            }
        }
    }
    fn stack_slot(
        &mut self,
        r: [f32; 4],
        stack: Option<Stack>,
        action: Option<Action>,
        selected: bool,
    ) {
        self.rect(
            r[0],
            r[1],
            r[2],
            r[3],
            if selected {
                [0.76, 0.69, 0.43, 1.]
            } else {
                [0.37, 0.41, 0.42, 1.]
            },
        );
        self.rect(
            r[0] + 2.,
            r[1] + 2.,
            r[2] - 4.,
            r[3] - 4.,
            [0.10, 0.13, 0.15, 1.],
        );
        if let Some(s) = stack {
            self.icon(r[0] + 5., r[1] + 4., s.item, (r[2] - 10.) / 16.);
            if s.count > 1 {
                let label = s.count.to_string();
                self.text(
                    r[0] + r[2] - label.len() as f32 * 8. - 4.,
                    r[1] + r[3] - 13.,
                    &label,
                    1.3,
                    INK,
                );
            }
            if s.item.durability() > 0 {
                self.rect(
                    r[0] + 5.,
                    r[1] + r[3] - 6.,
                    (r[2] - 10.) * (1. - s.wear as f32 / s.item.durability() as f32),
                    3.,
                    [0.39, 0.79, 0.31, 1.],
                );
            }
        }
        if let Some(a) = action {
            self.hits.push((r, a));
        }
    }
    fn pack(&mut self, g: &Game) {
        let x = (self.width - 1000.) / 2.;
        let y = (self.height - 650.) / 2.;
        let inv = &g.player.inventory;
        self.rect(0., 0., self.width, self.height, [0.015, 0.025, 0.035, 0.82]);
        self.rect(x, y, 1000., 650., PANEL);
        self.text(x + 24., y + 22., "FIELD PACK", 2.5, INK);
        self.button(
            [x + 24., y + 61., 160., 32.],
            "Personal 2x2",
            Action::Tab(0),
            true,
        );
        let (table, furnace) = g.stations();
        self.button(
            [x + 194., y + 61., 160., 32.],
            "Table 3x3",
            Action::Tab(1),
            table,
        );
        self.button(
            [x + 364., y + 61., 160., 32.],
            "Furnace",
            Action::Tab(2),
            furnace,
        );
        let mode = if self.mode == 1 && !table || self.mode == 2 && !furnace {
            0
        } else {
            self.mode
        };
        if mode < 2 {
            let size = if mode == 1 { 3 } else { 2 };
            self.text(x + 24., y + 112., "CRAFTING", 1.6, INK);
            for row in 0..size {
                for col in 0..size {
                    let n = row * 3 + col;
                    self.stack_slot(
                        [
                            x + 24. + col as f32 * 52.,
                            y + 143. + row as f32 * 52.,
                            48.,
                            48.,
                        ],
                        if mode == 0 { inv.hand[n] } else { inv.grid[n] },
                        Some(Action::Slot(if mode == 0 { 47 + n } else { 36 + n })),
                        false,
                    );
                }
            }
            self.text(x + 203., y + 199., ">", 2.5, INK);
            let output = grid_recipe(if mode == 0 { &inv.hand } else { &inv.grid }, size)
                .map(|n| Stack::new(RECIPES[n].output, RECIPES[n].amount));
            self.stack_slot(
                [x + 250., y + 184., 56., 56.],
                output,
                Some(Action::Output),
                true,
            );
            let r = &RECIPES[self.recipe];
            self.text(x + 355., y + 112., "RECIPE GUIDE", 1.3, MUTED);
            self.text(x + 355., y + 136., r.output.name(), 1.2, INK);
            let example = crate::items::recipe_grid(self.recipe);
            for (n, stack) in example.into_iter().enumerate() {
                self.stack_slot(
                    [
                        x + 355. + (n % 3) as f32 * 30.,
                        y + 160. + (n / 3) as f32 * 30.,
                        27.,
                        27.,
                    ],
                    stack,
                    None,
                    false,
                );
            }
            self.icon(x + 465., y + 182., r.output, 2.);
            self.text(x + 463., y + 224., &format!("x{}", r.amount), 1.2, INK);
            self.button(
                [x + 355., y + 258., 60., 28.],
                "<",
                Action::Recipe(-1),
                true,
            );
            self.button([x + 425., y + 258., 60., 28.], ">", Action::Recipe(1), true);
        } else {
            self.text(x + 24., y + 112., "FURNACE", 1.6, INK);
            self.text(x + 24., y + 139., "INPUT", 1.2, MUTED);
            self.text(x + 24., y + 224., "FUEL", 1.2, MUTED);
            self.stack_slot(
                [x + 87., y + 136., 48., 48.],
                inv.furnace[0],
                Some(Action::Slot(45)),
                false,
            );
            self.stack_slot(
                [x + 87., y + 220., 48., 48.],
                inv.furnace[1],
                Some(Action::Slot(46)),
                false,
            );
            self.text(x + 174., y + 191., ">", 2.5, INK);
            let output = crate::items::furnace_recipe(inv)
                .map(|n| Stack::new(RECIPES[n].output, RECIPES[n].amount));
            self.stack_slot(
                [x + 230., y + 179., 56., 56.],
                output,
                Some(Action::Output),
                true,
            );
            self.text(x + 322., y + 185., "Coal + ore or raw meat", 1.2, MUTED);
            self.text(x + 322., y + 209., "Take output to process", 1.2, MUTED);
        }
        self.text(x + 24., y + 313., "INVENTORY", 1.6, INK);
        for row in 0..3 {
            for col in 0..9 {
                let n = 9 + row * 9 + col;
                self.stack_slot(
                    [
                        x + 24. + col as f32 * 57.,
                        y + 342. + row as f32 * 57.,
                        52.,
                        52.,
                    ],
                    inv.slots[n],
                    Some(Action::Slot(n)),
                    false,
                );
            }
        }
        for n in 0..9 {
            self.stack_slot(
                [x + 24. + n as f32 * 57., y + 527., 52., 52.],
                inv.slots[n],
                Some(Action::Slot(n)),
                n == inv.selected,
            );
        }
        self.text(
            x + 24.,
            y + 602.,
            "Drag items | RMB: half stack / place one | E: close",
            1.25,
            MUTED,
        );
        self.text(
            x + 620.,
            y + 111.,
            &format!("ATTRIBUTES  {} points", g.player.points),
            1.5,
            INK,
        );
        for (n, name) in STAT_NAMES.iter().enumerate() {
            self.button(
                [x + 620., y + 144. + n as f32 * 45., 350., 35.],
                &format!(
                    "{} {}{}",
                    name,
                    g.player.stats[n],
                    if n == 3 || n == 4 { " (future)" } else { " +" }
                ),
                Action::Stat(n),
                n != 3 && n != 4 && g.player.points > 0 && g.player.stats[n] < 20,
            );
        }
        if g.message_time > 0. {
            self.text(
                x + 620.,
                y + 500.,
                &g.message.chars().take(38).collect::<String>(),
                1.1,
                MUTED,
            );
        }
        let px = self.pointer.0 / self.scale;
        let py = self.pointer.1 / self.scale;
        if let Some(stack) = inv.cursor {
            self.icon(px - 16., py - 16., stack.item, 2.);
            self.text(px + 14., py + 12., &stack.count.to_string(), 1.4, INK);
        } else if let Some(Action::Slot(n)) = self.click(self.pointer.0, self.pointer.1)
            && let Some(stack) = inv.slot(n)
        {
            let label = stack.item.name();
            let tx = px.min(self.width - 180.);
            let ty = (py + 24.).min(self.height - 30.);
            self.rect(tx, ty, label.len() as f32 * 8. + 16., 25., PANEL);
            self.text(tx + 7., ty + 7., label, 1.3, INK);
        }
    }
}
const PANEL: [f32; 4] = [0.045, 0.067, 0.075, 0.96];
const INK: [f32; 4] = [0.90, 0.91, 0.81, 1.];
const MUTED: [f32; 4] = [0.58, 0.66, 0.64, 1.];
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
        '<' => [1, 2, 4, 8, 4, 2, 1],
        '|' => [4; 7],
        '!' => [4, 4, 4, 4, 4, 0, 4],
        _ => [0; 7],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Block;
    #[test]
    fn scaled_inventory_hit_regions_and_icons_stay_inside_view() {
        let g = Game::new(8);
        for (w, h) in [(1280, 800), (800, 600), (1920, 1080)] {
            let mut ui = Ui::new();
            ui.build(&g, true, false, w, h, 60.);
            let x = (ui.width - 1000.) / 2.;
            let y = (ui.height - 650.) / 2.;
            assert!(matches!(
                ui.click((x + 40.) * ui.scale, (y + 545.) * ui.scale),
                Some(Action::Slot(0))
            ));
            assert!(matches!(
                ui.click((x + 40.) * ui.scale, (y + 360.) * ui.scale),
                Some(Action::Slot(9))
            ));
            assert!(
                ui.vertices
                    .iter()
                    .all(|v| v.pos.iter().all(|p| p.is_finite() && p.abs() <= 1.001))
            );
        }
    }
    #[test]
    fn table_and_furnace_expose_correct_slots_and_output() {
        let mut g = Game::new(9);
        let p = glam::Vec3::from_array(g.player.pos).floor().as_ivec3();
        g.world.set(p + glam::IVec3::X, Block::Table);
        g.world.set(p - glam::IVec3::X, Block::Furnace);
        let mut ui = Ui::new();
        ui.mode = 1;
        ui.build(&g, true, false, 1280, 800, 60.);
        assert_eq!(
            ui.hits
                .iter()
                .filter(|(_, a)| matches!(a, Action::Slot(36..=44)))
                .count(),
            9
        );
        ui.mode = 2;
        ui.build(&g, true, false, 1280, 800, 60.);
        assert!(ui.hits.iter().any(|(_, a)| matches!(a, Action::Slot(45))));
        assert!(ui.hits.iter().any(|(_, a)| matches!(a, Action::Slot(46))));
        assert!(ui.hits.iter().any(|(_, a)| matches!(a, Action::Output)));
    }
}
