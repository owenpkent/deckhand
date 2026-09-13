#!/usr/bin/env python3
"""
Deckhand Launcher
=================

One command to check the toolchain, build the surface and the Rust
workspace, and start the board. Windows only for now: the daemon's
no-activate window styling and Reveal are Win32.

Usage::

    python run.py              # check, build, restart the board
    python run.py --test       # also run both test suites before starting
    python run.py --no-build   # restart the last build without rebuilding
    python run.py --check      # report the toolchain and exit
    python run.py --stop       # stop a running instance and exit

What it does:

1. Checks Python (3.10+), cargo, node, and npm, and prints a winget
   install hint for anything missing. Warns, without stopping, when the
   WebView2 runtime or the ``claude`` CLI cannot be found, and when this
   repo's sessions are not wired into the shim.
2. Installs the TypeScript compiler into ``app/ui/node_modules`` on the
   first run. That is the only npm dependency.
3. Stops any running instance, because Windows will not let cargo
   relink an executable that is still running.
4. Builds the surface (tsc), then the Rust workspace. The surface goes
   first because tauri embeds ``app/ui`` into the binary.
5. With ``--test``, runs ``npm test`` and ``cargo test --workspace``.
6. Starts the new build detached and prints where the runtime files
   live.

See also ``scripts/build-app.ps1``, the build-only PowerShell equivalent
that CI mirrors.
"""

import argparse
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent
UI = REPO / "app" / "ui"
EXE = REPO / "target" / "debug" / "deckhand.exe"
SHIM = REPO / "target" / "debug" / "deckhand-shim.exe"
LOCAL = Path(os.environ.get("LOCALAPPDATA", "")) / "deckhand"

IS_WINDOWS = sys.platform == "win32"

# Install hints are winget commands because that is the package manager
# every Windows 11 machine already has.
TOOLS = [
    ("cargo", "Rust toolchain", "winget install Rustlang.Rustup", "then open a new terminal"),
    ("node", "Node.js", "winget install OpenJS.NodeJS.LTS", ""),
    ("npm", "npm (ships with Node.js)", "winget install OpenJS.NodeJS.LTS", ""),
]

# The Evergreen WebView2 runtime registers under this client id in one of
# these hives depending on whether it was a per-machine or per-user
# install. Tauri needs it at run time, not at build time.
WEBVIEW2_CLIENT = r"Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"


def banner() -> None:
    print("=" * 50)
    print("  Deckhand: control surface for Claude Code")
    print("=" * 50)
    print()


def check_platform() -> bool:
    if IS_WINDOWS:
        return True
    print(f"ERROR: Deckhand builds on Windows only for now (this is {sys.platform}).")
    print("  The no-activate window and Reveal are Win32; see docs/ARCHITECTURE.md.")
    return False


def check_python_version() -> bool:
    if sys.version_info < (3, 10):
        print("ERROR: Python 3.10 or higher is required")
        print(f"  Current version: {sys.version.split()[0]}")
        return False
    return True


def tool_version(name: str) -> str:
    """Best-effort first line of `<tool> --version`; blank on any failure."""
    try:
        out = subprocess.run(
            [name, "--version"],
            capture_output=True,
            text=True,
            shell=IS_WINDOWS,
            timeout=20,
        )
        lines = out.stdout.strip().splitlines()
        return lines[0] if lines else ""
    except Exception:
        return ""


def check_tools() -> bool:
    """Required tools. Prints one line per tool and a hint per missing one."""
    ok = True
    for name, label, hint, after in TOOLS:
        path = shutil.which(name)
        if path:
            print(f"  ok       {label}: {tool_version(name) or path}")
        else:
            ok = False
            print(f"  MISSING  {label}")
            print(f"           install: {hint}")
            if after:
                print(f"           {after}")
    return ok


