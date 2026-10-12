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
    FN_ADD_COOKIE,
    FN_WIN,
    FUNCTION_TABLE,
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
    def __init__(self, owner: tk.Misc) -> None:
        self.owner = owner
        self.window: tk.Toplevel | None = None
        self.pending: str | None = None

    def cancel_hide(self, _event: object = None) -> None:
        if self.pending:
            self.owner.after_cancel(self.pending)
            self.pending = None

    def schedule_hide(self, _event: object = None) -> None:
        self.cancel_hide()
        self.pending = self.owner.after(600, self.hide)

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
    FIELD_COLORS = ("lightblue", "lightgreen", "khaki", "lightcoral")

    def __init__(self, app: "CookieApp") -> None:
        self.app = app
        self.window: tk.Toplevel | None = None
        self.base = GLOBALS
        self.qwords = False
        self.selected: int | None = None
        self.jump_value: tk.StringVar | None = None
        self.edit_value: tk.StringVar | None = None
        self.mode_value: tk.StringVar | None = None
        self.status_value: tk.StringVar | None = None
        self.table: ttk.Frame | None = None
        self.function_frame: ttk.Frame | None = None
        self.cells: list[list[tk.Label]] = []
        self.address_labels: list[ttk.Label] = []
        self.ascii_labels: list[ttk.Label] = []
        self.tooltip: HoverPopup | None = None
        self.normal_font: tkfont.Font | None = None
        self.underlined_font: tkfont.Font | None = None
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
        self.underlined_font = self.normal_font.copy()
        self.underlined_font.configure(underline=True)

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
        ttk.Entry(controls, textvariable=self.jump_value, width=18).pack(side="left")
        ttk.Button(controls, text=text("memory.jump"), command=self._jump_from_entry).pack(
            side="left", padx=4
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
        ttk.Entry(editor, textvariable=self.edit_value, width=22).pack(side="left")
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
        vertical = ttk.Scrollbar(container, orient="vertical", command=canvas.yview)
        horizontal = ttk.Scrollbar(container, orient="horizontal", command=canvas.xview)
        canvas.configure(yscrollcommand=vertical.set, xscrollcommand=horizontal.set)
        vertical.pack(side="right", fill="y")
        horizontal.pack(side="bottom", fill="x")
        canvas.pack(side="left", fill="both", expand=True)
        self.table = ttk.Frame(canvas, padding=6)
        item = canvas.create_window((0, 0), window=self.table, anchor="nw")
        self.table.bind(
            "<Configure>", lambda _event: canvas.configure(scrollregion=canvas.bbox("all"))
        )
        canvas.bind("<Configure>", lambda event: canvas.itemconfigure(item, height=max(event.height, 1)))
        self._rebuild_grid()

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
        width = 16 if self.qwords else 2
        count = 2 if self.qwords else 16
        for row in range(32):
            address = ttk.Label(self.table, width=16, font="TkFixedFont", anchor="e")
            address.grid(row=row, column=0, padx=(0, 8), sticky="e")
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
                row_cells.append(cell)
            self.cells.append(row_cells)
            self.cell_states.append([None] * count)
            ascii_label = ttk.Label(self.table, width=16, font="TkFixedFont", anchor="w")
            ascii_label.grid(row=row, column=count + 1, padx=(12, 0), sticky="w")
            self.ascii_labels.append(ascii_label)
            self.ascii_states.append(None)

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
        if FUNCTION_TABLE <= address < FUNCTION_TABLE + 16:
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

    def field_color(self, address: int) -> str | None:
        if FN_ADD_COOKIE <= address < FN_ADD_COOKIE + 16:
            return self.FIELD_COLORS[0]
        if FN_WIN <= address < FN_WIN + 8:
            return self.FIELD_COLORS[1]
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
        for base, name in (
            (FUNCTION_TABLE, text("functions.add_cookie")),
            (FUNCTION_TABLE + 8, text("functions.win")),
        ):
            if base <= address < base + 8:
                value = self.app.machine.memory.read_u64(base)
                return self._with_pointer_preview(
                    f"{name}\n{text('schema.function_pointer')}\n{text('schema.value')} 0x{value:016X}",
                    address,
                )
        for start, end, name in (
            (FN_ADD_COOKIE, FN_ADD_COOKIE + 16, text("functions.add_cookie")),
            (FN_WIN, FN_WIN + 8, text("functions.win")),
        ):
            if start <= address < end:
                return (
                    f"{name}\n{text('schema.bytecode_function')}\n"
                    f"{text('schema.offset')} +0x{address-start:X}\n"
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
                    if number == FN_ADD_COOKIE:
                        symbol = f" ({text('functions.add_cookie')})"
                    elif number == FN_WIN:
                        symbol = f" ({text('functions.win')})"
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
        if self.status_value is not None:
            if self.status_value.get() != self.app.status:
                self.status_value.set(self.app.status)
        if self.selected is None:
            self.follow_button.state(["disabled"])
        elif self.pointer_field_base(self.selected) is None:
            self.follow_button.state(["disabled"])
        else:
            self.follow_button.state(["!disabled"])
        for row in range(32):
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
                    pointer = self.pointer_field_base(address) is not None
                    background = self.field_color(address) or self.window.cget("background")
                    display = f"{value:016X}" if self.qwords else f"{value:02X}"
                    font = (
                        self.underlined_font
                        if self.field_color(address) is not None or pointer
                        else self.normal_font
                    )
                    relief = "sunken" if self.selected == address else "flat"
                    state = (display, pointer, background, font, relief, address, value)
                    if self.cell_states[row][column] == state:
                        continue
                    cell.configure(
                        text=display,
                        fg="red" if pointer else "black",
                        bg=background,
                        font=font,
                        relief=relief,
                    )
                    cell.bind("<Button-1>", lambda _event, a=address, v=value: self._select(a, v))
                    cell.bind("<Double-Button-1>", lambda _event, a=address: self._double_click(a))
                    cell.bind("<Enter>", lambda event, a=address: self._show_cell_info(event.widget, a))
                    if self.tooltip is not None:
                        cell.bind("<Leave>", self.tooltip.schedule_hide)
                    self.cell_states[row][column] = state
            except MemoryFault:
                unmapped = text("memory.unmapped")
                if self.ascii_states[row] != unmapped:
                    self.ascii_labels[row].configure(text=unmapped)
                    self.ascii_states[row] = unmapped
                for column, cell in enumerate(self.cells[row]):
                    state = ("--", False, self.window.cget("background"), self.normal_font, "flat")
                    if self.cell_states[row][column] != state:
                        cell.configure(
                            text="--",
                            fg="black",
                            bg=self.window.cget("background"),
                            font=self.normal_font,
                            relief="flat",
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
        ttk.Entry(frame, textvariable=self.name).pack(fill="x")
        ttk.Button(frame, text=text("cursor.name.update"), command=self.update_name).pack(
            anchor="w", pady=(4, 12)
        )
        ttk.Label(frame, text=text("cursor.description")).pack(anchor="w")
        self.description = tk.Text(frame, height=5, width=48)
        self.description.insert("1.0", cursor.description)
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
        self.status_value = tk.StringVar()
        self.memory = MemoryWindow(self)
        self.pointer_tooltip = HoverPopup(self.root)
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
