# Cookie Clicker Pwn Workshop

A native, pure-Python teaching game for learning how values, structs, pointers, little-endian integers, buffer overflows, and function pointers appear in memory. Guest addresses live in checked Python byte arrays and never become native process pointers.

## Run

Install Python 3.11 or newer with its standard `tkinter` module, then run:

```sh
python -m cookie_clicker
```

No third-party runtime packages are required. `--seed NUMBER` starts a deterministic session and `--load FILE` opens an explicit save.

## Game

- Click the sprite for one cookie.
- Buy a Cursor for 10 cookies. Once per second, every Cursor dispatches its live
  `on_clicked` function pointer.
- Buy the flag for 1,000,000,000,000 cookies. Acquiring the flag and executing `win()` are separate goals.
- Edit the username, Cursor names, and Cursor descriptions. Every text editor accepts mixed text and raw `\xNN` bytes.

The worksheet remains separate from the application. The game contains no exploit walkthrough.

## Memory window

Open **Memory** to use the draggable debugger window. It opens on the heap and provides 16-byte rows grouped 8+8, ASCII, packed little-endian qwords, direct writes, address jumps, section shortcuts, typed field hover details, pointer previews and following, a live function table, and two-way updates with the game. Scroll the mouse wheel (or drag the scrollbar) to move through memory a row at a time, and use the `+`/`−`/reset controls (or Ctrl+wheel) to zoom the dump font.

Enable **Pointer view** to expose clickable guest addresses for the username, cookie count, and Cursor objects. Invalid data or function pointers produce a simulated segmentation fault and restore the last valid state.

## Customize

All interface copy and default strings live in [`cookie_clicker/assets/strings.txt`](cookie_clicker/assets/strings.txt). The clickable placeholder sprite lives in [`cookie_clicker/assets/cookie.sprite.txt`](cookie_clicker/assets/cookie.sprite.txt). The application uses platform-default `tkinter` controls and fonts.

## Verify

```sh
python -m compileall -q cookie_clicker tests
python -m unittest discover -v
```

See [architecture](docs/architecture.md), [memory layout](docs/memory-layout.md), and [specification compliance](docs/spec-compliance.md).