def webview2_present() -> bool:
    try:
        import winreg
    except ImportError:
        return True
    hives = [
        (winreg.HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\" + WEBVIEW2_CLIENT),
        (winreg.HKEY_LOCAL_MACHINE, "SOFTWARE\\" + WEBVIEW2_CLIENT),
        (winreg.HKEY_CURRENT_USER, "Software\\" + WEBVIEW2_CLIENT),
    ]
    for hive, key in hives:
        try:
            with winreg.OpenKey(hive, key) as k:
                winreg.QueryValueEx(k, "pv")
                return True
        except OSError:
            continue
    # Fall back to the install directory, which is where the runtime
    # lands even when the registry has been tidied.
    pf86 = os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")
    return (Path(pf86) / "Microsoft" / "EdgeWebView" / "Application").exists()


def shim_wired() -> bool:
    """True if this repo's local settings register the shim as a hook."""
    local = REPO / ".claude" / "settings.local.json"
    try:
        return "deckhand-shim" in local.read_text(encoding="utf-8")
    except OSError:
        return False


def optional_warnings() -> list[str]:
    """Things the board can run without, each with the consequence stated."""
    warnings = []
    if not webview2_present():
        warnings.append(
            "  WARNING: WebView2 runtime not found. Tauri needs it to open a window.\n"
            "  Windows 11 ships it; otherwise: winget install Microsoft.EdgeWebView2Runtime"
        )
    if not shutil.which("claude"):
        warnings.append(
            "  NOTE: `claude` is not on PATH. Cold start cannot enumerate running\n"
            "  sessions, so tiles fill only as hook events arrive."
        )
    if not shim_wired():
        warnings.append(
            "  NOTE: this repo's sessions are not wired into the shim, so the board\n"
            "  will not see them. Register target\\debug\\deckhand-shim.exe as a hook\n"
            "  in .claude/settings.local.json (docs/CLAUDE_CODE_ADAPTER.md);\n"
            "  installable registration is tracked in TODO.md."
        )
    return warnings


def run(cmd: list[str], cwd: Path) -> bool:
    where = "." if cwd == REPO else cwd.relative_to(REPO).as_posix()
    print(f"> {' '.join(cmd)}  (in {where})")
    result = subprocess.run(cmd, cwd=cwd, shell=IS_WINDOWS)
    if result.returncode != 0:
        print(f"ERROR: failed ({result.returncode}): {' '.join(cmd)}")
        return False
    return True


def setup_surface() -> bool:
    if (UI / "node_modules").exists():
        return True
    print("Installing the TypeScript compiler into app/ui/node_modules...")
    return run(["npm", "install", "--no-audit", "--no-fund"], cwd=UI)


def build() -> bool:
    return run(["npx", "tsc"], cwd=UI) and run(["cargo", "build", "--workspace"], cwd=REPO)


def test() -> bool:
    return run(["npm", "test"], cwd=UI) and run(["cargo", "test", "--workspace"], cwd=REPO)


def stop_running() -> bool:
    """Stop any running instance. True if there was one."""
    # taskkill exits 128 when nothing matched; that is the common case.
    result = subprocess.run(["taskkill", "/IM", "deckhand.exe", "/F"], capture_output=True)
    if result.returncode != 0:
        return False
    # A forced kill skips the daemon's clean exit, which is what removes
    # the contact file. Left behind, it makes every hook spend a connect
    # timeout on a dead port until the next start, so do the cleanup here.
    try:
        (LOCAL / "daemon.json").unlink()
    except OSError:
        pass
    return True


def start() -> int:
    if not EXE.exists():
        print(f"ERROR: not built: {EXE}")
        print("  run without --no-build first")
        return 1
    # Detach fully: the debug build is console-subsystem and would
    # otherwise open (and tie itself to) a console window.
    flags = subprocess.CREATE_NO_WINDOW | subprocess.DETACHED_PROCESS
    subprocess.Popen([str(EXE)], creationflags=flags, close_fds=True)
    time.sleep(2.0)
    contact = LOCAL / "daemon.json"
    print(f"started {EXE.name}")
    print(f"  daemon contact: {contact} ({'present' if contact.exists() else 'MISSING'})")
    print(f"  bindings:       {LOCAL / 'bindings.json'}")
    print(f"  reveal log:     {LOCAL / 'reveal.log'}")
    print(f"  shim:           {SHIM}")
    print("quit from the surface's Quit key, or: python run.py --stop")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check the toolchain, build, optionally test, and run the Deckhand board."
    )
    parser.add_argument("--no-build", action="store_true", help="skip building, just restart")
    parser.add_argument("--test", action="store_true", help="run both test suites before starting")
    parser.add_argument("--check", action="store_true", help="report the toolchain and exit")
    parser.add_argument("--stop", action="store_true", help="stop a running instance and exit")
    args = parser.parse_args()

    banner()
    os.chdir(REPO)

    if not check_platform():
        return 1
    if not check_python_version():
        return 1

    if args.stop:
        print("stopped" if stop_running() else "nothing was running")
        return 0

    print("Toolchain:")
    tools_ok = check_tools()
    print()
    warnings = optional_warnings()
    for w in warnings:
        print(w)
        print()

    if args.check:
        return 0 if tools_ok else 1
    if not tools_ok:
        print("ERROR: install the missing tools above, then run again.")
        return 1

    # Stop before building: cargo cannot relink a running executable.
    if stop_running():
        print("stopped the running instance")
        print()

    if not args.no_build:
        if not setup_surface() or not build():
            return 1
    if args.test and not test():
        return 1

    print()
    return start()


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("\ninterrupted")
        sys.exit(130)
