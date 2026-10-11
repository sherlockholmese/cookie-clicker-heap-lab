# Cookie Clicker Pwn Workshop

A native teaching game for learning how values, structs, pointers, little-endian integers, buffer overflows, and function pointers appear in memory. The application uses a checked virtual address space; guest addresses never become host pointers and guest functions never execute native code.

## Run

Install Rust 1.95 or newer, then run:

```sh
cargo run -p cookie-heap-lab
```

The same Rust codebase builds on Linux, Windows, and macOS. `--seed` starts a deterministic fresh session and `--load FILE` loads an explicit save. The memory tools are always available from the game window.

## Game

- Click the cookie for one cookie.
- Buy a Cursor for 10 cookies. Every Cursor produces one cookie per second.
- Buy the flag for 1,000,000,000,000 cookies.
- Edit the username, Cursor names, and Cursor descriptions. Text fields accept raw bytes with `\xNN`, such as `A\x00\xFF`.
- Click a Cursor to dispatch its simulated `on_clicked` function pointer.

The intended workshop exercises are described by the separate handout rather than by an in-game walkthrough. The game supports the two final goals from `spec.md`: overflowing the username buffer into the cookie count, and redirecting a Cursor callback to the simulated `win` function.

## Memory window

Open **Memory** from the game. The draggable window provides:

- 16-byte rows arranged as two groups of eight bytes, plus ASCII;
- packed little-endian qword mode;
- direct byte and qword writes;
- address jumps and shortcuts for `.text`, `.bss`, heap, and the function table;
- red pointer values, underlined/color-keyed struct bytes, typed hover details, and pointer following;
- live two-way updates between memory and the game.

Enable **Pointer view** to reveal guest addresses on game objects and jump to them. Invalid dereferences show a simulated segmentation fault and restore the last valid game state.

## Customize

Human-editable copy and placeholder sprite assets live in [`assets/`](assets). The UI deliberately uses the platform/default egui appearance; no project color scheme, font, or finished artwork is imposed by the implementation.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

See [architecture](docs/architecture.md) and [memory layout](docs/memory-layout.md) for instructor-facing implementation details.
