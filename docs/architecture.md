# Architecture

The application is a native Rust/egui program split into small crates. `vm-core` owns sparse mapped byte regions and checked reads/writes. `vm-heap` supplies deterministic allocations and out-of-band metadata. `cookie-game` defines the authoritative object graph and simulated function dispatch. `cookie-ui` renders the game and draggable memory window. `save-state` handles explicit versioned snapshots.

All guest pointers are 64-bit little-endian integers. A guest address is translated into a Rust-owned byte buffer only after bounds and permission checks. Unsafe Rust, host pointers, native guest execution, operating-system process access, and real segmentation faults are excluded.

The UI derives a fresh view from guest memory every frame. This makes debugger writes immediately visible in the game, while game actions flow through the same memory API and immediately appear in the hexdump. If a view or simulated callback dereferences an invalid pointer, the UI reports a simulated segmentation fault and restores its last valid snapshot.

The game implements two intentionally unsafe guest copies. `rename_user` obeys a mutable `max_field_size` stored in `.bss`, so changing that value permits the username buffer to reach the adjacent Cookies struct. `rename_cursor` can overflow the eight-byte name into the description and callback pointers. Editing a description writes through its current pointer. Callback dispatch accepts only the documented simulated `add_cookie` and `win` addresses.

Presentation uses egui defaults and editable resource files so a human can later choose fonts, colors, copy, and final art.
