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
    group.add_argument("--load", metavar="FILE", help="load an explicit workshop save")
    group.add_argument("--seed", type=int, metavar="SEED", help="seed a fresh deterministic session")
    return result


def main(argv: list[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        seed = arguments.seed if arguments.seed is not None else secrets.randbits(64)
        machine = load(arguments.load) if arguments.load else Machine(seed)
        return CookieApp(machine).run()
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
