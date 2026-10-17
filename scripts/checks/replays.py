"""Packaged UI replays and captures, with assertions on each run's report.

Cross-platform (Python 3.9+, no packages): builds the packaged game once
(scripts/build-bevy.sh, or scripts/build-bevy.ps1 on Windows), runs every check as its own
bounded launch of bin/hl2-bevy[.exe] with its own `--report`, and checks the labelled states the
replay records (console mode and output, the Options dialog's tab / focus / pending changes,
loaded maps, saved settings, captures). Prints one ok/FAIL line per check; details in
artifacts/replays/<check>.log, reports and captures next to it.

Every check starts from default settings: bin/hl2-config.json is set aside at the start and
restored at the end (a check that needs particular settings writes them itself).

Categories:
  shared      every platform, in the background (`--background`: no window, nothing takes
              focus, `--capture` reads the game frame)
  windowed    every platform, needs a real window and takes focus (opt in: WINDOWED=1)
  macos       macOS only, windowed (opt in: WINDOWED=1)
  windows     Windows only, windowed (opt in: WINDOWED=1)

Environment (no arguments, so the command line never changes):
  ONLY="console options"   run these checks only
  WINDOWED=1               also run the windowed checks of this platform
  NO_BUILD=1               use bin/ as it is
"""
from __future__ import annotations

import json
import os
import platform
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Optional

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / "artifacts" / "replays"
CONFIG = REPO / "bin" / "hl2-config.json"
SAVED = OUT / "user-settings.json"
SYSTEM = platform.system()  # Darwin, Windows, Linux
EXE = REPO / "bin" / ("hl2-bevy.exe" if SYSTEM == "Windows" else "hl2-bevy")


# ---- running the game ---------------------------------------------------------------------

@dataclass
class Run:
    name: str
    exit: int
    report: dict
    log: str
    capture: Optional[Path]

    def samples(self) -> dict:
        return {s["label"]: s for s in self.report["render"]["simulation"]["samples"] if s.get("label")}

    def sample(self, label: str) -> dict:
        found = self.samples().get(label)
        if found is None:
            raise Failure(f"{self.name}: no sample labelled {label!r}")
        return found

    def console(self) -> dict:
        return self.report["render"]["presentation"]["console"]

    def loaded(self) -> list[str]:
        return [line[len("Loaded "):] for line in self.console()["output"] if line.startswith("Loaded ")]

    def campaign(self) -> dict:
        return self.report["render"]["presentation"]["campaign"]


class Failure(Exception):
    pass


def game(name: str, *args: str, timeout: int = 240, background: bool = True, capture: bool = True) -> Run:
    """One bounded launch; the report and capture are written to artifacts/replays/<name>.*"""
    report = OUT / f"{name}.json"
    png = OUT / f"{name}.png" if capture else None
    for stale in (report, png):
        if stale and stale.exists():
            stale.unlink()
    command = [str(EXE), "--no-setup", "--volume", "0.01", *args, "--report", str(report)]
    command.insert(2, "--background" if background else "--no-focus")
    if png:
        command += ["--capture", str(png)]
    env = dict(os.environ, RUST_LOG=os.environ.get("RUST_LOG", "info,hl2_bevy=debug"))
    try:
        done = subprocess.run(command, cwd=REPO, env=env, capture_output=True, text=True,
                              errors="replace", timeout=timeout)
        status, log = done.returncode, done.stdout + done.stderr
    except subprocess.TimeoutExpired as expired:
        status, log = -1, f"timed out after {timeout} s\n{expired.stdout or ''}{expired.stderr or ''}"
    log = re.sub(r"\x1b\[[0-9;]*m", "", log if isinstance(log, str) else log.decode(errors="replace"))
    (OUT / f"{name}.game.log").write_text(log, encoding="utf-8")
    if status != 0:
        raise Failure(f"{name}: exit {status} (see {name}.game.log)")
    if not report.exists():
        raise Failure(f"{name}: no report")
    run = Run(name, status, json.loads(report.read_text(encoding="utf-8")), log, png)
    if png and not (png.exists() and png.stat().st_size > 0):
        raise Failure(f"{name}: no capture")
    if "--movement-script" in args and not run.report["render"]["simulation"].get("script_finished"):
        raise Failure(f"{name}: the replay did not finish")
    return run


