"""Opt-in owned GPU stress guard: native-resolution 4x MSAA, idle/turn/swing.

NO_BUILD=1 python3 scripts/checks/check_blur_performance.py
Defaults reproduce the reported 3456x2168 regression. BLUR_WIDTH/BLUR_HEIGHT,
BLUR_AA and BLUR_RUNS override the stress workload; not part of headless CI.
Runs only an isolated muted package and records frame tails as well as means.
"""
import json
import math
import os
import statistics
import replays as replay


def fixture(scene):
    frames = [{"tick": 1, "fly": True, "eye": [-3104, -2018, 128], "yaw": 12,
               "actions": [{"action": "loadout"}, {"action": "slot", "slot": 0}]},
              {"tick": 2, "actions": [{"action": "confirm"}]}]
    if scene == "turn":
        frames += [{"tick": t, "yaw": 12+90*math.sin((t-100)*0.075)} for t in range(200, 1200)]
    elif scene == "swing":
        for tick in range(100, 1150, 40):
            frames += [{"tick": tick, "primary": True}, {"tick": tick+1, "primary": False}]
    frames.append({"tick": 1200, "label": "performance"})
    return frames


def measure(scene, advanced, repeat):
    width, height = int(os.environ.get("BLUR_WIDTH", "3456")), int(os.environ.get("BLUR_HEIGHT", "2168"))
    aa = os.environ.get("BLUR_AA", "x4")
    if replay.CONFIG.exists():
        replay.CONFIG.unlink()
    replay.settings(video={"width": width, "height": height, "hdr": "none", "hdr_migrated": True,
                           "advanced": {"motion_blur": True, "motion_blur_viewmodel": advanced,
                                        "antialiasing": aa, "vsync": False}})
    name = f"{scene}-{'advanced' if advanced else 'retail'}-{repeat}"
    path = replay.OUT / (scene + ".input.json")
    path.write_text(json.dumps(fixture(scene)), encoding="utf-8")
    run = replay.game(name, "--map", "d1_trainstation_02", "--width", str(width), "--height", str(height),
                      "--movement-script", str(path), "--frames", "2400", "--profile", capture=False)
    replay.holds("no render errors", "ERROR" not in run.log and "Caught rendering error" not in run.log)
    hud = run.report["render"]["presentation"]["hud"]
    replay.expect("actual internal frame size", hud["viewport"], [float(width), float(height)])
    result = hud["developer_overlay"]["benchmark"]
    replay.holds("measured beyond warm-up", result["measured_frames"] >= 100, result)
    replay.holds("profiled frame tails", result.get("p95_frame_ms_upper_bound") is not None, result)
    print(name, json.dumps(result), flush=True)
    return result


def main():
    replay.OUT = replay.REPO / "artifacts/blur-performance-regression/guard"
    replay.GAME = replay.OUT / "game"
    replay.EXE = replay.GAME / replay.EXE_NAME
    replay.CONFIG = replay.GAME / "hl2-config.json"
    replay.OUT.mkdir(parents=True, exist_ok=True)
    replay.build()
    replay.stage()
    results = {}
    for scene in ("still", "turn", "swing"):
        modes = {False: [], True: []}
        for repeat in range(int(os.environ.get("BLUR_RUNS", "1"))):
            for advanced in ((False, True) if repeat % 2 == 0 else (True, False)):
                modes[advanced].append(measure(scene, advanced, repeat))
        median = lambda mode, key: statistics.median(r[key] for r in modes[mode])
        results[scene] = {"retail": modes[False], "advanced": modes[True]}
        (replay.OUT / "summary.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
        replay.holds(scene + " mean stays within 20% + 0.25 ms of retail",
                     median(True, "mean_frame_ms") <= median(False, "mean_frame_ms")*1.2+0.25, results[scene])
        replay.holds(scene + " p95 stays within 25% + 1 ms of retail",
                     median(True, "p95_frame_ms_upper_bound") <= median(False, "p95_frame_ms_upper_bound")*1.25+1,
                     results[scene])
    print("native-resolution blur performance guard passed")


if __name__ == "__main__":
    main()
