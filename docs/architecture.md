# Architecture

## Fixed invariants

- Rust host with unsafe code forbidden in every application crate.
- eframe/egui native UI, wgpu rendering, egui_dock floating tools, clap CLI, Serde snapshots.
- Guest addresses and pointers are `u64`; pointers occupy eight little-endian bytes.
- Addresses are translated into Rust-owned region buffers. No guest address becomes a host pointer.
- Debugger tools exist only when `--debug` is set at process startup.
- Explicit save and explicit load only; no persistence feature enabled in eframe.
- No offline production, process attachment, native code execution, guest syscalls, or lesson subsystem.

The UI uses tools opened on demand, floating above the game. Memory browses the writable heap and read-only code region. Its Inspector button opens a separate read-only popout; allocation metadata and optional object names appear there. Direct heap editing happens in the hex cells, advancing after each complete byte.

User-facing text and simple sprite art are loaded from the plain-text `assets` resource pack. The app checks the working directory and executable directory before using embedded defaults. UI code refers to stable text keys, while gameplay state and save files remain independent of presentation resources.

## Crates

| Crate | Responsibility |
| --- | --- |
| `vm-core` | Central architecture configuration, sparse mapped regions, permissions, checked transfers, typed access, faults, write events, execution interface |
| `vm-heap` | Deterministic aligned first-fit allocation, out-of-band metadata, free/reuse, coalescing, validation and ownership lookup |
| `cookie-game` | Complete authoritative machine, guest object graph, fixed-step integer economy, guest operations, corruption scenarios |
| `debugger` | Integer and byte-pattern scans, typed conversions, watches, pointer resolution and memory diff |
| `save-state` | Versioned binary envelope, CRC validation, bounded decode, complete machine validation, atomic explicit file writes |
| `cookie-ui` | Game projection, shared light/dark theme, file dialogs, floating debugger tools and direct hex editing |
| `cookie-heap-lab` | CLI parsing, fresh/load startup, native entry point |

The memory, heap, debugger and game crates do not depend on egui. `ExecutionBackend` exposes memory operations, regions, allocations, pause/step, reset and snapshots. The current backend is `Machine`; a real ISA CPU backend is future work. No fake register view is presented.

## Memory and allocation boundaries

The address space is sparse: mapping high addresses allocates only the region's backing bytes. Memory operations prevalidate all spans and permissions before writing. A transfer may span contiguous regions; a missing or unwritable span faults atomically. Reads and writes check arithmetic before converting region offsets to host indices.

Heap metadata governs ownership and diagnostics, not write permission. Cross-allocation writes inside mapped writable memory succeed and report the origin, affected allocations, and overflow length. Crossing an unmapped or read-only region fails. Metadata cannot be overwritten through guest memory.

Allocation history preserves freed records; reuse produces a new allocation ID and generation. Coalescing joins adjacent free blocks. Validation checks bounds, alignment, IDs, complete heap coverage, free-list order, and live-allocation nonoverlap. Fresh machines always use the same placement; the seed determines later simulated randomness.

## Authoritative game state

A root pointer in guest globals points to PlayerState. Gameplay reads player and subordinate object bytes through VM APIs; the UI reads a derived `GameView`. The view is reconstructed each frame. There is no host-side cookie count to synchronize after debugger edits.

The economy uses integer arithmetic with saturation at `u64::MAX`. Twenty guest ticks represent one simulated second; a guest remainder retains fractional production. Prices increase by approximately 15% after each purchase. Click/passive upgrades, prestige, and guest effect deadlines determine production. Catalogue names and base prices are immutable host configuration; mutable rates, ownership, current prices, flags and timers live in guest memory.

