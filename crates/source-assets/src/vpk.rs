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

pub struct Vfs {
    roots: Vec<PathBuf>,
    archives: Vec<Vpk>,
    embedded: BTreeMap<String, Vec<u8>>,
    pub warnings: Vec<String>,
    custom_roots: Vec<PathBuf>,
    custom_archives: Vec<Vpk>,
}
impl Vfs {
    pub fn mount(game: &Path) -> Result<Self> {
        let roots = vec![game.join("hl2"), game.join("platform")];
        let mut archives = Vec::new();
        let mut warnings = Vec::new();
        let mut custom_roots = Vec::new();
        let mut custom_archives = Vec::new();
        let custom = game.join("hl2/custom");
        if custom.is_dir() {
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
        for root in &roots {
            if !root.is_dir() {
                continue;
            }
            let mut paths = std::fs::read_dir(root)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .is_some_and(|n| n.to_string_lossy().ends_with("_dir.vpk"))
                })
                .collect::<Vec<_>>();
            paths.sort();
            for p in paths {
                match Vpk::open(&p) {
                    Ok(v) => archives.push(v),
                    Err(e) => warnings.push(format!("{}: {e}", p.display())),
                }
            }
        }
        Ok(Self {
            custom_roots,
            custom_archives,
            roots,
            archives,
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
        for root in &self.roots {
            let p = root.join(&name);
            if p.is_file() {
                return Ok(Some(std::fs::read(p)?));
            }
        }
        for archive in &self.archives {
            if let Some(d) = archive.read(&name)? {
                return Ok(Some(d));
            }
        }
        Ok(None)
    }
    pub fn archive_counts(&self) -> Vec<(String, usize)> {
        self.archives
            .iter()
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
            .chain(&self.archives)
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
