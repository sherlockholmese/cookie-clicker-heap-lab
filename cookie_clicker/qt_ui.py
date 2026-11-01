from __future__ import annotations

import html
import sys
from typing import Callable

from PyQt6.QtCore import QEvent, QObject, QPoint, Qt, QTimer
from PyQt6.QtGui import QFont, QFontDatabase
from PyQt6.QtWidgets import (
    QApplication,
    QDialog,
    QFrame,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

from .game import (
    CURSOR_NAME,
    CURSOR_PRICE,
    FLAG_PRICE,
    GameError,
    GameView,
    Machine,
    SegmentationFault,
)
from .memory import HEAP, MemoryFault
from .qt_dialogs import GRAY_THEME, FormDialog
from .resources import asset_path, text

UI_FONT_FAMILY = "Schibsted Grotesk"
UI_FONT_FILES = (
    "static/SchibstedGrotesk-Regular.ttf",
    "static/SchibstedGrotesk-Bold.ttf",
    "static/SchibstedGrotesk-Italic.ttf",
    "static/SchibstedGrotesk-BoldItalic.ttf",
)

TICK_INTERVAL_MS = 50

MONO_FAMILY = "Courier New"

STYLE = """
QWidget#root { background: white; }
QFrame#username { background: #595959; border: 1px solid #333333; }
QFrame#username QLabel, QFrame#username QLineEdit {
    background: transparent; border: none; color: white;
}
QLabel#count {
    background: #ffab40; border: 1px solid #5a5a5a; color: #333333;
    font-weight: bold; padding: 6px;
}
QPushButton#debug {
    background: #eeeeee; border: 1px solid #595959; color: #333333; padding: 8px;
}
QPushButton#debug:checked { background: #ffd9a0; }
QPushButton#debug:disabled { color: #a0a0a0; }
QPushButton#buycursor {
    background: #c9daf8; border: 1px solid #1155cc; color: #1155cc;
    font-size: 16px; padding: 8px;
}
QPushButton#buycursor:disabled { background: #eef3fd; border-color: #9bb7e8; color: #9bb7e8; }
QPushButton#buyflag {
    background: #d9ead3; border: 1px solid #274e13; color: #274e13;
    font-size: 16px; padding: 8px;
}
QPushButton#buyflag:disabled { background: #eef5ea; border-color: #9cbf8a; color: #9cbf8a; }
QFrame#divider { background: #595959; }
QScrollArea#objects { background: white; border: none; }
QWidget#objectsInner { background: white; }
"""

LIST_TEXT_STYLE = "color: #333333; font-size: 14px;"

CARD_STYLE = (
    "background: #c9daf8; border: 1px solid #1155cc; border-radius: 6px;"
    " padding: 8px; color: #1155cc; font-size: 14px;"
)

COOKIE_BUTTON_STYLE = """
QPushButton {
    background: #ffab40;
    border: 1px solid #5a5a5a;
    border-radius: 36px;
    color: #783f04;
}
QPushButton:pressed { background: #f09a2c; }
"""


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


def display_field(raw: bytes) -> str:
    out = []
    for byte in raw:
        out.append(chr(byte) if 32 <= byte < 127 else f"\\x{byte:02x}")
    return html.escape("".join(out))


def price_label(amount: int) -> str:
    if amount >= 1_000_000 and amount % 1_000_000 == 0:
        return f"{amount // 1_000_000} mil"
    if amount >= 1_000 and amount % 1_000 == 0:
        return f"{amount // 1_000}k"
    return str(amount)


class PointerTip(QLabel):
    def __init__(self, parent: QWidget) -> None:
        super().__init__(parent)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self.setTextFormat(Qt.TextFormat.RichText)
        self.setFont(QFont("Courier New", 13))
        self.setStyleSheet("background: #595959; padding: 10px 30px;")
        self.hide()

    def set_info(self, ctype: str, name: str, address: int) -> None:
        self.setText(
            f"<i style='color:#d9d9d9'>{ctype}</i> "
            f"<span style='color:#ffffff'>{name}</span> "
            f"<span style='color:#ff0000'><u>0x{address:016X}</u></span>"
        )
        self.adjustSize()


class MainWindow(QWidget):
    def __init__(self, machine: Machine, pointer_mode: bool = False) -> None:
        super().__init__()
        self.machine = machine
        self.last_valid = machine.clone()
        self.status = ""
        self.pointer_view = pointer_mode
        self.memory_window = None
        self._pointer_info: dict[QWidget, Callable[[], tuple[str, str, int]]] = {}
        self._cursor_signature: tuple[tuple[object, ...], ...] | None = None

        self.setObjectName("root")
        self.setWindowTitle(text("app.title"))
        self.setStyleSheet(STYLE)
        self.setFixedSize(560, 560)

        root = QHBoxLayout(self)
        root.setContentsMargins(20, 20, 20, 20)
        root.setSpacing(16)
        root.addLayout(self._build_left())

        divider = QFrame()
        divider.setObjectName("divider")
        divider.setFixedWidth(1)
        root.addWidget(divider)

        root.addLayout(self._build_right())

        self.tip = PointerTip(self)

        self._register_pointer(
            self.name_edit,
            lambda: (text("schema.char_pointer"), text("schema.username"), self.safe_view().username_ptr),
        )
        self._register_pointer(
            self.count_label,
            lambda: (text("schema.u64"), text("schema.cookies_value"), self.safe_view().cookies_ptr),
        )

        self.pointer_btn.toggled.connect(self._on_pointer_toggled)
        self.pointer_btn.setChecked(pointer_mode)

        self._timer = QTimer(self)
        self._timer.timeout.connect(self._tick)
        self._timer.start(TICK_INTERVAL_MS)

        self.refresh()

    def _build_left(self) -> QVBoxLayout:
        col = QVBoxLayout()
        col.setSpacing(14)
        center = Qt.AlignmentFlag.AlignHCenter

        self.name_edit = QLineEdit()
        self.name_edit.setFont(QFont(MONO_FAMILY, 13))
        self.name_edit.setPlaceholderText(text("game.username.placeholder"))
        self.name_edit.returnPressed.connect(self._change_username)
        self.name_edit.editingFinished.connect(self._change_username)
        self.name_edit.textEdited.connect(self._preview_username)
        prompt = QLabel(">")
        prompt.setFont(QFont(MONO_FAMILY, 13))
        self.username_bar = QFrame()
        self.username_bar.setObjectName("username")
        self.username_bar.setFixedWidth(240)
        bar_row = QHBoxLayout(self.username_bar)
        bar_row.setContentsMargins(10, 4, 10, 4)
        bar_row.setSpacing(6)
        bar_row.addWidget(prompt)
        bar_row.addWidget(self.name_edit)
        col.addWidget(self.username_bar, alignment=center)

        self.cookie_btn = QPushButton(text("game.cookie"))
        self.cookie_btn.setFixedSize(240, 215)
        self.cookie_btn.setFont(QFont(MONO_FAMILY, 14))
        self.cookie_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.cookie_btn.setStyleSheet(COOKIE_BUTTON_STYLE)
        self.cookie_btn.clicked.connect(lambda: self.execute(self.machine.click))
        col.addWidget(self.cookie_btn, alignment=center)

        self.count_label = QLabel()
        self.count_label.setObjectName("count")
        self.count_label.setFont(QFont(MONO_FAMILY, 14))
        self.count_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.count_label.setFixedWidth(240)
        col.addWidget(self.count_label, alignment=center)

        col.addStretch()

        debug_font = QFont(MONO_FAMILY, 13, QFont.Weight.Bold)
        debug_font.setUnderline(True)
        self.debug_header = QLabel(text("game.debug_options"))
        self.debug_header.setFont(debug_font)
        self.debug_header.setStyleSheet("color: #333333;")
        self.debug_header.setFixedWidth(240)
        self.debug_header.setAlignment(Qt.AlignmentFlag.AlignCenter)
        col.addWidget(self.debug_header, alignment=center)

        self.pointer_btn = QPushButton(text("pointer.toggle"))
        self.debug_btn = QPushButton(text("memory.open"))
        for btn in (self.pointer_btn, self.debug_btn):
            btn.setObjectName("debug")
            btn.setFont(QFont(MONO_FAMILY, 13))
            btn.setFixedWidth(240)
            btn.setMinimumHeight(40)
            btn.setCursor(Qt.CursorShape.PointingHandCursor)
            col.addWidget(btn, alignment=center)
        self.pointer_btn.setCheckable(True)
        self.debug_btn.clicked.connect(self._open_debugger)

        return col

    def _build_right(self) -> QVBoxLayout:
        col = QVBoxLayout()
        col.setSpacing(14)

        self.buy_cursor_btn = QPushButton()
        self.buy_flag_btn = QPushButton()
        self.buy_cursor_btn.setObjectName("buycursor")
        self.buy_flag_btn.setObjectName("buyflag")
        self.buy_cursor_btn.clicked.connect(lambda: self.execute(self.machine.buy_cursor))
        self.buy_flag_btn.clicked.connect(lambda: self.execute(self.machine.buy_flag))
        for btn in (self.buy_cursor_btn, self.buy_flag_btn):
            btn.setMinimumHeight(44)
            btn.setCursor(Qt.CursorShape.PointingHandCursor)
            col.addWidget(btn)

        self.status_label = QLabel("")
        self.status_label.setStyleSheet(LIST_TEXT_STYLE)
        self.status_label.setWordWrap(True)
        col.addWidget(self.status_label)

        col.addStretch()

        self.objects = QScrollArea()
        self.objects.setObjectName("objects")
        self.objects.setWidgetResizable(True)
        self.objects.setFixedHeight(330)

        inner = QWidget()
        inner.setObjectName("objectsInner")
        self.objects_layout = QVBoxLayout(inner)
        self.objects_layout.setContentsMargins(0, 0, 0, 0)
        self.objects_layout.setSpacing(10)

        self.objects_header = QLabel(text("game.cursors"))
        self.objects_header.setStyleSheet(LIST_TEXT_STYLE)
        self.objects_header.setTextFormat(Qt.TextFormat.RichText)
        self.objects_layout.addWidget(self.objects_header)
        self.objects_tail = QVBoxLayout()
        self.objects_tail.setSpacing(10)
        self.objects_layout.addLayout(self.objects_tail)
        self.objects_layout.addStretch()
        self.cursor_rows: list[QWidget] = []

        self.objects.setWidget(inner)
        col.addWidget(self.objects)

        self.win_label = QLabel(text("game.win.title"))
        self.win_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.win_label.setFont(QFont("Courier New", 22, QFont.Weight.Bold))
        self.win_label.hide()
        col.addWidget(self.win_label)
        return col

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
        if hasattr(self, "status_label"):
            self.status_label.setText(message)

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

    def _memory_preview_active(self) -> bool:
        return self.memory_window is not None and self.memory_window.isVisible()

    def _preview_username(self, raw: str) -> None:
        if not self._memory_preview_active():
            return
        try:
            data = parse_field(raw) + b"\0"
        except ValueError:
            return
        self.memory_window.set_preview({self.safe_view().username_ptr: data})

    def _change_username(self) -> None:
        if self.memory_window is not None:
            self.memory_window.clear_preview()
        raw = self.name_edit.text()
        try:
            data = parse_field(raw)
        except ValueError as error:
            self.set_status(str(error))
            return
        if data == self.safe_view().username.encode("utf-8"):
            return
        self.execute(lambda: self.machine.rename_user(data))

    def _open_debugger(self) -> None:
        from .qt_memory import MemoryWindow

        if self.memory_window is None:
            self.memory_window = MemoryWindow(self.machine, on_change=self.refresh)
        else:
            self.memory_window.machine = self.machine
        self.memory_window.open_at(HEAP)

    def _rebuild_cursor_rows(self, view: GameView) -> None:
        for widget in self.cursor_rows:
            self._pointer_info.pop(widget, None)
            widget.setParent(None)
            widget.deleteLater()
        self.cursor_rows.clear()
        for index, cursor in enumerate(view.cursors):
            label = QLabel()
            label.setStyleSheet(CARD_STYLE)
            label.setTextFormat(Qt.TextFormat.RichText)
            label.setWordWrap(True)
            label.setCursor(Qt.CursorShape.PointingHandCursor)
            label.mouseReleaseEvent = lambda _event, i=index: self.edit_cursor(i)  # type: ignore[assignment]
            self.objects_tail.addWidget(label)
            self.cursor_rows.append(label)
            self._register_pointer(
                label,
                lambda i=index: (
                    text("schema.cursor"),
                    text("schema.cursor"),
                    self.safe_view().cursors[i].address,
                ),
            )

    def _update_cursor_rows(self, view: GameView) -> None:
        for label, cursor in zip(self.cursor_rows, view.cursors, strict=True):
            name = display_field(cursor.name_bytes) or text("schema.cursor")
            label.setText(
                f"<b>{name} (id {cursor.id}):</b><br>"
                f"<i>{display_field(cursor.description_bytes)}</i>"
            )

    def edit_cursor(self, index: int) -> None:
        view = self.safe_view()
        if index >= len(view.cursors):
            return
        cursor = view.cursors[index]
        name_addr = cursor.address + CURSOR_NAME
        desc_addr = cursor.description_ptr

        dialog = FormDialog(self, text("cursor.edit.title"), GRAY_THEME)
        name_field = dialog.add_field("name", text("cursor.name"), value=cursor.name)
        desc_field = dialog.add_field("description", text("cursor.description"), value=cursor.description)
        dialog.finish()

        def update_preview() -> None:
            if not self._memory_preview_active():
                return
            spans: dict[int, bytes] = {}
            try:
                spans[name_addr] = parse_field(name_field.text()) + b"\0"
            except ValueError:
                pass
            try:
                spans[desc_addr] = parse_field(desc_field.text()) + b"\0"
            except ValueError:
                pass
            self.memory_window.set_preview(spans)

        name_field.textEdited.connect(lambda _t: update_preview())
        desc_field.textEdited.connect(lambda _t: update_preview())

        accepted = dialog.exec() == QDialog.DialogCode.Accepted
        if self.memory_window is not None:
            self.memory_window.clear_preview()
        if not accepted:
            return
        name = dialog.value("name")
        description = dialog.value("description")
        self.execute(lambda: self._apply_cursor_edits(index, name, description))

    def _apply_cursor_edits(self, index: int, name: str, description: str) -> None:
        self.machine.rename_cursor(index, parse_field(name))
        self.machine.describe_cursor(index, parse_field(description))

    def _register_pointer(
        self, widget: QWidget, provider: Callable[[], tuple[str, str, int]]
    ) -> None:
        self._pointer_info[widget] = provider
        widget.installEventFilter(self)

    def _on_pointer_toggled(self, on: bool) -> None:
        self.pointer_view = on
        if not on:
            self.tip.hide()

    def eventFilter(self, obj: QObject, event: QEvent) -> bool:
        if obj in self._pointer_info:
            if event.type() == QEvent.Type.Enter and self.pointer_btn.isChecked():
                self.show_tip(obj)
            elif event.type() == QEvent.Type.Leave:
                self.tip.hide()
        return super().eventFilter(obj, event)

    def show_tip(self, widget: QWidget) -> None:
        try:
            ctype, name, address = self._pointer_info[widget]()
        except (SegmentationFault, GameError, MemoryFault, IndexError):
            return
        self.tip.set_info(ctype, name, address)
        origin = widget.mapTo(self, QPoint(0, 0))
        x = origin.x() + 12
        y = origin.y() + widget.height() - 4
        x = max(0, min(x, self.width() - self.tip.width()))
        y = max(0, min(y, self.height() - self.tip.height()))
        self.tip.move(x, y)
        self.tip.raise_()
        self.tip.show()

    def _tick(self) -> None:
        try:
            self.machine.advance_tick()
        except SegmentationFault as error:
            self.machine = self.last_valid.clone()
            self.set_status(str(error))
        self.refresh()

    def refresh(self) -> None:
        view = self.safe_view()

        if view.won:
            self._set_game_visible(False)
            self.win_label.show()
            return
        self.win_label.hide()
        self._set_game_visible(True)

        if not self.name_edit.hasFocus() and self.name_edit.text() != view.username:
            self.name_edit.setText(view.username)

        self.count_label.setText(f"{view.cookies} {text('game.cookies')}")
        self.buy_cursor_btn.setText(
            f"{text('game.cursor.buy')} {price_label(CURSOR_PRICE)} {text('game.cookies')}"
        )
        self.buy_flag_btn.setText(
            f"{text('game.flag.buy')} {price_label(FLAG_PRICE)} {text('game.cookies')}"
        )
        self.buy_cursor_btn.setEnabled(view.cookies >= CURSOR_PRICE)
        self.buy_flag_btn.setEnabled(view.cookies >= FLAG_PRICE and not view.flag_owned)

        signature = tuple(
            (cursor.address, cursor.id, cursor.name_bytes, cursor.description_bytes)
            for cursor in view.cursors
        )
        if signature != self._cursor_signature:
            if len(view.cursors) != len(self.cursor_rows):
                self._rebuild_cursor_rows(view)
            self._update_cursor_rows(view)
            self._cursor_signature = signature

        if self.memory_window is not None and self.memory_window.isVisible():
            self.memory_window.machine = self.machine
            self.memory_window.refresh()

    def _set_game_visible(self, visible: bool) -> None:
        for widget in (
            self.username_bar,
            self.cookie_btn,
            self.count_label,
            self.debug_header,
            self.debug_btn,
            self.pointer_btn,
            self.buy_cursor_btn,
            self.buy_flag_btn,
            self.objects,
            self.status_label,
        ):
            widget.setVisible(visible)


class CookieApp:
    def __init__(self, machine: Machine) -> None:
        self._owns_app = QApplication.instance() is None
        self.app = QApplication.instance() or QApplication(sys.argv)
        for _font in UI_FONT_FILES:
            QFontDatabase.addApplicationFont(asset_path(_font))
        default_font = self.app.font()
        default_font.setFamily(UI_FONT_FAMILY)
        self.app.setFont(default_font)
        self.window = MainWindow(machine)

    def run(self) -> int:
        self.window.show()
        return self.app.exec()
