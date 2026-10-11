# Specification compliance

This checklist maps `spec.md` to the implementation. It is instructor-facing and is not shown as an in-game walkthrough.

## Game

- The game starts at zero cookies. The cookie adds one per click.
- A Cursor costs 10 cookies and every owned Cursor produces one cookie per second.
- The trillion-cookie flag and execution of `win()` are separate goals and separate guest fields.
- `win()` switches the main surface to a dedicated ending screen.
- Username, Cursor name, and Cursor description are editable.
- Every text editor accepts mixed text and raw `\xNN` bytes.

## Memory tools

- Memory is a draggable window separate from the main game surface.
- Its default byte view renders 16-byte rows as two groups of eight with an adjacent ASCII rendering.
- Qword mode displays and edits packed little-endian 64-bit values.
- Individual bytes and qwords can be selected and written.
- Addresses can be entered directly; `.text`, `.bss`, heap, and function-table shortcuts are provided.
- Pointer fields and pointer-table entries are red. Known struct bytes are underlined and fields use distinct diagnostic colors.
- Field hover text reports the field name, type, interpreted value, containing object, offset, and address.
- Pointer hover text reports the target address plus a 16-byte hex and ASCII preview, or `unmapped`.
- Pointer qwords can be followed by double-clicking. Selected pointer bytes expose a Follow pointer action.
- Game writes and debugger writes use the same authoritative memory and update in both directions.
- Invalid data and function pointers produce a simulated segmentation fault and restore the last valid state.

## Pointer view

- Pointer view is an explicit toggle.
- When enabled, username, cookie-count, and Cursor objects expose their live guest addresses.
- Each exposed address opens the relevant location in Memory. The original game actions remain available.
- Hover popups contain clickable address links, and the same links remain visible inline for reliable mouse and keyboard access.

## Virtual machine

- Guest pointers are checked 64-bit values and never become host pointers.
- The object heap begins at `0x055200000`.
- `.text` is read/execute-only after initialization, `.bss` is writable, and the heap is writable.
- `.text` contains labelled teaching bytecode for `add_cookie` and `win`, not native instructions.
- `.bss` contains the global Cookies, User, Cursor-array, function-table, count, input-limit, flag, and win fields.
- The Cursor array itself is in `.bss`; purchasing a Cursor adds its heap pointer to the next entry.
- The Functions view is a live table of identifiers and addresses. Addresses link to the corresponding `.text` bytes.
- Multi-byte fields and pointers are eight-byte little-endian values. Cursor names are the specified eight-byte inline field.

## Workshop primitives

- Changing `max_field_size` permits the vulnerable username copy to overflow its adjacent allocation into `Cookies.cookies`.
- Cursor name overflow reaches the adjacent description and `on_clicked` pointers.
- Opening a Cursor editor also opens Memory at that Cursor so the two surfaces can be used side by side.
- Redirecting description to `Cursor.on_clicked` makes the description editor write through that pointer.
- Setting `on_clicked` to the documented `win` address and clicking the Cursor executes the simulated win function.
- Direct cookie edits, endianness exercises, struct edits, string pointer replacement, pointer tracing, nonsense-pointer recovery, forged structs with indirect writes, and function-table exploration are all supported without an in-game walkthrough.

## Product constraints

- The Rust/egui codebase is shared across Linux, Windows, and macOS.
- CI runs formatting, strict linting, tests, and release builds on Ubuntu, Windows, and macOS.
- Interface copy and editable default object strings live in `assets/strings.txt`.
- The clickable cookie loads its human-replaceable placeholder sprite from `assets/cookie.sprite.txt` with nearest-neighbor scaling and text fallback.
- The app uses egui defaults and placeholder assets. Project typography, palette, and final artwork remain human decisions. Red pointers and per-field diagnostic colors exist only because the memory-view specification requires them.
