//! VPK v1/v2 reader with preload support, embedded data, and per-entry CRC checking.
use crate::{bytes, keyvalues, u16le, u32le};
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Entry {
    pub crc: u32,
    pub archive: u16,
    pub offset: u32,
    pub size: u32,
    pub preload: Vec<u8>,
}
pub struct Vpk {
    pub path: PathBuf,
    pub version: u32,
    pub entries: BTreeMap<String, Entry>,
    data_start: usize,
}
pub fn normalize(path: &str) -> Result<String> {
    let p = path.replace('\\', "/").to_lowercase();
    if p.starts_with('/') || p.contains(':') || p.split('/').any(|s| s == ".." || s == ".") {
        bail!("unsafe asset path: {path}");
    }
    Ok(p)
}
fn cstr(data: &[u8], pos: &mut usize) -> Result<String> {
    let tail = data.get(*pos..).context("string offset outside tree")?;
    let len = tail
        .iter()
        .position(|b| *b == 0)
        .context("unterminated VPK string")?;
    let s = std::str::from_utf8(&tail[..len])?.to_string();
    *pos += len + 1;
    Ok(s)
}
impl Vpk {
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;
        if u32le(&data, 0)? != 0x55aa1234 {
            bail!("invalid VPK signature");
        }
        let version = u32le(&data, 4)?;
        let header = match version {
            1 => 12,
            2 => 28,
            _ => bail!("unsupported VPK version {version}"),
        };
        let tree_size = u32le(&data, 8)? as usize;
        let tree = bytes(&data, header, tree_size)?;
        let mut p = 0;
        let mut entries = BTreeMap::new();
        loop {
            let extension = cstr(tree, &mut p)?;
            if extension.is_empty() {
                break;
            }
            loop {
                let directory = cstr(tree, &mut p)?;
                if directory.is_empty() {
                    break;
                }
                loop {
                    let stem = cstr(tree, &mut p)?;
                    if stem.is_empty() {
                        break;
                    }
                    let crc = u32le(tree, p)?;
                    let preload = u16le(tree, p + 4)? as usize;
                    let archive = u16le(tree, p + 6)?;
                    let offset = u32le(tree, p + 8)?;
                    let size = u32le(tree, p + 12)?;
                    if u16le(tree, p + 16)? != 0xffff {
                        bail!("VPK entry terminator missing");
                    }
                    p += 18;
                    let preload = bytes(tree, p, preload)?.to_vec();
                    p += preload.len();
                    let name = format!(
                        "{}{}{}",
                        if directory == " " {
                            String::new()
                        } else {
                            format!("{directory}/")
                        },
                        stem,
                        if extension == " " {
                            String::new()
                        } else {
                            format!(".{extension}")
                        }
                    );
                    entries.insert(
                        normalize(&name)?,
                        Entry {
                            crc,
                            archive,
                            offset,
                            size,
                            preload,
                        },
                    );
                }
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            version,
            entries,
            data_start: header + tree_size,
        })
    }
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(e) = self.entries.get(&normalize(name)?) else {
            return Ok(None);
        };
        if e.size as u64 + e.preload.len() as u64 > 512 * 1024 * 1024 {
            bail!("asset exceeds 512 MiB limit");
        }
        let mut data = e.preload.clone();
        if e.size > 0 {
            let (archive, offset) = if e.archive == 0x7fff {
                (self.path.clone(), self.data_start as u64 + e.offset as u64)
            } else {
                let name = self
                    .path
                    .file_name()
                    .context("VPK filename")?
                    .to_string_lossy();
                let base = name
                    .strip_suffix("_dir.vpk")
                    .context("VPK directory filename must end in _dir.vpk")?;
                (
                    self.path
                        .with_file_name(format!("{base}_{:03}.vpk", e.archive)),
                    e.offset as u64,
                )
            };
            let mut file =
                File::open(&archive).with_context(|| format!("open {}", archive.display()))?;
            let end = offset
                .checked_add(e.size as u64)
                .context("entry offset overflow")?;
            if end > file.metadata()?.len() {
                bail!("truncated VPK archive entry {name}");
            }
            file.seek(SeekFrom::Start(offset))?;
            let old = data.len();
            data.resize(old + e.size as usize, 0);
            file.read_exact(&mut data[old..])?;
        }
        if crc32fast::hash(&data) != e.crc {
            bail!("VPK CRC mismatch for {name}");
        }
        Ok(Some(data))
    }
}

