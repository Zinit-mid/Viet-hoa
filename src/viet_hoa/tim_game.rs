//! Tìm game đã cài (giống tim_steam / tim_epic / tim_gog của app cũ) + game người dùng tự thêm.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use winreg::enums::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
};
use winreg::RegKey;

use crate::duong_dan;

/// Game công cụ / runtime của Steam, không phải game.
const STEAM_BO_QUA: &[&str] = &[
    "228980", "1070560", "1391110", "1628350", "1493710", "250820", "1826330",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Game {
    /// steam | epic | gog | tay
    pub nguon: String,
    pub ma: String,
    pub ten: String,
    pub thu_muc: PathBuf,
    pub phien_ban: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoaiGame {
    CrimsonDesert,
    Rise,
    Tr2013,
    Shadow,
    Khac,
}

impl Game {
    pub fn loai(&self) -> LoaiGame {
        let co = |f: &str| self.thu_muc.join(f).is_file();
        let steam = |ma: &str| self.nguon == "steam" && self.ma == ma;
        if steam("3321460") || co("bin64/CrimsonDesert.exe") {
            LoaiGame::CrimsonDesert
        } else if steam("391220") || co("ROTTR.exe") {
            LoaiGame::Rise
        } else if steam("203160") || co("TombRaider.exe") {
            LoaiGame::Tr2013
        } else if steam("750920") || co("SOTTR.exe") {
            LoaiGame::Shadow
        } else {
            LoaiGame::Khac
        }
    }
}

fn doc_reg(goc: winreg::HKEY, khoa: &str, ten: &str) -> Option<String> {
    for co in [0, KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
        if let Ok(k) = RegKey::predef(goc).open_subkey_with_flags(khoa, KEY_READ | co) {
            if let Ok(v) = k.get_value::<String, _>(ten) {
                return Some(v);
            }
        }
    }
    None
}

/// Đọc file .vdf / .acf của Steam thành cây (khóa viết thường).
#[derive(Debug, Clone)]
pub enum Vdf {
    Chu(String),
    Khoi(BTreeMap<String, Vdf>),
}

impl Vdf {
    pub fn lay(&self, k: &str) -> Option<&Vdf> {
        match self {
            Vdf::Khoi(m) => m.get(k),
            _ => None,
        }
    }
    pub fn chu(&self) -> Option<&str> {
        match self {
            Vdf::Chu(s) => Some(s),
            _ => None,
        }
    }
}

pub fn doc_vdf(chu: &str) -> Vdf {
    // tách "chuỗi" và { }
    let mut the: Vec<Result<String, char>> = Vec::new();
    let c: Vec<char> = chu.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '"' => {
                let mut s = String::new();
                i += 1;
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && i + 1 < c.len() {
                        s.push(c[i]);
                        i += 1;
                    }
                    s.push(c[i]);
                    i += 1;
                }
                the.push(Ok(s));
            }
            '{' | '}' => the.push(Err(c[i])),
            _ => {}
        }
        i += 1;
    }
    fn khoi(the: &[Result<String, char>], vt: &mut usize) -> Vdf {
        let mut m = BTreeMap::new();
        while *vt < the.len() {
            let t = &the[*vt];
            *vt += 1;
            match t {
                Err('}') => return Vdf::Khoi(m),
                Err(_) => continue,
                Ok(k) => {
                    let k = k.to_lowercase();
                    match the.get(*vt) {
                        Some(Err('{')) => {
                            *vt += 1;
                            m.insert(k, khoi(the, vt));
                        }
                        Some(Ok(v)) => {
                            m.insert(k, Vdf::Chu(v.replace("\\\\", "\\")));
                            *vt += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        Vdf::Khoi(m)
    }
    let mut vt = 0;
    khoi(&the, &mut vt)
}

pub fn tim_steam() -> Vec<Game> {
    let mut goc: Vec<PathBuf> = [
        doc_reg(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath"),
        doc_reg(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Valve\Steam",
            "InstallPath",
        ),
        doc_reg(HKEY_LOCAL_MACHINE, r"SOFTWARE\Valve\Steam", "InstallPath"),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .collect();
    goc.push(
        PathBuf::from(
            std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into()),
        )
        .join("Steam"),
    );
    let mut thu_vien = Vec::new();
    for g in goc.into_iter().filter(|g| g.is_dir()) {
        thu_vien.push(g.clone());
        if let Ok(chu) = std::fs::read_to_string(g.join("steamapps").join("libraryfolders.vdf")) {
            if let Some(Vdf::Khoi(m)) = doc_vdf(&chu).lay("libraryfolders") {
                for v in m.values() {
                    let p = v.lay("path").and_then(Vdf::chu).or_else(|| v.chu());
                    if let Some(p) = p.filter(|p| !p.is_empty()) {
                        thu_vien.push(PathBuf::from(p));
                    }
                }
            }
        }
    }
    let mut da = HashSet::new();
    let mut kq = Vec::new();
    for tv in thu_vien {
        if !da.insert(chuan(&tv)) {
            continue;
        }
        let Ok(ds) = std::fs::read_dir(tv.join("steamapps")) else {
            continue;
        };
        for f in ds.flatten() {
            let ten = f.file_name().to_string_lossy().to_string();
            if !(ten.starts_with("appmanifest_") && ten.ends_with(".acf")) {
                continue;
            }
            let Ok(chu) = std::fs::read_to_string(f.path()) else {
                continue;
            };
            let a = doc_vdf(&chu);
            let Some(a) = a.lay("appstate") else { continue };
            let lay = |k: &str| a.lay(k).and_then(Vdf::chu).unwrap_or("").to_string();
            let (ma, dir) = (lay("appid"), lay("installdir"));
            let thu_muc = tv.join("steamapps").join("common").join(&dir);
            if ma.is_empty()
                || STEAM_BO_QUA.contains(&ma.as_str())
                || dir.is_empty()
                || !thu_muc.is_dir()
            {
                continue;
            }
            let ten = Some(lay("name")).filter(|s| !s.is_empty()).unwrap_or(dir);
            kq.push(Game {
                nguon: "steam".into(),
                ma,
                ten,
                thu_muc,
                phien_ban: lay("buildid"),
            });
        }
    }
    kq
}

pub fn tim_epic() -> Vec<Game> {
    let pd = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    let thu = Path::new(&pd).join(r"Epic\EpicGamesLauncher\Data\Manifests");
    let mut kq = Vec::new();
    for f in std::fs::read_dir(thu).into_iter().flatten().flatten() {
        if f.path().extension().is_none_or(|e| e != "item") {
            continue;
        }
        let Ok(b) = std::fs::read(f.path()) else {
            continue;
        };
        let Ok(d) = serde_json::from_slice::<serde_json::Value>(&b) else {
            continue;
        };
        let thu_muc = PathBuf::from(d["InstallLocation"].as_str().unwrap_or(""));
        if !thu_muc.is_dir() || d["bIsApplication"] == serde_json::Value::Bool(false) {
            continue;
        }
        let s = |k: &str| d[k].as_str().unwrap_or("").to_string();
        let ma = Some(s("AppName"))
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| thu_muc.display().to_string());
        let ten = Some(s("DisplayName"))
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| ten_thu_muc(&thu_muc));
        kq.push(Game {
            nguon: "epic".into(),
            ma,
            ten,
            thu_muc,
            phien_ban: s("AppVersionString"),
        });
    }
    kq
}

pub fn tim_gog() -> Vec<Game> {
    let mut kq: Vec<Game> = Vec::new();
    for khoa_goc in [
        r"SOFTWARE\WOW6432Node\GOG.com\Games",
        r"SOFTWARE\GOG.com\Games",
    ] {
        let Ok(k) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(khoa_goc) else {
            continue;
        };
        for con in k.enum_keys().flatten() {
            let duong = format!("{khoa_goc}\\{con}");
            let thu_muc = doc_reg(HKEY_LOCAL_MACHINE, &duong, "path").map(PathBuf::from);
            if let Some(thu_muc) = thu_muc.filter(|p| p.is_dir()) {
                if !kq.iter().any(|g| g.ma == con) {
                    kq.push(Game {
                        nguon: "gog".into(),
                        ten: doc_reg(HKEY_LOCAL_MACHINE, &duong, "gameName")
                            .unwrap_or_else(|| ten_thu_muc(&thu_muc)),
                        ma: con,
                        thu_muc,
                        phien_ban: doc_reg(HKEY_LOCAL_MACHINE, &duong, "ver").unwrap_or_default(),
                    });
                }
            }
        }
        if !kq.is_empty() {
            break;
        }
    }
    kq
}

fn ten_thu_muc(p: &Path) -> String {
    p.file_name()
        .map(|t| t.to_string_lossy().to_string())
        .unwrap_or_else(|| p.display().to_string())
}

/// Đường dẫn chuẩn hóa để so (giống os.path.normcase(normpath)).
pub fn chuan(p: &Path) -> String {
    p.display()
        .to_string()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

// ---------------- game tự thêm ----------------

#[derive(Default, Serialize, Deserialize)]
struct CauHinhVh {
    #[serde(default)]
    game_tay: Vec<serde_json::Value>,
    #[serde(flatten)]
    khac: serde_json::Map<String, serde_json::Value>,
}

fn file_cau_hinh() -> PathBuf {
    duong_dan::viet_hoa_game().join("cau_hinh.json")
}

fn doc_ch() -> CauHinhVh {
    std::fs::read(file_cau_hinh())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn game_tay() -> Vec<Game> {
    doc_ch()
        .game_tay
        .iter()
        .filter_map(|g| {
            let thu_muc = PathBuf::from(g["thu_muc"].as_str()?);
            thu_muc.is_dir().then(|| Game {
                nguon: "tay".into(),
                ma: chuan(&thu_muc),
                ten: g["ten"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| ten_thu_muc(&thu_muc)),
                thu_muc,
                phien_ban: String::new(),
            })
        })
        .collect()
}

pub fn them_game_tay(thu_muc: &Path, ten: Option<&str>) {
    let mut ch = doc_ch();
    let k = chuan(thu_muc);
    ch.game_tay
        .retain(|g| g["thu_muc"].as_str().map(|p| chuan(Path::new(p))) != Some(k.clone()));
    ch.game_tay.push(serde_json::json!({"thu_muc": thu_muc.display().to_string(), "ten": ten.map(String::from).unwrap_or_else(|| ten_thu_muc(thu_muc))}));
    if let Ok(b) = serde_json::to_vec_pretty(&ch) {
        let _ = duong_dan::ghi_file(&file_cau_hinh(), &b);
    }
}

pub fn bo_game_tay(thu_muc: &Path) {
    let mut ch = doc_ch();
    let k = chuan(thu_muc);
    ch.game_tay
        .retain(|g| g["thu_muc"].as_str().map(|p| chuan(Path::new(p))) != Some(k.clone()));
    if let Ok(b) = serde_json::to_vec_pretty(&ch) {
        let _ = duong_dan::ghi_file(&file_cau_hinh(), &b);
    }
}

/// Tất cả game (Steam, Epic, GOG, tự thêm), bỏ trùng thư mục, xếp theo tên.
pub fn tim_tat_ca() -> Vec<Game> {
    let mut kq = tim_steam();
    kq.extend(tim_epic());
    kq.extend(tim_gog());
    kq.extend(game_tay());
    let mut da = HashSet::new();
    kq.retain(|g| da.insert(chuan(&g.thu_muc)));
    kq.sort_by_key(|g| g.ten.to_lowercase());
    kq
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thử thật trên máy: cargo test -- --ignored tim_that --nocapture
    #[test]
    #[ignore]
    fn tim_that() {
        for g in tim_tat_ca() {
            println!(
                "{:6} | {:40} | {:?} | {}",
                g.nguon,
                g.ten,
                g.loai(),
                g.thu_muc.display()
            );
        }
    }
    #[test]
    fn doc_vdf_steam() {
        let chu = r#""libraryfolders" { "0" { "path" "C:\\Program Files (x86)\\Steam" "apps" { "228980" "1" } } "1" { "path" "E:\\SteamLibrary" } }"#;
        let v = doc_vdf(chu);
        let lf = v.lay("libraryfolders").unwrap();
        assert_eq!(
            lf.lay("1").unwrap().lay("path").unwrap().chu(),
            Some(r"E:\SteamLibrary")
        );
        assert_eq!(
            lf.lay("0").unwrap().lay("path").unwrap().chu(),
            Some(r"C:\Program Files (x86)\Steam")
        );
    }
}
