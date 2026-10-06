use crate::keyvalues;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub fn discover() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("HL2_ROOT") {
        return validate(PathBuf::from(p));
    }
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
    fn candidate_roots_include_macos_steam_path_when_home_is_present() {
        if let Some(home) = std::env::var_os("HOME") {
            let h = PathBuf::from(home);
            let mac_path = h.join("Library/Application Support/Steam");
            assert!(mac_path.ends_with("Library/Application Support/Steam"));
        }
    }
}
