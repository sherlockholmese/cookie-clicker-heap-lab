"""Command-line entry point."""

from __future__ import annotations

import argparse
import secrets
import sys

from .game import Machine
from .save_state import load
from .qt_ui import CookieApp


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(prog="cookie-clicker-pwn", description="Cookie Clicker pwn workshop")
    group = result.add_mutually_exclusive_group()
    return result


def main(argv: list[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        machine = Machine()
        return CookieApp(machine).run()
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
