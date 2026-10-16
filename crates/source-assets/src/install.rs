use crate::keyvalues;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub fn candidate_steam_roots() -> Vec<PathBuf> {
    let mut steam_roots = vec![
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
    ];
    #[cfg(windows)]
    if let Ok(out) = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(line) = s.lines().find(|l| l.contains("REG_SZ")) {
            if let Some((_, v)) = line.split_once("REG_SZ") {
                steam_roots.insert(0, PathBuf::from(v.trim()));
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let h = PathBuf::from(home);
        steam_roots.push(h.join("Library/Application Support/Steam"));
        steam_roots.push(h.join(".steam/steam"));
        steam_roots.push(h.join(".local/share/Steam"));
    }
    steam_roots
}

pub fn discover() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("HL2_ROOT") {
        return validate(PathBuf::from(p));
    }
    let steam_roots = candidate_steam_roots();
    let mut libraries = steam_roots.clone();
    for root in &steam_roots {
        if let Ok(s) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            if let Ok(t) = keyvalues::tokens(&s) {
                for p in t.windows(2) {
                    if p[0] == "path" {
                        libraries.push(PathBuf::from(&p[1]));
                    }
                }
            }
        }
    }
    libraries.dedup();
    for library in libraries {
        let manifest = library.join("steamapps/appmanifest_220.acf");
        let dir = std::fs::read_to_string(&manifest)
            .ok()
            .and_then(|s| keyvalues::value(&s, "installdir").ok().flatten())
            .unwrap_or("Half-Life 2".into());
        let candidate = library.join("steamapps/common").join(dir);
        if validate(candidate.clone()).is_ok() {
            return Ok(candidate);
        }
    }
    bail!("Half-Life 2 not found; pass --game <install-folder> or set HL2_ROOT")
}
pub fn validate(path: PathBuf) -> Result<PathBuf> {
    if !path.join("hl2/gameinfo.txt").is_file() {
        bail!(
            "{} is not an HL2 install (missing hl2/gameinfo.txt)",
            path.display()
        );
    }
    path.canonicalize().context("resolve game root")
}
/// The installation folder of the base game, whose search path every other game extends.
pub const BASE_GAME: &str = "hl2";

/// The games of an installation: the base game first, then every other folder with a
/// `gameinfo.txt` (`episodic`, `ep2`, `lostcoast`, ...), sorted by name.
pub fn games(root: &Path) -> Vec<String> {
    let mut others = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("gameinfo.txt").is_file())
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .filter(|name| name != BASE_GAME)
        .collect::<Vec<_>>();
    others.sort();
    std::iter::once(BASE_GAME.to_owned())
        .chain(others)
        .collect()
}

/// The game whose own folder ships `maps/<map>.bsp`, loose or in one of its own VPKs, for map
/// names given without a game (`map`, `--map`). Like the engine, which searches the running
/// game first, `current` wins, then the base game, then the other games: Half-Life 2 and Lost
/// Coast both ship `background01`.
pub fn game_for_map(root: &Path, map: &str, current: Option<&str>) -> Option<String> {
    let path = crate::vpk::normalize(&format!("maps/{}.bsp", map.trim_end_matches(".bsp"))).ok()?;
    let mut games = games(root);
    if let Some(current) = current.and_then(|c| games.iter().position(|g| g == c)) {
        let current = games.remove(current);
        games.insert(0, current);
    }
    games.into_iter().find(|game| {
        let folder = root.join(game);
        folder.join(&path).is_file()
            || std::fs::read_dir(&folder)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().ends_with("_dir.vpk"))
                })
                .any(|p| crate::vpk::Vpk::open(&p).is_ok_and(|v| v.entries.contains_key(&path)))
    })
}

pub fn maps(root: &Path) -> Result<Vec<PathBuf>> {
    let mut maps = std::fs::read_dir(root.join("hl2/maps"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "bsp"))
        .collect::<Vec<_>>();
    maps.sort();
    Ok(maps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_root_fails_validation() {
        let temp = std::env::temp_dir();
        assert!(validate(temp).is_err());
    }

    #[test]
    fn game_for_map_reads_each_game_folder_base_game_first() {
        let root = std::env::temp_dir().join(format!("hl2-rs-games-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (game, maps) in [
            ("hl2", &["background01", "d1_trainstation_01"][..]),
            ("lostcoast", &["background01", "d2_lostcoast"][..]),
            ("episodic", &["ep1_citadel_00"][..]),
            ("hl2_complete", &[][..]),
        ] {
            std::fs::create_dir_all(root.join(game).join("maps")).unwrap();
            std::fs::write(root.join(game).join("gameinfo.txt"), "").unwrap();
            for map in maps {
                std::fs::write(root.join(game).join(format!("maps/{map}.bsp")), "").unwrap();
            }
        }
        std::fs::create_dir_all(root.join("platform")).unwrap();
        assert_eq!(
            games(&root),
            ["hl2", "episodic", "hl2_complete", "lostcoast"]
        );
        let game = |map: &str| game_for_map(&root, map, None);
        assert_eq!(game("background01").as_deref(), Some("hl2"));
        assert_eq!(game("d2_lostcoast.bsp").as_deref(), Some("lostcoast"));
        assert_eq!(game("ep1_citadel_00").as_deref(), Some("episodic"));
        assert_eq!(game("ep2_outland_01"), None);
        assert_eq!(game("../hl2/maps/background01"), None);
        let from_lostcoast = |map: &str| game_for_map(&root, map, Some("lostcoast"));
        assert_eq!(from_lostcoast("background01").as_deref(), Some("lostcoast"));
        assert_eq!(from_lostcoast("d1_trainstation_01").as_deref(), Some("hl2"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires an owned Half-Life 2 installation"]
    fn owned_install_maps_belong_to_the_game_that_ships_them() {
        let root = discover().expect("owned Half-Life 2 installation");
        let game = |map: &str| game_for_map(&root, map, None);
        assert_eq!(game("d1_trainstation_01").as_deref(), Some("hl2"));
        assert_eq!(game("background01").as_deref(), Some("hl2"));
        assert_eq!(game("ep1_citadel_00").as_deref(), Some("episodic"));
        assert_eq!(game("ep2_outland_01").as_deref(), Some("ep2"));
        assert_eq!(game("d2_lostcoast").as_deref(), Some("lostcoast"));
        assert_eq!(game("no_such_map"), None);
    }

    #[test]
    fn candidate_roots_include_macos_steam_path_when_home_is_present() {
        if let Some(home) = std::env::var_os("HOME") {
            let roots = candidate_steam_roots();
            let expected = PathBuf::from(home).join("Library/Application Support/Steam");
            assert!(roots.contains(&expected));
        }
    }
}