def settings(**sections: dict) -> None:
    """Default settings with the given sections merged in (written before a launch)."""
    current = json.loads(CONFIG.read_text(encoding="utf-8")) if CONFIG.exists() else {}
    for key, values in sections.items():
        current.setdefault(key, {}).update(values)
    CONFIG.write_text(json.dumps(current, indent=2), encoding="utf-8")


def saved() -> dict:
    return json.loads(CONFIG.read_text(encoding="utf-8")) if CONFIG.exists() else {}


def expect(what: str, actual, expected) -> None:
    print(f"{what}: {actual!r}")
    if actual != expected:
        raise Failure(f"{what}: expected {expected!r}, got {actual!r}")


def holds(what: str, condition: bool, detail="") -> None:
    print(f"{'ok' if condition else 'FAILED'}: {what}{f' ({detail})' if detail != '' else ''}")
    if not condition:
        raise Failure(what)


MENU = ("--map", "background01", "--main-menu", "--width", "1024", "--height", "768", "--frames", "400")


# ---- shared checks (background) ------------------------------------------------------------

def check_submenus():
    """Episode One, back, Half-Life 2, back, Extras > Lost Coast: each game's background loads."""
    run = game("submenus", "--map", "background01", "--main-menu", "--frames", "2400",
               "--movement-script", "test-inputs/bevy-menu-submenus.json")
    expect("loaded", run.loaded(), ["hl2/background01", "episodic/ep1_background01",
                                    "hl2/background01", "lostcoast/background01"])
    expect("mode", run.console()["mode"], "MainMenu")


def check_return_menu():
    """Episode Two menu, `map d1_trainstation_02`, Return to main menu: back over hl2/background01."""
    run = game("return_menu", "--map", "background01", "--main-menu", "--frames", "3200",
               "--movement-script", "test-inputs/bevy-menu-return.json")
    expect("loaded", run.loaded(), ["hl2/background01", "ep2/ep2_background01",
                                    "hl2/d1_trainstation_02", "hl2/background01"])
    expect("mode", run.console()["mode"], "MainMenu")
    expect("saved menu background", saved()["gameplay"].get("background_map"), "background01")


def check_chapter_menu():
    """Leaving d2_coast_01 shows hl2/background04 (chapter 7), d3_c17_10a hl2/background06."""
    run = game("chapter_menu", "--map", "background01", "--main-menu", "--frames", "4400",
               "--movement-script", "test-inputs/bevy-menu-chapter-return.json", timeout=300)
    expect("loaded", run.loaded(), ["hl2/background01", "hl2/d2_coast_01", "hl2/background04",
                                    "hl2/d3_c17_10a", "hl2/background06"])


def check_chapter_quit():
    """Loading a gameplay map saves its chapter's background, so a mid-game quit relaunches over it."""
    settings(gameplay={"background_game": "hl2", "background_map": "background01"})
    game("chapter_quit", "--map", "d2_coast_01", "--frames", "600", timeout=180)
    gameplay = saved()["gameplay"]
    expect("saved menu background", f"{gameplay.get('background_game')}/{gameplay.get('background_map')}",
           "hl2/background04")


KEYBOARD_FOCUS = {
    # replay suffix: (label, Options tab, focused control)
    "keyboard-check": ("focus-reverse-mouse", "Mouse", "ReverseMouse"),
    "keyboard-slider": ("focus-sensitivity-slider", "Mouse", "Slider"),
    "keyboard-entry": ("focus-sensitivity-entry", "Mouse", "SensitivityLabel"),
    "keyboard-radio": ("focus-skill-radio", "Gameplay", "Skill2Radio"),
    "keyboard-combo": ("focus-resolution", "Video", "Resolution"),
    "keyboard-button": ("focus-advanced", "Video", "AdvancedButton"),
}
OPTIONS_STATES = {
    # replay suffix: (label, what the report must show there)
    "": ("keyboard-advanced", {"mode": "Options", "sub": "KeyboardAdvanced"}),
    "video-advanced": ("video-advanced", {"mode": "Options", "sub": "VideoAdvanced"}),
    "drag": ("options-dragged", {"mode": "Options", "sub": None}),
    "console": ("type-map", {"mode": "Console"}),
    "console-resize": ("console-moved", {"mode": "Console"}),
    "console-wrap": ("long-word", {"mode": "Console"}),
}


