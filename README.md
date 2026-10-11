# Cookie Heap Lab

A native Rust cookie game with a safe, simulated 64-bit machine. Bake cookies, race a baking rush, experiment with recipes, buy nine generator tiers, unlock upgrades and achievements, and prestige. Launch with `--debug` to inspect and modify the guest machine.

## Run

Install Rust 1.95 or newer, then:

```sh
cargo run -p cookie-heap-lab -- --help
cargo run -p cookie-heap-lab -- --seed 1234
cargo run -p cookie-heap-lab -- --debug --seed 1234
cargo run -p cookie-heap-lab -- --debug --load bakery.chls
```

Build the executable:

```sh
cargo build --release --locked
./target/release/cookie-heap-lab --debug
```

On Windows the executable is `target\release\cookie-heap-lab.exe`. Linux supports X11 and Wayland; rendering uses wgpu. Windows and macOS builds use the same codebase. The CI workflow builds and uploads a native executable on all three operating systems; those remote builds require running CI.

## Play and inspect

Cookies available to spend are the largest number on the main screen, with lifetime cookies underneath. Generator rows show ownership and production per unit; hover for total production. Purchase buttons share one right column.

- **Baking rush:** offers arrive randomly, first after 45–90 simulated seconds, then 90–180 seconds after each event. Accept within 30 seconds and make 12–20 clicks in 20 seconds for a bonus. Missing a rush costs nothing.
- **Recipes:** unlock Chocolate chip for 40 cookies (3× clicking) or Vanilla crunch for 80 (3× generator production). Unlocks are permanent within the saved game; switching is free, with one recipe active at a time. Choose recipes from the Recipes tab.
- **Golden batch:** choose a temporary 7× boost or an instant cookie shower. The first arrives after 3–5 simulated minutes; later rewards are 4–7 minutes apart.
- **Cookie-eating visitors:** sugar ants, crumb mice, and cookie gobblers appear occasionally. Shoo them away in 3–5 hits to recover what they ate. They leave after 20 seconds if ignored. Theft reduces spendable cookies, never lifetime production.

Goals, rewards, recipe multipliers, rush progress, and deadlines are ordinary guest bytes. The same scanner and hex editor can modify them. See the [activity layouts](docs/memory-layout.md#bakery-activities) for instructor reference.

The bakery name is followed by two guest callback objects. In debug mode, the name field accepts `hex:` followed by byte pairs, sending them through the vulnerable rename operation. An overflow can replace a callback object's vtable pointer with a fake vtable in the name allocation. **File → Quit…** invokes the callbacks before closing; redirecting one to the documented guest-shell function opens a bounded in-app shell with no host process access. A normal Quit closes the native application.

Debug mode starts with the game visible. Open **Memory tools → Find a value…** for the scanner or **Inspect memory…** for a floating memory window. Tools can be moved, resized, grouped, and closed. No debugger panels are constructed during an ordinary launch. There is no GUI toggle to enable debugging.

The scanner accepts:

- Numbers: `127`, `0x7F`, or `0b1111111`, using 64 bits by default.
- Text: exact UTF-8 strings such as `Crumb & Co.`.
- Hex bytes: patterns such as `43 72 75 6D 62`, matched in the given byte order.

Run a first scan, change the value in the game, then narrow results. Click **Inspect** beside a result to open its memory address. Scan options contain integer widths, alignment, unknown initial values, and comparison filters. String and byte filters retain the original pattern length.

Memory has **Heap** and **Code** views. Toggle between **Bytes** and **Qwords (LE)** to inspect individual bytes or grouped 64-bit little-endian values. The code region is read-only. In the heap, double-click a byte cell and type two hex digits to write and advance automatically. Double-click a qword to enter its hexadecimal u64 value; the editor stores its eight bytes in little-endian order and advances after 16 digits. You can also paste multiple byte pairs in byte mode. Escape stops editing; completed values remain written. All edits pass through the checked VM API.

The **Inspector** button in Memory opens a separate read-only popout with aligned allocation details, typed interpretations, pointer following, and watches. **Show object names** optionally reveals simulator names such as PlayerState or Generator[]. Other tools provide write history and snapshots/diffs.

## Save and load

Use **File → Save Game As…** (`Ctrl/Cmd+Shift+S`) to select an explicit destination. **File → Load Game…** (`Ctrl/Cmd+O`) validates the file before asking to replace the session. `--load FILE` loads at startup; it conflicts with `--seed`.

Every launch without `--load` starts fresh, even if save files exist. There is no autosave, save on exit, implicit restoration, or offline production. Saves include mapped memory, allocator metadata and free list, seed/RNG state, guest tick, pointers, fractional production, achievements, and effect deadlines. Debugger windows, watches, scans, and diffs are excluded. Corrupted guest state can be saved and loaded exactly, including pointers that subsequently fault.

Current saves resume recipes, rushes, visitors, shutdown callbacks, and timers exactly. Older saves are upgraded explicitly on load with any missing guest allocations. Existing heap addresses, balances, RNG, simulated time, and active rushes are preserved.

## Customize text and art

The editable resource pack lives in [`assets/`](assets). Change [`strings.txt`](assets/strings.txt) to rewrite menus, buttons, game copy, catalogue names, and debugger labels. Named placeholders such as `{count}` and `{price}` are filled at runtime. The cookie and visitor art use the documented plain-text palette format in `cookie.sprite.txt` and `visitor.sprite.txt`.

At runtime the app checks `./assets` first, then an `assets` folder beside the executable, and finally its embedded defaults. Use **Appearance → Reload resources** to apply text and sprite edits without restarting. Copy the whole `assets` directory beside packaged builds when you want an editable resource pack.

## Architecture and checks

See [architecture](docs/architecture.md) and [guest memory layout](docs/memory-layout.md).

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Tests cover address/permission faults, atomic memory transfers, little-endian pointers, allocator reuse/coalescing/invariants, deliberate corruption, deterministic economy, CLI gating, exact save continuation and malformed saves, scanner formats, pointer input, inline hex editing, and contrast in both themes. UI tests run headlessly; they do not control the desktop.
