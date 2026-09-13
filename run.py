#!/usr/bin/env python3
"""Build and run Deckhand for a test drive.

Usage, from anywhere:

    python scripts/run.py             # build surface + workspace, restart app
    python scripts/run.py --no-build  # just restart the last build
    python scripts/run.py --stop      # stop a running instance and exit

The script is deliberately boring: it stops any running instance, builds
the TypeScript surface (tsc) and then the Rust workspace (the surface
must build first because tauri embeds app/ui into the binary), starts
the app detached, and prints where the runtime files live. Any build
failure stops it before the old instance would be replaced by nothing.
"""

import argparse
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
UI = REPO / "app" / "ui"
EXE = REPO / "target" / "debug" / "deckhand.exe"
LOCAL = Path(os.environ.get("LOCALAPPDATA", "")) / "deckhand"


def run(cmd: list[str], cwd: Path) -> None:
    print(f"> {' '.join(cmd)}  (in {cwd})")
    result = subprocess.run(cmd, cwd=cwd, shell=(os.name == "nt"))
    if result.returncode != 0:
        sys.exit(f"failed ({result.returncode}): {' '.join(cmd)}")


def stop_running() -> None:
    # taskkill returns 128 when no process matched; both outcomes are fine.
    subprocess.run(
        ["taskkill", "/IM", "deckhand.exe", "/F"],
        capture_output=True,
    )


def build() -> None:
    if not shutil.which("npm"):
        sys.exit("npm not found on PATH; the surface needs it once for typescript")
    if not (UI / "node_modules").exists():
        run(["npm", "install", "--no-audit", "--no-fund"], cwd=UI)
    run(["npx", "tsc"], cwd=UI)
    run(["cargo", "build", "--workspace"], cwd=REPO)


def start() -> None:
    if not EXE.exists():
        sys.exit(f"not built: {EXE}\nrun without --no-build first")
    flags = 0
    if os.name == "nt":
        # Detach fully: the debug build is console-subsystem and would
        # otherwise open (and tie itself to) a console window.
        flags = subprocess.CREATE_NO_WINDOW | subprocess.DETACHED_PROCESS
    subprocess.Popen([str(EXE)], creationflags=flags, close_fds=True)
    time.sleep(2.0)
    contact = LOCAL / "daemon.json"
    print(f"started {EXE.name}")
    print(f"daemon contact: {contact} ({'present' if contact.exists() else 'MISSING'})")
    print(f"bindings:       {LOCAL / 'bindings.json'}")
    print(f"reveal log:     {LOCAL / 'reveal.log'}")
    print("quit from the surface's Quit key, or: python scripts/run.py --stop")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-build", action="store_true", help="skip building, just restart")
    parser.add_argument("--stop", action="store_true", help="stop a running instance and exit")
    args = parser.parse_args()

    stop_running()
    if args.stop:
        print("stopped")
        return
    if not args.no_build:
        build()
    start()


if __name__ == "__main__":
    main()
