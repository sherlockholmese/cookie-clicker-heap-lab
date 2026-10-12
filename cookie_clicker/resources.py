"""Human-editable copy and sprite resources."""

from __future__ import annotations

from importlib.resources import files


def _resource(name: str) -> str:
    return files("cookie_clicker.assets").joinpath(name).read_text(encoding="utf-8")


def _load_copy() -> dict[str, str]:
    result: dict[str, str] = {}
    for raw in _resource("strings.txt").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in raw:
            continue
        key, value = raw.split("=", 1)
        result[key.strip()] = value.strip()
    return result


COPY = _load_copy()


def text(key: str) -> str:
    return COPY.get(key, f"<{key}>")


def cookie_sprite() -> tuple[int, int, dict[str, tuple[str, bool]], list[str]]:
    width = height = 0
    palette: dict[str, tuple[str, bool]] = {}
    rows: list[str] = []
    reading_pixels = False
    for raw in _resource("cookie.sprite.txt").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line == "pixels:":
            reading_pixels = True
        elif reading_pixels:
            rows.append(line)
        else:
            key, value = (part.strip() for part in line.split("=", 1))
            if key == "size":
                width, height = (int(part) for part in value.lower().split("x", 1))
            else:
                if len(value) != 8:
                    raise ValueError("sprite colors must use RRGGBBAA")
                palette[key[0]] = (f"#{value[:6]}", value[6:8].lower() == "00")
    if width <= 0 or height <= 0 or len(rows) != height:
        raise ValueError("invalid cookie sprite dimensions")
    if any(len(row) != width or any(symbol not in palette for symbol in row) for row in rows):
        raise ValueError("invalid cookie sprite pixels")
    return width, height, palette, rows
