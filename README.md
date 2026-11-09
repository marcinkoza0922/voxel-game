# Voxel Adventure

A Linux voxel adventure built in Rust with an owned engine and GUI. This first playable slice follows [PROPOSAL.md](PROPOSAL.md); architecture, decisions, and current limits are in [docs/DESIGN.md](docs/DESIGN.md).

Requires a Rust toolchain supporting edition 2024, a Linux desktop (Wayland or X11), and a Vulkan-capable GPU/driver. No downloaded game assets are required.

```sh
cargo run --release
```

The first release build takes a while. `cargo run` also works for development. By default the game loads `saves/world.json`, or creates a world with seed `314159`. Use `--save saves/another.json --seed 42` for a new world; an existing save supplies its own seed. Save files are versioned JSON and are excluded from Git. Failed or incompatible loads stop with an error and preserve the original file.

| Control | Action |
| --- | --- |
| WASD / mouse | Walk / look |
| Space / Left Shift | Jump / sprint |
| Hold left mouse | Mine or attack |
| Right mouse | Place selected block, eat selected food, or use bed |
| 1–9 / mouse wheel | Select hotbar slot |
| E | Open/close pack, recipes, and attributes |
| Click inventory item | Equip it into the selected hotbar slot |
| Click recipe | Craft once, if ingredients/station are available |
| R | Sleep near a bedroll, away from enemies |
| F5 | Save |
| Esc | Pause menu; save and quit |

Losing window focus pauses the game. The pack also pauses simulation. Autosave runs every 30 seconds of gameplay; closing the window saves too.

## First expedition

You start with a wooden axe, a wooden pickaxe, and berries. Chop logs with the axe. Open the pack and turn logs into planks, then craft a table. Equip it into your hotbar and place it against a nearby block. Stations work within three blocks.

Mine through the dirt to granite with the pickaxe. Make sticks and stone tools at the table. Stone pickaxes can harvest copper ore. Craft a furnace from eight cobblestone at the table, place it, and use coal plus copper ore to smelt ingots. Make copper tools and a sword at the table. Furnace recipes consume fuel immediately in this slice.

Leaves yield berries; eat them with right mouse while selected. Husks pursue and attack, and drop meat that can be cooked in the furnace. Bedrolls restore rest and advance to dawn; nearby enemies prevent sleep. Torches provide warm light while held or placed; placed torches burn out after ten minutes of active gameplay. Death returns you to spawn with your inventory.

Discovering biomes and resources, crafting, and defeating sufficiently strong enemies award XP. Repeated crafting awards one quarter as much XP each time, rounding down to zero. Spend level points on health, melee damage, attack speed, mobility, or crafting yield. Wisdom and Willpower are reserved for the future magic system and cannot consume points yet.

The terrain streams in chunks and supports a vertical range from **-512 to 1535**. Venture far from spawn for large mountains. Grasslands, oak forests, caves, coal and copper are implemented; the broader biomes, metallurgy, magic, and automation remain future milestones.

## Development

Install `cargo-nextest` if needed, then:

```sh
cargo nextest run
cargo clippy --message-format=short
./tools/verify.sh
```

Cargo output and nextest are configured to be quiet. The verification helper prints diagnostics only on failure. A display-free gameplay/persistence check is available with `cargo run -- --smoke` and uses a temporary save, leaving your world untouched.

For reproducible GPU checks, `--frames 180 --capture /tmp/frame.ppm --save /tmp/test-world.json` renders a fixed number of frames, captures the final framebuffer, saves, and exits. Add `--pack` to inspect the GUI. `--capture` defaults to 240 frames if no frame count is supplied. PPM is intentionally dependency-free and readable by standard image viewers/converters.