def check_options():
    """Every Options capture replay: the labelled state (dialog, sub-dialog, keyboard focus)."""
    for script in sorted((REPO / "test-inputs").glob("bevy-options-capture*.json")):
        suffix = script.stem[len("bevy-options-capture"):].lstrip("-")
        if suffix not in KEYBOARD_FOCUS and suffix not in OPTIONS_STATES:
            raise Failure(f"{script.name}: no expectation in replays.py")
        if CONFIG.exists():
            CONFIG.unlink()
        run = game(f"options-{suffix or 'keyboard-advanced'}", *MENU,
                   "--movement-script", str(script.relative_to(REPO)))
        if suffix in KEYBOARD_FOCUS:
            label, tab, focus = KEYBOARD_FOCUS[suffix]
            options = run.sample(label)["console"].get("options") or {}
            expect(f"{suffix} tab", options.get("tab"), tab)
            expect(f"{suffix} focus", options.get("focus"), focus)
            holds(f"{suffix} focus drawn", options.get("focus_visible") is True)
            if suffix == "keyboard-entry":
                holds("the focused entry edits", options.get("editing") is True)
        else:
            label, wanted = OPTIONS_STATES[suffix]
            console = run.sample(label)["console"]
            expect(f"{suffix or 'keyboard-advanced'} mode", console["mode"], wanted["mode"])
            if "sub" in wanted:
                expect(f"{suffix or 'keyboard-advanced'} sub-dialog", (console.get("options") or {}).get("sub"),
                       wanted["sub"])


def check_placement():
    """A dragged Options dialog is saved, and reopens there in a second launch."""
    game("placement-drag", *MENU, "--movement-script", "test-inputs/bevy-options-capture-drag.json")
    dialogs = saved().get("dialogs", {})
    holds("the Options placement is saved", bool(dialogs.get("options")), dialogs)
    steps = json.loads((REPO / "test-inputs/bevy-options-capture.json").read_text(encoding="utf-8"))
    reopen = OUT / "placement-reopen-input.json"
    reopen.write_text(json.dumps([s for s in steps if s["tick"] <= 25] + [steps[-1]]), encoding="utf-8")
    run = game("placement-reopen", *MENU, "--movement-script", str(reopen))
    expect("reopened", run.console()["mode"], "Options")
    expect("placement kept", saved().get("dialogs", {}).get("options"), dialogs.get("options"))


def check_captions():
    """ep1_background02 (Kleiner's broadcast): captions shown when on, none when off."""
    counts = {}
    for mode in ("closed_captions", "none"):
        if CONFIG.exists():
            CONFIG.unlink()
        settings(gameplay={"background_game": "episodic", "background_map": "ep1_background02"},
                 audio={"captions": mode})
        run = game(f"captions-{mode}", "--frames", "420", "--width", "1024", "--height", "768", timeout=180)
        captions = run.report["render"]["presentation"]["hud"]["captions"]
        counts[mode] = captions.get("processed", 0)
        print(f"captions {mode}: processed {counts[mode]}, dictionaries {captions.get('dictionaries')}")
    holds("captions on processes speech", counts["closed_captions"] > 0, counts)
    holds("captions off processes none", counts["none"] == 0, counts)


def check_bg_compare():
    """Each game's chapter 1 background at 1024x768, FOV 75 (compare with the retail images)."""
    for game_, map_ in (("hl2", "background01"), ("episodic", "ep1_background01"),
                        ("ep2", "ep2_background01"), ("lostcoast", "background01")):
        if CONFIG.exists():
            CONFIG.unlink()
        settings(gameplay={"background_game": game_, "background_map": map_}, video={"fov": 75.0})
        run = game(f"bg_compare-{game_}", "--frames", "300", "--width", "1024", "--height", "768", timeout=180)
        campaign = run.campaign()
        expect(game_, f"{campaign['loaded_game']}/{campaign['loaded_map']}", f"{game_}/{map_}")


