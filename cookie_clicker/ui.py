"""Cross-platform tkinter interface and live draggable memory window."""

from __future__ import annotations

import tkinter as tk
import tkinter.font as tkfont
from tkinter import ttk
from typing import Callable

from .game import (
    CURSOR_DESCRIPTION,
    CURSOR_NAME,
    CURSOR_ON_CLICKED,
    CURSOR_PRICE,
    FLAG_PRICE,
    FN_SLOT_SIZE,
    FUNCTION_TABLE,
    FUNCTIONS,
    G_COOKIES,
    G_CURSOR_COUNT,
    G_CURSOR_TABLE,
    G_CURSORS,
    G_FLAG_OWNED,
    G_FUNCTIONS,
    G_MAX_FIELD_SIZE,
    G_USER,
    G_WON,
    MAX_CURSORS,
    USER_USERNAME,
    GameError,
    GameView,
    Machine,
    SegmentationFault,
)
from .memory import CODE, GLOBALS, HEAP, MemoryFault
from .resources import cookie_sprite, text


def bind_select_all(widget: tk.Widget) -> None:
    """Make Ctrl+A / Cmd+A select all text in an Entry or Text widget.

    tkinter does not bind this by default on every platform, so we add it
    explicitly for the game's text inputs.
    """

    def _select_all(event: tk.Event) -> str:
        target = event.widget
        try:
            if isinstance(target, tk.Text):
                target.tag_add("sel", "1.0", "end-1c")
                target.mark_set("insert", "end-1c")
            else:
                target.select_range(0, "end")  # type: ignore[attr-defined]
                target.icursor("end")  # type: ignore[attr-defined]
        except tk.TclError:
            return ""
        return "break"

    widget.bind("<Control-a>", _select_all)
    widget.bind("<Control-A>", _select_all)
    widget.bind("<Command-a>", _select_all)
    widget.bind("<Command-A>", _select_all)


def parse_address(value: str) -> int:
    candidate = value.strip().replace("_", "")
    try:
        return int(candidate, 16 if candidate.lower().startswith("0x") else 10)
    except ValueError as error:
        raise ValueError(text("error.address")) from error


def parse_field(value: str) -> bytes:
    output = bytearray()
    index = 0
    while index < len(value):
        if value[index : index + 2] == "\\x":
            pair = value[index + 2 : index + 4]
            if len(pair) != 2:
                raise ValueError(text("error.raw_byte_format"))
            try:
                output.append(int(pair, 16))
            except ValueError as error:
                raise ValueError(text("error.raw_byte_format")) from error
            index += 4
        else:
            output.extend(value[index].encode("utf-8"))
            index += 1
    return bytes(output)


class HoverPopup:
    def __init__(self, owner: tk.Misc, hide_delay: int = 120) -> None:
        self.owner = owner
        self.window: tk.Toplevel | None = None
        self.pending: str | None = None
        self.hide_delay = hide_delay

    def cancel_hide(self, _event: object = None) -> None:
        if self.pending:
            self.owner.after_cancel(self.pending)
            self.pending = None

    def schedule_hide(self, _event: object = None) -> None:
        self.cancel_hide()
        self.pending = self.owner.after(self.hide_delay, self.hide)

    def hide(self) -> None:
        if self.pending:
            try:
                self.owner.after_cancel(self.pending)
            except tk.TclError:
                pass
            self.pending = None
        if self.window is not None:
            self.window.destroy()
            self.window = None

    def _keep_open_over(self, widget: tk.Misc) -> None:
        widget.bind("<Enter>", self.cancel_hide)
        widget.bind("<Leave>", self.schedule_hide)
        for child in widget.winfo_children():
            self._keep_open_over(child)

    def show_text(self, widget: tk.Widget, message: str) -> None:
        self.hide()
        window = tk.Toplevel(widget)
        window.withdraw()
        window.overrideredirect(True)
        ttk.Label(window, text=message, justify="left", padding=6).pack()
        window.update_idletasks()
        window.geometry(f"+{widget.winfo_rootx() + 12}+{widget.winfo_rooty() + widget.winfo_height() + 4}")
        window.deiconify()
        self._keep_open_over(window)
        self.window = window

    def show_pointer(
        self, widget: tk.Widget, label: str, address: int, command: Callable[[int], None]
    ) -> None:
        self.hide()
        window = tk.Toplevel(widget)
        window.withdraw()
        window.overrideredirect(True)
        frame = ttk.Frame(window, padding=6)
        frame.pack()
        ttk.Label(frame, text=label).pack(anchor="w")
        button = ttk.Button(frame, text=f"0x{address:016X}", command=lambda: command(address))
        button.pack(anchor="w")
        window.update_idletasks()
        window.geometry(f"+{widget.winfo_rootx() + 12}+{widget.winfo_rooty() + widget.winfo_height() + 4}")
        window.deiconify()
        self._keep_open_over(window)
        self.window = window


