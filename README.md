# Cookie Clicker Pwn Workshop

A native teaching game for learning how values, structs, pointers, little-endian integers, buffer overflows, and function pointers appear in memory. Guest addresses live in checked Python byte arrays and never become native process pointers.

## Run

Install Python 3.11 or newer and the one runtime dependency (PyQt6), then run:

```sh
pip install PyQt6
python -m cookie_clicker
```

PyQt6 is the only third-party runtime dependency; the game logic itself is pure standard library. `--seed NUMBER` starts a deterministic session and `--load FILE` opens an explicit save.

## Game

- Click the sprite for one cookie.
- Buy a Cursor for 10 cookies. Once per second, every Cursor dispatches its live
  `on_clicked` function pointer.
- Buy the flag for 1,000,000,000,000 cookies. Acquiring the flag and executing `win()` are separate goals.
- Edit the username, Cursor names, and Cursor descriptions. Every text editor accepts mixed text and raw `\xNN` bytes.

The worksheet remains separate from the application. The game contains no exploit walkthrough.

## Memory window

Open **Memory** to use the separate draggable debugger window. It opens on the heap and shows monospace `ADDR | qword0 qword1 | ASCII` rows with `.bss` / `.text` / `heap` section shortcuts, a bytes/qwords toggle, address jump, and `+`/`−`/reset zoom (or Ctrl+wheel). Scroll the mouse wheel to move through memory a row at a time. Pointers are red and underlined, `.text` bytecode is orange (function entries red), string buffers are blue, and struct fields use distinct colors. A docked panel on the right shows a hover tooltip and the live function table. Hovering a field or pointer updates the tooltip; clicking a pointer navigates to its target; clicking a struct outlines it and shows its field breakdown. Bytes that change (from the game or a debugger write) briefly flash red. Edits made in the window update the game and vice-versa.

Enable **Pointer view** on the main menu to expose a guest-address hover tip for the username, cookie count, and Cursor objects. Invalid data or function pointers produce a simulated segmentation fault and restore the last valid state.

## Customize

All interface copy and default strings live in [`cookie_clicker/assets/strings.txt`](cookie_clicker/assets/strings.txt). The clickable placeholder sprite lives in [`cookie_clicker/assets/cookie.sprite.txt`](cookie_clicker/assets/cookie.sprite.txt). The main menu is built with PyQt6; its layout and colors live in [`cookie_clicker/qt_ui.py`](cookie_clicker/qt_ui.py).

## Verify

```sh
python -m compileall -q cookie_clicker tests
python -m unittest discover -v
```

See [architecture](docs/architecture.md), [memory layout](docs/memory-layout.md), and [specification compliance](docs/spec-compliance.md).
