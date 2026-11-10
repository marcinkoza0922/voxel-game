mod art;
mod audio;
mod ecs;
#[path = "../tools/expedition.rs"]
mod expedition;
mod feedback;
mod fluid;
mod game;
#[path = "../tools/headless.rs"]
mod headless;
mod items;
#[path = "../tools/performance.rs"]
mod performance;
#[path = "../tools/playtest.rs"]
mod playtest;
mod render;
mod save;
mod streaming;
mod ui;
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

struct Options {
    headless: bool,
    seed: u64,
    new: bool,
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
        headless: false,
        seed: 314159,
        new: false,
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
            "--headless" => o.headless = true,
            "--new" => o.new = true,
            "--mute" => o.mute = true,
            "--seed" => {
                explicit_seed = true;
                o.seed = args
                    .next()
                    .ok_or("--seed needs an integer")?
                    .parse()
                    .map_err(|_| "Invalid seed")?
            }
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
                    "Voxel Adventure\n  --headless     render offscreen without display/audio (default 240 frames)\n  --new          create a separate fresh save with a random seed\n  --seed N       seed for a new world (default 314159)\n  --save PATH    world file (default saves/world.json)\n  --smoke        headless gameplay/persistence check; uses a temporary save\n  --frames N     render N frames then save and exit\n  --capture PATH save final frame as PPM (defaults to 240 frames)\n  --tour         scripted camera relocations for streaming measurements\n  --playtest     run headless mechanics course and progression expedition\n  --expedition   run controlled headless progression expedition\n  --profile PATH write frame timings and chunk backlog as CSV\n  --mute         disable sound playback\n  --pack         start with the inventory open\n\nWASD move; mouse look; Space jump; double-tap W sprint; Shift walk\nLMB mine/attack; RMB place/eat/use; wheel or 1-9 select\nE inventory/crafting/stats; R sleep near bed; F5 save; Esc menu"
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
    if o.new && !explicit_seed {
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
            let grab = !self.pack && !self.paused && self.focused;
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
    fn new_world(&mut self) {
        if !self.save() {
            return;
        }
        let mut game = Game::new(fresh_seed());
        match save::create(&game, &self.options.save) {
            Ok(path) => {
                game.say(format!("New world: {}", path.display()));
                self.game = game;
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
            Err(e) => self.game.say(format!("New world failed: {e}")),
        }
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
            ui::Action::NewWorld => self.new_world(),
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
                            .with_title(format!(
                                "Voxel Adventure - {}",
                                self.options.save.display()
                            ))
                            .with_inner_size(winit::dpi::LogicalSize::new(1280., 800.))
                            .with_min_inner_size(winit::dpi::LogicalSize::new(900., 600.)),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let renderer = pollster::block_on(render::Renderer::new(window.clone()))?;
            Ok::<_, String>((window, renderer))
        })();
        match result {
            Ok((window, renderer)) => {
                self.window = Some(window);
                self.renderer = Some(renderer);
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
        if !self.pack
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
            WindowEvent::MouseWheel { delta, .. } if !self.pack && !self.paused => {
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
                if self.pack || self.paused {
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
                let interval = now.duration_since(self.last);
                let dt = interval.as_secs_f32().min(0.1);
                self.last = now;
                self.fps = self.fps * 0.95 + 0.05 / dt.max(0.001);
                if self.focused && !self.paused {
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
                    if let Some(audio) = &self.audio {
                        audio.play(cue);
                    }
                }
                let simulation_ms = now.elapsed().as_secs_f64() * 1000.;
                let stream_start = Instant::now();
                if let Some(r) = &mut self.renderer {
                    r.update_world(
                        &mut self.game.world,
                        glam::Vec3::from_array(self.game.player.pos),
                    );
                    let stream_ms = stream_start.elapsed().as_secs_f64() * 1000.;
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
                        ) {
                            self.error = Some(format!("Profile write failed: {e}"));
                            self.exit(event_loop);
                            return;
                        }
                    }
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
        // Presentation and compositor frame callbacks pace rendering at the
        // display refresh rate, independently of the fixed simulation tick.
        if let Some(w) = &self.window {
            w.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::Wait);
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
fn run() -> Result<(), String> {
    let mut options = options()?;
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
    let game = if options.new {
        let game = Game::new(options.seed);
        options.save = save::create(&game, &options.save)?;
        println!("Created {} (seed {})", options.save.display(), options.seed);
        game
    } else {
        save::read(&options.save)?.unwrap_or_else(|| Game::new(options.seed))
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
    let mut app = App {
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
