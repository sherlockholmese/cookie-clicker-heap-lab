from __future__ import annotations

from typing import Callable

from PyQt6.QtCore import QEvent, QObject, Qt, QTimer
from PyQt6.QtGui import QFont
from PyQt6.QtWidgets import (
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

from .game import (
    CURSOR_DESCRIPTION,
    CURSOR_ID,
    CURSOR_NAME,
    CURSOR_ON_CLICKED,
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
    USER_ID,
    USER_USERNAME,
    Machine,
)
from .memory import CODE, GLOBALS, HEAP, MemoryFault
from .resources import text

ROW_COUNT = 25
BYTES_PER_ROW = 16

COLOR_POINTER = "#d00000"
COLOR_POINTER_DARK = "#ff0000"
COLOR_BYTECODE = "#e08a2c"
COLOR_STRING = "#2ea043"
COLOR_INT = "#1f6feb"
COLOR_STRUCT_PTR = "#e08a2c"
COLOR_SELECTED_BG = "#ffd9a0"
COLOR_PLAIN = "#333333"
COLOR_FLASH_BG = "#ff5050"
STRUCT_FIELD_COLORS = ("#1f6feb", "#2ea043", "#b08900", "#8957e5")
STRUCT_BORDER = "#595959"
TOOLTIP_LABEL = "#333333"

WINDOW_STYLE = """
QWidget#memroot { background: white; }
QPushButton#section, QPushButton#mode {
    background: #efefef; border: 1px solid #5a5a5a; color: black; padding: 5px 10px;
}
QPushButton#section:checked, QPushButton#mode:checked { background: #d7d7d7; font-weight: bold; }
QLabel#title { background: #efefef; border: 1px solid #5a5a5a; padding: 5px; }
QLabel#tooltip { background: #bcbcbc; padding: 10px 16px;}
QLineEdit#jump { background: #efefef; border: 1px solid #5a5a5a; padding: 3px; }
"""


class MemoryWindow(QWidget):
    def __init__(self, machine: Machine, on_change: Callable[[], None] | None = None) -> None:
        super().__init__()
        self.machine = machine
        self.on_change = on_change
        self.base = HEAP
        self.qwords = True
        self.selected: int | None = None
        self.struct_outline: tuple[int, int] | None = None
        self.bold_range: tuple[int, int] | None = None
        self._heap_list_signature: tuple | None = None
        self.font_size = 13

        self.cell_values: dict[int, int] = {}
        self._cell_style_cache: dict = {}
        self.flashing: set[int] = set()
        self.flash_handles: dict[int, QTimer] = {}
        self._hover_clear = QTimer(self)
        self._hover_clear.setSingleShot(True)
        self._hover_clear.setInterval(50)
        self._hover_clear.timeout.connect(self._clear_hover)

        self.setObjectName("memroot")
        self.setWindowTitle(text("memory.title"))
        self.setStyleSheet(WINDOW_STYLE)
        self._mono = QFont("Courier New", self.font_size)

        self.addr_cells: list[QLabel] = []
        self.value_cells: list[list[QLabel]] = []
        self.ascii_cells: list[QLabel] = []
        self._cell_pos: dict[QLabel, tuple[int, int]] = {}

        self._build()
        self.refresh()

    def _build(self) -> None:
        outer = QHBoxLayout(self)
        outer.setContentsMargins(14, 12, 14, 12)
        outer.setSpacing(16)

        left = QVBoxLayout()
        left.setSpacing(8)
        outer.addLayout(left, 1)

        controls = QHBoxLayout()
        controls.setSpacing(6)
        self.section_buttons: dict[str, QPushButton] = {}
        for key, address in (
            ("memory.section.bss", GLOBALS),
            ("memory.section.text", CODE),
            ("memory.section.heap", HEAP),
        ):
            btn = QPushButton(text(key))
            btn.setObjectName("section")
            btn.setCheckable(True)
            btn.clicked.connect(lambda _c, a=address: self.jump_to(a))
            btn.setCursor(Qt.CursorShape.PointingHandCursor)
            controls.addWidget(btn)
            self.section_buttons[key] = btn
        controls.addSpacing(12)
        self.mode_btn = QPushButton(text("memory.bytes"))
        self.mode_btn.setObjectName("mode")
        self.mode_btn.setCheckable(True)
        self.mode_btn.setChecked(True)
        self.mode_btn.clicked.connect(self._toggle_mode)
        controls.addWidget(self.mode_btn)
        controls.addStretch()
        left.addLayout(controls)

        nav = QHBoxLayout()
        nav.setSpacing(6)
        self.jump_edit = QLineEdit()
        self.jump_edit.setObjectName("jump")
        self.jump_edit.setPlaceholderText(text("memory.address"))
        self.jump_edit.setFixedWidth(150)
        self.jump_edit.returnPressed.connect(self._jump_from_entry)
        nav.addWidget(self.jump_edit)
        jump_btn = QPushButton(text("memory.jump"))
        jump_btn.setObjectName("section")
        jump_btn.clicked.connect(self._jump_from_entry)
        nav.addWidget(jump_btn)
        for label, delta in ((text("memory.zoom_out"), -1), (text("memory.zoom_in"), 1)):
            zb = QPushButton(label)
            zb.setObjectName("section")
            zb.setFixedWidth(34)
            zb.clicked.connect(lambda _c, d=delta: self._zoom(d))
            nav.addWidget(zb)
        reset = QPushButton(text("memory.zoom_reset"))
        reset.setObjectName("section")
        reset.clicked.connect(lambda: self._zoom(0, reset=True))
        nav.addWidget(reset)

        nav.addSpacing(12)
        self.selected_label = QLabel("\u2014")
        self.selected_label.setFont(self._mono)
        nav.addWidget(self.selected_label)
        self.edit_value = QLineEdit()
        self.edit_value.setObjectName("jump")
        self.edit_value.setFixedWidth(140)
        self.edit_value.returnPressed.connect(self._write_selected)
        nav.addWidget(self.edit_value)
        write_btn = QPushButton(text("memory.write"))
        write_btn.setObjectName("section")
        write_btn.clicked.connect(self._write_selected)
        nav.addWidget(write_btn)
        self.follow_btn = QPushButton(text("memory.follow"))
        self.follow_btn.setObjectName("section")
        self.follow_btn.clicked.connect(self._follow_selected)
        nav.addWidget(self.follow_btn)
        nav.addStretch()
        left.addLayout(nav)

        title = QLabel(text("memory.title"))
        title.setObjectName("title")
        left.addWidget(title)

        self.grid_host = QWidget()
        self.grid_host.setMouseTracking(True)
        self.grid_host.installEventFilter(self)
        self.grid = QGridLayout(self.grid_host)
        self.grid.setContentsMargins(2, 2, 2, 2)
        self.grid.setHorizontalSpacing(1)
        self.grid.setVerticalSpacing(1)
        left.addWidget(self.grid_host)

        self.status = QLabel("")
        self.status.setStyleSheet("color: #5a5a5a;")
        left.addWidget(self.status)
        left.addStretch()

        right = QVBoxLayout()
        right.setSpacing(8)
        outer.addLayout(right, 0)
        tip_title = QLabel(text("memory.tooltip"))
        tip_title.setObjectName("title")
        tip_title.setFixedWidth(230)
        right.addWidget(tip_title)
        self.tooltip = QLabel("")
        self.tooltip.setObjectName("tooltip")
        self.tooltip.setTextFormat(Qt.TextFormat.RichText)
        self.tooltip.setFont(QFont("Courier New", 12))
        self.tooltip.setWordWrap(True)
        self.tooltip.setFixedWidth(230)
        self.tooltip.setFixedHeight(210)
        self.tooltip.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)
        right.addWidget(self.tooltip)

        self.fn_title = QLabel(text("memory.section.functions"))
        self.fn_title.setObjectName("title")
        self.fn_title.setFixedWidth(230)
        right.addWidget(self.fn_title)
        self.fn_table = QLabel("")
        self.fn_table.setFont(QFont("Courier New", 11))
        self.fn_table.setTextFormat(Qt.TextFormat.RichText)
        self.fn_table.setFixedWidth(230)
        self.fn_table.setAlignment(Qt.AlignmentFlag.AlignTop)
        right.addWidget(self.fn_table)

        self.heap_title = QLabel(text("memory.heap_objects"))
        self.heap_title.setObjectName("title")
        self.heap_title.setFixedWidth(230)
        right.addWidget(self.heap_title)
        self.heap_scroll = QScrollArea()
        self.heap_scroll.setWidgetResizable(True)
        self.heap_scroll.setFixedWidth(230)
        self.heap_scroll.setFrameShape(QScrollArea.Shape.NoFrame)
        self.heap_list_inner = QWidget()
        self.heap_list_layout = QVBoxLayout(self.heap_list_inner)
        self.heap_list_layout.setContentsMargins(16, 10, 16, 10)
        self.heap_list_layout.setSpacing(8)
        self.heap_list_layout.addStretch()
        self.heap_scroll.setWidget(self.heap_list_inner)
        right.addWidget(self.heap_scroll)

        right.addStretch()

        self._build_grid()

    def _build_grid(self) -> None:
        while self.grid.count():
            item = self.grid.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.setParent(None)
                widget.deleteLater()
        self.addr_cells.clear()
        self.value_cells.clear()
        self.ascii_cells.clear()
        self._cell_pos.clear()
        self.cell_values.clear()
        self._cell_style_cache.clear()

        count = 2 if self.qwords else BYTES_PER_ROW
        metrics = self._mono_metrics()
        digits = 16 if self.qwords else 2
        cell_w = metrics.horizontalAdvance("0" * digits) + 10
        addr_w = metrics.horizontalAdvance("0x000000000000") + 16
        ascii_w = metrics.horizontalAdvance("0" * (BYTES_PER_ROW + 1)) + 6
        for row in range(ROW_COUNT):
            addr = QLabel()
            addr.setFont(self._mono)
            addr.setTextFormat(Qt.TextFormat.RichText)
            addr.setFixedWidth(addr_w)
            self.grid.addWidget(addr, row, 0)
            self.addr_cells.append(addr)
            self.grid.addWidget(self._sep(), row, 1)
            cells: list[QLabel] = []
            for col in range(count):
                cell = QLabel()
                cell.setFont(self._mono)
                cell.setFixedWidth(cell_w)
                cell.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter)
                cell.setCursor(Qt.CursorShape.PointingHandCursor)
                cell.setMouseTracking(True)
                cell.installEventFilter(self)
                self._cell_pos[cell] = (row, col)
                grid_col = 2 + col + (1 if (not self.qwords and col >= 8) else 0)
                self.grid.addWidget(cell, row, grid_col)
                cells.append(cell)
            if not self.qwords:
                self.grid.addWidget(self._sep(), row, 10)
            self.value_cells.append(cells)
            sep_col = 2 + count + (1 if not self.qwords else 0)
            self.grid.addWidget(self._sep(), row, sep_col)
            ascii_label = QLabel()
            ascii_label.setFont(self._mono)
            ascii_label.setFixedWidth(ascii_w)
            self.grid.addWidget(ascii_label, row, sep_col + 1)
            self.ascii_cells.append(ascii_label)
        self.grid_host.setFixedHeight(metrics.height() * (ROW_COUNT + 1) + 10)
        self.grid.setRowStretch(ROW_COUNT, 1)
        bar_w = self._sep().sizeHint().width()
        bars = 2 if self.qwords else 3
        columns = 2 + bars + count
        margins = self.grid.contentsMargins().left() + self.grid.contentsMargins().right()
        grid_width = (
            addr_w
            + bars * bar_w
            + count * cell_w
            + ascii_w
            + (columns - 1) * self.grid.horizontalSpacing()
            + margins
        )
        self.grid_host.setFixedWidth(grid_width)
        old_box = getattr(self, "struct_box", None)
        if old_box is not None:
            old_box.setParent(None)
            old_box.deleteLater()
        self.struct_box = QFrame(self.grid_host)
        self.struct_box.setStyleSheet(
            f"background: transparent; border: 2px solid {STRUCT_BORDER};"
        )
        self.struct_box.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self.struct_box.hide()
        self.layout().activate()
        self.setFixedSize(self.sizeHint())

    def _mono_metrics(self):
        from PyQt6.QtGui import QFontMetrics

        return QFontMetrics(self._mono)

    def _sep(self) -> QLabel:
        bar = QLabel("|")
        bar.setFont(self._mono)
        bar.setStyleSheet(f"color: {COLOR_PLAIN};")
        return bar

    def open_at(self, address: int) -> None:
        self.base = address & ~0xF
        self._sync_jump_text()
        self.show()
        self.raise_()
        self.activateWindow()
        self.refresh()

    def jump_to(self, address: int) -> None:
        self.base = address & ~0xF
        self._sync_jump_text()
        self.refresh()

    def _sync_jump_text(self) -> None:
        self.jump_edit.setText(f"0x{self.base:X}")

    def _jump_from_entry(self) -> None:
        value = self.jump_edit.text().strip().replace("_", "")
        try:
            address = int(value, 16 if value.lower().startswith("0x") else 10)
        except ValueError:
            self.status.setText(text("error.address"))
            return
        self.jump_to(address)

    def _mapped_bounds(self) -> tuple[int, int]:
        regions = self.machine.memory.regions
        lowest = min(r.base for r in regions)
        highest = max(r.base + r.size for r in regions)
        return lowest, highest

    def _current_region(self) -> tuple[int, int] | None:
        for region in self.machine.memory.regions:
            if region.base <= self.base < region.base + region.size:
                return region.base, region.size
        return None

    def _scroll_rows(self, rows: int) -> None:
        lowest, highest = self._mapped_bounds()
        new_base = self.base + rows * BYTES_PER_ROW
        new_base = max(lowest, min(new_base, highest - BYTES_PER_ROW))
        self.base = new_base & ~0xF
        self._sync_jump_text()
        self.refresh()

    def wheelEvent(self, event) -> None:
        if event.modifiers() & Qt.KeyboardModifier.ControlModifier:
            self._zoom(1 if event.angleDelta().y() > 0 else -1)
            return
        delta = event.angleDelta().y()
        steps = delta // 120 or (1 if delta > 0 else -1)
        self._scroll_rows(-steps)

    def _zoom(self, direction: int, *, reset: bool = False) -> None:
        if reset:
            self.font_size = 13
        else:
            self.font_size = max(7, min(28, self.font_size + direction))
        self._mono = QFont("Courier New", self.font_size)
        self._build_grid()
        self.refresh()

    def _toggle_mode(self) -> None:
        self.qwords = self.mode_btn.isChecked()
        self.mode_btn.setText(text("memory.bytes") if self.qwords else text("memory.qwords"))
        self._mono = QFont("Courier New", self.font_size)
        self._build_grid()
        self.refresh()

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
        for allocation in self.machine.allocations:
            if allocation.semantic_type == "struct User" and allocation.base + 8 <= address < allocation.base + 16:
                return allocation.base + 8
            if allocation.semantic_type == "struct Cursor":
                if allocation.base + 16 <= address < allocation.base + 24:
                    return allocation.base + 16
                if allocation.base + 24 <= address < allocation.base + 32:
                    return allocation.base + 24
        return None

    def function_slot(self, address: int) -> tuple[str, int] | None:
        for key, entry in FUNCTIONS:
            if entry <= address < entry + FN_SLOT_SIZE:
                return key, entry
        return None

    def string_region(self, address: int) -> tuple[int, int] | None:
        for allocation in self.machine.allocations:
            if not allocation.base <= address < allocation.base + allocation.size:
                continue
            if allocation.semantic_type in ("char username[32]", "char Cursor.description[64]"):
                return allocation.base, allocation.size
            if allocation.semantic_type == "struct Cursor":
                offset = address - allocation.base
                if CURSOR_NAME <= offset < CURSOR_NAME + 8:
                    return allocation.base + CURSOR_NAME, 8
        return None

    def struct_at(self, address: int):
        for allocation in self.machine.allocations:
            if (
                allocation.semantic_type in ("struct Cookies", "struct User", "struct Cursor")
                and allocation.base <= address < allocation.base + allocation.size
            ):
                return allocation
        return None

    def _is_pointer_cell(self, address: int) -> bool:
        return self._field_kind(address) in ("ptr:function", "ptr:char", "ptr:struct")

    def _field_kind(self, address: int) -> str | None:
        slot = self.function_slot(address)
        if slot is not None:
            return "ptr:function" if address < slot[1] + 8 else "bytecode"

        g_fields = {
            G_COOKIES: "ptr:struct",
            G_USER: "ptr:struct",
            G_CURSORS: "ptr:struct",
            G_CURSOR_COUNT: "int",
            G_MAX_FIELD_SIZE: "int",
            G_FUNCTIONS: "ptr:function",
            G_WON: "int",
            G_FLAG_OWNED: "int",
        }
        for offset, kind in g_fields.items():
            if GLOBALS + offset <= address < GLOBALS + offset + 8:
                return kind
        table = GLOBALS + G_CURSOR_TABLE
        if table <= address < table + MAX_CURSORS * 8:
            return "ptr:struct"
        if FUNCTION_TABLE <= address < FUNCTION_TABLE + len(FUNCTIONS) * 8:
            return "ptr:function"

        for alloc in self.machine.allocations:
            if not alloc.base <= address < alloc.base + alloc.size:
                continue
            offset = address - alloc.base
            semantic = alloc.semantic_type
            if semantic == "struct Cookies":
                return "int"
            if semantic == "struct User":
                if offset < USER_USERNAME:
                    return "int"
                return "ptr:char"
            if semantic == "struct Cursor":
                if offset < CURSOR_NAME:
                    return "int"
                if offset < CURSOR_DESCRIPTION:
                    return "char"
                if offset < CURSOR_ON_CLICKED:
                    return "ptr:char"
                return "ptr:function"
            if semantic.startswith("char "):
                return "char"
            return None
        return None

    def _kind_color(self, kind: str | None) -> str:
        return {
            "int": COLOR_INT,
            "char": COLOR_STRING,
            "bytecode": COLOR_BYTECODE,
            "ptr:function": COLOR_POINTER,
            "ptr:char": COLOR_STRING,
            "ptr:struct": COLOR_STRUCT_PTR,
        }.get(kind, COLOR_PLAIN)

    def _byte_color(self, address: int) -> str:
        return self._kind_color(self._field_kind(address))

    def _row_base(self, row: int) -> int:
        return self.base + row * BYTES_PER_ROW

    def _cell_address(self, row: int, col: int) -> int:
        return self._row_base(row) + col * (8 if self.qwords else 1)

    def refresh(self) -> None:
        for key, address in (
            ("memory.section.bss", GLOBALS),
            ("memory.section.text", CODE),
            ("memory.section.heap", HEAP),
        ):
            region = self._current_region()
            in_section = region is not None and region[0] == address
            self.section_buttons[key].setChecked(in_section)
        self.follow_btn.setEnabled(
            self.selected is not None and self.pointer_field_base(self.selected) is not None
        )
        self._update_context_panel()

        count = 2 if self.qwords else BYTES_PER_ROW
        for row in range(ROW_COUNT):
            base = self._row_base(row)
            self.addr_cells[row].setText(f"<b>0x{base:012X}</b>")
            try:
                raw = self.machine.read_memory(base, BYTES_PER_ROW)
            except MemoryFault:
                unmapped_style = f"color: {COLOR_PLAIN};"
                for col, cell in enumerate(self.value_cells[row]):
                    self.cell_values.pop(self._cell_address(row, col), None)
                    key = ("--", unmapped_style, False)
                    if self._cell_style_cache.get(cell) != key:
                        cell.setStyleSheet(unmapped_style)
                        cell.setFont(self._mono)
                        cell.setText("--")
                        self._cell_style_cache[cell] = key
                self.ascii_cells[row].setText(text("memory.unmapped"))
                continue
            ascii_text = "".join(chr(b) if 32 <= b < 127 else "." for b in raw)
            self.ascii_cells[row].setText(ascii_text[:8] + " " + ascii_text[8:])
            self.ascii_cells[row].setStyleSheet(f"color: {COLOR_PLAIN};")
            if self.qwords:
                values = [int.from_bytes(raw[0:8], "little"), int.from_bytes(raw[8:16], "little")]
            else:
                values = list(raw)
            for col, value in enumerate(values):
                address = self._cell_address(row, col)
                cell = self.value_cells[row][col]
                previous = self.cell_values.get(address)
                if previous is not None and previous != value:
                    self._flash(address)
                self.cell_values[address] = value
                self._style_cell(cell, address, value)
        self._apply_selection_marks()
        self._update_struct_overlay()

    def _style_cell(self, cell: QLabel, address: int, value: int) -> None:
        display = f"{value:016X}" if self.qwords else f"{value:02X}"
        color = self._byte_color(address)
        is_pointer = self._is_pointer_cell(address)
        bold = self.bold_range is not None and self.bold_range[0] <= address < self.bold_range[1]
        css = [f"color: {color};", "padding: 0px;"]
        css.append("border: 1px solid transparent;")
        if is_pointer:
            css.append(f"border-bottom: 1px solid {color};")
        if address in self.flashing:
            css.append(f"background: {COLOR_FLASH_BG}; color: white;")
        elif self.selected == address:
            css.append(f"background: {COLOR_SELECTED_BG};")
        style = " ".join(css)
        key = (display, style, bold)
        if self._cell_style_cache.get(cell) != key:
            cell.setStyleSheet(style)
            font = QFont(self._mono)
            font.setBold(bold)
            cell.setFont(font)
            cell.setText(display)
            self._cell_style_cache[cell] = key

    def _restyle_cells(self) -> None:
        for row in range(ROW_COUNT):
            for col, cell in enumerate(self.value_cells[row]):
                address = self._cell_address(row, col)
                value = self.cell_values.get(address)
                if value is not None:
                    self._style_cell(cell, address, value)
        self._update_struct_overlay()

    def _address_span_rect(self, start: int, size: int):
        end = start + size
        width = 8 if self.qwords else 1
        rect = None
        for row in range(ROW_COUNT):
            for col, cell in enumerate(self.value_cells[row]):
                address = self._cell_address(row, col)
                if address + width <= start or address >= end:
                    continue
                top_left = cell.mapTo(self.grid_host, cell.rect().topLeft())
                geo = cell.rect().translated(top_left)
                rect = geo if rect is None else rect.united(geo)
        return rect

    def _update_struct_overlay(self) -> None:
        box = getattr(self, "struct_box", None)
        if box is None:
            return
        if self.struct_outline is None:
            box.hide()
            return
        start, size = self.struct_outline
        rect = self._address_span_rect(start, size)
        if rect is None:
            box.hide()
            return
        rect = rect.adjusted(-2, -1, 2, 1)
        box.setGeometry(rect)
        box.raise_()
        box.show()

    def _apply_selection_marks(self) -> None:
        if self.selected is None:
            self.selected_label.setText("\u2014")
        else:
            self.selected_label.setText(f"0x{self.selected:016X}")

    def _update_context_panel(self) -> None:
        region = self._current_region()
        in_text = region is not None and region[0] == CODE
        in_heap = region is not None and region[0] == HEAP
        self.fn_title.setVisible(in_text)
        self.fn_table.setVisible(in_text)
        self.heap_title.setVisible(in_heap)
        self.heap_scroll.setVisible(in_heap)
        if in_text:
            self._render_function_table()
        elif in_heap:
            self._render_heap_list()

    def _render_function_table(self) -> None:
        try:
            rows = self.machine.function_table()
        except Exception:
            self.fn_table.setText(text("memory.unmapped"))
            return
        lines = [
            f"<span style='color:{COLOR_PLAIN}'>{name}</span> "
            f"<span style='color:{COLOR_POINTER}'><u>0x{address:012X}</u></span>"
            for name, address in rows
        ]
        self.fn_table.setText("<br>".join(lines))

    def _render_heap_list(self) -> None:
        struct_names = {
            "struct Cookies": text("schema.object.cookies"),
            "struct User": text("schema.object.user"),
            "struct Cursor": text("schema.object.cursor"),
        }
        structs = [
            (struct_names[alloc.semantic_type], alloc.base, alloc.size)
            for alloc in self.machine.allocations
            if getattr(alloc, "live", True) and alloc.semantic_type in struct_names
        ]
        signature = tuple(structs)
        if signature == self._heap_list_signature:
            return
        self._heap_list_signature = signature
        while self.heap_list_layout.count():
            item = self.heap_list_layout.takeAt(0)
            widget = item.widget()
            if widget is not None:
                widget.setParent(None)
                widget.deleteLater()
        for name, base, size in structs:
            entry = QLabel(
                f"<span style='color:{COLOR_PLAIN}'>{name}</span><br>"
                f"<span style='color:{COLOR_STRUCT_PTR}'><u>0x{base:012X}</u></span>"
            )
            entry.setFont(QFont("Courier New", 11))
            entry.setTextFormat(Qt.TextFormat.RichText)
            entry.setCursor(Qt.CursorShape.PointingHandCursor)
            entry.mousePressEvent = lambda _e, a=base: self.jump_to(a)  # type: ignore[assignment]
            entry.enterEvent = lambda _e, b=base, s=size: self._hover_struct(b, s)  # type: ignore[assignment]
            entry.leaveEvent = lambda _e: self._hover_clear.start()  # type: ignore[assignment]
            self.heap_list_layout.addWidget(entry)
        self.heap_list_layout.addStretch()

    def _hover_struct(self, base: int, size: int) -> None:
        self._hover_clear.stop()
        allocation = self.struct_at(base)
        if allocation is not None:
            self.tooltip.setText(self._struct_tooltip(allocation))
        if self.struct_outline == (base, size) and self.bold_range is None:
            return
        self.struct_outline = (base, size)
        self.bold_range = None
        self._restyle_cells()

    def _flash(self, address: int) -> None:
        self.flashing.add(address)
        timer = self.flash_handles.get(address)
        if timer is not None:
            timer.stop()
        timer = QTimer(self)
        timer.setSingleShot(True)
        timer.timeout.connect(lambda a=address: self._end_flash(a))
        timer.start(450)
        self.flash_handles[address] = timer

    def _end_flash(self, address: int) -> None:
        self.flashing.discard(address)
        self.flash_handles.pop(address, None)
        self.refresh()

    def eventFilter(self, obj: QObject, event: QEvent) -> bool:
        if obj in self._cell_pos:
            etype = event.type()
            row, col = self._cell_pos[obj]
            if etype == QEvent.Type.Enter:
                self._hover_clear.stop()
                self._hover(self._cell_address(row, col))
            elif etype == QEvent.Type.Leave:
                self._hover_clear.start()
            elif etype == QEvent.Type.MouseButtonDblClick:
                self._click(self._cell_address(row, col))
                self._follow_selected()
            elif etype == QEvent.Type.MouseButtonPress:
                self._click(self._cell_address(row, col))
        elif obj is getattr(self, "grid_host", None):
            etype = event.type()
            if etype == QEvent.Type.MouseMove:
                address = self._address_at_pos(event.position().toPoint())
                if address is None:
                    self._hover_clear.start()
                else:
                    self._hover_clear.stop()
                    self._hover(address)
            elif etype == QEvent.Type.Leave:
                self._hover_clear.start()
        return super().eventFilter(obj, event)

    def _address_at_pos(self, pos) -> int | None:
        if not self.value_cells:
            return None
        row = 0
        best_dy: int | None = None
        for r in range(ROW_COUNT):
            cell = self.value_cells[r][0]
            top = cell.mapTo(self.grid_host, cell.rect().topLeft()).y()
            dy = max(top - pos.y(), 0, pos.y() - (top + cell.height()))
            if best_dy is None or dy < best_dy:
                best_dy = dy
                row = r
        cells = self.value_cells[row]
        left = cells[0].mapTo(self.grid_host, cells[0].rect().topLeft()).x()
        right = cells[-1].mapTo(self.grid_host, cells[-1].rect().topRight()).x()
        if pos.x() < left or pos.x() > right:
            return None
        best_col = 0
        best_dx: int | None = None
        for col, cell in enumerate(cells):
            cl = cell.mapTo(self.grid_host, cell.rect().topLeft()).x()
            dx = max(cl - pos.x(), 0, pos.x() - (cl + cell.width()))
            if best_dx is None or dx < best_dx:
                best_dx = dx
                best_col = col
        return self._cell_address(row, best_col)

    def _clear_hover(self) -> None:
        if self.struct_outline is None and self.bold_range is None:
            return
        self.struct_outline = None
        self.bold_range = None
        self._restyle_cells()

    def _hover(self, address: int) -> None:
        info = self._hover_text(address)
        if info:
            self.tooltip.setText(info)
        allocation = self.struct_at(address)
        if allocation is not None:
            new_outline: tuple[int, int] | None = (allocation.base, allocation.size)
        else:
            new_outline = self.string_region(address)
        new_range: tuple[int, int] | None = None
        base = self.pointer_field_base(address)
        if base is not None:
            try:
                target = self.machine.memory.read_u64(base)
                region = self.string_region(target)
                if region is not None:
                    new_range = (region[0], region[0] + region[1])
            except MemoryFault:
                pass
        if new_outline == self.struct_outline and new_range == self.bold_range:
            return
        self.struct_outline = new_outline
        self.bold_range = new_range
        self._restyle_cells()

    def _pointer_field_type(self, address: int) -> str:
        """Declared type of a pointer field (for .bss table entries)."""
        exact = {
            GLOBALS + G_COOKIES: text("schema.cookies_pointer"),
            GLOBALS + G_USER: text("schema.user_pointer"),
            GLOBALS + G_CURSORS: text("schema.cursor_array_pointer"),
            GLOBALS + G_FUNCTIONS: text("schema.functions"),
        }
        base = self.pointer_field_base(address)
        if base in exact:
            return exact[base]
        table = GLOBALS + G_CURSOR_TABLE
        if base is not None and table <= base < table + MAX_CURSORS * 8:
            return f"{text('schema.cursor')}*"
        if base is not None and FUNCTION_TABLE <= base < FUNCTION_TABLE + len(FUNCTIONS) * 8:
            return text("schema.function_pointer")
        return ""

    def _pointee_label(self, target: int) -> str:
        """Human label describing what a pointer target actually is."""
        allocation = self.struct_at(target)
        if allocation is not None:
            return {
                "struct Cookies": text("schema.object.cookies"),
                "struct User": text("schema.object.user"),
                "struct Cursor": text("schema.object.cursor"),
            }.get(allocation.semantic_type, allocation.semantic_type)
        slot = self.function_slot(target)
        if slot is not None:
            return text(slot[0])
        if target == GLOBALS + G_CURSOR_TABLE:
            return text("schema.object.cursor_array")
        if target == FUNCTION_TABLE:
            return text("schema.object.function_table")
        for alloc in self.machine.allocations:
            if alloc.base <= target < alloc.base + alloc.size:
                return alloc.semantic_type
        return text("schema.pointer")

    def _hover_text(self, address: int) -> str:
        slot = self.function_slot(address)
        if slot is not None:
            key, entry = slot
            return (
                f"<span style='color:{COLOR_POINTER_DARK}'>{text(key)}</span> "
                f"<span style='color:{TOOLTIP_LABEL}'>{text('schema.bytecode_function')}</span><br>"
                f"<span style='color:{TOOLTIP_LABEL}'>{text('schema.offset')} +0x{address-entry:X}</span><br>"
                f"<span style='color:{TOOLTIP_LABEL}'>{text('schema.address')} </span>"
                f"<span style='color:{COLOR_POINTER_DARK}'><u>0x{address:012X}</u></span>"
            )
        base = self.pointer_field_base(address)
        if base is not None:
            try:
                target = self.machine.memory.read_u64(base)
            except MemoryFault:
                target = 0
            region = self.string_region(target)
            if region is not None:
                try:
                    raw = self.machine.read_memory(region[0], min(16, region[1]))
                except MemoryFault:
                    raw = b""
                preview = "".join(chr(b) if 32 <= b < 127 else "?" for b in raw[:0x08])
                if len(raw) > 0x08: preview += "..."
                return (
                    f"<span style='color:{TOOLTIP_LABEL}'>{text('schema.char_pointer')} "
                    f"{text('schema.address')} </span>"
                    f"<span style='color:{COLOR_POINTER_DARK}'><u>0x{target:012X}</u></span><br><br>"
                    f"<i style='color:{TOOLTIP_LABEL}'>ASCII: {preview}</i>"
                )
            field_type = self._pointer_field_type(address)
            target_label = self._pointee_label(target)
            type_line = (
                f"<i style='color:{TOOLTIP_LABEL}'>{field_type}</i><br>" if field_type else ""
            )
            return (
                f"{type_line}"
                f"<span style='color:{TOOLTIP_LABEL}'>{text('schema.points_to')} </span>"
                f"<span style='color:{COLOR_POINTER_DARK}'>{target_label}</span><br>"
                f"<span style='color:{TOOLTIP_LABEL}'>{text('schema.address')} </span>"
                f"<span style='color:{COLOR_POINTER_DARK}'><u>0x{target:012X}</u></span>"
            )
        allocation = self.struct_at(address)
        if allocation is not None:
            return self._struct_tooltip(allocation)
        if self.string_region(address) is not None:
            return f"<span style='color:{COLOR_STRING}'>{text('schema.char_pointer')}</span>"
        return ""

    def _struct_tooltip(self, allocation) -> str:
        name = {
            "struct Cookies": text("schema.object.cookies"),
            "struct User": text("schema.object.user"),
            "struct Cursor": text("schema.object.cursor"),
        }.get(allocation.semantic_type, allocation.semantic_type)
        header = (
            f"<span style='color:{TOOLTIP_LABEL}'>{name} {text('schema.address')} </span>"
            f"<span style='color:{COLOR_POINTER_DARK}'><u>0x{allocation.base:012X}</u></span>"
        )
        if allocation.semantic_type == "struct Cookies":
            raw_fields = [(text("schema.u64"), text("schema.cookies"), 0)]
        elif allocation.semantic_type == "struct User":
            raw_fields = [
                (text("schema.u64"), text("schema.id"), 0),
                (text("schema.char_pointer"), text("schema.username"), 8),
            ]
        elif allocation.semantic_type == "struct Cursor":
            raw_fields = [
                (text("schema.u64"), text("schema.id"), 0),
                (text("schema.char8"), text("schema.name"), 8),
                (text("schema.char_pointer"), text("schema.description"), 16),
                (text("schema.function_pointer"), text("schema.on_clicked"), 24),
            ]
        else:
            raw_fields = []
        lines = [header, ""]
        for ctype, fname, offset in raw_fields:
            color = self._kind_color(self._field_kind(allocation.base + offset))
            lines.append(
                f"<i style='color:{TOOLTIP_LABEL}'>{ctype}</i> "
                f"<span style='color:{color}'>{fname}</span>"
            )
        return "<br>".join(lines)

    def _click(self, address: int) -> None:
        self.selected = address
        allocation = self.struct_at(address)
        if allocation is not None:
            self.struct_outline = (allocation.base, allocation.size)
            self.tooltip.setText(self._struct_tooltip(allocation))
        else:
            self.struct_outline = None
        try:
            raw = self.machine.read_memory(address, 8 if self.qwords else 1)
            value = int.from_bytes(raw, "little")
            self.edit_value.setText(f"0x{value:X}" if self.qwords else f"{value:02X}")
        except MemoryFault:
            pass
        self.refresh()

    def _write_selected(self) -> None:
        if self.selected is None:
            return
        raw = self.edit_value.text().strip()
        try:
            if self.qwords:
                number = int(raw, 16 if raw.lower().startswith("0x") else 10)
                data = (number & ((1 << 64) - 1)).to_bytes(8, "little")
            else:
                number = int(raw.lower().removeprefix("0x"), 16)
                if not 0 <= number <= 0xFF:
                    raise ValueError
                data = bytes((number,))
            self.machine.write_memory(self.selected, data)
            self.status.setText("")
            self.refresh()
            if self.on_change is not None:
                self.on_change()
        except (MemoryFault, OverflowError, ValueError):
            self.status.setText(text("error.hex_byte"))

    def _follow_selected(self) -> None:
        if self.selected is None:
            return
        base = self.pointer_field_base(self.selected)
        if base is None:
            return
        try:
            target = self.machine.memory.read_u64(base)
            self.machine.memory.read(target, 1)
            self.jump_to(target)
        except MemoryFault:
            self.status.setText(text("memory.unmapped"))
