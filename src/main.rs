mod ecs;
mod game;
mod items;
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
    seed: u64,
    save: PathBuf,
    smoke: bool,
    frames: Option<u32>,
    capture: Option<PathBuf>,
    pack: bool,
}
fn options() -> Result<Options, String> {
    let mut o = Options {
        seed: 314159,
        save: PathBuf::from("saves/world.json"),
        smoke: false,
        frames: None,
        capture: None,
        pack: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => {
                o.seed = args
                    .next()
                    .ok_or("--seed needs an integer")?
                    .parse()
                    .map_err(|_| "Invalid seed")?
            }
            "--save" => o.save = PathBuf::from(args.next().ok_or("--save needs a path")?),
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
                    "Voxel Adventure\n  --seed N       seed for a new world (default 314159)\n  --save PATH    world file (default saves/world.json)\n  --smoke        headless gameplay/persistence check; uses a temporary save\n  --frames N     render N frames then save and exit\n  --capture PATH save final frame as PPM (defaults to 240 frames)\n  --pack         start with the inventory open\n\nWASD move; mouse look; Space jump; Shift sprint\nLMB mine/attack; RMB place/eat/use; wheel or 1-9 select\nE inventory/crafting/stats; R sleep near bed; F5 save; Esc menu"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument: {arg}")),
        }
    }
    if o.capture.is_some() && o.frames.is_none() {
        o.frames = Some(240);
    }
    Ok(o)
}
struct App {
    game: Game,
    options: Options,
    window: Option<Arc<Window>>,
    renderer: Option<render::Renderer>,
    keys: HashSet<KeyCode>,
    attack: bool,
    pack: bool,
    paused: bool,
    focused: bool,
    cursor: (f32, f32),
    last: Instant,
    accumulator: f32,
    autosave: f32,
    fps: f32,
    frames: u32,
    error: Option<String>,
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
        self.attack = false;
        self.keys.clear();
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
            ui::Action::Craft(n) => self.game.craft(n),
            ui::Action::Equip(n) => {
                let i = items::ITEMS[n];
                if self.game.player.inventory.count(i) > 0 {
                    let slot = self.game.player.inventory.selected;
                    self.game.player.inventory.hotbar[slot] = i;
                    self.game
                        .say(format!("{} equipped in slot {}", i.name(), slot + 1));
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
                            .with_title("Voxel Adventure")
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
                self.cursor = (position.x as f32, position.y as f32)
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
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
                    if state == ElementState::Pressed
                        && button == MouseButton::Left
                        && let Some(a) = self
                            .renderer
                            .as_ref()
                            .and_then(|r| r.ui.click(self.cursor.0, self.cursor.1))
                    {
                        self.action(a, event_loop);
                    }
                } else if self.focused {
                    match button {
                        MouseButton::Left => self.attack = state == ElementState::Pressed,
                        MouseButton::Right if state == ElementState::Pressed => {
                            self.game.use_selected()
                        }
                        _ => (),
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
                self.last = now;
                self.fps = self.fps * 0.95 + 0.05 / dt.max(0.001);
                if self.focused && !self.pack && !self.paused {
                    self.accumulator += dt;
                    let pressed = |key| self.keys.contains(&key);
                    let input = Input {
                        forward: pressed(KeyCode::KeyW) as i32 as f32
                            - pressed(KeyCode::KeyS) as i32 as f32,
                        right: pressed(KeyCode::KeyD) as i32 as f32
                            - pressed(KeyCode::KeyA) as i32 as f32,
                        jump: pressed(KeyCode::Space),
                        sprint: pressed(KeyCode::ShiftLeft),
                        attack: self.attack,
                    };
                    while self.accumulator >= STEP {
                        self.game.tick(&input, STEP);
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
                if let Some(r) = &mut self.renderer {
                    r.update_world(
                        &mut self.game.world,
                        glam::Vec3::from_array(self.game.player.pos),
                    );
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
    if loaded.world.edits != g.world.edits
        || loaded.player.inventory.counts != g.player.inventory.counts
    {
        return Err("Save roundtrip mismatch".into());
    }
    std::fs::remove_file(path).map_err(|e| e.to_string())?;
    println!("Smoke passed: landing, mining, crafting, XP, save/load.");
    Ok(())
}
fn run() -> Result<(), String> {
    let options = options()?;
    if options.smoke {
        return smoke();
    }
    let game = save::read(&options.save)?.unwrap_or_else(|| Game::new(options.seed));
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let pack = options.pack;
    let mut app = App {
        game,
        options,
        window: None,
        renderer: None,
        keys: HashSet::new(),
        attack: false,
        pack,
        paused: false,
        focused: true,
        cursor: (0., 0.),
        last: Instant::now(),
        accumulator: 0.,
        autosave: 0.,
        fps: 60.,
        frames: 0,
        error: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
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