def check_save_config():
    """Console variables persist: volume, sensitivity, fov, mat_hdr_level."""
    game("save_config", "--map", "d1_trainstation_02", "--frames", "120",
         "--movement-script", "test-inputs/bevy-save-config.json", capture=False)
    config = saved()
    expect("volume", config["audio"]["volume"], 0.55)
    expect("sensitivity", config["mouse"]["sensitivity"], 4.2)
    expect("fov", config["video"]["fov"], 95.0)
    expect("hdr", config["video"]["hdr"], "full")


def check_console():
    """Console: cheats, setpos/ent_fire while paused, Unicode editing, weapon selection vs Escape,
    the pause menu and the console over it (test-inputs/bevy-console.json)."""
    run = game("console", "--map", "d1_trainstation_02", "--width", "1280", "--height", "720",
               "--movement-script", "test-inputs/bevy-console.json")
    s = run.samples()
    out = lambda label: s[label]["console"]["output"]
    mode = lambda label: s[label]["console"]["mode"]
    g = lambda label: s[label]["gameplay"]
    holds("impulse 101 refused without sv_cheats",
          not g("cheat-denied")["inventory"]["suit"]
          and any("Can't use impulse" in x for x in out("cheat-denied")))
    holds("sv_cheats 1 then impulse 101 grants the suit and weapons",
          g("cheat-granted")["inventory"]["suit"] and len(g("cheat-granted")["inventory"]["owned"]) >= 6)
    holds("a quoted semicolon stays in the argument", "value;kept" in out("cheat-granted"))
    holds("the console freezes the scene", g("setpos-paused")["time"] == g("cheat-denied")["time"] == 0)
    expect("setpos moves the feet", s["setpos-paused"]["player"]["feet"], [-3260, -2024, 64])
    holds("ent_fire is queued while paused",
          any("Queued station_entrance.Close" in x for x in out("setpos-paused")))
    expect("Unicode input", s["unicode-input"]["console"]["input"], "é\U0001f980")
    expect("Unicode editing keeps whole characters", s["unicode-edit"]["console"]["input"], "é")
    holds("the weapon selection opens without switching",
          g("selection-open")["pending"] == "weapon_smg1"
          and g("selection-open")["inventory"]["active"] == "weapon_pistol")
    holds("Escape cancels the selection before pausing",
          g("selection-canceled")["pending"] is None and mode("selection-canceled") == "Gameplay")
    holds("Escape opens the pause menu", mode("pause-open") == "Pause" and s["pause-open"]["paused"])
    holds("a click outside the menu stays paused",
          mode("outside-click") == "Pause" and g("outside-click")["time"] == g("pause-open")["time"])
    holds("the console opens over the pause menu and returns to it",
          mode("console-from-pause") == "Console" and mode("return-to-pause") == "Pause")
    holds("the scene stays frozen meanwhile",
          g("console-paused")["time"] == g("return-to-pause")["time"] == g("pause-open")["time"])
    holds("unknown commands are rejected",
          "Unknown or unimplemented command: unsupported_command" in out("console-paused"))
    # The replay's `menu-resume` step presses Enter with no pause-menu row selected; here Escape
    # clears the menu selection, so Enter does nothing and its resume assertions do not apply.


def check_console_map():
    """`map` from the console: two direct loads, a missing map reported without losing the scene."""
    run = game("console_map", "--map", "d1_trainstation_02", "--width", "1280", "--height", "720",
               "--movement-script", "test-inputs/bevy-console-map.json")
    s = run.samples()
    expect("loaded", run.loaded()[-2:], ["hl2/d1_trainstation_01", "hl2/d1_trainstation_02"])
    holds("a direct map starts without the old inventory",
          not s["direct-map-arrival"]["gameplay"]["inventory"]["owned"]
          and s["direct-map-arrival"]["console"]["mode"] == "Gameplay")
    failures = run.campaign()["failures"]
    holds("the missing map is one explicit error",
          len(failures) == 1 and "hl2_rs_missing_test_map" in failures[0], failures)
    holds("the failed load keeps the player and scene",
          s["missing-map-request"]["player"] == s["missing-map-recovered"]["player"]
          and s["missing-map-request"]["gameplay"]["time"] == s["missing-map-recovered"]["gameplay"]["time"])
    holds("console state survives a load",
          s["second-direct-arrival"]["console"]["sv_cheats"]
          and "history-survives" in s["second-direct-arrival"]["console"]["output"])
    holds("nothing is loading at the end", run.campaign()["loading"] is None)