A second globals pointer references BakeryActivities. Recipes independently multiply clicking or passive production and are purchased once. Baking rushes arrive as random offers and use guest progress counters, absolute deadlines, and explicit reward claims. BakeryVisitors, reached through globals +16, stores enemy health, bite size, stolen cookies, and arrival/departure timers. Golden batches offer a boost or a production-scaled instant reward. Prestige cancels the current rush while retaining recipe unlocks and activity statistics. None of these features requires the debugger.

UI execution uses bounded fixed-step catch-up. Wall-clock timestamps never enter saves, and loading resets the UI accumulator. Input actions and tick counts define deterministic replays. Runtime pause is a debugger/UI preference and is excluded from snapshots.

## Controlled vulnerabilities

- `rename_player`: writes supplied bytes plus NUL through the VM, without enforcing the 32-byte name allocation. Two virtual callback objects are immediately adjacent in fresh games.
- Exit dispatch reads each object's vtable pointer, then its first eight-byte function entry. A fake vtable in the name allocation can redirect this call to the simulated guest-shell function.
- `label_recipe`: writes bytes plus NUL without enforcing the 24-byte label field. It can overwrite a multiplier and an eight-byte pointer in ShopState.
- Recipe-pointer corruption redirects a subsequent guest read used in click calculations.
- Trading a charm frees its allocation and creates a coupon that reuses the space. Statistics retains the old charm pointer, which later reads the coupon's value.

Guest object reads validate their complete mapped extent before field offset arithmetic. Shutdown dispatch recognizes only its clean functions and the guest-shell teaching target. Other values cause recoverable guest faults; they do not panic, spawn a process, or reach host memory. UI fault handling pauses simulation and allows memory repair or a fresh run. Each guest operation caps its input at 4096 bytes, independently of the deliberate logical-boundary violation.

## Observability

All guest writes pass through Memory. Changed writes retain tick, operation, source, old/new bytes, origin allocation, affected allocations and allocation/field overflow lengths in a bounded 512-event history. Sources distinguish guest, debugger, restore and initialization. Initialization history is cleared before a fresh game is presented; gameplay saves exclude transient history.

Snapshots/diffs, scans, windows, selection, bookmarks and watches are separate debugger state. Memory symbols are hidden by default. Inspector can show semantic names explicitly. Direct hex edits are debugger writes; unsafe game copies remain guest writes.

## Save envelope

The file begins with 36 bytes:

| Offset | Bytes | Content |
| --- | --- | --- |
| 0 | 8 | `CHLS64\r\n` magic |
| 8 | 4 | Format version, little-endian u32 |
| 12 | 4 | Application compatibility version |
| 16 | 8 | `U64LE` architecture/endianness identifier plus NUL padding |
| 24 | 8 | Payload length, little-endian u64 |
| 32 | 4 | CRC32 of payload |

The payload is a Serde/bincode snapshot with fixed integer encoding and little-endian semantics. Version 1 permits at most 32 MiB and rejects trailing data. Loading checks size, magic, versions, architecture, exact length, checksum, mapped regions, and allocator invariants before returning a complete replacement machine. Invalid guest values remain valid snapshot bytes; invalid host metadata is rejected.

The envelope remains format version 1; application compatibility is 4. Older saves gain missing activity, visitor, and callback allocations at globals +8/+16/+24. Migration preserves balances, existing allocation addresses, RNG, ticks, and running or completed rushes. Corrupted pointers remain corrupted. Compatibility-4 snapshots resume exactly and never silently repair corrupted roots.

Saving creates a uniquely named temporary file in the destination directory, writes and syncs it, then atomically replaces the destination using tempfile's platform implementation. Unix additionally syncs the parent directory. There is no implicit default slot. CRC detects accidental damage; it is not an authenticity signature.

## Verification and release

Headless tests cover core semantics and simulated UI input. Native GUI validation was explicitly declined by the user; no desktop interaction is needed for tests. The CI matrix runs formatting, strict clippy, tests, and release builds on Linux, Windows, and macOS. Local builds establish the Linux binary; other platform artifacts require CI or a build on that platform.
