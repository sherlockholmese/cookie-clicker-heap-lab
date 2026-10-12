# Architecture

The application is entirely Python 3.11+ and uses only the standard library at runtime. `memory.py` owns checked virtual regions and the deterministic first-fit heap. `game.py` stores authoritative game state in those regions and dispatches simulated bytecode functions. `debugger.py` contains GUI-independent typed values, scans, watches, pointer labels, and diffs. `ui.py` renders the native game, pointer popups, Cursor editors, cookie sprite, and draggable memory window with `tkinter`. `save_state.py` provides versioned, checksummed, atomic snapshots.

All guest pointers are 64-bit little-endian integers. A guest address reaches a Python-owned `bytearray` only after bounds and permission checks. The implementation does not use `ctypes`, native pointers, native guest execution, process-memory access, compiled extensions, or unsafe foreign-function calls.

The UI derives a fresh view from guest memory on every refresh and updates persistent widgets in place. Game actions and debugger writes therefore update the same bytes without recreating or flickering the interface. Invalid pointer dereferences report a simulated segmentation fault and restore the last valid deep snapshot.

The two intentional guest vulnerabilities are preserved. `rename_user` obeys the mutable `max_field_size` global, allowing the username allocation to overflow into `Cookies.cookies`. `rename_cursor` can overflow its eight-byte name into the description and callback pointers. Description edits write through the current description pointer, and callback dispatch recognizes only the simulated `add_cookie` and `win` addresses.

Package resources contain all visible copy and the palette-based placeholder cookie sprite. The GUI uses platform-default controls and fonts so human designers retain control of final presentation.