def check_campaign():
    """Level transitions d1_trainstation_01 > 02 > 03 through their triggers, inventory kept."""
    run = game("campaign", "--map", "d1_trainstation_01", "--width", "1280", "--height", "720",
               "--movement-script", "test-inputs/bevy-campaign.json")
    s = run.samples()
    expect("loaded", run.loaded()[-3:], ["hl2/d1_trainstation_01", "hl2/d1_trainstation_02",
                                         "hl2/d1_trainstation_03"])
    campaign = run.campaign()
    holds("ends on station 03 without failures",
          campaign["active_map"] == "d1_trainstation_03" and not campaign["failures"]
          and campaign["loading"] is None, campaign)
    holds("the inventory crosses the transitions",
          s["station03-final"]["gameplay"]["inventory"]["suit"]
          and "weapon_pistol" in s["station03-final"]["gameplay"]["inventory"]["owned"])
    holds("the clock restarts on the new map",
          s["station02-fire-after-reset"]["gameplay"]["time"] < s["station01-exit-trigger"]["gameplay"]["time"])


def check_dev_overlay():
    """F1 developer overlay with live statistics, also while paused."""
    run = game("dev_overlay", "--map", "d1_trainstation_02", "--width", "1280", "--height", "720",
               "--movement-script", "test-inputs/bevy-dev-overlay.json")
    overlay = run.report["render"]["presentation"]["hud"]["developer_overlay"]
    holds("visible with five lines", overlay["visible"] and len(overlay["lines"]) == 5)
    holds("frame statistics", overlay["fps"] > 0 and overlay["frame_ms"] > 0)
    holds("renderer statistics", any("triangles" in x and "textures" in x for x in overlay["lines"]))


def check_hud():
    """Weapon HUD: loadout, slot selection panels and the secondary ammo row."""
    run = game("hud", "--map", "d1_trainstation_02", "--width", "1280", "--height", "720",
               "--movement-script", "test-inputs/bevy-hud.json")
    s = run.samples()
    holds("unarmed at the start", not s["unarmed"]["gameplay"]["inventory"]["owned"])
    holds("the loadout arrives", s["hud-final"]["gameplay"]["inventory"]["suit"])
    holds("HUD quads drawn", run.report["render"]["presentation"]["hud"]["quads"] > 0)


# ---- windowed checks -----------------------------------------------------------------------

def check_window():
    """The presented window, a console resolution change and an Options (keyboard) Apply."""
    settings(video={"display_mode": "windowed", "width": 1024, "height": 768})
    run = game("window-console-resolution", "--map", "background01", "--main-menu", "--windowed",
               "--width", "1024", "--height", "768", "--frames", "600", "--capture-window",
               "--movement-script", "test-inputs/bevy-menu-resolution-change.json", background=False)
    holds("the frame was resized", "frame resize done" in run.log)
    expect("Options opened after the resize", run.console()["mode"], "Options")
    if CONFIG.exists():
        CONFIG.unlink()
    settings(video={"display_mode": "windowed", "width": 1024, "height": 768})
    run = game("window-options-apply", "--map", "background01", "--main-menu", "--windowed",
               "--width", "1024", "--height", "768", "--frames", "600", "--capture-window",
               "--movement-script", "test-inputs/bevy-options-keyboard-apply-resolution.json",
               background=False)
    options = run.sample("focus-apply")["console"].get("options") or {}
    expect("Shift+Tab reaches Apply", options.get("focus"), "apply")
    video = saved()["video"]
    holds("Space on Apply changed the resolution", (video["width"], video["height"]) != (1024, 768), video)
    holds("the window took it", "[video] apply" in run.log)


def check_window_place():
    """macOS: a windowed window is sized and centred in the screen's visible frame."""
    for width, height in ((2560, 1600), (1024, 768)):
        settings(video={"display_mode": "windowed"})
        run = game(f"window_place-{width}x{height}", "--main-menu", "--windowed", "--width", str(width),
                   "--height", str(height), "--frames", "240", background=False, capture=False)
        placed = re.search(r"desktop area DesktopArea \{ visible: Rect \{ min: Vec2\(([-\d.]+), ([-\d.]+)\), "
                           r"max: Vec2\(([-\d.]+), ([-\d.]+)\) \}, title_bar: ([-\d.]+) \} at \[(-?\d+), (-?\d+)\]",
                           run.log)
        holds(f"{width}x{height} placed in the visible frame", placed is not None)
        top = float(placed.group(7)) / 2  # points on a 2x display; the log is in physical pixels
        holds(f"{width}x{height} below the menu bar", top >= float(placed.group(2)) - 1, top)