struct SearchPath {
    root: PathBuf,
    archives: Vec<Vpk>,
}

pub struct Vfs {
    search_paths: Vec<SearchPath>,
    embedded: BTreeMap<String, Vec<u8>>,
    pub warnings: Vec<String>,
    custom_roots: Vec<PathBuf>,
    custom_archives: Vec<Vpk>,
}
impl Vfs {
    /// Mounts the installed game and platform content, defaulting to the base Half-Life 2 search path.
    pub fn mount(game: &Path) -> Result<Self> {
        Self::mount_expansion(game, None)
    }

    /// Mounts the search path of the game installed in folder `game` of the installation
    /// (`hl2`, `episodic`, `ep2`, `lostcoast`).
    pub fn mount_game(root: &Path, game: &str) -> Result<Self> {
        let expansion = (!game.eq_ignore_ascii_case(crate::install::BASE_GAME)).then_some(game);
        Self::mount_expansion(root, expansion)
    }

    /// Mounts content for an optional specific expansion (`episodic`, `ep2`, `lostcoast`).
    ///
    /// Search paths follow Source engine inheritance:
    /// - `ep2` -> `ep2` + `episodic` + `hl2` + `platform`
    /// - `episodic` -> `episodic` + `hl2` + `platform`
    /// - `lostcoast` -> `lostcoast` + `hl2` + `platform`
    /// - base `hl2` -> `hl2` + `platform`
    pub fn mount_expansion(game: &Path, expansion: Option<&str>) -> Result<Self> {
        let mut roots = Vec::new();
        if let Some(exp) = expansion {
            match exp {
                "ep2" => {
                    let ep2 = game.join("ep2");
                    if ep2.is_dir() {
                        roots.push(ep2);
                    }
                    let episodic = game.join("episodic");
                    if episodic.is_dir() {
                        roots.push(episodic);
                    }
                }
                "episodic" => {
                    let episodic = game.join("episodic");
                    if episodic.is_dir() {
                        roots.push(episodic);
                    }
                }
                "lostcoast" => {
                    let lostcoast = game.join("lostcoast");
                    if lostcoast.is_dir() {
                        roots.push(lostcoast);
                    }
                }
                other => {
                    let path = game.join(other);
                    if path.is_dir() {
                        roots.push(path);
                    }
                }
            }
        }
        roots.push(game.join("hl2"));
        roots.push(game.join("platform"));
        let mut warnings = Vec::new();
        let mut custom_roots = Vec::new();
        let mut custom_archives = Vec::new();
        let mut custom_dirs = Vec::new();
        if let Some(exp) = expansion {
            let p = game.join(format!("{exp}/custom"));
            if p.is_dir() {
                custom_dirs.push(p);
            }
        }
        let hl2_custom = game.join("hl2/custom");
        if hl2_custom.is_dir() {
            custom_dirs.push(hl2_custom);
        }
        for custom in custom_dirs {
            let mut paths = std::fs::read_dir(custom)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .collect::<Vec<_>>();
            paths.sort();
            for path in paths {
                if path.is_dir() {
                    custom_roots.push(path);
                } else if path.extension().is_some_and(|e| e == "vpk")
                    && path.file_name().is_some_and(|n| {
                        !n.to_string_lossy()
                            .trim_end_matches("_dir.vpk")
                            .rsplit('_')
                            .next()
                            .is_some_and(|n| n.len() == 3 && n.chars().all(|c| c.is_ascii_digit()))
                    })
                {
                    match Vpk::open(&path) {
                        Ok(v) => custom_archives.push(v),
                        Err(e) => warnings.push(format!("{}: {e}", path.display())),
                    }
                }
            }
        }
        let mut search_paths = Vec::new();
        for root in roots {
            if !root.is_dir() {
                continue;
            }
            let mut paths = std::fs::read_dir(&root)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().ends_with("_dir.vpk"))
                })
                .collect::<Vec<_>>();
            paths.sort();
            let mut archives = Vec::new();
            for p in paths {
                match Vpk::open(&p) {
                    Ok(v) => archives.push(v),
                    Err(e) => warnings.push(format!("{}: {e}", p.display())),
                }
            }
            search_paths.push(SearchPath { root, archives });
        }
        Ok(Self {
            custom_roots,
            custom_archives,
            search_paths,
            embedded: BTreeMap::new(),
            warnings,
        })
    }
    pub fn mount_pak(&mut self, data: &[u8]) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data))?;
        let mut total = 0;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i)?;
            if f.is_dir() {
                continue;
            }
            let name = normalize(f.name())?;
            if f.size() > 64 * 1024 * 1024 {
                bail!("embedded asset too large");
            }
            total += f.size();
            if total > 256 * 1024 * 1024 {
                bail!("map pak exceeds limit");
            }
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes)?;
            self.embedded.insert(name, bytes);
        }
        Ok(())
    }
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let name = normalize(name)?;
        if let Some(d) = self.embedded.get(&name) {
            return Ok(Some(d.clone()));
        }
        for root in &self.custom_roots {
            let p = root.join(&name);
            if p.is_file() {
                return Ok(Some(std::fs::read(p)?));
            }
        }
        for archive in &self.custom_archives {
            if let Some(d) = archive.read(&name)? {
                return Ok(Some(d));
            }
        }
        for layer in &self.search_paths {
            let p = layer.root.join(&name);
            if p.is_file() {
                return Ok(Some(std::fs::read(p)?));
            }
            for archive in &layer.archives {
                if let Some(d) = archive.read(&name)? {
                    return Ok(Some(d));
                }
            }
        }
        Ok(None)
    }
    /// The installation root passed to [`Vfs::mount`] (parent of the mounted `hl2` folder).
    pub fn game_root(&self) -> Option<&Path> {
        self.search_paths.first().and_then(|s| s.root.parent())
    }
    pub fn archive_counts(&self) -> Vec<(String, usize)> {
        self.search_paths
            .iter()
            .flat_map(|s| &s.archives)
            .map(|v| {
                (
                    v.path.file_name().unwrap().to_string_lossy().to_string(),
                    v.entries.len(),
                )
            })
            .collect()
    }
    pub fn base_texture(&self, material: &str) -> Result<Option<String>> {
        self.material_texture(material, 0)
    }
    pub fn material_value(&self, material: &str, key: &str) -> Result<Option<String>> {
        self.material_property(material, key, 0)
    }
    fn material_property(&self, material: &str, key: &str, depth: usize) -> Result<Option<String>> {
        if depth > 8 {
            bail!("VMT include cycle/depth limit");
        }
        let path = format!(
            "materials/{}.vmt",
            material
                .trim_end_matches(".vmt")
                .trim_start_matches("materials/")
        );
        let Some(data) = self.read(&path)? else {
            return Ok(None);
        };
        let text = std::str::from_utf8(&data)?;
        if let Some(v) = keyvalues::value(text, key)? {
            return Ok(Some(v));
        }
        if let Some(include) = keyvalues::value(text, "include")? {
            return self.material_property(&include, key, depth + 1);
        }
        Ok(None)
    }
    pub fn resolve_material_name(&self, name: &str) -> Option<String> {
        let suffix = format!("/{}.vmt", name.to_lowercase());
        let mut found = self
            .custom_archives
            .iter()
            .chain(self.search_paths.iter().flat_map(|s| &s.archives))
            .flat_map(|a| a.entries.keys())
            .filter(|p| p.starts_with("materials/") && p.ends_with(&suffix));
        let first = found.next()?;
        if found.next().is_some() {
            return None;
        }
        Some(
            first
                .trim_start_matches("materials/")
                .trim_end_matches(".vmt")
                .into(),
        )
    }
    fn material_texture(&self, material: &str, depth: usize) -> Result<Option<String>> {
        if depth > 8 {
            bail!("VMT include cycle/depth limit");
        }
        let path = format!(
            "materials/{}.vmt",
            material
                .trim_end_matches(".vmt")
                .trim_start_matches("materials/")
        );
        let Some(data) = self.read(&path)? else {
            return Ok(None);
        };
        let text = std::str::from_utf8(&data)?;
        if let Some(v) = keyvalues::value(text, "$basetexture")? {
            return Ok(Some(v));
        }
        if let Some(include) = keyvalues::value(text, "include")? {
            return self.material_texture(&include, depth + 1);
        }
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal() {
        assert!(normalize("../save/x").is_err());
        assert!(normalize("C:\\x").is_err());
        assert_eq!(normalize("Materials\\A.VTF").unwrap(), "materials/a.vtf");
    }
    #[test]
    fn rejects_truncated_vpk() {
        let p = std::env::temp_dir().join(format!("hl2-rs-vpk-{}.vpk", std::process::id()));
        std::fs::write(&p, b"bad").unwrap();
        assert!(Vpk::open(&p).is_err());
        std::fs::remove_file(p).unwrap();
    }
}
