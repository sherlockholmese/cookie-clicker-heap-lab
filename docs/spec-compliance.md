# Specification compliance

This checklist maps `spec.md` to the pure-Python implementation. It is instructor-facing and does not appear as an in-game walkthrough.

## Game

- The game starts at zero; the cookie adds one per click.
- A Cursor costs 10 and every owned Cursor produces one cookie per second.
- Every automatic Cursor activation dispatches that Cursor's live `on_clicked` pointer.
- The trillion-cookie flag and `win()` are separate goals and separate guest fields.
- `win()` switches the main surface to a dedicated ending screen.
- Username, Cursor name, and Cursor description are editable.
- Every text editor accepts mixed text and raw `\xNN` bytes.
- The supplied palette sprite is the clickable cookie, with text fallback if the asset is invalid.

## Memory tools

> Status: implemented as a separate PyQt6 window (`qt_memory.py`), reachable
> from the main menu's **Memory** button. The docked tooltip panel replaces the
> old floating popups; the dump, coloring, scrolling, zoom, change-flash,
> writes, jump, follow, and live function table carry over.

- Memory is a separate PyQt6 window and is draggable by the operating system.
- Byte mode is the default and renders 16-byte rows in visible 8+8 groups with adjacent ASCII.
- The dump opens on the heap by default; the mouse wheel (and the vertical scrollbar) page through guest memory a row at a time, and `+`/`−`/reset controls (or Ctrl+wheel) zoom the dump font.
- Qword mode reads and writes packed little-endian 64-bit values.
- Individual bytes and qwords are selectable and writable.
- Direct address entry and `.text`, `.bss`, heap, and function-table shortcuts are present.
- Pointer fields and pointer-table entries are red. Struct fields are underlined and use distinct diagnostic colors.
- Field hover data includes the name, type, interpreted value, object, offset, and address.
- Pointer hover data includes its target and a 16-byte hex/ASCII preview or `unmapped`.
- Pointer qwords can be followed by double-click; selected pointer bytes expose **Follow pointer**.
- Game operations and debugger operations use the same checked memory and update both surfaces.
- Invalid data and function pointers report a simulated segmentation fault and restore the last valid snapshot.
- Persistent widgets and render-state caches update only changed values, preventing timer-driven flicker.

## Pointer view

- Pointer view is an explicit toggle.
- Username, cookie-count, and Cursor objects expose live guest addresses only while enabled.
- Hover popups contain clickable address buttons, cancel stale hide timers, and remain open while the pointer crosses any popup child.
- Equivalent inline buttons support reliable keyboard and mouse access.
- Address activation opens Memory at the relevant location without replacing the original game action.

## Virtual machine

- All guest addresses are checked Python integers; none become native pointers.
- The object heap begins exactly at `0x555555555000`.
- `.text` becomes read/execute-only after initialization; `.bss` and the heap remain writable.
- `.text` contains deterministic, realistic-looking bytecode in fixed 0x40-byte slots for `add_cookie`, `win`, and the inert `add_cursor`, `delete_cursor`, and `modify_cursor` helpers; hover tooltips and the function table supply the names.
- `.bss` contains Cookies, User, Cursor-array, count, input-limit, function-table, flag, and win globals.
- The Cursor pointer array is in `.bss`; each purchase appends the new heap address.
- Functions renders a live name/address table with clickable `.text` destinations.
- Multi-byte fields and pointers are eight-byte little-endian values; Cursor names are inline `char[8]` fields.

## Workshop primitives

- The default `max_field_size` permits the username overflow into `Cookies.cookies`; the guest limit remains visible and editable for workshop experiments.
- Cursor name overflow reaches the adjacent description and `on_clicked` pointers.
- Opening a Cursor editor opens Memory at that Cursor for side-by-side work.
- Redirecting description to `Cursor.on_clicked` makes description editing write through that pointer.
- Writing the `win` address reaches the ending screen on the Cursor's next automatic activation.
- Direct cookie edits, endianness exercises, struct edits, string-pointer replacement, pointer tracing, nonsense-pointer recovery, forged structs, indirect writes, and function-table exploration are supported.

## Complete Python port

- Game, memory, heap, debugger algorithms, saves, CLI, GUI, tests, and assets are Python.
- Runtime dependencies are the Python standard library plus PyQt6 for the GUI.
- The same sources run on Linux, Windows, and macOS; CI compiles and tests Python 3.11 on all three.
- All interface copy and defaults live in `cookie_clicker/assets/strings.txt`.
- The placeholder cookie lives in `cookie_clicker/assets/cookie.sprite.txt` and uses nearest-neighbor scaling.
- Platform-default controls, typography, and colors are retained except for the diagnostic pointer/field colors required by the specification.
