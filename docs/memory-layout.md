# Guest memory layout

All fields and pointers are eight-byte little-endian values unless stated otherwise.

| Section | Start | Size | Permission |
| --- | ---: | ---: | --- |
| `.text` | `0x00400000` | 4 KiB | R-X |
| `.bss` | `0x55100000` | 4 KiB | RW- |
| heap | `0x55200000` | 64 KiB | RW- |

## `.bss` pointer table

| Offset | Type | Meaning |
| ---: | --- | --- |
| `+0x00` | `Cookies*` | Global Cookies object |
| `+0x08` | `User*` | Global User object |
| `+0x10` | `Cursor**` | Cursor pointer array at `.bss + 0x100` |
| `+0x18` | `u64` | Cursor count |
| `+0x20` | `u64` | Maximum username input size, initially 32 |
| `+0x28` | pointer | Function table at `.bss + 0x300` |
| `+0x30` | `u64` | Win state |
| `+0x38` | `u64` | Flag-owned state |

The function table contains `add_cookie` (`0x00400100`) followed by `win` (`0x00400180`). The corresponding `.text` locations contain labelled teaching bytecode rather than native instructions.

## Heap objects

Fresh allocation order is intentional: the 32-byte username buffer begins at `0x55200000`, then the eight-byte Cookies object, then User. New Cursors allocate a 64-byte description followed by a 32-byte Cursor.

`Cookies` contains one `u64 cookies` field. `User` contains `u64 id` and `char* username`.

| Cursor offset | Type | Field |
| ---: | --- | --- |
| `+0x00` | `u64` | `id` |
| `+0x08` | `char[8]` | `name` |
| `+0x10` | `char*` | `description` |
| `+0x18` | function pointer | `on_clicked` |

The Cursor pointer array holds up to 32 entries and grows when Cursors are purchased.
