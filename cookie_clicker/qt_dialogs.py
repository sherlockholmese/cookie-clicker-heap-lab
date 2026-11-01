from __future__ import annotations

from PyQt6.QtCore import Qt
from PyQt6.QtGui import QFont
from PyQt6.QtWidgets import (
    QDialog,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
)

from .resources import text

MONO_FAMILY = "Courier New"

BLUE_THEME = {
    "panel": "#c9daf8",
    "accent": "#1155cc",
    "field_border": "#a4c2f4",
    "field_text": "#1155cc",
    "submit": "#3671d6",
    "cancel": "#e99494",
}

GRAY_THEME = {
    "panel": "#d9d9d9",
    "accent": "#595959",
    "field_border": "#b7b7b7",
    "field_text": "#333333",
    "submit": "#a1a1a1",
    "cancel": "#f4cccc",
}

CANCEL_TEXT = "#990000"


class FormDialog(QDialog):
    def __init__(self, parent, title: str, theme: dict[str, str]) -> None:
        super().__init__(parent)
        self.theme = theme
        self.setModal(True)
        self.setStyleSheet(f"QDialog {{ background: {theme['panel']}; }}")

        self._root = QVBoxLayout(self)
        self._root.setContentsMargins(24, 20, 24, 20)
        self._root.setSpacing(10)

        title_label = QLabel(title)
        title_font = QFont()
        title_font.setPointSize(18)
        title_font.setBold(True)
        title_font.setUnderline(True)
        title_label.setFont(title_font)
        title_label.setStyleSheet(f"color: {theme['accent']};")
        title_label.setAlignment(Qt.AlignmentFlag.AlignHCenter)
        self._root.addWidget(title_label)
        self._root.addSpacing(6)

        self._fields: dict[str, QLineEdit] = {}

    def add_field(
        self,
        key: str,
        label: str,
        placeholder: str = "",
        value: str = "",
        readonly: bool = False,
    ) -> QLineEdit:
        caption = QLabel(label)
        caption_font = QFont()
        caption_font.setBold(True)
        caption.setFont(caption_font)
        caption.setStyleSheet(f"color: {self.theme['accent']};")
        self._root.addWidget(caption)

        edit = QLineEdit()
        edit.setFont(QFont(MONO_FAMILY, 12))
        edit.setPlaceholderText(placeholder)
        edit.setText(value)
        edit.setReadOnly(readonly)
        background = "#eeeeee" if readonly else "white"
        edit.setStyleSheet(
            f"background: {background}; border: 1px solid {self.theme['field_border']};"
            f" color: {self.theme['field_text']}; padding: 6px;"
        )
        edit.setMinimumWidth(280)
        self._root.addWidget(edit)
        self._fields[key] = edit
        return edit

    def value(self, key: str) -> str:
        return self._fields[key].text()

    def finish(self) -> None:
        self._root.addSpacing(10)
        buttons = QHBoxLayout()
        buttons.setSpacing(12)

        submit = QPushButton(text("dialog.submit"))
        submit.setCursor(Qt.CursorShape.PointingHandCursor)
        submit.setStyleSheet(
            f"background: {self.theme['submit']}; color: white; border: none;"
            " padding: 8px 20px; font-weight: bold;"
        )
        submit.clicked.connect(self.accept)

        cancel = QPushButton(text("dialog.cancel"))
        cancel.setCursor(Qt.CursorShape.PointingHandCursor)
        cancel.setStyleSheet(
            f"background: {self.theme['cancel']}; color: {CANCEL_TEXT};"
            f" border: 1px solid {CANCEL_TEXT}; padding: 8px 20px; font-weight: bold;"
        )
        cancel.clicked.connect(self.reject)

        buttons.addWidget(submit)
        buttons.addWidget(cancel)
        self._root.addLayout(buttons)
