mod art;
mod audio;
mod ecs;
#[path = "../tools/expedition.rs"]
mod expedition;
mod feedback;
mod fluid;
#[path = "../tools/frontier_review.rs"]
mod frontier_review;
mod game;
#[path = "../tools/gpu_profile.rs"]
mod gpu_profile;
#[path = "../tools/headless.rs"]
mod headless;
mod hydrology;
mod items;
#[path = "../tools/performance.rs"]
mod performance;
#[path = "../tools/playtest.rs"]
mod playtest;
mod render;
mod save;
mod settings;
mod smaa;
#[path = "../tools/still_review.rs"]
mod still_review;
mod streaming;
#[path = "../tools/stress.rs"]
mod stress;
mod temporal;
mod ui;
#[path = "../tools/water_review.rs"]
mod water_review;
mod world;

use game::{Game, Input, STEP};
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

const DEFAULT_SEED: u64 = 314159;

struct Options {
    stress: bool,
    frontier_review: bool,
    still_review: bool,
    water_review: bool,
    headless: bool,
    title: bool,
    settings_path: PathBuf,
    seed: u64,
    new: bool,
    biome_test: Option<world::Biome>,
    save: PathBuf,
    smoke: bool,
    expedition: bool,
    playtest: bool,
    tour: bool,
    mute: bool,
    frames: Option<u32>,
    capture: Option<PathBuf>,
    pack: bool,
    profile: Option<PathBuf>,
}
fn options() -> Result<Options, String> {
    let mut o = Options {
        stress: false,
        frontier_review: false,
        still_review: false,
        water_review: false,
        headless: false,
        title: false,
        settings_path: PathBuf::from("saves/settings.json"),
        seed: DEFAULT_SEED,
        new: false,
        biome_test: None,
        save: PathBuf::from("saves/world.json"),
        smoke: false,
        expedition: false,
        playtest: false,
        tour: false,
        mute: false,
        frames: None,
        capture: None,
        pack: false,
        profile: None,
    };
    let mut explicit_save = false;
    let mut explicit_seed = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--still-review" => o.still_review = true,
            "--stress" => o.stress = true,
            "--water-review" => o.water_review = true,
            "--headless" => o.headless = true,
            "--title" => o.title = true,
            "--settings" => {
                o.settings_path = PathBuf::from(args.next().ok_or("--settings needs a path")?)
            }
            "--new" => o.new = true,
            "--biome-test" => {
                let name = args.next().ok_or("--biome-test needs a biome name")?;
                o.biome_test = Some(world::Biome::from_spawn_name(&name).ok_or_else(|| {
                    format!(
                        "Unknown biome {name:?}. Choose from {}",
                        world::Biome::SPAWN_NAMES
                    )
                })?);
                o.new = true;
            }
            "--mute" => o.mute = true,
            "--seed" => {
                explicit_seed = true;
                o.seed = args
                    .next()
                    .ok_or("--seed needs an integer")?
                    .parse()
                    .map_err(|_| "Invalid seed")?
            }
            "--frontier-review" => o.frontier_review = true,
            "--save" => {
                o.save = PathBuf::from(args.next().ok_or("--save needs a path")?);
                explicit_save = true;
            }
            "--profile" => {
                o.profile = Some(PathBuf::from(
                    args.next().ok_or("--profile needs a CSV path")?,
                ))
            }
            "--tour" => o.tour = true,
            "--playtest" => o.playtest = true,
            "--expedition" => o.expedition = true,
            "--smoke" => o.smoke = true,
            "--capture" => {
                o.capture = Some(PathBuf::from(
                    args.next().ok_or("--capture needs a PPM path")?,
                ))
            }
            "--pack" => o.pack = true,
            "--frames" => {
                o.frames = Some(
                    args.next()
                        .ok_or("--frames needs a count")?
                        .parse()
                        .map_err(|_| "Invalid frame count")?,
                )
            }
            "--help" | "-h" => {
                println!(
                    "Voxel Adventure\n  --still-review frozen temporal captures; --save is read only, --capture is output directory\n  --frontier-review benchmark mining invalidation without writing saves\n  --water-review render a natural waterfall to target/water-review (isolated save/settings)\n  --stress       CPU stress suite; --frames sets repetitions (default 12), --profile writes samples\n  --headless     render offscreen without display/audio (default 240 frames)\n  --new          create a separate fresh save with a random seed\n  --seed N       seed for a new world (default 314159)\n  --save PATH    world file (default saves/world.json)\n  --smoke        headless gameplay/persistence check; uses a temporary save\n  --frames N     render N frames then save and exit\n  --capture PATH save final frame as PPM (defaults to 240 frames)\n  --tour         scripted camera relocations for streaming measurements\n  --playtest     run headless mechanics course and progression expedition\n  --expedition   run controlled headless progression expedition\n  --profile PATH write frame timings and chunk backlog as CSV\n  --mute         disable sound playback\n  --pack         start with the inventory open\n  --title        show title screen (also supports headless previews)\n  --settings PATH graphics settings JSON (default saves/settings.json)\n\nWASD move; mouse look; Space jump; double-tap W sprint; Shift walk\nLMB mine/attack; RMB place/eat/use; wheel or 1-9 select\nE inventory/crafting/stats; R sleep near bed; F5 save; Esc menu"
                );
                println!(
                    "\n--biome-test BIOME starts a fresh test world in that biome (seed 314159 unless specified).\nBiomes: {}",
                    world::Biome::SPAWN_NAMES
                );
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    if o.frames == Some(0) {
        return Err("--frames must be positive".into());
    }
    if (o.capture.is_some() || o.headless) && o.frames.is_none() {
        o.frames = Some(240);
    }
    if o.tour && (!explicit_save || o.frames.is_none() || o.save.exists()) {
        return Err(
            "--tour requires --frames and an explicit --save path that does not already exist"
                .into(),
        );
    }
    if o.biome_test.is_some()
        && (o.title
            || o.tour
            || o.stress
            || o.frontier_review
            || o.still_review
            || o.water_review
            || o.smoke
            || o.expedition
            || o.playtest)
    {
        return Err("--biome-test starts directly in a new world and cannot be combined with title, tour, or scripted tests".into());
    }
    if o.new && !explicit_seed && o.biome_test.is_none() {
        o.seed = fresh_seed();
    }
    Ok(o)
}
fn fresh_seed() -> u64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    use std::hash::{BuildHasher, Hasher};
    hasher.write_u32(std::process::id());
    hasher.finish()
}
struct App {
    game: Game,
    active: bool,
    screen: ui::Screen,
    settings_back: ui::Screen,
    settings: settings::Settings,
    notice: String,
    next_frame: Instant,
    options: Options,
    window: Option<Arc<Window>>,
    renderer: Option<render::Renderer>,
    audio: Option<audio::Audio>,
    keys: HashSet<KeyCode>,
    attack: bool,
    forward_tap: Option<Instant>,
    sprinting: bool,
    pack: bool,
    paused: bool,
    focused: bool,
    cursor: (f32, f32),
    drag_origin: Option<usize>,
    last: Instant,
    accumulator: f32,
    autosave: f32,
    fps: f32,
    frames: u32,
    error: Option<String>,
    profile: Option<performance::Profile>,
}
impl App {
    fn capture(&mut self) {
        if let Some(w) = &self.window {
            let grab =
                self.screen == ui::Screen::Game && !self.pack && !self.paused && self.focused;
            if grab {
                if w.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                    let _ = w.set_cursor_grab(CursorGrabMode::Confined);
                }
            } else {
                let _ = w.set_cursor_grab(CursorGrabMode::None);
            }
            w.set_cursor_visible(!grab);
        }
        self.drag_origin = None;
        self.attack = false;
        self.keys.clear();
        self.forward_tap = None;
        self.sprinting = false;
    }
    fn save(&mut self) -> bool {
        if !self.active {
            return true;
        }
        match save::write(&self.game, &self.options.save) {
            Ok(()) => {
                self.game.say("World saved");
                true
            }
            Err(e) => {
                eprintln!("Save failed: {e}");
                self.game.say(format!("Save failed: {e}"));
                false
            }
        }
    }
    fn new_world(&mut self, biome: Option<world::Biome>) {
        if !self.save() {
            return;
        }
        let mut game = if let Some(biome) = biome {
            match Game::new_in_biome(DEFAULT_SEED, biome) {
                Ok(game) => game,
                Err(e) => {
                    self.notice = e;
                    return;
                }
            }
        } else {
            Game::new(fresh_seed())
        };
        match save::create(&game, &self.options.save) {
            Ok(path) => {
                game.say(format!("New world: {}", path.display()));
                self.game = game;
                self.notice.clear();
                self.active = true;
                self.screen = ui::Screen::Game;
                self.options.save = path;
                if let Some(r) = &mut self.renderer {
                    r.reset_world();
                }
                if let Some(w) = &self.window {
                    w.set_title(&format!(
                        "Voxel Adventure - {}",
                        self.options.save.display()
                    ));
                }
                self.pack = false;
                self.paused = false;
                self.accumulator = 0.;
                self.autosave = 0.;
                self.last = Instant::now();
                self.capture();
            }
            Err(e) => {
                self.notice = format!("New world failed: {e}");
                self.game.say(self.notice.clone());
            }
        }
    }
    fn apply_settings(&mut self) {
        if let Some(w) = &self.window {
            w.set_fullscreen(
                self.settings
                    .fullscreen
                    .then(|| winit::window::Fullscreen::Borderless(w.current_monitor())),
            );
        }
        if let Some(r) = &mut self.renderer {
            r.apply_settings(&self.settings);
        }
        self.next_frame = Instant::now();
    }
    fn exit(&mut self, event_loop: &ActiveEventLoop) {
        if !self.save() {
            self.paused = true;
            self.capture();
            return;
        }
        // Native GPU resources must release while winit's Wayland connection lives.
        if let Some(renderer) = self.renderer.take() {
            renderer.finish();
        }
        self.window.take();
        event_loop.exit();
    }
    fn action(&mut self, a: ui::Action, event_loop: &ActiveEventLoop) {
        match a {
            ui::Action::Settings => {
                self.settings_back = self.screen;
                self.screen = ui::Screen::Settings;
                self.capture();
            }
            ui::Action::Back => {
                if self.screen == ui::Screen::BiomeTest
                    && let Some(r) = &mut self.renderer
                {
                    r.ui.focus = 0;
                    r.ui.hits.clear();
                }
                self.screen = if self.screen == ui::Screen::Settings {
                    self.settings_back
                } else {
                    ui::Screen::Title
                };
                self.capture();
            }
            ui::Action::Title => {
                if self.save() {
                    self.active = false;
                    self.screen = ui::Screen::Title;
                    if let Some(w) = &self.window {
                        w.set_title("Voxel Adventure");
                    }
                    self.pack = false;
                    self.capture();
                }
            }
            ui::Action::Setting(n) => {
                self.notice.clear();
                self.settings.change(n);
                self.apply_settings();
                if let Err(e) = self.settings.write(&self.options.settings_path) {
                    self.notice = format!("Settings save failed: {e}");
                }
            }
            ui::Action::Page(delta) => {
                if let Some(r) = &mut self.renderer {
                    r.ui.page = (r.ui.page as i32 + delta)
                        .clamp(0, (r.ui.saves.len().saturating_sub(1) / 6) as i32)
                        as usize;
                    r.ui.focus = 0;
                }
            }
            ui::Action::LoadMenu => {
                self.screen = ui::Screen::Load;
                if let Some(r) = &mut self.renderer {
                    r.ui.saves = saved_worlds(&self.options.save, &self.options.settings_path);
                    r.ui.page = 0;
                    r.ui.focus = 0;
                }
            }
            ui::Action::Load(i) => {
                let path = self
                    .renderer
                    .as_ref()
                    .and_then(|r| r.ui.saves.get(i))
                    .cloned();
                if let Some(path) = path {
                    match save::read(&path) {
                        Ok(Some(game)) => {
                            self.game = game;
                            self.options.save = path;
                            if let Some(w) = &self.window {
                                w.set_title(&format!(
                                    "Voxel Adventure - {}",
                                    self.options.save.display()
                                ));
                            }
                            self.active = true;
                            self.screen = ui::Screen::Game;
                            self.paused = false;
                            self.pack = false;
                            self.accumulator = 0.;
                            self.autosave = 0.;
                            if let Some(r) = &mut self.renderer {
                                r.reset_world();
                            }
                            self.last = Instant::now();
                            self.notice.clear();
                            self.capture();
                        }
                        Ok(None) => self.notice = "Save file no longer exists".into(),
                        Err(e) => self.notice = format!("Load failed: {e}"),
                    }
                }
            }
            ui::Action::Recipe(delta) => {
                if let Some(r) = &mut self.renderer {
                    loop {
                        r.ui.recipe = (r.ui.recipe as i32 + delta)
                            .rem_euclid(items::RECIPES.len() as i32)
                            as usize;
                        if !matches!(
                            items::RECIPES[r.ui.recipe].station,
                            items::Station::Furnace | items::Station::Smelter
                        ) {
                            break;
                        }
                    }
                }
            }
            ui::Action::Output => {
                let mut mode = self.renderer.as_ref().map_or(0, |r| r.ui.mode);
                let (table, furnace) = self.game.stations();
                if mode == 1 && !table || mode == 2 && !furnace {
                    mode = 0;
                }
                let empty = self.game.player.inventory.cursor.is_none();
                if self.keys.contains(&KeyCode::ShiftLeft)
                    || self.keys.contains(&KeyCode::ShiftRight)
                {
                    self.drag_origin = None;
                    self.game.quick_output(mode);
                    return;
                }
                self.game.craft_grid(mode);
                if empty && self.game.player.inventory.cursor.is_some() {
                    self.drag_origin = Some(usize::MAX);
                }
            }
            ui::Action::Slot(n) => self.game.transfer(n, false),
            ui::Action::Tab(mode) => {
                let (table, furnace) = self.game.stations();
                if ((mode == 0) || (mode == 1 && table) || (mode == 2 && furnace))
                    && let Some(r) = &mut self.renderer
                {
                    r.ui.mode = mode;
                }
            }
            ui::Action::Stat(n) => self.game.allocate(n),
            ui::Action::Resume => {
                self.paused = false;
                self.capture();
            }
            ui::Action::Save => {
                self.save();
            }
            ui::Action::NewWorld => self.new_world(None),
            ui::Action::BiomeMenu => {
                self.screen = ui::Screen::BiomeTest;
                self.notice.clear();
                if let Some(r) = &mut self.renderer {
                    r.ui.focus = 0;
                    r.ui.hits.clear();
                }
                self.capture();
            }
            ui::Action::StartBiome(biome) => self.new_world(Some(biome)),
            ui::Action::Quit => self.exit(event_loop),
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title(if self.active {
                                format!("Voxel Adventure - {}", self.options.save.display())
                            } else {
                                "Voxel Adventure".into()
                            })
                            .with_inner_size(winit::dpi::LogicalSize::new(1280., 800.))
                            .with_min_inner_size(winit::dpi::LogicalSize::new(900., 600.)),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let mut renderer = pollster::block_on(render::Renderer::new(window.clone()))?;
            if let Some(profile) = &self.profile {
                renderer.enable_gpu_profile(profile.gpu_sender());
            }
            Ok::<_, String>((window, renderer))
        })();
        match result {
            Ok((window, renderer)) => {
                self.window = Some(window);
                self.renderer = Some(renderer);
                self.apply_settings();
                self.capture();
                self.last = Instant::now();
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }
    fn device_event(&mut self, _: &ActiveEventLoop, _: winit::event::DeviceId, event: DeviceEvent) {
        if self.screen == ui::Screen::Game
            && !self.pack
            && !self.paused
            && self.focused
            && let DeviceEvent::MouseMotion { delta } = event
        {
            self.game.player.yaw =
                (self.game.player.yaw + delta.0 as f32 * 0.0025).rem_euclid(std::f32::consts::TAU);
            self.game.player.pitch =
                (self.game.player.pitch - delta.1 as f32 * 0.0025).clamp(-1.55, 1.55);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.exit(event_loop),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::Focused(f) => {
                self.focused = f;
                if !f {
                    self.paused = true;
                }
                self.capture();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
                if let Some(r) = &mut self.renderer {
                    r.ui.pointer = self.cursor;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if code == KeyCode::KeyW {
                        if event.state == ElementState::Pressed
                            && !event.repeat
                            && !self.keys.contains(&code)
                            && !self.pack
                            && !self.paused
                            && self.focused
                        {
                            let now = Instant::now();
                            self.sprinting = self
                                .forward_tap
                                .is_some_and(|last| now.duration_since(last).as_secs_f32() <= 0.3);
                            self.forward_tap = Some(now);
                        } else if event.state == ElementState::Released {
                            self.sprinting = false;
                        }
                    }
                    if event.state == ElementState::Pressed {
                        self.keys.insert(code);
                    } else {
                        self.keys.remove(&code);
                    }
                    if event.state == ElementState::Pressed && !event.repeat {
                        if self.screen != ui::Screen::Game {
                            if code == KeyCode::Escape {
                                self.action(ui::Action::Back, event_loop);
                            }
                            if let Some(r) = &mut self.renderer {
                                let count = r.ui.hits.len();
                                if count > 0 {
                                    if code == KeyCode::ArrowDown {
                                        r.ui.focus = (r.ui.focus + 1) % count;
                                    }
                                    if code == KeyCode::ArrowUp {
                                        r.ui.focus = (r.ui.focus + count - 1) % count;
                                    }
                                    if code == KeyCode::Enter || code == KeyCode::Space {
                                        let a = r.ui.hits[r.ui.focus.min(count - 1)].1;
                                        self.action(a, event_loop);
                                    }
                                }
                            }
                            if code == KeyCode::F11 {
                                self.settings.fullscreen = !self.settings.fullscreen;
                                self.apply_settings();
                                if let Err(e) = self.settings.write(&self.options.settings_path) {
                                    self.notice = e;
                                }
                            }
                            return;
                        }
                        match code {
                            KeyCode::Escape => {
                                if self.pack {
                                    self.pack = false;
                                } else {
                                    self.paused = !self.paused;
                                }
                                self.capture();
                            }
                            KeyCode::KeyE => {
                                if !self.paused {
                                    self.pack = !self.pack;
                                    self.game.active_furnace = None;
                                    if self.pack
                                        && let Some(r) = &mut self.renderer
                                    {
                                        r.ui.mode = 0;
                                    }
                                    self.capture();
                                }
                            }
                            KeyCode::KeyR if !self.pack && !self.paused => self.game.sleep(),
                            KeyCode::F11 => {
                                self.settings.fullscreen = !self.settings.fullscreen;
                                self.apply_settings();
                                if let Err(e) = self.settings.write(&self.options.settings_path) {
                                    self.notice = e;
                                }
                            }
                            KeyCode::F6 if !self.pack && !self.paused => {
                                self.game.toggle_god_mode();
                            }
                            KeyCode::F5 => {
                                self.save();
                            }
                            KeyCode::Digit1 => self.game.player.inventory.selected = 0,
                            KeyCode::Digit2 => self.game.player.inventory.selected = 1,
                            KeyCode::Digit3 => self.game.player.inventory.selected = 2,
                            KeyCode::Digit4 => self.game.player.inventory.selected = 3,
                            KeyCode::Digit5 => self.game.player.inventory.selected = 4,
                            KeyCode::Digit6 => self.game.player.inventory.selected = 5,
                            KeyCode::Digit7 => self.game.player.inventory.selected = 6,
                            KeyCode::Digit8 => self.game.player.inventory.selected = 7,
                            KeyCode::Digit9 => self.game.player.inventory.selected = 8,
                            _ => (),
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. }
                if self.screen == ui::Screen::Game && !self.pack && !self.paused =>
            {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                if y != 0. {
                    self.game.player.inventory.selected =
                        (self.game.player.inventory.selected as i32 - if y > 0. { 1 } else { -1 })
                            .rem_euclid(9) as usize;
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.screen != ui::Screen::Game || self.pack || self.paused {
                    if state == ElementState::Released
                        && button == MouseButton::Left
                        && let Some(origin) = self.drag_origin.take()
                        && let Some(ui::Action::Slot(n)) = self
                            .renderer
                            .as_ref()
                            .and_then(|r| r.ui.click(self.cursor.0, self.cursor.1))
                        && n != origin
                    {
                        self.game.transfer(n, false);
                    }

                    if state == ElementState::Pressed
                        && matches!(button, MouseButton::Left | MouseButton::Right)
                        && let Some(a) = self
                            .renderer
                            .as_ref()
                            .and_then(|r| r.ui.click(self.cursor.0, self.cursor.1))
                    {
                        if let ui::Action::Slot(n) = a {
                            if button == MouseButton::Left
                                && (self.keys.contains(&KeyCode::ShiftLeft)
                                    || self.keys.contains(&KeyCode::ShiftRight))
                            {
                                self.drag_origin = None;
                                let mode = self.renderer.as_ref().map_or(0, |r| r.ui.mode);
                                self.game.quick_transfer(n, mode);
                                return;
                            }
                            self.drag_origin = if self.game.player.inventory.cursor.is_none()
                                && button == MouseButton::Left
                            {
                                Some(n)
                            } else {
                                None
                            };
                            self.game.transfer(n, button == MouseButton::Right);
                        } else {
                            self.action(a, event_loop);
                        }
                    }
                } else if self.focused {
                    match button {
                        MouseButton::Left => self.attack = state == ElementState::Pressed,
                        MouseButton::Right if state == ElementState::Pressed => {
                            self.game.target();
                            let station =
                                self.game
                                    .target
                                    .and_then(|(p, _)| match self.game.world.get(p) {
                                        world::Block::Table => Some(1),
                                        world::Block::Furnace | world::Block::Smelter => Some(2),
                                        _ => None,
                                    });
                            if let Some(mode) = station {
                                if mode == 2 {
                                    self.game.active_furnace =
                                        self.game.target.map(|(p, _)| p.to_array());
                                }
                                self.pack = true;
                                if let Some(r) = &mut self.renderer {
                                    r.ui.mode = mode;
                                }
                                self.capture();
                            } else {
                                self.game.use_selected();
                            }
                        }
                        _ => (),
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                if settings::CAPS[self.settings.cap] > 0 && now < self.next_frame {
                    return;
                }
                let interval = now.duration_since(self.last);
                let dt = interval.as_secs_f32().min(0.1);
                self.last = now;
                self.fps = self.fps * 0.95 + 0.05 / dt.max(0.001);
                if self.screen == ui::Screen::Game && self.focused && !self.paused {
                    self.accumulator += dt;
                    let pressed = |key| self.keys.contains(&key);
                    let input = Input {
                        forward: pressed(KeyCode::KeyW) as i32 as f32
                            - pressed(KeyCode::KeyS) as i32 as f32,
                        right: pressed(KeyCode::KeyD) as i32 as f32
                            - pressed(KeyCode::KeyA) as i32 as f32,
                        jump: pressed(KeyCode::Space),
                        sprint: self.sprinting && pressed(KeyCode::KeyW),
                        walk: pressed(KeyCode::ShiftLeft) || pressed(KeyCode::ShiftRight),
                        attack: self.attack,
                    };
                    while self.accumulator >= STEP {
                        if self.pack {
                            self.game.tick_furnace();
                        } else {
                            self.game.tick(&input, STEP);
                        }
                        self.accumulator -= STEP;
                    }
                    self.autosave += dt;
                    if self.autosave >= 30. {
                        self.autosave = 0.;
                        self.save();
                    }
                } else {
                    self.accumulator = 0.;
                    self.game.target();
                }
                if self.options.tour {
                    // Diagnostic relocations, intentionally independent of input/focus.
                    // Adjacent chunks exercise replanning; distant views exercise eviction.
                    if self.frames.is_multiple_of(180) {
                        let stage = (self.frames / 180) % 10;
                        let (x, z) = match stage {
                            0..=5 => (stage as i32 * 16, 0),
                            6 => (240, 240),
                            7 => (1024, 1024),
                            8 => (1040, 1024),
                            _ => (0, 0),
                        };
                        self.game.player.pos = [
                            x as f32 + 0.5,
                            self.game.world.height(x, z) as f32 + 3.,
                            z as f32 + 0.5,
                        ];
                        self.game.velocity = glam::Vec3::ZERO;
                    }
                    self.game.player.yaw += 0.003;
                }
                for cue in self.game.feedback.sounds.drain(..) {
                    if self.settings.sound
                        && let Some(audio) = &self.audio
                    {
                        audio.play(cue);
                    }
                }
                let simulation_ms = now.elapsed().as_secs_f64() * 1000.;
                let stream_start = Instant::now();
                if let Some(r) = &mut self.renderer {
                    r.ui.screen = self.screen;
                    r.ui.settings = self.settings.clone();
                    r.ui.notice = self.notice.clone();
                    r.update_world(
                        &mut self.game.world,
                        glam::Vec3::from_array(self.game.player.pos),
                    );
                    let stream_ms = stream_start.elapsed().as_secs_f64() * 1000.;
                    r.profile_draw = self.profile.is_some();
                    r.profile_frame = self.frames;
                    let draw_start = Instant::now();
                    if let Err(e) = r.draw(
                        &self.game,
                        self.pack,
                        self.paused,
                        self.fps,
                        if self.options.frames.is_some_and(|n| self.frames + 1 >= n) {
                            self.options.capture.as_deref()
                        } else {
                            None
                        },
                    ) {
                        self.error = Some(e);
                        self.exit(event_loop);
                        return;
                    }
                    if let Some(profile) = &mut self.profile {
                        let times = [
                            interval.as_secs_f64() * 1000.,
                            simulation_ms,
                            stream_ms,
                            draw_start.elapsed().as_secs_f64() * 1000.,
                        ];
                        if let Err(e) = profile.record(
                            self.frames,
                            times,
                            self.game.world.stream_counts(),
                            self.focused,
                            self.pack || self.paused,
                            r.draw_times,
                        ) {
                            self.error = Some(format!("Profile write failed: {e}"));
                            self.exit(event_loop);
                            return;
                        }
                    }
                }
                let cap = settings::CAPS[self.settings.cap];
                if cap > 0 {
                    self.next_frame = now + std::time::Duration::from_secs_f64(1. / cap as f64);
                }
                self.frames += 1;
                if self.options.frames.is_some_and(|n| self.frames >= n) {
                    self.exit(event_loop);
                }
            }
            _ => (),
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let capped = settings::CAPS[self.settings.cap] > 0;
        let now = Instant::now();
        if !capped || now >= self.next_frame {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Wait);
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        }
    }
}
fn smoke() -> Result<(), String> {
    let mut g = Game::new(314159);
    for _ in 0..180 {
        g.tick(&Input::default(), STEP);
    }
    if !g.grounded {
        return Err("Player did not land".into());
    }
    // Exercise actual mining against controlled nearby blocks, not inventory-only shortcuts.
    let p = glam::Vec3::from_array(g.player.pos).floor().as_ivec3() + glam::IVec3::new(0, 1, -2);
    g.world.set(p, world::Block::Log);
    g.player.yaw = 0.;
    g.player.pitch = 0.;
    g.player.inventory.selected = 0;
    for _ in 0..150 {
        g.tick(
            &Input {
                attack: true,
                ..Default::default()
            },
            STEP,
        );
    }
    if let Some(drop) = g.drops.first() {
        g.player.pos = (glam::Vec3::from_array(drop.pos) - glam::Vec3::Y).to_array();
    }
    g.collect_drops();
    if g.player.inventory.count(items::Item::Log) == 0 {
        return Err("Mining did not yield a log".into());
    }
    g.craft(0);
    g.craft(2);
    if g.player.inventory.count(items::Item::Table) != 1 {
        return Err("Crafting progression failed".into());
    }
    let path = std::env::temp_dir().join(format!("voxel-smoke-{}.json", std::process::id()));
    save::write(&g, &path)?;
    let loaded = save::read(&path)?.ok_or("Save not created")?;
    if loaded.world.edits != g.world.edits || loaded.player.inventory != g.player.inventory {
        return Err("Save roundtrip mismatch".into());
    }
    std::fs::remove_file(path).map_err(|e| e.to_string())?;
    println!("Smoke passed: landing, mining, crafting, XP, save/load.");
    Ok(())
}
fn saved_worlds(current: &std::path::Path, settings: &std::path::Path) -> Vec<PathBuf> {
    let parent = current
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut paths: Vec<_> = std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p != settings
                && p.extension().is_some_and(|e| e == "json")
                && p.file_name().is_none_or(|n| n != "settings.json")
                && p.is_file()
        })
        .collect();
    paths.sort();
    if current.is_file() {
        paths.retain(|p| p != current);
        paths.insert(0, current.into());
    }
    paths
}

fn run() -> Result<(), String> {
    let mut options = options()?;
    if options.still_review {
        return still_review::run(&options);
    }
    if options.frontier_review {
        return frontier_review::run(&options);
    }
    if options.water_review {
        return water_review::run(options);
    }
    if options.stress {
        return stress::run(
            options.seed,
            options.frames.unwrap_or(12),
            options.profile.as_deref(),
        );
    }
    if options.playtest {
        playtest::run()?;
        return expedition::run();
    }
    if options.expedition {
        return expedition::run();
    }
    if options.smoke {
        return smoke();
    }
    let direct = !options.title
        && (options.new || options.headless || options.frames.is_some() || options.pack);
    let game = if options.new {
        let game = if let Some(biome) = options.biome_test {
            let game = Game::new_in_biome(options.seed, biome)?;
            let [x, y, z] = game.player.spawn;
            println!("{} spawn at ({x:.1}, {y:.1}, {z:.1})", biome.name());
            game
        } else {
            Game::new(options.seed)
        };
        options.save = save::create(&game, &options.save)?;
        println!("Created {} (seed {})", options.save.display(), options.seed);
        game
    } else if direct {
        save::read(&options.save)?.unwrap_or_else(|| Game::new(options.seed))
    } else {
        Game::new(options.seed)
    };
    if options.headless {
        return headless::run(game, &options);
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let pack = options.pack;
    let profile = options
        .profile
        .as_deref()
        .map(performance::Profile::new)
        .transpose()?;
    let audio = (!options.mute).then(audio::Audio::new);
    let (settings, notice) = match settings::Settings::read(&options.settings_path) {
        Ok(s) => (s, String::new()),
        Err(e) => (settings::Settings::default(), e),
    };
    let mut app = App {
        active: direct,
        screen: if direct {
            ui::Screen::Game
        } else {
            ui::Screen::Title
        },
        settings_back: ui::Screen::Title,
        settings,
        notice,
        next_frame: Instant::now(),
        game,
        options,
        window: None,
        renderer: None,
        audio,
        keys: HashSet::new(),
        forward_tap: None,
        sprinting: false,
        attack: false,
        pack,
        paused: false,
        focused: true,
        cursor: (0., 0.),
        drag_origin: None,
        last: Instant::now(),
        accumulator: 0.,
        autosave: 0.,
        fps: 60.,
        frames: 0,
        error: None,
        profile,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    if let Some(profile) = &mut app.profile {
        profile.finish()?;
    }
    if let Some(e) = app.error {
        Err(e)
    } else {
        Ok(())
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Voxel Adventure: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
#[path = "../tools/movement_review.rs"]
mod movement_review;