class MemoryWindow:
    # Distinct, higher-contrast per-field fill colors. Underlining is kept
    # (per the spec) and these backgrounds make adjacent fields clearly
    # different from one another.
    FIELD_COLORS = ("#8ec8ff", "#8fe39a", "#ffe066", "#ff9a8c")
    # Character buffers / string fields get their own highlight so students can
    # distinguish text data from numeric fields and pointers at a glance.
    STRING_COLOR = "#ffb3de"
    # Each struct instance is wrapped in a solid bounding box drawn in a color
    # keyed to its type so Cookies / User / Cursor blocks are easy to tell
    # apart at a glance.
    STRUCT_BORDERS = {
        "struct Cookies": "#1f6feb",
        "struct User": "#2ea043",
        "struct Cursor": "#8957e5",
    }
    STRUCT_TYPES = frozenset(STRUCT_BORDERS)

    def __init__(self, app: "CookieApp") -> None:
        self.app = app
        self.window: tk.Toplevel | None = None
        self.base = HEAP
        self.qwords = False
        self.row_count = 32
        self.base_font_size = 0
        self.zoom = 0
        self.selected: int | None = None
        self.jump_value: tk.StringVar | None = None
        self.edit_value: tk.StringVar | None = None
        self.mode_value: tk.StringVar | None = None
        self.status_value: tk.StringVar | None = None
        self.table: ttk.Frame | None = None
        self.canvas: tk.Canvas | None = None
        self.vertical_scrollbar: ttk.Scrollbar | None = None
        self.function_frame: ttk.Frame | None = None
        self.cells: list[list[tk.Label]] = []
        self.address_labels: list[ttk.Label] = []
        self.ascii_labels: list[ttk.Label] = []
        self.tooltip: HoverPopup | None = None
        self.normal_font: tkfont.Font | None = None
        self.underlined_font: tkfont.Font | None = None
        self.bold_font: tkfont.Font | None = None
        self.bold_underlined_font: tkfont.Font | None = None
        self.bold_range: tuple[int, int] | None = None
        self.cell_values: dict[int, int] = {}
        self.flashing: set[int] = set()
        self.flash_handles: dict[int, str] = {}
        self._pending_scroll = 0
        self._scroll_pending_handle: str | None = None
        self.function_signature: object = object()
        self.cell_states: list[list[tuple[object, ...] | None]] = []
        self.address_states: list[str | None] = []
        self.ascii_states: list[str | None] = []

    def open(self, address: int | None = None) -> None:
        if address is not None:
            self.base = address & ~0xF
        if self.window is None or not self.window.winfo_exists():
            self._build()
        else:
            self.window.deiconify()
            self.window.lift()
        if self.jump_value is not None:
            self.jump_value.set(f"0x{self.base:X}")
        self.refresh()

    def _build(self) -> None:
        self.window = tk.Toplevel(self.app.root)
        self.window.title(text("memory.title"))
        self.window.geometry("1020x720+30+60")
        self.window.minsize(760, 420)
        self.window.protocol("WM_DELETE_WINDOW", self.window.withdraw)
        self.tooltip = HoverPopup(self.window)
        self.normal_font = tkfont.nametofont("TkFixedFont").copy()
        self.base_font_size = self.normal_font.cget("size") or 10
        self.underlined_font = self.normal_font.copy()
        self.underlined_font.configure(underline=True)
        self.bold_font = self.normal_font.copy()
        self.bold_font.configure(weight="bold")
        self.bold_underlined_font = self.normal_font.copy()
        self.bold_underlined_font.configure(weight="bold", underline=True)
        self._apply_zoom_fonts()

        controls = ttk.Frame(self.window, padding=6)
        controls.pack(fill="x")
        self.mode_value = tk.StringVar(value="bytes")
        ttk.Radiobutton(
            controls,
            text=text("memory.bytes"),
            variable=self.mode_value,
            value="bytes",
            command=self._change_mode,
        ).pack(side="left")
        ttk.Radiobutton(
            controls,
            text=text("memory.qwords"),
            variable=self.mode_value,
            value="qwords",
            command=self._change_mode,
        ).pack(side="left")
        ttk.Label(controls, text=text("memory.address")).pack(side="left", padx=(14, 4))
        self.jump_value = tk.StringVar(value=f"0x{self.base:X}")
        jump_entry = ttk.Entry(controls, textvariable=self.jump_value, width=18)
        jump_entry.pack(side="left")
        bind_select_all(jump_entry)
        ttk.Button(controls, text=text("memory.jump"), command=self._jump_from_entry).pack(
            side="left", padx=4
        )
        ttk.Button(controls, text=text("memory.zoom_out"), width=3, command=lambda: self._adjust_zoom(-1)).pack(
            side="left", padx=(14, 0)
        )
        ttk.Button(controls, text=text("memory.zoom_in"), width=3, command=lambda: self._adjust_zoom(1)).pack(
            side="left", padx=(2, 0)
        )
        ttk.Button(controls, text=text("memory.zoom_reset"), command=lambda: self._adjust_zoom(0, reset=True)).pack(
            side="left", padx=(2, 0)
        )

        sections = ttk.Frame(self.window, padding=(6, 0, 6, 4))
        sections.pack(fill="x")
        for label, address in (
            (text("memory.section.text"), CODE),
            (text("memory.section.bss"), GLOBALS),
            (text("memory.section.heap"), HEAP),
            (text("memory.section.functions"), FUNCTION_TABLE),
        ):
            ttk.Button(sections, text=label, command=lambda value=address: self.jump_to(value)).pack(
                side="left", padx=(0, 4)
            )

        editor = ttk.Frame(self.window, padding=(6, 0, 6, 4))
        editor.pack(fill="x")
        self.selected_label = ttk.Label(editor, text="—", width=20)
        self.selected_label.pack(side="left")
        self.edit_value = tk.StringVar()
        edit_entry = ttk.Entry(editor, textvariable=self.edit_value, width=22)
        edit_entry.pack(side="left")
        bind_select_all(edit_entry)
        ttk.Button(editor, text=text("memory.write"), command=self._write_selected).pack(
            side="left", padx=4
        )
        self.follow_button = ttk.Button(editor, text=text("memory.follow"), command=self._follow_selected)
        self.follow_button.pack(side="left")
        self.status_value = tk.StringVar()
        ttk.Label(self.window, textvariable=self.status_value, padding=(6, 0, 6, 4)).pack(
            fill="x"
        )

        self.function_frame = ttk.Frame(self.window, padding=(6, 0, 6, 4))
        self.function_frame.pack(fill="x")

        container = ttk.Frame(self.window)
        container.pack(fill="both", expand=True)
        canvas = tk.Canvas(container, highlightthickness=0)
        vertical = ttk.Scrollbar(container, orient="vertical", command=self._scrollbar_scroll)
        horizontal = ttk.Scrollbar(container, orient="horizontal", command=canvas.xview)
        canvas.configure(xscrollcommand=horizontal.set)
        self.canvas = canvas
        self.vertical_scrollbar = vertical
        vertical.pack(side="right", fill="y")
        horizontal.pack(side="bottom", fill="x")
        canvas.pack(side="left", fill="both", expand=True)
        self.table = ttk.Frame(canvas, padding=6)
        item = canvas.create_window((0, 0), window=self.table, anchor="nw")
        self.table.bind(
            "<Configure>", lambda _event: canvas.configure(scrollregion=canvas.bbox("all"))
        )
        canvas.bind("<Configure>", lambda event: canvas.itemconfigure(item, height=max(event.height, 1)))
        for widget in (canvas, self.table):
            widget.bind("<MouseWheel>", self._on_mouse_wheel)
            widget.bind("<Button-4>", self._on_mouse_wheel)
            widget.bind("<Button-5>", self._on_mouse_wheel)
        self._rebuild_grid()

    def _scrollbar_scroll(self, *args: str) -> None:
        # The vertical scrollbar pages through guest memory a row at a time
        # rather than panning a fixed widget, so the dump is effectively
        # infinite within the mapped regions.
        if not args:
            return
        if args[0] == "scroll":
            amount = int(args[1])
            step = self.row_count if args[2] == "pages" else 1
            self._scroll_rows(amount * step)
        elif args[0] == "moveto":
            region = self._current_region()
            if region is None:
                return
            start, size = region
            fraction = max(0.0, min(1.0, float(args[1])))
            target = start + int(fraction * max(size - self.row_count * 16, 0))
            self.base = target & ~0xF
            if self.jump_value is not None:
                self.jump_value.set(f"0x{self.base:X}")
            self.refresh()

    def _current_region(self) -> tuple[int, int] | None:
        for region in self.app.machine.memory.regions:
            if region.base <= self.base < region.base + region.size:
                return region.base, region.size
        return None

    def _update_scrollbar(self) -> None:
        if self.vertical_scrollbar is None:
            return
        region = self._current_region()
        if region is None:
            self.vertical_scrollbar.set(0.0, 1.0)
            return
        start, size = region
        span = self.row_count * 16
        offset = self.base - start
        first = offset / size if size else 0.0
        last = min(1.0, (offset + span) / size) if size else 1.0
        self.vertical_scrollbar.set(max(0.0, first), max(first, last))

    def _on_mouse_wheel(self, event: tk.Event) -> str:
        if getattr(event, "state", 0) & 0x0004:  # Control held -> zoom
            direction = 1 if getattr(event, "delta", 0) > 0 or event.num == 4 else -1
            self._adjust_zoom(direction)
            return "break"
        if getattr(event, "num", None) == 4:
            rows = -1
        elif getattr(event, "num", None) == 5:
            rows = 1
        else:
            # One notch == one row. Trackpads report finer deltas; accumulate
            # them so a flick still moves a sensible amount.
            delta = getattr(event, "delta", 0)
            rows = -1 if delta > 0 else 1
            if abs(delta) >= 120:
                rows = -(delta // 120)
        self._queue_scroll(rows)
        return "break"

    def _queue_scroll(self, rows: int) -> None:
        # Coalesce a burst of wheel events into a single refresh on the next
        # idle cycle, so fast scrolling stays smooth instead of running a full
        # redraw per event.
        self._pending_scroll += rows
        if self._scroll_pending_handle is None and self.window is not None:
            self._scroll_pending_handle = self.window.after_idle(self._flush_scroll)

    def _flush_scroll(self) -> None:
        self._scroll_pending_handle = None
        rows, self._pending_scroll = self._pending_scroll, 0
        if rows:
            self._scroll_rows(rows)

    def _mapped_bounds(self) -> tuple[int, int] | None:
        regions = self.app.machine.memory.regions
        if not regions:
            return None
        lowest = min(region.base for region in regions)
        highest = max(region.base + region.size for region in regions)
        return lowest, highest

    def _scroll_rows(self, rows: int) -> None:
        bounds = self._mapped_bounds()
        new_base = self.base + rows * 16
        if bounds is not None:
            lowest, highest = bounds
            # Keep the top of the dump within mapped memory: never scroll up
            # past the first mapped region, and keep at least one row of mapped
            # bytes visible when scrolling down.
            new_base = max(lowest, min(new_base, highest - 16))
        if new_base < 0:
            new_base = 0
        self.base = new_base & ~0xF
        if self.jump_value is not None:
            self.jump_value.set(f"0x{self.base:X}")
        self.refresh()

    def _apply_zoom_fonts(self) -> None:
        if self.normal_font is None or self.underlined_font is None:
            return
        size = max(6, self.base_font_size + self.zoom)
        self.normal_font.configure(size=size)
        self.underlined_font.configure(size=size)
        if self.bold_font is not None:
            self.bold_font.configure(size=size)
        if self.bold_underlined_font is not None:
            self.bold_underlined_font.configure(size=size)

    def _adjust_zoom(self, direction: int, *, reset: bool = False) -> None:
        if reset:
            self.zoom = 0
        else:
            self.zoom = max(-4, min(24, self.zoom + direction))
        self._apply_zoom_fonts()
        self._rebuild_grid()
        self.refresh()

    def _change_mode(self) -> None:
        self.qwords = self.mode_value is not None and self.mode_value.get() == "qwords"
        self._rebuild_grid()
        self.refresh()

    def _rebuild_grid(self) -> None:
        if self.table is None:
            return
        for child in self.table.winfo_children():
            child.destroy()
        self.cells.clear()
        self.address_labels.clear()
        self.ascii_labels.clear()
        self.cell_states.clear()
        self.address_states.clear()
        self.ascii_states.clear()
        # Granularity (byte vs qword) changes with the mode, so the value cache
        # and any in-flight change flashes must be reset to avoid false flashes.
        for handle in self.flash_handles.values():
            try:
                self.window.after_cancel(handle)
            except (tk.TclError, AttributeError):
                pass
        self.flash_handles.clear()
        self.flashing.clear()
        self.cell_values.clear()
        width = 16 if self.qwords else 2
        count = 2 if self.qwords else 16
        for row in range(self.row_count):
            address = ttk.Label(self.table, width=16, font=self.normal_font, anchor="e")
            address.grid(row=row, column=0, padx=(0, 8), sticky="e")
            self._bind_wheel(address)
            self.address_labels.append(address)
            self.address_states.append(None)
            row_cells: list[tk.Label] = []
            for column in range(count):
                pad = (10, 2) if (not self.qwords and column == 8) else (2, 2)
                cell = tk.Label(
                    self.table,
                    width=width,
                    anchor="center",
                    font=self.normal_font,
                    cursor="hand2",
                    padx=1,
                )
                cell.grid(row=row, column=column + 1, padx=pad, sticky="w")
                self._bind_wheel(cell)
                # Bind interaction events once (not every refresh). The address
                # is resolved from the fixed grid position + current base at
                # event time, which keeps scrolling cheap.
                cell.bind("<Button-1>", lambda _e, r=row, c=column: self._cell_click(r, c))
                cell.bind("<Double-Button-1>", lambda _e, r=row, c=column: self._cell_double(r, c))
                cell.bind(
                    "<Enter>",
                    lambda event, r=row, c=column: self._show_cell_info(
                        event.widget, self._cell_address(r, c)
                    ),
                )
                cell.bind("<Leave>", self._on_cell_leave)
                row_cells.append(cell)
            self.cells.append(row_cells)
            self.cell_states.append([None] * count)
            ascii_label = ttk.Label(self.table, width=16, font=self.normal_font, anchor="w")
            ascii_label.grid(row=row, column=count + 1, padx=(12, 0), sticky="w")
            self._bind_wheel(ascii_label)
            self.ascii_labels.append(ascii_label)
            self.ascii_states.append(None)

    def _bind_wheel(self, widget: tk.Widget) -> None:
        widget.bind("<MouseWheel>", self._on_mouse_wheel)
        widget.bind("<Button-4>", self._on_mouse_wheel)
        widget.bind("<Button-5>", self._on_mouse_wheel)

    def _cell_address(self, row: int, column: int) -> int:
        return self.base + row * 16 + column * (8 if self.qwords else 1)

    def _cell_click(self, row: int, column: int) -> None:
        address = self._cell_address(row, column)
        try:
            raw = self.app.machine.read_memory(address, 8 if self.qwords else 1)
        except MemoryFault:
            return
        value = int.from_bytes(raw, "little")
        self._select(address, value)

    def _cell_double(self, row: int, column: int) -> None:
        self._double_click(self._cell_address(row, column))

    def _jump_from_entry(self) -> None:
        try:
            assert self.jump_value is not None
            self.jump_to(parse_address(self.jump_value.get()))
        except (AssertionError, ValueError) as error:
            self.app.set_status(str(error))

    def jump_to(self, address: int) -> None:
        self.base = address & ~0xF
        if self.jump_value is not None:
            self.jump_value.set(f"0x{address:X}")
        self.open()

    def _write_selected(self) -> None:
        if self.selected is None or self.edit_value is None:
            return
        try:
            if self.qwords:
                data = parse_address(self.edit_value.get()).to_bytes(8, "little", signed=False)
            else:
                candidate = self.edit_value.get().strip().lower().removeprefix("0x")
                try:
                    value = int(candidate, 16)
                except ValueError as error:
                    raise ValueError(text("error.hex_byte")) from error
                if not 0 <= value <= 0xFF:
                    raise ValueError(text("error.hex_byte"))
                data = bytes((value,))
            self.app.machine.write_memory(self.selected, data)
            self.app.set_status("")
            self.app.refresh()
        except (MemoryFault, OverflowError, ValueError) as error:
            self.app.set_status(str(error) if str(error) else text("error.hex_byte"))

    def _follow_selected(self) -> None:
        if self.selected is None:
            return
        target = self.pointer_target(self.selected)
        if target is not None:
            self.jump_to(target)

    def pointer_field_base(self, address: int) -> int | None:
        for offset in (G_COOKIES, G_USER, G_CURSORS, G_FUNCTIONS):
            base = GLOBALS + offset
            if base <= address < base + 8:
                return base
        table = GLOBALS + G_CURSOR_TABLE
        if table <= address < table + MAX_CURSORS * 8:
            return table + ((address - table) // 8) * 8
        if FUNCTION_TABLE <= address < FUNCTION_TABLE + len(FUNCTIONS) * 8:
            return FUNCTION_TABLE + ((address - FUNCTION_TABLE) // 8) * 8
        for allocation in self.app.machine.allocations:
            if (
                allocation.semantic_type == "struct User"
                and allocation.base + 8 <= address < allocation.base + 16
            ):
                return allocation.base + 8
            if allocation.semantic_type == "struct Cursor":
                if allocation.base + 16 <= address < allocation.base + 24:
                    return allocation.base + 16
                if allocation.base + 24 <= address < allocation.base + 32:
                    return allocation.base + 24
        return None

    def pointer_target(self, address: int) -> int | None:
        base = self.pointer_field_base(address)
        if base is None:
            return None
        try:
            target = self.app.machine.memory.read_u64(base)
            self.app.machine.memory.read(target, 1)
            return target
        except MemoryFault:
            return None

    def string_region(self, address: int) -> tuple[int, int] | None:
        """Return the (base, length) of the character buffer containing address.

        Covers the heap string buffers (username / Cursor description) and the
        inline ``char[8]`` Cursor name field. Returns ``None`` for non-string
        bytes.
        """
        for allocation in self.app.machine.allocations:
            if not allocation.base <= address < allocation.base + allocation.size:
                continue
            if allocation.semantic_type in (
                "char username[32]",
                "char Cursor.description[64]",
            ):
                return allocation.base, allocation.size
            if allocation.semantic_type == "struct Cursor":
                offset = address - allocation.base
                if CURSOR_NAME <= offset < CURSOR_NAME + 8:
                    return allocation.base + CURSOR_NAME, 8
        return None

    def field_color(self, address: int) -> str | None:
        for index, (_key, entry) in enumerate(FUNCTIONS):
            if entry <= address < entry + FN_SLOT_SIZE:
                return self.FIELD_COLORS[index % len(self.FIELD_COLORS)]
        if self.string_region(address) is not None:
            return self.STRING_COLOR
        for allocation in self.app.machine.allocations:
            if allocation.base <= address < allocation.base + allocation.size:
                offset = address - allocation.base
                if allocation.semantic_type == "struct User":
                    field_index = 0 if offset < 8 else 1
                elif allocation.semantic_type == "struct Cursor":
                    field_index = min(offset // 8, 3)
                else:
                    field_index = 0
                return self.FIELD_COLORS[field_index]
        return None

    def struct_border_color(self, address: int) -> str | None:
        """Border color for a cell that falls inside a struct instance.

        Used to draw a bounding box around each Cookies / User / Cursor block
        so struct boundaries are obvious in the dump.
        """
        for allocation in self.app.machine.allocations:
            if (
                allocation.semantic_type in self.STRUCT_TYPES
                and allocation.base <= address < allocation.base + allocation.size
            ):
                return self.STRUCT_BORDERS[allocation.semantic_type]
        return None

    def field_info(self, address: int) -> str | None:
        global_fields = (
            (G_COOKIES, text("schema.cookies_pointer"), text("schema.pointer")),
            (G_USER, text("schema.user_pointer"), text("schema.pointer")),
            (G_CURSORS, text("schema.cursor_array_pointer"), text("schema.pointer")),
            (G_CURSOR_COUNT, text("schema.cursor_count"), text("schema.u64")),
            (G_MAX_FIELD_SIZE, text("schema.max_field_size"), text("schema.u64")),
            (G_FUNCTIONS, text("schema.functions"), text("schema.pointer")),
            (G_WON, text("schema.won"), text("schema.u64")),
            (G_FLAG_OWNED, text("schema.flag_owned"), text("schema.u64")),
        )
        for offset, name, kind in global_fields:
            base = GLOBALS + offset
            if base <= address < base + 8:
                value = self.app.machine.memory.read_u64(base)
                return self._with_pointer_preview(
                    f"{name}\n{kind}\n{text('schema.value')} 0x{value:016X}\n"
                    f"{text('schema.address')} 0x{base:016X}",
                    address,
                )
        table = GLOBALS + G_CURSOR_TABLE
        if table <= address < table + MAX_CURSORS * 8:
            index = (address - table) // 8
            base = table + index * 8
            value = self.app.machine.memory.read_u64(base)
            return self._with_pointer_preview(
                f"{text('schema.cursor')}*[{index}]\n{text('schema.pointer')}\n"
                f"{text('schema.value')} 0x{value:016X}\n{text('schema.address')} 0x{base:016X}",
                address,
            )
        for index, (key, _entry) in enumerate(FUNCTIONS):
            base = FUNCTION_TABLE + index * 8
            if base <= address < base + 8:
                value = self.app.machine.memory.read_u64(base)
                return self._with_pointer_preview(
                    f"{text(key)}\n{text('schema.function_pointer')}\n{text('schema.value')} 0x{value:016X}",
                    address,
                )
        for key, entry in FUNCTIONS:
            if entry <= address < entry + FN_SLOT_SIZE:
                return (
                    f"{text(key)}\n{text('schema.bytecode_function')}\n"
                    f"{text('schema.offset')} +0x{address-entry:X}\n"
                    f"{text('schema.address')} 0x{address:016X}"
                )
        for allocation in self.app.machine.allocations:
            if not allocation.base <= address < allocation.base + allocation.size:
                continue
            offset = address - allocation.base
            semantic = allocation.semantic_type
            if semantic == "struct Cookies":
                name, kind = text("schema.cookies"), text("schema.u64")
            elif semantic == "struct User":
                name, kind = (
                    (text("schema.id"), text("schema.u64"))
                    if offset < 8
                    else (text("schema.username"), text("schema.char_pointer"))
                )
            elif semantic == "struct Cursor":
                if offset < 8:
                    name, kind = text("schema.id"), text("schema.u64")
                elif offset < 16:
                    name, kind = text("schema.name"), text("schema.char8")
                elif offset < 24:
                    name, kind = text("schema.description"), text("schema.char_pointer")
                else:
                    name, kind = text("schema.on_clicked"), text("schema.function_pointer")
            elif semantic == "char username[32]":
                name, kind = text("schema.username"), text("schema.char32")
            elif semantic == "char Cursor.description[64]":
                name, kind = text("schema.description"), text("schema.char64")
            else:
                name, kind = semantic, text("schema.bytes")
            character_buffer = kind in (text("schema.char32"), text("schema.char64"))
            field_base = allocation.base if character_buffer else allocation.base + (offset // 8) * 8
            field_length = allocation.size if character_buffer else 8
            try:
                raw = self.app.machine.read_memory(field_base, field_length)
                if kind == text("schema.u64"):
                    number = int.from_bytes(raw, "little")
                    value = f"{number} (0x{number:016X})"
                elif kind == text("schema.char8"):
                    value = repr(raw.split(b"\0", 1)[0].decode("utf-8", errors="replace"))
                elif character_buffer:
                    value = repr(raw.split(b"\0", 1)[0].decode("utf-8", errors="replace"))
                elif kind in (text("schema.char_pointer"), text("schema.function_pointer")):
                    number = int.from_bytes(raw, "little")
                    symbol = ""
                    for function_key, function_entry in FUNCTIONS:
                        if number == function_entry:
                            symbol = f" ({text(function_key)})"
                            break
                    value = f"0x{number:016X}{symbol}"
                else:
                    value = "".join(chr(byte) if 32 <= byte < 127 else "." for byte in raw)
            except MemoryFault:
                value = text("schema.unreadable")
            message = (
                f"{name}\n{kind}\n{text('schema.value')} {value}\n"
                f"{self._object_name(semantic)} +0x{offset:X}\n"
                f"{text('schema.address')} 0x{address:016X}"
            )
            return self._with_pointer_preview(message, address)
        return None

    @staticmethod
    def _object_name(semantic: str) -> str:
        return {
            "struct Cookies": text("schema.object.cookies"),
            "struct User": text("schema.object.user"),
            "struct Cursor": text("schema.object.cursor"),
            "char username[32]": text("schema.object.username"),
            "char Cursor.description[64]": text("schema.object.description"),
        }.get(semantic, semantic)

    def _with_pointer_preview(self, message: str, address: int) -> str:
        base = self.pointer_field_base(address)
        if base is None:
            return message
        try:
            target = self.app.machine.memory.read_u64(base)
            raw = self.app.machine.read_memory(target, 16)
            hexadecimal = " ".join(f"{byte:02X}" for byte in raw)
            ascii_view = "".join(chr(byte) if 32 <= byte < 127 else "." for byte in raw)
            return f"{message}\n{text('schema.points_to')} 0x{target:016X}\n{hexadecimal}\n{ascii_view}"
        except MemoryFault:
            target = self.app.machine.memory.read_u64(base)
            return f"{message}\n{text('schema.points_to')} 0x{target:016X}\n{text('memory.unmapped')}"

    def _select(self, address: int, value: int) -> None:
        self.selected = address
        self.selected_label.configure(text=f"0x{address:016X}")
        assert self.edit_value is not None
        self.edit_value.set(f"0x{value:X}" if self.qwords else f"{value:02X}")
        self.refresh()

    def _double_click(self, address: int) -> None:
        target = self.pointer_target(address)
        if target is not None:
            self.jump_to(target)

    def _show_cell_info(self, widget: tk.Widget, address: int) -> None:
        if self.tooltip is None:
            return
        info = self.field_info(address)
        if info:
            self.tooltip.show_text(widget, info)
        # When hovering a pointer that targets a character buffer, bold that
        # string's bytes in the dump (if they are currently visible).
        new_range: tuple[int, int] | None = None
        if self.pointer_field_base(address) is not None:
            target = self.pointer_target(address)
            if target is not None:
                region = self.string_region(target)
                if region is not None:
                    base, length = region
                    new_range = (base, base + length)
        if new_range != self.bold_range:
            self.bold_range = new_range
            self.refresh()

    def _clear_bold(self, _event: object = None) -> None:
        if self.bold_range is not None:
            self.bold_range = None
            self.refresh()

    def _start_flash(self, address: int) -> None:
        """Flash a changed cell red briefly, then let it revert to normal."""
        if self.window is None:
            return
        self.flashing.add(address)
        existing = self.flash_handles.get(address)
        if existing is not None:
            try:
                self.window.after_cancel(existing)
            except tk.TclError:
                pass
        self.flash_handles[address] = self.window.after(
            450, lambda a=address: self._end_flash(a)
        )

    def _end_flash(self, address: int) -> None:
        self.flashing.discard(address)
        self.flash_handles.pop(address, None)
        self.refresh()

    def _on_cell_leave(self, event: object = None) -> None:
        if self.tooltip is not None:
            self.tooltip.schedule_hide(event)
        self._clear_bold(event)

    def _render_functions(self) -> None:
        assert self.function_frame is not None
        if self.base == FUNCTION_TABLE:
            try:
                signature: object = ("visible", self.app.machine.function_table())
            except GameError:
                signature = ("visible", "unmapped")
        else:
            signature = ("hidden",)
        if signature == self.function_signature:
            return
        self.function_signature = signature
        for child in self.function_frame.winfo_children():
            child.destroy()
        if self.base != FUNCTION_TABLE:
            return
        ttk.Label(self.function_frame, text=text("functions.name")).grid(row=0, column=0, padx=8)
        ttk.Label(self.function_frame, text=text("functions.address")).grid(row=0, column=1, padx=8)
        try:
            for row, (name, address) in enumerate(self.app.machine.function_table(), 1):
                ttk.Label(self.function_frame, text=name, font="TkFixedFont").grid(row=row, column=0)
                tk.Button(
                    self.function_frame,
                    text=f"0x{address:016X}",
                    fg="red",
                    relief="flat",
                    command=lambda value=address: self.jump_to(value),
                ).grid(row=row, column=1)
        except GameError:
            ttk.Label(self.function_frame, text=text("memory.unmapped")).grid(row=1, column=0)

    def refresh(self) -> None:
        if self.window is None or not self.window.winfo_exists() or self.window.state() == "withdrawn":
            return
        self._render_functions()
        self._update_scrollbar()
        if self.status_value is not None:
            if self.status_value.get() != self.app.status:
                self.status_value.set(self.app.status)
        if self.selected is None:
            self.follow_button.state(["disabled"])
        elif self.pointer_field_base(self.selected) is None:
            self.follow_button.state(["disabled"])
        else:
            self.follow_button.state(["!disabled"])
        window_bg = self.window.cget("background")
        for row in range(self.row_count):
            base = self.base + row * 16
            address_text = f"{base:016X}"
            if self.address_states[row] != address_text:
                self.address_labels[row].configure(text=address_text)
                self.address_states[row] = address_text
            try:
                raw = self.app.machine.read_memory(base, 16)
                ascii_text = "".join(chr(byte) if 32 <= byte < 127 else "." for byte in raw)
                if self.ascii_states[row] != ascii_text:
                    self.ascii_labels[row].configure(text=ascii_text)
                    self.ascii_states[row] = ascii_text
                values = (
                    [int.from_bytes(raw[0:8], "little"), int.from_bytes(raw[8:16], "little")]
                    if self.qwords
                    else list(raw)
                )
                for column, (cell, value) in enumerate(zip(self.cells[row], values, strict=True)):
                    address = base + column * (8 if self.qwords else 1)
                    # Flash a cell red when its displayed value changes for any
                    # reason (game tick, direct edit, overflow, pointer follow).
                    previous = self.cell_values.get(address)
                    if previous is not None and previous != value:
                        self._start_flash(address)
                    self.cell_values[address] = value
                    flash = address in self.flashing
                    pointer = self.pointer_field_base(address) is not None
                    border_color = self.struct_border_color(address)
                    field = self.field_color(address)
                    display = f"{value:016X}" if self.qwords else f"{value:02X}"
                    bold = (
                        self.bold_range is not None
                        and self.bold_range[0] <= address < self.bold_range[1]
                    )
                    decorated = field is not None or pointer
                    if bold:
                        font = self.bold_underlined_font if decorated else self.bold_font
                    elif decorated:
                        font = self.underlined_font
                    else:
                        font = self.normal_font
                    if flash:
                        background = "red"
                        foreground = "white"
                    else:
                        background = field or window_bg
                        foreground = "red" if pointer else "black"
                    relief = "sunken" if self.selected == address else "flat"
                    highlight = border_color or window_bg
                    thickness = 2 if border_color is not None else 0
                    state = (
                        display, pointer, background, foreground, font, relief,
                        highlight, thickness, bold, flash, address, value,
                    )
                    if self.cell_states[row][column] == state:
                        continue
                    cell.configure(
                        text=display,
                        fg=foreground,
                        bg=background,
                        font=font,
                        relief=relief,
                        highlightthickness=thickness,
                        highlightbackground=highlight,
                        highlightcolor=highlight,
                    )
                    self.cell_states[row][column] = state
            except MemoryFault:
                unmapped = text("memory.unmapped")
                if self.ascii_states[row] != unmapped:
                    self.ascii_labels[row].configure(text=unmapped)
                    self.ascii_states[row] = unmapped
                for column, cell in enumerate(self.cells[row]):
                    address = base + column * (8 if self.qwords else 1)
                    self.cell_values.pop(address, None)
                    state = ("--", False, window_bg, self.normal_font, "flat")
                    if self.cell_states[row][column] != state:
                        cell.configure(
                            text="--",
                            fg="black",
                            bg=window_bg,
                            font=self.normal_font,
                            relief="flat",
                            highlightthickness=0,
                            highlightbackground=window_bg,
                            highlightcolor=window_bg,
                        )
                        self.cell_states[row][column] = state


class CursorEditor:
    def __init__(self, app: "CookieApp", index: int, view: GameView) -> None:
        self.app = app
        self.index = index
        cursor = view.cursors[index]
        self.window = tk.Toplevel(app.root)
        self.window.title(text("cursor.edit.title"))
        self.window.geometry(f"430x280+{app.root.winfo_rootx() + app.root.winfo_width() + 10}+80")
        frame = ttk.Frame(self.window, padding=10)
        frame.pack(fill="both", expand=True)
        ttk.Label(frame, text=text("cursor.name")).pack(anchor="w")
        self.name = tk.StringVar(value=cursor.name)
        name_entry = ttk.Entry(frame, textvariable=self.name)
        name_entry.pack(fill="x")
        bind_select_all(name_entry)
        ttk.Button(frame, text=text("cursor.name.update"), command=self.update_name).pack(
            anchor="w", pady=(4, 12)
        )
        ttk.Label(frame, text=text("cursor.description")).pack(anchor="w")
        self.description = tk.Text(frame, height=5, width=48)
        self.description.insert("1.0", cursor.description)
        bind_select_all(self.description)
        self.description.pack(fill="both", expand=True)
        ttk.Button(
            frame, text=text("cursor.description.update"), command=self.update_description
        ).pack(anchor="w", pady=(4, 0))

    def update_name(self) -> None:
        self.app.execute(lambda: self.app.machine.rename_cursor(self.index, parse_field(self.name.get())))

    def update_description(self) -> None:
        value = self.description.get("1.0", "end-1c")
        self.app.execute(lambda: self.app.machine.describe_cursor(self.index, parse_field(value)))


class CookieApp:
    def __init__(self, machine: Machine) -> None:
        self.machine = machine
        self.last_valid = machine.clone()
        self.status = ""
        self.root = tk.Tk()
        self.root.title(text("app.title"))
        self.root.geometry("900x760")
        self.root.minsize(500, 560)
        self.pointer_mode = tk.BooleanVar(value=False)
        self.username = tk.StringVar(value=self.safe_view().username)
        self._last_synced_username = self.username.get()
        self.status_value = tk.StringVar()
        self.memory = MemoryWindow(self)
        self.pointer_tooltip = HoverPopup(self.root, hide_delay=600)
        self.cookie_image = self._load_cookie_image()
        self.cursor_editors: list[CursorEditor] = []
        self.cursor_signature: tuple[tuple[object, ...], ...] | None = None
        self.cursor_widgets: list[tuple[ttk.Label, ttk.Button]] = []
        self._build()
        self.root.after(50, self._tick)

    def _load_cookie_image(self) -> tk.PhotoImage | None:
        try:
            width, height, palette, rows = cookie_sprite()
            image = tk.PhotoImage(width=width, height=height)
            for y, row in enumerate(rows):
                for x, symbol in enumerate(row):
                    color, transparent = palette[symbol]
                    image.put(color, (x, y))
                    if transparent:
                        image.transparency_set(x, y, True)
            return image.zoom(10, 10)
        except (ValueError, tk.TclError):
            return None

    def _build(self) -> None:
        self.main = ttk.Frame(self.root, padding=12)
        self.main.pack(fill="both", expand=True)
        toolbar = ttk.Frame(self.main)
        toolbar.pack(fill="x")
        ttk.Button(toolbar, text=text("memory.open"), command=self.memory.open).pack(side="right")
        ttk.Checkbutton(
            toolbar,
            text=text("pointer.toggle"),
            variable=self.pointer_mode,
            command=self.refresh,
        ).pack(side="right", padx=(0, 8))

        self.game_frame = ttk.Frame(self.main)
        self.game_frame.pack(fill="both", expand=True)
        self.play_frame = ttk.Frame(self.game_frame)
        self.play_frame.pack(fill="both", expand=True)
        self.win_frame = ttk.Frame(self.game_frame)
        ttk.Label(self.win_frame, text=text("game.win.title"), font="TkHeadingFont").pack(pady=100)
        ttk.Button(self.win_frame, text=text("memory.open"), command=self.memory.open).pack()

        self.user_row = ttk.Frame(self.play_frame)
        self.user_row.pack(fill="x", pady=(12, 0))
        self.user_label = ttk.Label(self.user_row)
        self.user_label.pack(side="left")
        self.user_pointer = self._pointer_widget(
            self.user_row, text("schema.username"), 0
        )
        self.username_entry = ttk.Entry(self.user_row, textvariable=self.username)
        self.username_entry.pack(side="left", fill="x", expand=True, padx=6)
        bind_select_all(self.username_entry)
        ttk.Button(
            self.user_row,
            text=text("game.username.change"),
            command=lambda: self.execute(
                lambda: self.machine.rename_user(parse_field(self.username.get()))
            ),
        ).pack(side="left")

        self.cookie_count = ttk.Label(self.play_frame, font="TkHeadingFont")
        self.cookie_count.pack(pady=(28, 8))
        self.cookie_button = ttk.Button(
            self.play_frame,
            text=text("game.cookie") if self.cookie_image is None else "",
            image=self.cookie_image,
            command=lambda: self.execute(self.machine.click),
            compound="center",
        )
        self.cookie_button.pack(ipadx=8, ipady=8)
        self.cookie_pointer = self._pointer_widget(
            self.play_frame, text("schema.cookies_value"), 0
        )

        self.purchases = ttk.Frame(self.play_frame)
        self.purchases.pack(pady=18)
        self.buy_cursor_button = ttk.Button(
            self.purchases,
            text=f"{text('game.cursor.buy')} — {CURSOR_PRICE}",
            command=lambda: self.execute(self.machine.buy_cursor),
        )
        self.buy_cursor_button.pack(side="left", padx=4)
        self.buy_flag_button = ttk.Button(
            self.purchases,
            text=f"{text('game.flag.buy')} — {FLAG_PRICE}",
            command=lambda: self.execute(self.machine.buy_flag),
        )
        self.buy_flag_button.pack(side="left", padx=4)
        self.flag_owned_label = ttk.Label(self.purchases, text=text("game.flag.owned"))

        ttk.Separator(self.play_frame).pack(fill="x", pady=(0, 10))
        ttk.Label(self.play_frame, text=text("game.cursors"), font="TkHeadingFont").pack(
            anchor="w"
        )
        self.cursor_list = ttk.Frame(self.play_frame)
        self.cursor_list.pack(fill="both", expand=True)
        self.status_label = ttk.Label(self.main, textvariable=self.status_value)
        self.status_label.pack(fill="x", pady=(8, 0))
        self.refresh()

    def safe_view(self) -> GameView:
        try:
            view = self.machine.view()
            self.last_valid = self.machine.clone()
            return view
        except SegmentationFault as error:
            self.machine = self.last_valid.clone()
            self.set_status(str(error))
            return self.machine.view()

    def set_status(self, message: str) -> None:
        self.status = message
        if hasattr(self, "status_value"):
            self.status_value.set(message)
        if hasattr(self, "memory"):
            self.memory.refresh()

    def execute(self, operation: Callable[[], None]) -> None:
        try:
            operation()
            self.set_status("")
        except SegmentationFault as error:
            self.machine = self.last_valid.clone()
            self.set_status(str(error))
        except (GameError, MemoryFault, ValueError) as error:
            self.set_status(str(error))
        self.refresh()

    def _pointer_widget(self, parent: tk.Misc, label: str, address: int) -> ttk.Button:
        button = ttk.Button(parent)
        self._configure_pointer_widget(button, label, address)
        return button

    def _configure_pointer_widget(
        self, button: ttk.Button, label: str, address: int
    ) -> None:
        button.configure(text=f"0x{address:016X}", command=lambda: self.memory.open(address))
        button.bind(
            "<Enter>",
            lambda _event: self.pointer_tooltip.show_pointer(button, label, address, self.memory.open),
        )
        button.bind("<Leave>", self.pointer_tooltip.schedule_hide)

    def _bind_pointer_hover(self, widget: ttk.Widget, label: str, address: int) -> None:
        widget.bind(
            "<Enter>",
            lambda _event: self.pointer_tooltip.show_pointer(
                widget, label, address, self.memory.open
            ),
        )
        widget.bind("<Leave>", self.pointer_tooltip.schedule_hide)

    def _rebuild_cursors(self, view: GameView) -> None:
        for child in self.cursor_list.winfo_children():
            child.destroy()
        self.cursor_widgets.clear()
        for index, cursor in enumerate(view.cursors):
            row = ttk.Frame(self.cursor_list)
            row.pack(fill="x", pady=3)
            label = ttk.Label(row, text=f"{cursor.name} #{cursor.id}")
            label.pack(side="left")
            pointer = self._pointer_widget(row, text("schema.cursor"), cursor.address)
            ttk.Label(row, text=cursor.description).pack(
                side="left", fill="x", expand=True, padx=8
            )
            ttk.Button(
                row,
                text=text("game.cursor.edit"),
                command=lambda value=index: self.edit_cursor(value),
            ).pack(side="right", padx=4)
            self.cursor_widgets.append((label, pointer))

    def _update_pointer_controls(self, view: GameView) -> None:
        enabled = self.pointer_mode.get()
        self._configure_pointer_widget(
            self.user_pointer, text("schema.username"), view.username_ptr
        )
        self._configure_pointer_widget(
            self.cookie_pointer, text("schema.cookies_value"), view.cookies_ptr
        )
        if enabled:
            if not self.user_pointer.winfo_manager():
                self.user_pointer.pack(side="left", padx=6, after=self.user_label)
            if not self.cookie_pointer.winfo_manager():
                self.cookie_pointer.pack(pady=(4, 0), after=self.cookie_button)
            self._bind_pointer_hover(
                self.user_label, text("schema.username"), view.username_ptr
            )
            self._bind_pointer_hover(
                self.cookie_button, text("schema.cookies_value"), view.cookies_ptr
            )
        else:
            self.user_pointer.pack_forget()
            self.cookie_pointer.pack_forget()
            self.user_label.unbind("<Enter>")
            self.user_label.unbind("<Leave>")
            self.cookie_button.unbind("<Enter>")
            self.cookie_button.unbind("<Leave>")
            self.pointer_tooltip.hide()
        for cursor, (label, pointer) in zip(view.cursors, self.cursor_widgets, strict=True):
            self._configure_pointer_widget(pointer, text("schema.cursor"), cursor.address)
            if enabled:
                if not pointer.winfo_manager():
                    pointer.pack(side="left", padx=5, after=label)
                self._bind_pointer_hover(label, text("schema.cursor"), cursor.address)
            else:
                pointer.pack_forget()
                label.unbind("<Enter>")
                label.unbind("<Leave>")

    def refresh(self) -> None:
        if not hasattr(self, "game_frame"):
            return
        view = self.safe_view()
        if view.won:
            self.play_frame.pack_forget()
            if not self.win_frame.winfo_manager():
                self.win_frame.pack(fill="both", expand=True)
            self.memory.refresh()
            return
        self.win_frame.pack_forget()
        if not self.play_frame.winfo_manager():
            self.play_frame.pack(fill="both", expand=True)
        self.user_label.configure(text=f"{text('game.user')} {view.username}")
        # Sync the editable textbox only when the username changes in the model
        # (memory edit, overflow, rename), not on every refresh. Comparing the
        # entry to the view would wipe whatever the user is currently typing on
        # the next 50 ms tick, so we compare against the last value we synced.
        if view.username != self._last_synced_username:
            self._last_synced_username = view.username
            self.username.set(view.username)
        self.cookie_count.configure(text=f"{view.cookies} {text('game.cookies')}")
        if view.cookies < CURSOR_PRICE:
            self.buy_cursor_button.state(["disabled"])
        else:
            self.buy_cursor_button.state(["!disabled"])
        if view.cookies < FLAG_PRICE or view.flag_owned:
            self.buy_flag_button.state(["disabled"])
        else:
            self.buy_flag_button.state(["!disabled"])
        if view.flag_owned:
            if not self.flag_owned_label.winfo_manager():
                self.flag_owned_label.pack(side="left", padx=6)
        else:
            self.flag_owned_label.pack_forget()
        signature = tuple(
            (cursor.address, cursor.id, cursor.name, cursor.description)
            for cursor in view.cursors
        )
        if signature != self.cursor_signature:
            self._rebuild_cursors(view)
            self.cursor_signature = signature
        self._update_pointer_controls(view)
        self.status_value.set(self.status)
        self.memory.refresh()

    def edit_cursor(self, index: int) -> None:
        view = self.safe_view()
        self.memory.open(view.cursors[index].address)
        self.cursor_editors.append(CursorEditor(self, index, view))

    def _tick(self) -> None:
        try:
            self.machine.advance_tick()
        except SegmentationFault as error:
            self.machine = self.last_valid.clone()
            self.set_status(str(error))
        self.refresh()
        self.root.after(50, self._tick)

    def run(self) -> None:
        self.root.mainloop()