def check_no_border():
    """Windows: "Windowed (No Border)" is a borderless window covering the monitor."""
    settings(video={"display_mode": "no_border"})
    run = game("no_border", "--map", "background01", "--main-menu", "--borderless", "--frames", "240",
               "--capture-window", background=False)
    holds("the window is borderless full screen", "BorderlessFullscreen" in run.log)


@dataclass
class Check:
    name: str
    run: Callable[[], None]
    category: str  # shared | windowed | macos | windows


CHECKS = [
    Check("submenus", check_submenus, "shared"),
    Check("return_menu", check_return_menu, "shared"),
    Check("chapter_menu", check_chapter_menu, "shared"),
    Check("chapter_quit", check_chapter_quit, "shared"),
    Check("options", check_options, "shared"),
    Check("placement", check_placement, "shared"),
    Check("captions", check_captions, "shared"),
    Check("bg_compare", check_bg_compare, "shared"),
    Check("save_config", check_save_config, "shared"),
    Check("console", check_console, "shared"),
    Check("console_map", check_console_map, "shared"),
    Check("campaign", check_campaign, "shared"),
    Check("dev_overlay", check_dev_overlay, "shared"),
    Check("hud", check_hud, "shared"),
    Check("window", check_window, "windowed"),
    Check("window_place", check_window_place, "macos"),
    Check("no_border", check_no_border, "windows"),
]


def selected() -> list[Check]:
    only = os.environ.get("ONLY", "").split()
    windowed = os.environ.get("WINDOWED") == "1"
    platform_category = {"Darwin": "macos", "Windows": "windows"}.get(SYSTEM)
    chosen = []
    for check in CHECKS:
        if only and check.name not in only:
            continue
        if check.category in ("macos", "windows") and check.category != platform_category:
            continue
        if check.category != "shared" and not windowed and not only:
            continue
        chosen.append(check)
    return chosen


def build() -> None:
    if os.environ.get("NO_BUILD") == "1":
        return
    if SYSTEM == "Windows":
        command = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                   str(REPO / "scripts" / "build-bevy.ps1")]
    else:
        command = ["bash", str(REPO / "scripts" / "build-bevy.sh")]
    done = subprocess.run(command, cwd=REPO, capture_output=True, text=True, errors="replace")
    (OUT / "build.log").write_text(done.stdout + done.stderr, encoding="utf-8")
    if done.returncode != 0:
        sys.exit(f"build failed: {OUT / 'build.log'}")


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    build()
    had_settings = CONFIG.exists()
    if had_settings:
        shutil.copy2(CONFIG, SAVED)
    failed, started = 0, time.time()
    try:
        for check in selected():
            if CONFIG.exists():
                CONFIG.unlink()  # every check starts from default settings
            begin = time.time()
            log = OUT / f"{check.name}.log"
            with log.open("w", encoding="utf-8") as handle:
                stdout, sys.stdout = sys.stdout, handle
                try:
                    check.run()
                    ok, why = True, ""
                except Failure as failure:
                    ok, why = False, str(failure)
                    print(f"FAIL: {failure}")
                except Exception as error:  # a broken report or check is a failure too
                    ok, why = False, f"{type(error).__name__}: {error}"
                    print(f"FAIL: {why}")
                finally:
                    sys.stdout = stdout
            seconds = time.time() - begin
            if ok:
                print(f"  ok    {check.name:14} {check.category:8} {seconds:5.0f} s")
            else:
                failed += 1
                print(f"  FAIL  {check.name:14} {check.category:8} {seconds:5.0f} s  {why}")
    finally:
        if had_settings:
            shutil.copy2(SAVED, CONFIG)
        elif CONFIG.exists():
            CONFIG.unlink()
    print(f"=== {len(selected())} checks, {failed} failed, {time.time() - started:.0f} s; logs in {OUT} ===")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
