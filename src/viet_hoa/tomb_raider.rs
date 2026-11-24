//! Việt hóa Tomb Raider (2013), Rise of the Tomb Raider, Shadow of the Tomb Raider (engine Foundation).
//! Chuyển từ viet_hoa_rottr.py + viet_hoa_tr.py của app cũ (cùng tên gói mod, cùng file dữ liệu).
//!   Rise:   gói mod riêng bigfile._mod.viethoa_dichgame.* (chữ + phụ đề phim + lời thoại lúc chơi
//!           + giọng đã lồng + font tiếng Việt). Gỡ = xóa gói.
//!   TR2013: game không đọc gói mod → tạo lại patch2.000.tiger (giữ bản gốc bigfile._orig.patch2.000.tiger).
//!   Shadow: gói mod riêng v5 + .specmasks.toc.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use super::mul::{self, Mul};
use super::tiger::{self, Locals, Muc, Nfo, MAGIC_CDRM};
use super::viec_dich::{BangTen, ViecDich};
use crate::duong_dan;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoaiTr {
    Rise,
    Tr2013,
    Shadow,
}

impl LoaiTr {
    pub fn ten(self) -> &'static str {
        match self {
            LoaiTr::Rise => "Rise of the Tomb Raider",
            LoaiTr::Tr2013 => "Tomb Raider (2013)",
            LoaiTr::Shadow => "Shadow of the Tomb Raider",
        }
    }
    fn thu_muc(self) -> &'static str {
        match self {
            LoaiTr::Rise => "rottr",
            LoaiTr::Tr2013 => "tr2013",
            LoaiTr::Shadow => "shadow",
        }
    }
}

/// Font Be Vietnam Pro (OFL) – lấy hình dấu hỏi và râu
const TTF_DAU: &[u8] = include_bytes!("../../assets/fonts/BeVietnamPro-Bold.ttf");
const TEN_MOD: &str = "viethoa_dichgame";
const TEN_LOCALS: &str = "pcx64-w\\local\\locals.bin";
const LOCALE_EN: u64 = 0xFFFF_0001;
const TEP_NFO: &str = "bigfile._mod.viethoa_dichgame.000.000.nfo";
const TEP_TIGER: &str = "bigfile._mod.viethoa_dichgame.000_ENGLISH.000.tiger";
const TEP_TIGER_GOC: &str = "bigfile._mod.viethoa_dichgame.000.000.tiger";
const DAU_TE: &[u8] = b"Version 1\r\nTextEntry";
const PHIEN_BAN_MUL: i64 = 2;
const PHIEN_BAN_TE: i64 = 1;
const MARKER_TR13: &str = "viethoa_dichgame_tr2013.txt";
const PATCH2: &str = "patch2.000.tiger";
const PATCH2_GOC: &str = "bigfile._orig.patch2.000.tiger";
const ID_PATCH2: i32 = 67;
const GIONG_TR13: &str = "patch2_ENGLISH.000.tiger";
const GIONG_TR13_GOC: &str = "bigfile._orig.patch2_ENGLISH.000.tiger";
const NGON_NGU: [&str; 15] = [
    "ENGLISH",
    "FRENCH",
    "GERMAN",
    "ITALIAN",
    "LATAMSPANISH",
    "IBERSPANISH",
    "JAPANESE",
    "PORTUGUESE",
    "POLISH",
    "RUSSIAN",
    "DUTCH",
    "KOREAN",
    "CHINESE",
    "SIMPLECHINESE",
    "ARABIC",
];
const NGON_NGU_SHADOW: [Option<&str>; 16] = [
    Some("english"),
    Some("french"),
    Some("german"),
    Some("italian"),
    Some("latamspanish"),
    Some("iberspanish"),
    Some("japanese"),
    Some("portuguese"),
    Some("polish"),
    Some("russian"),
    Some("dutch"),
    Some("korean"),
    Some("chinese"),
    Some("simplechinese"),
    None,
    Some("arabic"),
];

const HUONG_DAN: &str = "Bạn là dịch giả chuyên Việt hóa game. Game: Rise of the Tomb Raider (phiêu lưu hành động; \
nhân vật chính Lara Croft đi tìm thành phố cổ Kitezh ở Siberia; tổ chức Trinity; người Remnant; Deathless Ones).
Bạn nhận một mảng JSON các chuỗi tiếng Anh lấy từ file chữ của game và trả về mảng JSON các bản dịch tiếng Việt.

Quy tắc bắt buộc:
- CHỈ trả về một mảng JSON hợp lệ, CÙNG SỐ PHẦN TỬ, CÙNG THỨ TỰ. Không giải thích, không ```.
- GIỮ NGUYÊN tiếng Anh: tên người, địa danh, tổ chức, phe phái, tên vũ khí / trang bị / kỹ năng riêng, \
và mọi từ trong danh sách \"Giữ nguyên\" nếu có.
- Giữ NGUYÊN từng ký tự mọi thẻ và mã: {...}, [...], %s, %d, \\n, $..., kể cả khoảng trắng quanh chúng. \
Không dịch, không thêm, không bớt thẻ. Riêng chú thích âm thanh trong <...> (vd <coughing>, <via radio>): \
giữ dấu < > nhưng DỊCH chữ bên trong (vd <ho>, <qua bộ đàm>).
- Câu cảm thán, câu ngắn, câu chửi thề cũng phải dịch (vd \"Shit!\" -> \"Chết tiệt!\"), không trả lại tiếng Anh.
- Giữ nguyên số, ký hiệu, khoảng trắng đầu/cuối và cách xuống dòng.
- Văn phong tự nhiên như phụ đề phim phiêu lưu hiện đại. Lara xưng \"tôi\"; xưng hô hợp ngữ cảnh.
- Menu, nút bấm, tiêu đề: ngắn gọn, rõ nghĩa như game Việt hóa chính thức.
- Chuỗi chỉ gồm tên riêng hoặc không có gì để dịch: trả lại y nguyên.";

fn huong_dan(loai: LoaiTr) -> String {
    let dong1 = HUONG_DAN.lines().next().unwrap();
    let thay = match loai {
        LoaiTr::Rise => return HUONG_DAN.to_string(),
        LoaiTr::Tr2013 => "Game: Tomb Raider (2013) (phiêu lưu sinh tồn; Lara Croft trẻ tuổi mắc kẹt trên đảo Yamatai; giáo phái Solarii; Nữ hoàng Mặt Trời Himiko; con tàu Endurance).",
        LoaiTr::Shadow => "Game: Shadow of the Tomb Raider (phiêu lưu hành động; Lara Croft ở rừng rậm Mỹ Latinh, thành phố ẩn Paititi; tổ chức Trinity; Dr. Dominguez; ngày tận thế của người Maya).",
    };
    HUONG_DAN.replacen(
        dong1,
        &format!("Bạn là dịch giả chuyên Việt hóa game. {thay}"),
        1,
    )
}

fn ten_rieng(loai: LoaiTr) -> &'static [&'static str] {
    match loai {
        LoaiTr::Rise => &[
            "Lara",
            "Croft",
            "Lara Croft",
            "Lord Croft",
            "Richard Croft",
            "Jonah",
            "Jacob",
            "Ana",
            "Konstantin",
            "Sofia",
            "Nadia",
            "Trinity",
            "Remnant",
            "Kitezh",
            "Siberia",
            "Syria",
            "Baba Yaga",
            "Deathless Ones",
            "Divine Source",
            "Prophet",
            "Atlas",
            "Croft Manor",
            "Soviet",
            "Geothermal Valley",
            "Lost City",
            "Ice Cave",
            "Acropolis",
            "Copper Mill",
            "Gulag",
            "Flooded Archives",
        ],
        LoaiTr::Tr2013 => &[
            "Lara",
            "Croft",
            "Lara Croft",
            "Roth",
            "Conrad Roth",
            "Sam",
            "Samantha",
            "Jonah",
            "Reyes",
            "Alex",
            "Grim",
            "Whitman",
            "Mathias",
            "Himiko",
            "Yamatai",
            "Solarii",
            "Stormguard",
            "Oni",
            "Endurance",
            "Sun Queen",
            "Shipwreck Beach",
            "Shantytown",
            "Mountain Village",
            "Chasm Monastery",
        ],
        LoaiTr::Shadow => &[
            "Lara",
            "Croft",
            "Lara Croft",
            "Jonah",
            "Dominguez",
            "Pedro Dominguez",
            "Unuratu",
            "Etzli",
            "Abby",
            "Uchu",
            "Trinity",
            "Paititi",
            "Kuwaq Yaku",
            "Cozumel",
            "Peru",
            "Mexico",
            "Yaaxil",
            "Cult of Kukulkan",
            "Kukulkan",
            "Ix Chel",
            "Mission",
            "Porvenir",
            "San Juan",
            "Silver Box",
            "Chak Chel",
            "Hidden City",
        ],
    }
}

// ---------------- thẻ / lọc chữ ----------------

fn re_the() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"<[^<>]*>|\{[^{}]*\}|\[[^\[\]]*\]|%[0-9]*[sdif]|\\n|\$[A-Za-z_][A-Za-z0-9_]*")
            .unwrap()
    })
}

fn re_viet() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)[ăâđêôơưạảấầẩẫậắằẳẵặẹẻẽếềểễệỉịọỏốồổỗộớờởỡợụủứừửữựỳỵỷỹ]").unwrap()
    })
}

/// Thẻ của chuỗi; chú thích âm thanh <...> được phép dịch bên trong → "<~>".
pub fn the_tr(s: &str) -> Vec<String> {
    static R2: OnceLock<Regex> = OnceLock::new();
    let r2 = R2.get_or_init(|| Regex::new(r"[a-z]{2}").unwrap());
    let mut v: Vec<String> = re_the()
        .find_iter(s)
        .map(|m| {
            let t = m.as_str();
            if t.starts_with('<') && r2.is_match(t) {
                "<~>".to_string()
            } else {
                t.to_string()
            }
        })
        .collect();
    v.sort();
    v
}

fn re_te_dong() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?m)^(lang_english\b[^"\r\n]*")(.*)("[ \t]*\r?)$"#).unwrap())
}

/// Tách chữ TextEntry theo mốc thời gian "\(2.46)" – giữ mốc ở vị trí lẻ (như re.split của Python).
fn tach_moc(s: &str) -> Vec<&str> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"\\\(\d+(?:\.\d+)?\)").unwrap());
    let mut ra = Vec::new();
    let mut dau = 0;
    for m in r.find_iter(s) {
        ra.push(&s[dau..m.start()]);
        ra.push(m.as_str());
        dau = m.end();
    }
    ra.push(&s[dau..]);
    ra
}

fn doan_te(en: &str) -> Vec<&str> {
    tach_moc(en)
        .into_iter()
        .step_by(2)
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .collect()
}

pub fn doc_meta(thu_muc: &Path) -> HashMap<i64, Nfo> {
    let mut meta = HashMap::new();
    for t in ten_tep(thu_muc) {
        if t.ends_with(".nfo") && !t.starts_with("bigfile._mod.") {
            if let Ok(m) = tiger::doc_nfo(&thu_muc.join(&t)) {
                meta.insert(m.lay("dlcindex").unwrap_or(-1), m);
            }
        }
    }
    meta
}

fn ten_tep(thu_muc: &Path) -> Vec<String> {
    std::fs::read_dir(thu_muc)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

pub fn thu_tu_ngon_ngu(ten: &str) -> usize {
    let goc = ten.replace(".000.tiger", "").to_uppercase();
    NGON_NGU
        .iter()
        .position(|ng| goc.ends_with(ng))
        .map_or(0, |i| i + 1)
}

fn dau_van(goi: &[PathBuf]) -> Vec<(String, u64, u64)> {
    goi.iter()
        .map(|p| {
            let m = std::fs::metadata(p).ok();
            let luc = m
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            (
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                m.map_or(0, |m| m.len()),
                luc,
            )
        })
        .collect()
}

type Uu = (i64, i64, i64, i64, usize);

fn uu_cua(m: &Nfo, ten: &str) -> Uu {
    (
        m.lay("version").unwrap_or(0),
        -m.lay("required").unwrap_or(1),
        m.lay("package").unwrap_or(0),
        m.lay("chunk").unwrap_or(0),
        thu_tu_ngon_ngu(ten),
    )
}

/// Rise: các gói chứa file tiếng Anh, xếp theo ưu tiên tăng dần. kem_chung: thêm gói chung.
fn goi_tieng_anh(thu_muc: &Path, kem_chung: bool) -> Vec<(PathBuf, Vec<Muc>)> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"_[A-Z]+\.000\.TIGER$").unwrap());
    let meta = doc_meta(thu_muc);
    let mut ds: Vec<(Uu, PathBuf, Vec<Muc>)> = Vec::new();
    for t in ten_tep(thu_muc) {
        if !t.ends_with(".000.tiger") || !t.starts_with("bigfile") || t.starts_with("bigfile._mod.")
        {
            continue;
        }
        let up = t.to_uppercase();
        if up.contains("KSA_") {
            continue;
        }
        if !up.contains("_ENGLISH") && (!kem_chung || thu_tu_ngon_ngu(&t) > 0 || r.is_match(&up)) {
            continue;
        }
        let p = thu_muc.join(&t);
        let Ok((dau, muc)) = tiger::doc_tiger(&p) else {
            continue;
        };
        let Some(m) = meta.get(&(dau.id as i64)) else {
            continue;
        };
        ds.push((uu_cua(m, &t), p, muc));
    }
    ds.sort_by_key(|x| x.0);
    ds.into_iter().map(|x| (x.1, x.2)).collect()
}

#[derive(Clone, Debug)]
pub struct ThongTinQuet {
    nguon: (PathBuf, Muc),
    id_max: i32,
    bap: i64,
    steam: Option<String>,
    phien_ban_max: i64,
}

fn bo_sung_meta(kq: &mut ThongTinQuet, meta: &HashMap<i64, Nfo>) {
    kq.bap = meta
        .values()
        .find_map(|m| m.lay("bap").filter(|b| *b != 0))
        .unwrap_or(0);
    kq.steam = meta
        .values()
        .flat_map(|m| m.custom.iter())
        .find(|c| c.starts_with("steamID:"))
        .cloned();
    kq.phien_ban_max = meta
        .values()
        .map(|m| m.lay("version").unwrap_or(0))
        .max()
        .unwrap_or(0);
}

/// Rise: tìm locals.bin tiếng Anh đang được game dùng (bỏ qua gói mod).
fn quet_rise(thu_muc: &Path) -> Result<ThongTinQuet> {
    let bam_locals = tiger::bam_cdc(TEN_LOCALS) as u64;
    let meta = doc_meta(thu_muc);
    let mut ung: Vec<(Uu, PathBuf, Muc)> = Vec::new();
    let mut id_max = 0;
    let ds_ten = ten_tep(thu_muc);
    for t in &ds_ten {
        if !t.ends_with(".000.tiger") || !t.starts_with("bigfile") || t.contains(TEN_MOD) {
            continue;
        }
        let p = thu_muc.join(t);
        let Ok((dau, ds)) = tiger::doc_tiger(&p) else {
            continue;
        };
        id_max = id_max.max(dau.id);
        if t.starts_with("bigfile._mod.") {
            continue;
        }
        let Some(m) = meta.get(&(dau.id as i64)) else {
            continue;
        };
        for muc in ds {
            if muc.bam == bam_locals && (muc.locale == 0xFFFF_FFFF || muc.locale & 1 != 0) {
                ung.push((uu_cua(m, t), p.clone(), muc));
            }
        }
    }
    for t in &ds_ten {
        if t.starts_with("bigfile._mod.") && t.ends_with(".nfo") && !t.contains(TEN_MOD) {
            if let Ok(n) = tiger::doc_nfo(&thu_muc.join(t)) {
                id_max = id_max.max(n.lay("dlcindex").unwrap_or(0) as i32);
            }
        }
    }
    ung.sort_by_key(|x| x.0.clone());
    let (_, p, muc) = ung.pop().ok_or_else(|| {
        anyhow!("không tìm thấy file chữ tiếng Anh (locals.bin) trong các gói .tiger của game")
    })?;
    let mut kq = ThongTinQuet {
        nguon: (p, muc),
        id_max,
        bap: 0,
        steam: None,
        phien_ban_max: 0,
    };
    bo_sung_meta(&mut kq, &meta);
    Ok(kq)
}

/// locale có phải chữ tiếng Anh (hoặc chung) không: 2 = riêng tiếng Anh, 1 = chung, 0 = không.
fn la_anh(loai: LoaiTr, loc: u64) -> u8 {
    if loai == LoaiTr::Shadow {
        if loc == u64::MAX {
            return 1;
        }
        let txt = loc & 0xFFF_FFFF;
        if txt == 0xFFF_FFFF {
            return 1;
        }
        return if txt & 1 != 0 { 2 } else { 0 };
    }
    if loc == 0xFFFF_FFFF {
        return 1;
    }
    if loc & 1 != 0 {
        2
    } else {
        0
    }
}

fn hop_ly(chuoi: &[(String, &str)]) -> bool {
    let v: Vec<&str> = chuoi
        .iter()
        .map(|x| x.1)
        .filter(|s| !s.is_empty())
        .collect();
    if v.len() < 200 {
        return false;
    }
    let anh = v
        .iter()
        .take(3000)
        .filter(|s| {
            s.chars().any(char::is_alphabetic) && s.chars().take(40).all(|c| (c as u32) < 0x250)
        })
        .count();
    anh as f64 >= 0.8 * v.len().min(3000) as f64
}

/// TR2013 / Shadow: tìm locals.bin tiếng Anh đang được game dùng.
fn quet_tr(thu_muc: &Path, loai: LoaiTr, bao: &dyn Fn(&str)) -> Result<ThongTinQuet> {
    let ten_locals = match loai {
        LoaiTr::Tr2013 => [
            "pc-w\\local\\locals.bin",
            "pc-w\\locals.bin",
            "local\\locals.bin",
        ],
        _ => [
            "pcx64-w\\local\\locals.bin",
            "pc-w\\local\\locals.bin",
            "local\\locals.bin",
        ],
    };
    let bam: Vec<u64> = ten_locals
        .iter()
        .map(|n| {
            if loai == LoaiTr::Shadow {
                tiger::bam64(n)
            } else {
                tiger::bam_cdc(n) as u64
            }
        })
        .collect();
    let meta = if loai == LoaiTr::Shadow {
        doc_meta(thu_muc)
    } else {
        HashMap::new()
    };
    let mut ung: Vec<(Vec<i64>, PathBuf, Muc)> = Vec::new();
    let mut du_phong: Vec<(Vec<i64>, PathBuf, Muc)> = Vec::new();
    let mut id_max = 0;
    let goc_patch2 = thu_muc.join(PATCH2_GOC).is_file();
    let mut ds_ten: Vec<String> = ten_tep(thu_muc)
        .into_iter()
        .filter(|t| t.to_lowercase().ends_with(".000.tiger") && !t.contains(TEN_MOD))
        .collect();
    ds_ten.sort();
    for t in ds_ten {
        if loai == LoaiTr::Tr2013 {
            if t == PATCH2 && goc_patch2 {
                continue; // patch2 hiện tại là bản app tạo → dùng bản gốc
            }
            if t.starts_with("bigfile._orig.") && t != PATCH2_GOC {
                continue;
            }
        } else if t.starts_with("bigfile._orig.") {
            continue;
        }
        let p = thu_muc.join(&t);
        let Ok((dau, ds)) = tiger::doc_tiger(&p) else {
            continue;
        };
        if t.starts_with("bigfile._mod.") {
            if loai == LoaiTr::Shadow {
                id_max = id_max.max(dau.id);
            }
            continue;
        }
        id_max = id_max.max(dau.id);
        let uu: Vec<i64> = if loai == LoaiTr::Tr2013 {
            if dau.id > ID_PATCH2 {
                continue;
            }
            vec![dau.id as i64]
        } else {
            let Some(m) = meta.get(&(dau.id as i64)) else {
                continue;
            };
            vec![
                m.lay("version").unwrap_or(0),
                -m.lay("required").unwrap_or(1),
                m.lay("package").unwrap_or(0),
                m.lay("chunk").unwrap_or(0),
                dau.sub as i64,
            ]
        };
        for muc in ds {
            let la = la_anh(loai, muc.locale);
            if la == 0 {
                continue;
            }
            let mut k = uu.clone();
            k.push(la as i64);
            if bam.contains(&muc.bam) {
                ung.push((k, p.clone(), muc));
            } else if la == 2 && 20000 < muc.co && muc.co < 40 << 20 && muc.id == dau.id {
                du_phong.push((k, p.clone(), muc));
            }
        }
    }
    if ung.is_empty() {
        // không biết tên file → dò từng file tiếng Anh xem file nào là bảng chữ
        bao("Đang tìm file chữ tiếng Anh (lần đầu hơi lâu)...");
        du_phong.sort_by(|a, b| b.0.cmp(&a.0));
        for (uu, p, muc) in du_phong.into_iter().take(4000) {
            let Ok(b) = tiger::doc_du_lieu(&p, &muc, false) else {
                continue;
            };
            let l = if loai == LoaiTr::Shadow {
                tiger::doc_locals_khoa(&b)
            } else {
                tiger::doc_locals_so(&b, 4)
            };
            if let Ok(l) = l {
                if hop_ly(&l.cac_chuoi()) {
                    ung.push((uu, p, muc));
                    break;
                }
            }
        }
    }
    ung.sort_by(|a, b| a.0.cmp(&b.0));
    let (_, p, muc) = ung.pop().ok_or_else(|| {
        anyhow!("không tìm thấy file chữ tiếng Anh (locals.bin) trong các gói .tiger của game")
    })?;
    let mut kq = ThongTinQuet {
        nguon: (p, muc),
        id_max,
        bap: 0,
        steam: None,
        phien_ban_max: 0,
    };
    if !meta.is_empty() {
        bo_sung_meta(&mut kq, &meta);
    }
    Ok(kq)
}

fn duoi_shadow(loc: u64) -> String {
    if loc == u64::MAX {
        return String::new();
    }
    let ten = |co: u64, tat_ca: &str| -> String {
        if co == 0xFFF_FFFF || (co & 0xFFF_0000) != 0xFFF_0000 {
            return tat_ca.into();
        }
        let v = co & 0xFFFF;
        for (i, n) in NGON_NGU_SHADOW.iter().enumerate() {
            if let Some(n) = n {
                if v == (0x400 | (1 << i)) {
                    return n.to_string();
                }
            }
        }
        tat_ca.into()
    };
    let txt = ten(loc & 0xFFF_FFFF, "alltxt");
    let vo = ten((loc >> 28) & 0xFFF_FFFF, "allvo");
    let pl = loc >> 56;
    let plt = if pl == 0xFF {
        "allplt".to_string()
    } else if pl == 0x20 {
        "neutral".into()
    } else {
        format!("{pl:02X}")
    };
    format!("_{txt}_{vo}_{plt}")
}

// ---------------- mục lục phụ đề (.mul) / lời thoại lúc chơi (TextEntry) ----------------

#[derive(Serialize, Deserialize, Clone)]
pub struct MucMul {
    goi: String,
    muc: Muc,
    k: usize,
    phim: bool,
    /// (khung, chữ tiếng Anh)
    pd: Vec<(i32, String)>,
    #[serde(default)]
    ten: String,
    #[serde(default)]
    mau: u64,
    /// (loại, tên FSB, số mẫu)
    #[serde(default)]
    kenh: Vec<(u8, String, u64)>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MucTe {
    goi: String,
    muc: Muc,
    en: String,
}

#[derive(Serialize, Deserialize)]
struct MucLuc<T> {
    ban: i64,
    #[serde(default)]
    game: String,
    goi: Vec<(String, u64, u64)>,
    #[serde(alias = "te")]
    mul: BTreeMap<String, T>,
}

fn doc_muc_luc<T: for<'a> Deserialize<'a>>(
    p: &Path,
    ban: i64,
    game: Option<&Path>,
    dv: &[(String, u64, u64)],
) -> Option<BTreeMap<String, T>> {
    let cu: MucLuc<T> = serde_json::from_slice(&std::fs::read(p).ok()?).ok()?;
    let cung_game =
        game.is_none_or(|g| cu.game.to_lowercase() == g.to_string_lossy().to_lowercase());
    (cu.ban == ban && cu.goi == dv && cung_game).then_some(cu.mul)
}

fn ghi_muc_luc<T: Serialize>(
    p: &Path,
    ban: i64,
    game: &Path,
    dv: Vec<(String, u64, u64)>,
    kq: &BTreeMap<String, T>,
    khoa: &str,
) -> Result<()> {
    let mut v = serde_json::json!({"ban": ban, "game": game.to_string_lossy(), "goi": dv});
    v[khoa] = serde_json::to_value(kq)?;
    if let Some(cha) = p.parent() {
        std::fs::create_dir_all(cha)?;
    }
    duong_dan::ghi_file(p, v.to_string().as_bytes())?;
    Ok(())
}

/// Kênh giọng nhân vật (không phải nhạc / tiếng động) – k = (loại, tên FSB, số mẫu).
fn la_kenh_giong(k: &(u8, String, u64), so_kenh: usize) -> bool {
    if so_kenh == 1 {
        return true;
    }
    let ten = k.1.to_lowercase();
    if ["music", "_mus", "musi", "sfx", "_amb", "ambien"]
        .iter()
        .any(|x| ten.contains(x))
    {
        return false;
    }
    k.0 <= 1 && (ten.contains('#') || ten.contains("_vo_") || ten.contains("_vo."))
}

/// Kênh phải đi cặp với kênh trái c (cùng tên).
fn kenh_phai(t: &MucMul, c: usize) -> Option<usize> {
    let k = &t.kenh;
    (c + 1 < k.len() && k[c].0 == 1 && k[c + 1].0 == 2 && k[c + 1].1 == k[c].1).then_some(c + 1)
}

fn doc_mul_tu(p: &Path, muc: &Muc) -> Result<Option<Mul>> {
    if muc.co < 0x2010 {
        return Ok(None);
    }
    let mut f = std::fs::File::open(tiger::duong_dan_phan(p, muc.phan))?;
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(muc.vt as u64))?;
    let mut dau = vec![0u8; mul::CO_HDR];
    let n = f.read(&mut dau)?;
    dau.truncate(n);
    if dau.len() >= 4 && u32::from_le_bytes(dau[..4].try_into().unwrap()) == MAGIC_CDRM {
        dau = tiger::doc_du_lieu(p, muc, false)?;
    }
    if !Mul::la_mul_phu_de(&dau) {
        return Ok(None);
    }
    Ok(Some(Mul::doc(&tiger::doc_du_lieu(p, muc, false)?)?))
}

fn muc_tu_mul(x: &Mul, goi: &str, muc: &Muc, giong: bool) -> Option<MucMul> {
    let pd: Vec<(i32, String)> = x
        .cac_phu_de()
        .into_iter()
        .map(|(i, khung)| {
            (
                khung,
                x.phu_de(i)
                    .and_then(|d| mul::lay(d, "0"))
                    .unwrap_or("")
                    .to_string(),
            )
        })
        .collect();
    if !pd.iter().any(|(_, en)| !en.is_empty()) {
        return None;
    }
    let mut tt = MucMul {
        goi: goi.into(),
        muc: muc.clone(),
        k: x.so_kenh(),
        phim: x.co_phim(),
        pd,
        ten: String::new(),
        mau: 0,
        kenh: Vec::new(),
    };
    let loai = x.loai_kenh();
    let mut kenh = Vec::new();
    let mut ok = true;
    for (c, f) in x.fsb_kenh().iter().enumerate() {
        match mul::fsb_thong_tin(f) {
            Ok((ten, so)) => kenh.push((*loai.get(c).unwrap_or(&0), ten, so as u64)),
            Err(_) => {
                ok = false;
                break;
            }
        }
    }
    if ok && !kenh.is_empty() {
        tt.ten = kenh[0].1.clone();
        tt.mau = kenh[0].2;
        if giong {
            let g: Vec<&(u8, String, u64)> = kenh
                .iter()
                .filter(|k| la_kenh_giong(k, x.so_kenh()))
                .collect();
            if let Some(f) = g.first() {
                tt.ten = f.1.clone();
                tt.mau = g.iter().map(|k| k.2).max().unwrap_or(0);
            }
        }
        tt.kenh = kenh;
    }
    Some(tt)
}

// ---------------- việc Việt hóa ----------------

pub struct TombRaider {
    pub loai: LoaiTr,
    pub thu_muc_game: PathBuf,
    tt: ThongTinQuet,
    locals: Locals,
    mul: BTreeMap<String, MucMul>,
    te: BTreeMap<String, MucTe>,
    pub viec: ViecDich,
    /// game đã có bản Việt hóa chữ của nhóm khác
    pub co_viet_hoa_san: bool,
    pub loi_phu_de: Option<String>,
    font_viet: bool,
    pub ket_qua_cai: String,
}

/// Thư mục giọng lồng tiếng của app cũ: <ổ>:\DichGame_LongTieng (ổ game hoặc ổ người dùng).
pub fn thu_muc_lam_viec(thu_muc_game: &Path) -> PathBuf {
    let o = |p: &Path| -> Option<PathBuf> {
        let s = p.to_string_lossy();
        s.get(..2)
            .filter(|x| x.ends_with(':'))
            .map(|x| PathBuf::from(format!("{x}\\")))
    };
    let mut ung: Vec<PathBuf> = Vec::new();
    for p in [
        o(thu_muc_game),
        std::env::var("USERPROFILE")
            .ok()
            .and_then(|u| o(Path::new(&u))),
    ]
    .into_iter()
    .flatten()
    {
        if !ung.contains(&p) {
            ung.push(p);
        }
    }
    // ưu tiên ổ đã có thư mục giọng; không có thì ổ game
    ung.iter()
        .map(|g| g.join("DichGame_LongTieng"))
        .find(|p| p.is_dir())
        .unwrap_or_else(|| {
            ung.first()
                .cloned()
                .unwrap_or_default()
                .join("DichGame_LongTieng")
        })
}

impl TombRaider {
    pub fn thu_muc_du_lieu(loai: LoaiTr) -> PathBuf {
        duong_dan::viet_hoa_game().join(loai.thu_muc())
    }

    pub fn nap(
        thu_muc_game: &Path,
        loai: LoaiTr,
        bao: &dyn Fn(&str),
        dung: &AtomicBool,
    ) -> Result<Self> {
        bao("Đang tìm file chữ tiếng Anh của game (locals.bin)...");
        let tt = if loai == LoaiTr::Rise {
            quet_rise(thu_muc_game)?
        } else {
            quet_tr(thu_muc_game, loai, bao)?
        };
        let (p, muc) = &tt.nguon;
        bao(&format!(
            "Đang đọc chữ từ {}...",
            p.file_name().unwrap_or_default().to_string_lossy()
        ));
        let b = tiger::doc_du_lieu(p, muc, false)?;
        let locals = match loai {
            LoaiTr::Rise => tiger::doc_locals_so(&b, 8)?,
            LoaiTr::Tr2013 => tiger::doc_locals_so(&b, 4)?,
            LoaiTr::Shadow => tiger::doc_locals_khoa(&b)?,
        };
        let v: Vec<(String, &str)> = locals.cac_chuoi();
        let co_viet = v
            .iter()
            .filter(|x| !x.1.is_empty() && re_viet().is_match(x.1))
            .count();
        let khong_rong = v.iter().filter(|x| !x.1.is_empty()).count();
        let co_viet_hoa_san =
            loai != LoaiTr::Rise && khong_rong > 0 && co_viet as f64 > 0.3 * khong_rong as f64;
        let thu_muc = Self::thu_muc_du_lieu(loai);
        std::fs::create_dir_all(&thu_muc)?;
        let mut viec = ViecDich::moi(
            thu_muc,
            &huong_dan(loai),
            BangTen::tao(ten_rieng(loai).iter().copied()),
        );
        viec.the = the_tr;
        let mut x = TombRaider {
            loai,
            thu_muc_game: thu_muc_game.to_path_buf(),
            tt,
            locals,
            mul: BTreeMap::new(),
            te: BTreeMap::new(),
            viec,
            co_viet_hoa_san,
            loi_phu_de: None,
            font_viet: false,
            ket_qua_cai: String::new(),
        };
        if loai != LoaiTr::Shadow {
            if let Err(e) = x.quet_mul(bao, dung) {
                if dung.load(Ordering::SeqCst) {
                    return Err(e);
                }
                x.loi_phu_de = Some(e.to_string());
            }
        }
        if loai == LoaiTr::Rise {
            if let Err(e) = x.quet_loi_thoai(bao, dung) {
                if dung.load(Ordering::SeqCst) {
                    return Err(e);
                }
                x.loi_phu_de = Some(e.to_string());
            }
        }
        Ok(x)
    }

    fn file_du_lieu(&self, ten: &str) -> PathBuf {
        Self::thu_muc_du_lieu(self.loai).join(ten)
    }

    fn quet_mul(&mut self, bao: &dyn Fn(&str), dung: &AtomicBool) -> Result<()> {
        let tm = self.thu_muc_game.clone();
        let goi: Vec<(PathBuf, Vec<Muc>)> = if self.loai == LoaiTr::Rise {
            goi_tieng_anh(&tm, false)
        } else {
            let mut g: Vec<(i32, PathBuf, Vec<Muc>)> = Vec::new();
            let mut ds = ten_tep(&tm);
            ds.sort();
            for t in ds {
                let tl = t.to_lowercase();
                if !tl.ends_with("_english.000.tiger") || tl.starts_with("bigfile._") {
                    continue;
                }
                let mut p = tm.join(&t);
                if t == GIONG_TR13 && tm.join(GIONG_TR13_GOC).is_file() {
                    p = tm.join(GIONG_TR13_GOC); // gói hiện tại là gói giọng của app → đọc bản gốc
                }
                let Ok((dau, muc)) = tiger::doc_tiger(&p) else {
                    continue;
                };
                g.push((dau.id, p, muc));
            }
            g.sort_by_key(|x| x.0);
            g.into_iter().map(|x| (x.1, x.2)).collect()
        };
        let dv = dau_van(&goi.iter().map(|x| x.0.clone()).collect::<Vec<_>>());
        let file = self.file_du_lieu("muc_luc_mul.json");
        let (ban, game) = if self.loai == LoaiTr::Rise {
            (PHIEN_BAN_MUL, Some(tm.as_path()))
        } else {
            (1, None)
        };
        if let Some(m) = doc_muc_luc(&file, ban, game, &dv) {
            self.mul = m;
            return Ok(());
        }
        let mut chon: BTreeMap<u64, (usize, Muc)> = BTreeMap::new();
        let mut thu_tu: Vec<u64> = Vec::new();
        for (gi, (_, ds)) in goi.iter().enumerate() {
            for muc in ds {
                let ok = if self.loai == LoaiTr::Rise {
                    muc.locale & 1 != 0
                } else {
                    la_anh(LoaiTr::Tr2013, muc.locale) > 0 && muc.co >= 0x2010
                };
                if ok {
                    if chon.insert(muc.bam, (gi, muc.clone())).is_none() {
                        thu_tu.push(muc.bam);
                    }
                }
            }
        }
        let mut kq = BTreeMap::new();
        for (n, bam) in thu_tu.iter().enumerate() {
            if n % 300 == 0 {
                bao(&format!(
                    "Đang quét phụ đề phim và lời thoại: {n}/{} file",
                    chon.len()
                ));
                if dung.load(Ordering::SeqCst) {
                    bail!("đã dừng");
                }
            }
            let (gi, muc) = &chon[bam];
            let p = &goi[*gi].0;
            let Ok(Some(x)) = doc_mul_tu(p, muc) else {
                continue;
            };
            let ten = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if let Some(tt) = muc_tu_mul(&x, &ten, muc, self.loai == LoaiTr::Rise) {
                kq.insert(format!("{bam:08x}"), tt);
            }
        }
        ghi_muc_luc(&file, ban, &tm, dv, &kq, "mul")?;
        self.mul = kq;
        Ok(())
    }

    /// Rise: lời thoại lúc chơi nằm trong file TextEntry ("Version 1 / TextEntry") của gói chung.
    fn quet_loi_thoai(&mut self, bao: &dyn Fn(&str), dung: &AtomicBool) -> Result<()> {
        let tm = self.thu_muc_game.clone();
        let goi = goi_tieng_anh(&tm, true);
        let dv = dau_van(&goi.iter().map(|x| x.0.clone()).collect::<Vec<_>>());
        let file = self.file_du_lieu("muc_luc_loi_thoai.json");
        if let Some(m) = doc_muc_luc(&file, PHIEN_BAN_TE, Some(&tm), &dv) {
            self.te = m;
            return Ok(());
        }
        let mut chon: HashMap<u64, (usize, Muc)> = HashMap::new();
        for (gi, (_, ds)) in goi.iter().enumerate() {
            for muc in ds {
                if (muc.locale == 0xFFFF_FFFF || muc.locale & 1 != 0)
                    && (40..=0x40000).contains(&muc.co)
                {
                    chon.insert(muc.bam, (gi, muc.clone()));
                }
            }
        }
        let mut ds: Vec<(u64, usize, Muc)> =
            chon.into_iter().map(|(b, (g, m))| (b, g, m)).collect();
        ds.sort_by(|a, b| (&goi[a.1].0, a.2.phan, a.2.vt).cmp(&(&goi[b.1].0, b.2.phan, b.2.vt)));
        let tong = ds.len();
        let mut kq = BTreeMap::new();
        let mut mo: HashMap<(usize, i32), std::fs::File> = HashMap::new();
        use std::io::{Read, Seek, SeekFrom};
        for (n, (bam, gi, muc)) in ds.into_iter().enumerate() {
            if n % 2000 == 0 {
                bao(&format!("Đang quét lời thoại lúc chơi: {n}/{tong} file"));
                if dung.load(Ordering::SeqCst) {
                    bail!("đã dừng");
                }
            }
            let p = &goi[gi].0;
            let f = match mo.entry((gi, muc.phan)) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(e) => {
                    match std::fs::File::open(tiger::duong_dan_phan(p, muc.phan)) {
                        Ok(f) => e.insert(f),
                        Err(_) => continue,
                    }
                }
            };
            let mut dau = vec![0u8; DAU_TE.len()];
            if f.seek(SeekFrom::Start(muc.vt as u64)).is_err() || f.read_exact(&mut dau).is_err() {
                continue;
            }
            let b = if dau != DAU_TE {
                if dau[..4] != MAGIC_CDRM.to_le_bytes() {
                    continue;
                }
                match tiger::doc_du_lieu(p, &muc, false) {
                    Ok(b) if b.starts_with(DAU_TE) => b,
                    _ => continue,
                }
            } else {
                let mut b = vec![0u8; muc.co as usize];
                if f.seek(SeekFrom::Start(muc.vt as u64)).is_err() || f.read_exact(&mut b).is_err()
                {
                    continue;
                }
                b
            };
            let s = String::from_utf8_lossy(&b);
            if let Some(m) = re_te_dong().captures(&s) {
                let en = m[2].to_string();
                if !en.trim().is_empty() {
                    let ten = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    kq.insert(format!("{bam:08x}"), MucTe { goi: ten, muc, en });
                }
            }
        }
        ghi_muc_luc(&file, PHIEN_BAN_TE, &tm, dv, &kq, "te")?;
        self.te = kq;
        Ok(())
    }

    fn nen_dich(&self, v: &str) -> bool {
        static MA: OnceLock<Regex> = OnceLock::new();
        static CHU: OnceLock<Regex> = OnceLock::new();
        if self.loai != LoaiTr::Rise && re_viet().is_match(v) {
            return false; // đã là tiếng Việt (bản Việt hóa có sẵn)
        }
        let t = re_the().replace_all(v, "");
        let t = t.trim();
        if t.is_empty()
            || !CHU
                .get_or_init(|| Regex::new(r"[A-Za-z]{2,}").unwrap())
                .is_match(t)
        {
            return false;
        }
        if MA
            .get_or_init(|| Regex::new(r"^[A-Za-z0-9_./\\-]+$").unwrap())
            .is_match(t)
            && (t.contains('_') || t.contains('/') || t.contains('\\'))
        {
            return false; // mã nội bộ (IDS_..., đường dẫn)
        }
        !self.viec.bang_ten.la_ten(v)
    }

    fn con_chu_anh(&self, v: &str) -> bool {
        static R: OnceLock<Regex> = OnceLock::new();
        if !self.nen_dich(v) {
            return false;
        }
        let t = re_the().replace_all(v, " ");
        R.get_or_init(|| Regex::new(r"[A-Za-z']{2,}").unwrap())
            .find_iter(&t)
            .any(|w| w.as_str().chars().any(|c| c.is_ascii_lowercase()))
    }

    /// Mọi chuỗi cần Việt hóa: chữ game + phụ đề phim + lời thoại lúc chơi.
    fn cac_chuoi(&self) -> Vec<&str> {
        let mut ds: Vec<&str> = self
            .locals
            .cac_chuoi()
            .into_iter()
            .map(|x| x.1)
            .filter(|v| self.nen_dich(v))
            .collect();
        for t in self.mul.values() {
            ds.extend(
                t.pd.iter()
                    .map(|x| x.1.as_str())
                    .filter(|v| !v.is_empty() && self.nen_dich(v)),
            );
        }
        for t in self.te.values() {
            ds.extend(doan_te(&t.en).into_iter().filter(|v| self.nen_dich(v)));
        }
        ds
    }

    pub fn cau_can_dich(&self) -> Vec<String> {
        let bn = self.viec.bo_nho.lock().unwrap_or_else(|e| e.into_inner());
        let mut da = std::collections::HashSet::new();
        let mut ra = Vec::new();
        for v in self.cac_chuoi() {
            if da.contains(v) {
                continue;
            }
            match bn.get(v) {
                Some(vi) if vi != v && the_tr(vi) == the_tr(v) => continue,
                Some(vi) if vi == v && !self.con_chu_anh(v) => continue,
                _ => {}
            }
            da.insert(v);
            ra.push(v.to_string());
        }
        ra
    }

    /// (dòng chữ game, tổng câu, đã dịch, còn, ký tự còn)
    pub fn thong_ke(&self) -> (usize, usize, usize, usize, usize) {
        let tat_ca: std::collections::HashSet<&str> = self.cac_chuoi().into_iter().collect();
        let con = self.cau_can_dich();
        let ky = con.iter().map(|v| v.chars().count()).sum();
        (
            self.locals.cac_chuoi().len(),
            tat_ca.len(),
            tat_ca.len() - con.len(),
            con.len(),
            ky,
        )
    }

    /// (câu phụ đề / lời thoại, đã dịch, đoạn đã lồng tiếng)
    pub fn thong_ke_phu_de(&self) -> (usize, usize, usize) {
        let mut cau: Vec<&str> = self
            .mul
            .values()
            .flat_map(|t| t.pd.iter().map(|x| x.1.as_str()))
            .filter(|e| !e.is_empty())
            .collect();
        for t in self.te.values() {
            cau.extend(doan_te(&t.en));
        }
        let da = cau
            .iter()
            .filter(|e| self.chuoi_viet(e, false, false).is_some())
            .count();
        let long = self
            .mul
            .keys()
            .filter(|m| !self.file_giong(m).is_empty())
            .count();
        (cau.len(), da, long)
    }

    pub fn chuoi_viet(&self, en: &str, khong_dau: bool, goc: bool) -> Option<String> {
        if self.loai != LoaiTr::Rise && re_viet().is_match(en) {
            let vi: String = en.nfc().collect();
            return Some(if goc || !khong_dau {
                vi
            } else {
                tiger::bo_dau(&vi)
            });
        }
        let vi = self.viec.tim(en)?;
        if vi.is_empty() || vi == en || the_tr(&vi) != the_tr(en) {
            return None;
        }
        if goc || (!khong_dau && self.font_viet) {
            return Some(vi.nfc().collect());
        }
        Some(if khong_dau {
            tiger::bo_dau(&vi)
        } else {
            tiger::dau_ghep(&vi)
        })
    }

    fn te_viet(&self, en: &str, khong_dau: bool) -> Option<String> {
        let mut ra = String::new();
        let mut doi = false;
        for (i, d) in tach_moc(en).into_iter().enumerate() {
            if i % 2 == 0 && !d.trim().is_empty() {
                if let Some(vi) = self.chuoi_viet(d.trim(), khong_dau, false) {
                    let dau = &d[..d.len() - d.trim_start().len()];
                    let duoi = &d[d.trim_end().len()..];
                    ra.push_str(dau);
                    ra.push_str(&vi);
                    ra.push_str(duoi);
                    doi = true;
                    continue;
                }
            }
            ra.push_str(d);
        }
        doi.then_some(ra)
    }

    pub fn da_cai(&self) -> bool {
        let tm = &self.thu_muc_game;
        match self.loai {
            LoaiTr::Rise => tm.join(TEP_TIGER).is_file(),
            LoaiTr::Tr2013 => tm.join(MARKER_TR13).is_file(),
            LoaiTr::Shadow => tep_mod_shadow(tm).iter().any(|t| t.ends_with(".tiger")),
        }
    }

    // ---------------- giọng đã lồng (app cũ) ----------------

    fn thu_muc_giong(&self) -> PathBuf {
        if let Some(d) = std::env::var_os("VIET_HOA_GIONG") {
            return PathBuf::from(d); // kiểm thử: không đụng giọng đã lồng của app cũ
        }
        thu_muc_lam_viec(&self.thu_muc_game).join(if self.loai == LoaiTr::Rise {
            "RotTR"
        } else {
            "TR2013"
        })
    }

    /// {kênh: mp3} các kênh đã lồng tiếng của 1 file .mul.
    fn file_giong(&self, ma: &str) -> BTreeMap<usize, PathBuf> {
        let Some(t) = self.mul.get(ma) else {
            return BTreeMap::new();
        };
        let thu = self.thu_muc_giong().join("mp3");
        (0..t.k)
            .map(|c| {
                (
                    c,
                    thu.join(if t.k == 1 {
                        format!("{ma}.mp3")
                    } else {
                        format!("{ma}_{c}.mp3")
                    }),
                )
            })
            .filter(|(_, p)| p.is_file())
            .collect()
    }

    fn doc_mul(&self, ma: &str) -> Result<Mul> {
        let t = &self.mul[ma];
        Mul::doc(&tiger::doc_du_lieu(
            &self.thu_muc_game.join(&t.goi),
            &t.muc,
            false,
        )?)
    }

    /// File .mul mới: phụ đề Việt (+ giọng Việt). Lỗi phần giọng → vẫn giữ phụ đề.
    fn mul_moi(
        &self,
        ma: &str,
        khong_dau: bool,
        dem: &std::cell::RefCell<[usize; 3]>,
    ) -> Result<Vec<u8>> {
        let phu_de = |x: &mut Mul| -> usize {
            let mut n = 0;
            for (i, _) in x.cac_phu_de() {
                let Some(d) = x.phu_de_mut(i) else { continue };
                let Some(en) = mul::lay(d, "0").map(String::from) else {
                    continue;
                };
                if let Some(vi) = self.chuoi_viet(&en, khong_dau, false) {
                    for (k, v) in d.iter_mut() {
                        if k == "0" || (k == "14" && *v == en) {
                            *v = vi.clone();
                        }
                    }
                    n += 1;
                }
            }
            n
        };
        let giong = self.file_giong(ma);
        if !giong.is_empty() {
            let thu = || -> Result<Vec<u8>> {
                let mut x = self.doc_mul(ma)?;
                let n = phu_de(&mut x);
                let mut fsb = x.fsb_kenh();
                for (&c, p) in &giong {
                    let mp3 = std::fs::read(p)?;
                    for cc in [Some(c), kenh_phai(&self.mul[ma], c)].into_iter().flatten() {
                        if cc < fsb.len() {
                            let ten = mul::fsb_thong_tin(&fsb[cc])?.0;
                            fsb[cc] = mul::tao_fsb4(&mp3, &ten, Some(&fsb[cc]))?;
                        }
                    }
                }
                x.thay_am_thanh(&fsb)?;
                let mut d = dem.borrow_mut();
                d[0] += n;
                d[1] += 1;
                Ok(x.ghi())
            };
            match thu() {
                Ok(b) => return Ok(b),
                Err(e) => self.ghi_loi(ma, "giọng", &e),
            }
        }
        match self.doc_mul(ma).map(|mut x| (phu_de(&mut x), x)) {
            Ok((n, x)) => {
                dem.borrow_mut()[0] += n;
                Ok(x.ghi())
            }
            Err(e) => {
                self.ghi_loi(ma, "phụ đề", &e);
                dem.borrow_mut()[2] += 1;
                Ok(self.doc_mul(ma)?.ghi())
            }
        }
    }

    fn te_moi(
        &self,
        ma: &str,
        khong_dau: bool,
        _dem: &std::cell::RefCell<[usize; 3]>,
    ) -> Result<Vec<u8>> {
        let t = &self.te[ma];
        let b = tiger::doc_du_lieu(&self.thu_muc_game.join(&t.goi), &t.muc, false)?;
        let s = String::from_utf8_lossy(&b).into_owned();
        if let Some(m) = re_te_dong().captures(&s) {
            let g = m.get(2).unwrap();
            if let Some(vi) = self.te_viet(g.as_str(), khong_dau) {
                return Ok(format!(
                    "{}{}{}",
                    &s[..g.start()],
                    vi.replace('"', "'"),
                    &s[g.end()..]
                )
                .into_bytes());
            }
        }
        Ok(b)
    }

    fn ghi_loi(&self, ma: &str, phan: &str, e: &anyhow::Error) {
        let p = self.file_du_lieu("loi_cai.txt");
        let ten = self.mul.get(ma).map(|t| t.ten.as_str()).unwrap_or("");
        let dong = format!(
            "--- {} {ma} {ten} ({phan})\n{e:#}\n",
            duong_dan::gio_dia_phuong()
        );
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
        {
            let _ = f.write_all(dong.as_bytes());
        }
    }

    // ---------------- cài / gỡ ----------------

    /// Ghi bản Việt hóa vào thư mục game. Trả số dòng chữ đã đổi; chi tiết ở `ket_qua_cai`.
    pub fn cai(&mut self, khong_dau: bool, bao: &dyn Fn(&str), dung: &AtomicBool) -> Result<usize> {
        let _ = std::fs::remove_file(self.file_du_lieu("loi_cai.txt"));
        match self.loai {
            LoaiTr::Rise => self.cai_rise(khong_dau, bao, dung),
            LoaiTr::Tr2013 => self.cai_tr2013(khong_dau, bao),
            LoaiTr::Shadow => self.cai_shadow(khong_dau),
        }
    }

    fn doi_locals(&self, khong_dau: bool) -> (Vec<u8>, usize) {
        let dem = std::cell::Cell::new(0);
        let b = self.locals.ghi(&|en| {
            let vi = self.chuoi_viet(en, khong_dau, false);
            if vi.is_some() {
                dem.set(dem.get() + 1);
            }
            vi
        });
        (b, dem.get())
    }

    fn cai_rise(
        &mut self,
        khong_dau: bool,
        bao: &dyn Fn(&str),
        dung: &AtomicBool,
    ) -> Result<usize> {
        // font tiếng Việt (chỉ khi cài có dấu); lỗi → dùng cách ghép dấu cũ
        let mut loi_font = None;
        let font = if khong_dau {
            None
        } else {
            match super::font_rottr::chuan_bi(&self.thu_muc_game, TTF_DAU, bao, dung) {
                Ok(f) => Some(f),
                Err(e) => {
                    loi_font = Some(e.to_string());
                    None
                }
            }
        };
        self.font_viet = font.is_some();
        let (du_lieu, doi) = self.doi_locals(khong_dau);
        let ma = self.tt.id_max + 1;
        let dem = std::cell::RefCell::new([0usize; 3]); // phụ đề, giọng, lỗi
        let so_te = std::cell::Cell::new(0usize);
        let this = &*self;
        let mut cac: Vec<tiger::TepGhi> = vec![(
            tiger::bam_cdc(TEN_LOCALS) as u64,
            LOCALE_EN,
            Box::new(move || Ok(du_lieu)),
        )];
        for (m, t) in &this.mul {
            let co_giong = !this.file_giong(m).is_empty();
            if co_giong
                || t.pd.iter().any(|(_, en)| {
                    !en.is_empty() && this.chuoi_viet(en, khong_dau, false).is_some()
                })
            {
                let dem = &dem;
                cac.push((
                    u64::from_str_radix(m, 16)?,
                    t.muc.locale,
                    Box::new(move || this.mul_moi(m, khong_dau, dem)),
                ));
            }
        }
        for (m, t) in &this.te {
            if this.te_viet(&t.en, khong_dau).is_some() {
                let (dem, so_te) = (&dem, &so_te);
                cac.push((
                    u64::from_str_radix(m, 16)?,
                    t.muc.locale,
                    Box::new(move || {
                        so_te.set(so_te.get() + 1);
                        this.te_moi(m, khong_dau, dem)
                    }),
                ));
            }
        }
        if doi == 0 && cac.len() == 1 {
            bail!("chưa có câu nào được dịch – bấm Dịch trước");
        }
        let tm = &this.thu_muc_game;
        let tam_chung = tm.join(format!("{TEP_TIGER_GOC}.moi"));
        if let Some(f) = &font {
            let chung: Vec<(u64, u64, Box<dyn Fn(&[u32]) -> Vec<u8> + '_>)> = f
                .bst
                .iter()
                .filter(|b| b.locale != LOCALE_EN)
                .map(|b| {
                    (
                        b.bam,
                        b.locale,
                        Box::new(move |cv: &[u32]| f.tao_bst(b, ma, cv))
                            as Box<dyn Fn(&[u32]) -> Vec<u8>>,
                    )
                })
                .collect();
            let cac_vt = super::font_rottr::tao_goi_chung(&tam_chung, ma, &f.tai_nguyen, chung)?;
            for b in f.bst.iter().filter(|b| b.locale == LOCALE_EN) {
                let cv = cac_vt.clone();
                cac.push((b.bam, b.locale, Box::new(move || Ok(f.tao_bst(b, ma, &cv)))));
            }
        }
        let tam = tm.join(format!("{TEP_TIGER}.moi"));
        tiger::tao_tiger_v4(&tam, ma, cac, bao)?;
        go_rise(tm)?;
        std::fs::rename(&tam, tm.join(TEP_TIGER))?;
        if font.is_some() {
            std::fs::rename(&tam_chung, tm.join(TEP_TIGER_GOC))?;
        } else {
            tiger::tao_tiger_v4(&tm.join(TEP_TIGER_GOC), ma, Vec::new(), &|_| {})?;
        }
        tiger::ghi_nfo(
            &tm.join(TEP_NFO),
            this.tt.bap,
            this.tt.phien_ban_max + 1,
            ma as i64,
            this.tt.steam.as_deref(),
        )?;
        let [pd, giong, loi] = *dem.borrow();
        let kieu = if khong_dau {
            "không dấu"
        } else {
            "có dấu"
        };
        let mut s = format!("Đã cài Việt hóa ({kieu}): {doi} dòng chữ – {pd} câu phụ đề phim – {} file lời thoại lúc chơi – {giong} đoạn lồng tiếng.", so_te.get());
        if !khong_dau {
            match (&font, loi_font) {
                (Some(_), _) => {
                    s.push_str("\nĐã thêm chữ tiếng Việt vào font của game (có dấu đầy đủ).")
                }
                (None, Some(e)) => s.push_str(&format!(
                    "\nChưa thêm được font tiếng Việt: {e} – chữ thiếu dấu được ghép tạm."
                )),
                _ => {}
            }
        }
        if loi > 0 {
            s.push_str(&format!(
                "\n{loi} file phim / thoại lỗi khi ghép (xem rottr\\loi_cai.txt)."
            ));
        }
        self.ket_qua_cai = s;
        Ok(doi)
    }

    fn cai_tr2013(&mut self, khong_dau: bool, bao: &dyn Fn(&str)) -> Result<usize> {
        let (du_lieu, doi) = self.doi_locals(khong_dau);
        if doi == 0 {
            bail!("chưa có câu nào được dịch – bấm Dịch trước");
        }
        let tm = self.thu_muc_game.clone();
        let goc = tm.join(PATCH2_GOC);
        let hien = tm.join(PATCH2);
        if !goc.is_file() && hien.is_file() {
            std::fs::copy(&hien, &goc)?; // giữ bản gốc (1 lần)
        }
        let muc = &self.tt.nguon.1;
        let khoa_moi = (muc.bam, muc.locale);
        let mut hang: Vec<tiger::HangV3> = Vec::new();
        if goc.is_file() {
            let (_, ds) = tiger::doc_tiger(&goc)?;
            for m in ds {
                if (m.bam, m.locale) == khoa_moi {
                    continue;
                }
                if m.id != ID_PATCH2 {
                    let g = m.vt | ((m.id as u32) << 4) | m.phan as u32;
                    hang.push((m.bam, m.locale, m.co, Some(g), None));
                } else {
                    let goc = goc.clone();
                    hang.push((
                        m.bam,
                        m.locale,
                        m.co,
                        None,
                        Some(Box::new(move || tiger::doc_du_lieu(&goc, &m, true))),
                    ));
                }
            }
        }
        let n = du_lieu.len() as u32;
        hang.push((
            khoa_moi.0,
            khoa_moi.1,
            n,
            None,
            Some(Box::new(move || Ok(du_lieu))),
        ));
        tiger::tao_tiger_v3(&hien, ID_PATCH2, hang, bao)?;
        std::fs::write(tm.join(MARKER_TR13), format!("Viet Hoa: patch2.000.tiger da duoc thay bang ban Viet hoa. Ban goc: {PATCH2_GOC}\n"))?;
        self.ket_qua_cai = format!(
            "Đã cài Việt hóa ({}): {doi} dòng chữ.",
            if khong_dau {
                "không dấu"
            } else {
                "có dấu"
            }
        );
        Ok(doi)
    }

    fn cai_shadow(&mut self, khong_dau: bool) -> Result<usize> {
        let (du_lieu, doi) = self.doi_locals(khong_dau);
        if doi == 0 {
            bail!("chưa có câu nào được dịch – bấm Dịch trước");
        }
        let tm = self.thu_muc_game.clone();
        go_shadow(&tm)?;
        let ma = self.tt.id_max + 1;
        let muc = &self.tt.nguon.1;
        let loc = muc.locale;
        let goc_ten = format!("bigfile._mod.{TEN_MOD}.000.000.tiger");
        let en_ten = format!("bigfile._mod.{TEN_MOD}.000{}.000.tiger", duoi_shadow(loc));
        if en_ten == goc_ten {
            tiger::tao_tiger_v5(&tm.join(&goc_ten), ma, 0, vec![(muc.bam, loc, du_lieu)])?;
        } else {
            tiger::tao_tiger_v5(&tm.join(&goc_ten), ma, 0, vec![])?;
            tiger::tao_tiger_v5(&tm.join(&en_ten), ma, 1, vec![(muc.bam, loc, du_lieu)])?;
            std::fs::write(
                tm.join(format!("_mod.{TEN_MOD}.specmasks.toc")),
                format!(
                    "c7fff1c00fff0400\r\n{:016x} {goc_ten}\r\n{loc:016x} {en_ten}\r\n",
                    u64::MAX
                ),
            )?;
        }
        tiger::ghi_nfo(
            &tm.join(format!("bigfile._mod.{TEN_MOD}.000.000.nfo")),
            self.tt.bap,
            self.tt.phien_ban_max + 1,
            ma as i64,
            self.tt.steam.as_deref(),
        )?;
        self.ket_qua_cai = format!(
            "Đã cài Việt hóa ({}): {doi} dòng chữ.",
            if khong_dau {
                "không dấu"
            } else {
                "có dấu"
            }
        );
        Ok(doi)
    }

    /// Gỡ bản Việt hóa: game trở lại tiếng Anh.
    pub fn go(&self) -> Result<()> {
        let tm = &self.thu_muc_game;
        match self.loai {
            LoaiTr::Rise => go_rise(tm),
            LoaiTr::Shadow => go_shadow(tm),
            LoaiTr::Tr2013 => {
                let mk = tm.join(MARKER_TR13);
                if !mk.is_file() {
                    return Ok(());
                }
                let goc = tm.join(PATCH2_GOC);
                if goc.is_file() {
                    std::fs::copy(&goc, tm.join(PATCH2))?;
                } else {
                    std::fs::remove_file(tm.join(PATCH2))?;
                }
                std::fs::remove_file(mk)?;
                Ok(())
            }
        }
    }
}

fn go_rise(tm: &Path) -> Result<()> {
    for t in [TEP_TIGER, TEP_TIGER_GOC, TEP_NFO] {
        for duoi in ["", ".disabled", ".moi.tam"] {
            if t == TEP_NFO && duoi == ".moi.tam" {
                continue;
            }
            let p = tm.join(format!("{t}{duoi}"));
            if p.is_file() {
                std::fs::remove_file(p)?;
            }
        }
    }
    Ok(())
}

fn tep_mod_shadow(tm: &Path) -> Vec<String> {
    ten_tep(tm)
        .into_iter()
        .filter(|t| {
            t.starts_with(&format!("bigfile._mod.{TEN_MOD}."))
                || t.starts_with(&format!("_mod.{TEN_MOD}."))
        })
        .collect()
}

fn go_shadow(tm: &Path) -> Result<()> {
    for t in tep_mod_shadow(tm) {
        std::fs::remove_file(tm.join(t))?;
    }
    Ok(())
}

// ---------------- lồng tiếng (VieNeu) ----------------

/// 'Người nói' từ tên FSB: "610_by_050_230_nadia.str_0" → "nadia"; "looter_a" → "looter".
pub fn nguoi_noi(ten_fsb: &str) -> String {
    let goc = ten_fsb.split('.').next().unwrap_or("").split('#').next().unwrap_or("");
    let t: Vec<&str> = goc.split('_').filter(|x| !x.is_empty()).collect();
    let j = t
        .iter()
        .rposition(|v| v.chars().all(|c| c.is_ascii_digit()))
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut t = t[j..].to_vec();
    if t.len() > 1 && t[t.len() - 1].chars().count() == 1 {
        t.pop();
    }
    let s = t.join("_").to_lowercase();
    if s.is_empty() {
        "khac".into()
    } else {
        s
    }
}

/// Bỏ thẻ [..] <..> {..} *tiếng cười* và "Tên:" để VieNeu đọc.
pub fn chu_de_doc(vi: &str) -> String {
    static R: OnceLock<[regex::Regex; 4]> = OnceLock::new();
    let r = R.get_or_init(|| {
        [
            regex::Regex::new(r"\[[^\[\]]*\]|<[^<>]*>|\{[^{}]*\}|\*[^*]*\*").unwrap(),
            regex::Regex::new(r"^\s*[A-ZÀ-Ỹ][\wÀ-ỹ.'\- ]{0,24}:\s+").unwrap(),
            regex::Regex::new(r"\s+").unwrap(),
            regex::Regex::new(r"[A-Za-zÀ-ỹ]{2,}").unwrap(),
        ]
    });
    let t = r[0].replace_all(vi, " ");
    let t = r[1].replace(&t, "");
    let t = r[2].replace_all(&t, " ");
    let t = t.trim_matches(|c: char| c == ' ' || c == '-' || c == '–');
    if r[3].is_match(t) {
        t.to_string()
    } else {
        String::new()
    }
}

/// Khóa xếp theo tiến trình game (số đầu tên = khu vực / màn).
fn thu_tu_game(ten_fsb: &str) -> Vec<(u8, u64, String)> {
    ten_fsb
        .split('.')
        .next()
        .unwrap_or("")
        .split('#')
        .next()
        .unwrap_or("")
        .split('_')
        .map(|x| match x.parse::<u64>() {
            Ok(n) if x.chars().all(|c| c.is_ascii_digit()) => (0, n, String::new()),
            _ => (1, 0, x.to_string()),
        })
        .collect()
}

#[derive(Debug, Default)]
pub struct KetQuaLongTieng {
    pub file: usize,
    pub da_co: usize,
    pub tong: usize,
}

type CauDoc = Vec<(i32, String, Option<String>)>;

impl TombRaider {
    fn ban_mau(&self) -> &'static str {
        if self.loai == LoaiTr::Tr2013 {
            "v3"
        } else {
            "v2"
        }
    }
    /// độ to tối thiểu (RMS) của giọng Việt khi ghép; 0 = theo giọng gốc
    fn san_giong(&self) -> f64 {
        if self.loai == LoaiTr::Tr2013 {
            0.07
        } else {
            0.0
        }
    }
    fn mau_giong(&self, g: &str) -> PathBuf {
        self.thu_muc_giong()
            .join("mau")
            .join(format!("{g}.{}.wav", self.ban_mau()))
    }
    fn thu_wav(&self, g: &str) -> PathBuf {
        self.thu_muc_giong()
            .join("wav")
            .join(format!("{g}.{}", self.ban_mau()))
    }
    fn ten_mp3(&self, ma: &str, c: usize) -> PathBuf {
        let k = self.mul.get(ma).map(|t| t.k).unwrap_or(1);
        self.thu_muc_giong().join("mp3").join(if k == 1 {
            format!("{ma}.mp3")
        } else {
            format!("{ma}_{c}.mp3")
        })
    }

    /// Các file .mul có kênh giọng nhân vật: {mã: [chỉ số kênh giọng]}.
    fn mul_long_tieng_duoc(&self) -> BTreeMap<String, Vec<usize>> {
        self.mul
            .iter()
            .filter(|(_, t)| !t.phim && !t.kenh.is_empty())
            .filter_map(|(m, t)| {
                let kg: Vec<usize> = (0..t.kenh.len())
                    .filter(|&c| la_kenh_giong(&t.kenh[c], t.k))
                    .collect();
                (!kg.is_empty()).then(|| (m.clone(), kg))
            })
            .collect()
    }

    /// [(khung, chữ Việt để đọc | "", tên người nói trong phụ đề)]
    fn cau_doc(&self, t: &MucMul) -> CauDoc {
        static R: OnceLock<regex::Regex> = OnceLock::new();
        let r = R.get_or_init(|| {
            regex::Regex::new(r"^\s*(?:\[[^\]]*\]\s*)*([A-Z][A-Za-z.'\- ]{1,24}):\s").unwrap()
        });
        t.pd
            .iter()
            .filter(|(_, en)| !en.is_empty())
            .map(|(khung, en)| {
                let vi = self.chuoi_viet(en, false, true);
                let ten = r
                    .captures(en)
                    .map(|m| m[1].trim().to_lowercase().replace(' ', "_"));
                (*khung, vi.map(|v| chu_de_doc(&v)).unwrap_or_default(), ten)
            })
            .collect()
    }

    /// Xuất MP3 gốc của 1 kênh (dùng làm giọng mẫu / nền ghép).
    fn xuat_mp3_goc(&self, ma: &str, c: usize, thu: &Path) -> Result<PathBuf> {
        let p = thu.join(format!("{ma}_{c}.mp3"));
        if !p.is_file() {
            let x = self.doc_mul(ma)?;
            let fsb = x.fsb_kenh();
            let f = fsb
                .get(c)
                .ok_or_else(|| anyhow::anyhow!("{ma}: không có kênh {c}"))?;
            let d: Vec<u8> = mul::mp3_khung(mul::fsb_mp3(f)?)?.concat();
            let tam = p.with_extension("mp3.tam");
            std::fs::write(&tam, d)?;
            std::fs::rename(&tam, &p)?;
        }
        Ok(p)
    }

    /// TR2013: giọng mẫu người dùng gửi (`phan_bo.json` {"TR2013": {người_nói: file.wav}}).
    /// Đổi file → chép làm giọng mẫu, xóa câu đã đọc + đoạn đã lồng của người đó để làm lại.
    fn dung_giong_rieng(&self) {
        let mut cac = vec![thu_muc_lam_viec(&self.thu_muc_game).join("giong_rieng")];
        cac.extend(
            "CDEFGHIJK"
                .chars()
                .map(|o| PathBuf::from(format!(r"{o}:\DichGame_LongTieng\giong_rieng"))),
        );
        let Some((goc, pb)) = cac.into_iter().find_map(|d| {
            let j: serde_json::Value =
                serde_json::from_slice(&std::fs::read(d.join("phan_bo.json")).ok()?).ok()?;
            let pb = j
                .get("TR2013")?
                .as_object()
                .filter(|o| !o.is_empty())?
                .clone();
            Some((d, pb))
        }) else {
            return;
        };
        for (g, ten) in pb {
            let Some(ten) = ten.as_str() else { continue };
            let nguon = goc.join(if ten.to_lowercase().ends_with(".wav") {
                ten.to_string()
            } else {
                format!("{ten}.wav")
            });
            let Ok(b) = std::fs::read(&nguon) else { continue };
            let dau = format!(
                "{:x} {}",
                md5::compute(&b),
                nguon.file_name().unwrap().to_string_lossy()
            );
            let mau = self.mau_giong(&g);
            let ghi = PathBuf::from(format!("{}.nguon.txt", mau.display()));
            if mau.is_file() && std::fs::read_to_string(&ghi).is_ok_and(|s| s.trim() == dau) {
                continue;
            }
            let _ = std::fs::create_dir_all(mau.parent().unwrap());
            if std::fs::write(&mau, &b).is_err() {
                continue;
            }
            let _ = std::fs::remove_dir_all(self.thu_wav(&g));
            for (m, t) in &self.mul {
                if nguoi_noi(&t.ten) == g {
                    for c in 0..t.k.max(1) {
                        let _ = std::fs::remove_file(self.ten_mp3(m, c));
                    }
                }
            }
            let _ = std::fs::write(&ghi, dau);
        }
    }

    /// Lồng tiếng Việt cho lời thoại trong các file .mul (Rise, TR2013). Chạy lại = làm tiếp.
    /// `thu` = chỉ ~16 đoạn đầu game đã dịch sẵn, để nghe thử. → None nếu bị dừng.
    pub fn long_tieng(
        &self,
        thu: bool,
        tang_db: f64,
        dung: &AtomicBool,
        bao: &dyn Fn(&str),
        tien_do: &dyn Fn(usize, usize, &str),
    ) -> Result<Option<KetQuaLongTieng>> {
        use super::giong_vieneu as gv;
        use serde_json::json;
        if self.loai == LoaiTr::Shadow {
            anyhow::bail!("lồng tiếng chưa hỗ trợ game này");
        }
        if gv::tim_vieneu().is_none() {
            anyhow::bail!(
                "không tìm thấy app VieNeu (bản tăng tốc GPU) – mở VieNeu, cài 'GPU' rồi thử lại"
            );
        }
        let lv = self.thu_muc_giong();
        if self.loai == LoaiTr::Tr2013 {
            // đổi cách tạo giọng → bỏ các đoạn đã lồng bằng giọng cũ
            let dau = lv.join("ban_mau.txt");
            let cu = std::fs::read_to_string(&dau).unwrap_or_default();
            if cu.trim() != self.ban_mau() {
                let _ = std::fs::remove_dir_all(lv.join("mp3"));
                std::fs::create_dir_all(&lv)?;
                std::fs::write(&dau, self.ban_mau())?;
            }
            self.dung_giong_rieng();
        }
        for t in ["goc", "mau", "wav", "mp3", "tam"] {
            std::fs::create_dir_all(lv.join(t))?;
        }
        let tam = lv.join("tam");
        let goc_dir = lv.join("goc");
        let tat_ca = self.mul_long_tieng_duoc();
        if tat_ca.is_empty() {
            anyhow::bail!("không tìm thấy lời thoại nào trong game (bấm Quét lại)");
        }
        let mut thu_tu: Vec<&String> = tat_ca.keys().collect();
        thu_tu.sort_by_key(|m| (thu_tu_game(&self.mul[*m].ten), (*m).clone()));
        if thu {
            let co_dich: Vec<&String> = thu_tu
                .iter()
                .copied()
                .filter(|m| self.cau_doc(&self.mul[*m]).iter().any(|x| !x.1.is_empty()))
                .collect();
            let mo_dau = ["iceclimb", "ice_climb", "lara_ledge_catch"];
            let mut dau: Vec<&String> = co_dich
                .iter()
                .copied()
                .filter(|m| {
                    mo_dau
                        .iter()
                        .any(|x| self.mul[*m].ten.to_lowercase().contains(x))
                })
                .collect();
            for m in &co_dich {
                if !dau.contains(m) {
                    dau.push(m);
                }
            }
            dau.truncate(dau.len().min(16).max(10.min(dau.len())));
            if dau.is_empty() {
                anyhow::bail!("chưa có câu thoại nào được dịch – hãy dịch chữ trước rồi thử lại");
            }
            thu_tu = dau;
        }
        let mut chon: BTreeMap<String, CauDoc> = BTreeMap::new();
        for m in &thu_tu {
            let cau = self.cau_doc(&self.mul[*m]);
            if !cau.iter().any(|x| !x.1.is_empty()) {
                continue;
            }
            if !thu && !self.file_giong(m).is_empty() {
                continue;
            }
            chon.insert((*m).clone(), cau);
        }
        let da_co = || {
            tat_ca
                .keys()
                .filter(|m| !self.file_giong(m).is_empty())
                .count()
        };
        if chon.is_empty() {
            if thu {
                anyhow::bail!("không có câu thoại đã dịch ở đầu game – dịch chữ trước rồi thử lại");
            }
            return Ok(Some(KetQuaLongTieng {
                file: 0,
                da_co: da_co(),
                tong: tat_ca.len(),
            }));
        }

        // 1. cửa sổ thời gian từng câu + câu nào ở kênh giọng nào
        let mut cua_so: BTreeMap<&str, Vec<[i64; 2]>> = BTreeMap::new();
        for (m, cau) in &chon {
            let het = self.mul[m].mau as i64;
            let ds = (0..cau.len())
                .map(|i| {
                    let a = (cau[i].0 as f64 / 30.0 * 44100.0) as i64;
                    let b = cau
                        .get(i + 1)
                        .map(|n| (n.0 as f64 / 30.0 * 44100.0) as i64)
                        .unwrap_or(het);
                    [a, b.max(a + 4410)]
                })
                .collect();
            cua_so.insert(m, ds);
        }
        bao("Bước 1/4: xuất giọng gốc...");
        let mut phan_tich = Vec::new();
        for m in chon.keys() {
            let kg = &tat_ca[m];
            if kg.len() > 1 {
                let kenh: Vec<String> = kg
                    .iter()
                    .map(|&c| {
                        self.xuat_mp3_goc(m, c, &goc_dir)
                            .map(|p| p.to_string_lossy().into_owned())
                    })
                    .collect::<Result<_>>()?;
                phan_tich.push(json!({"id": m, "kenh": kenh, "cau": cua_so[m.as_str()]}));
            } else {
                self.xuat_mp3_goc(m, kg[0], &goc_dir)?;
            }
            if dung.load(Ordering::Relaxed) {
                return Ok(None);
            }
        }
        let mut gan: HashMap<&str, Vec<Option<usize>>> = HashMap::new();
        if !phan_tich.is_empty() {
            let ra_pt = tam.join("phan_tich.json");
            let viec =
                json!({"che_do": "phan_tich", "ds": phan_tich, "ra": ra_pt.to_string_lossy()});
            if !gv::chay_kich_ban(
                "long_tieng_tr.py",
                &viec,
                &tam,
                "Bước 1/4: tìm kênh giọng từng nhân vật",
                dung,
                bao,
            )? {
                return Ok(None);
            }
            let kq: HashMap<String, Vec<i64>> = serde_json::from_slice(&std::fs::read(&ra_pt)?)?;
            for (m, _) in &chon {
                let kg = &tat_ca[m];
                if let Some(l) = kq.get(m) {
                    gan.insert(
                        m,
                        l.iter()
                            .map(|&j| {
                                if j >= 0 {
                                    kg.get(j as usize).copied()
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    );
                }
            }
        }
        for (m, cau) in &chon {
            gan.entry(m)
                .or_insert_with(|| vec![Some(tat_ca[m][0]); cau.len()]);
        }

        // 2. người nói từng câu → giọng mẫu
        let mut nguoi_cau: HashMap<(&str, usize), String> = HashMap::new();
        let mut mau_nguon: BTreeMap<String, Vec<(&str, usize)>> = BTreeMap::new();
        for (m, cau) in &chon {
            let t = &self.mul[m];
            let mut ten_kenh: HashMap<usize, String> = HashMap::new();
            for (i, (_, _, ten)) in cau.iter().enumerate() {
                let Some(c) = gan[m.as_str()].get(i).copied().flatten() else {
                    continue;
                };
                let g = if t.k == 1 {
                    nguoi_noi(&t.ten)
                } else {
                    if let Some(n) = ten {
                        ten_kenh.insert(c, n.clone());
                    }
                    ten_kenh
                        .get(&c)
                        .cloned()
                        .unwrap_or_else(|| format!("kenh_{m}_{c}"))
                };
                nguoi_cau.insert((m, i), g.clone());
                mau_nguon.entry(g).or_default().push((m, c));
            }
        }
        // giọng mẫu: ưu tiên câu NÓI TO, DỨT KHOÁT (có "!", không phải tiếng thở / tiếng động)
        static THE: OnceLock<regex::Regex> = OnceLock::new();
        let the = THE.get_or_init(|| {
            regex::Regex::new(
                r"<[^<>]*>|\{[^{}]*\}|\[[^\[\]]*\]|%[0-9]*[sdif]|\\n|\$[A-Za-z_][A-Za-z0-9_]*",
            )
            .unwrap()
        });
        let tu = regex::Regex::new(r"[A-Za-z]+").unwrap();
        let mut don: HashMap<String, Vec<(i32, &str)>> = HashMap::new();
        for m2 in tat_ca.keys() {
            let t2 = &self.mul[m2];
            let d = t2.mau as f64 / 44100.0;
            if t2.k != 1 || !(1.5 < d && d < 9.0) {
                continue;
            }
            let en = t2
                .pd
                .iter()
                .filter(|x| !x.1.is_empty())
                .map(|x| x.1.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let chu = the.replace_all(&en, "");
            if chu.contains('<') || chu.contains('*') || tu.find_iter(&chu).count() < 3 {
                continue;
            }
            let diem = chu.matches('!').count() as i32 * 2 + (2.5 < d && d < 7.0) as i32;
            don.entry(nguoi_noi(&t2.ten)).or_default().push((-diem, m2));
        }
        let mut viec_mau = Vec::new();
        for (g, ds) in &mau_nguon {
            let ra = self.mau_giong(g);
            if ra.is_file() {
                continue;
            }
            if let Some(d) = don.get_mut(g) {
                d.sort();
                bao(&format!(
                    "Bước 2/4: chọn câu {g} nói mạnh nhất làm giọng mẫu..."
                ));
                let nguon: Vec<String> = d
                    .iter()
                    .take(24)
                    .map(|(_, m2)| {
                        self.xuat_mp3_goc(m2, 0, &goc_dir)
                            .map(|p| p.to_string_lossy().into_owned())
                    })
                    .collect::<Result<_>>()?;
                viec_mau.push(json!({"ra": ra.to_string_lossy(), "nguon": nguon, "chon": 3}));
            } else {
                let mut uniq: Vec<&(&str, usize)> = Vec::new();
                for x in ds {
                    if !uniq.contains(&x) {
                        uniq.push(x);
                    }
                }
                let nguon: Vec<String> = uniq
                    .iter()
                    .take(3)
                    .map(|(m2, c2)| {
                        self.xuat_mp3_goc(m2, *c2, &goc_dir)
                            .map(|p| p.to_string_lossy().into_owned())
                    })
                    .collect::<Result<_>>()?;
                viec_mau.push(json!({"ra": ra.to_string_lossy(), "nguon": nguon}));
            }
            if dung.load(Ordering::Relaxed) {
                return Ok(None);
            }
        }
        if !viec_mau.is_empty()
            && !gv::chay_kich_ban(
                "long_tieng_tr.py",
                &json!({"che_do": "mau", "ds": viec_mau}),
                &tam,
                "Bước 2/4: tạo giọng mẫu",
                dung,
                bao,
            )?
        {
            return Ok(None);
        }

        // 3. VieNeu đọc từng câu (tên riêng phiên âm sang cách đọc tiếng Việt)
        let mut nhom: BTreeMap<String, Vec<(String, String, f64)>> = BTreeMap::new();
        for ((m, i), g) in &nguoi_cau {
            let chu = &chon[*m][*i].1;
            if chu.is_empty() || !self.mau_giong(g).is_file() {
                continue;
            }
            let [a, b] = cua_so[m][*i];
            let giay = (((b - a) as f64 / 44100.0).max(0.5) * 1000.0).round() / 1000.0;
            nhom.entry(g.clone()).or_default().push((
                format!("{m}_{i}"),
                super::phien_am::doc_duoc(chu),
                giay,
            ));
        }
        let mut nhom: Vec<(String, Vec<(String, String, f64)>)> = nhom.into_iter().collect();
        nhom.sort_by_key(|(_, d)| std::cmp::Reverse(d.len()));
        let viec_doc: Vec<gv::Nhom> = nhom
            .into_iter()
            .map(|(g, ds)| gv::Nhom {
                mau: self.mau_giong(&g),
                ra: self.thu_wav(&g),
                ten: g,
                rms: 0.08,
                ds,
            })
            .collect();
        if !viec_doc.is_empty()
            && !gv::chay(&viec_doc, &tam, 1.35, dung, &|a, b, ct| {
                tien_do(a, b, &format!("Bước 3/4: VieNeu đã đọc{ct}"))
            })?
        {
            return Ok(None);
        }

        // 4. ghép vào đúng kênh, đúng thời điểm phụ đề → MP3 44,1 kHz
        bao("Bước 4/4: ghép giọng vào đúng thời điểm phụ đề...");
        let mut viec = Vec::new();
        for (m, cau) in &chon {
            let t = &self.mul[m];
            let mut theo_kenh: BTreeMap<usize, Vec<serde_json::Value>> = BTreeMap::new();
            for i in 0..cau.len() {
                let (Some(g), Some(c)) = (
                    nguoi_cau.get(&(m.as_str(), i)),
                    gan[m.as_str()].get(i).copied().flatten(),
                ) else {
                    continue;
                };
                let w = self.thu_wav(g).join(format!("{m}_{i}.wav"));
                if cau[i].1.is_empty() || !w.is_file() {
                    continue;
                }
                let [a, b] = cua_so[m.as_str()][i];
                theo_kenh
                    .entry(c)
                    .or_default()
                    .push(json!([a, b, w.to_string_lossy()]));
            }
            for (c, ds) in theo_kenh {
                viec.push(json!({
                    "goc": self.xuat_mp3_goc(m, c, &goc_dir)?.to_string_lossy(),
                    "so_mau": t.kenh[c].2,
                    "ra": self.ten_mp3(m, c).to_string_lossy(),
                    "cau": ds,
                }));
            }
        }
        if !viec.is_empty()
            && !gv::chay_kich_ban(
                "long_tieng_tr.py",
                &json!({"che_do": "ghep", "ds": viec, "tang_db": tang_db, "san": self.san_giong()}),
                &tam,
                "Bước 4/4: đã ghép",
                dung,
                bao,
            )?
        {
            return Ok(None);
        }
        let xong = chon
            .keys()
            .filter(|m| !self.file_giong(m).is_empty())
            .count();
        Ok(Some(KetQuaLongTieng {
            file: xong,
            da_co: da_co(),
            tong: tat_ca.len(),
        }))
    }

    pub fn xoa_giong(&self) {
        let _ = std::fs::remove_dir_all(self.thu_muc_giong().join("mp3"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_va_moc() {
        assert_eq!(the_tr("<coughing> Hi {Key} %s"), vec!["%s", "<~>", "{Key}"]);
        assert_eq!(
            the_tr("<ho> Chào {Key} %s"),
            the_tr("<coughing> Hi {Key} %s")
        );
        let s = r"Hello there.\(2.46) Go now!\(5)";
        assert_eq!(
            tach_moc(s),
            vec!["Hello there.", r"\(2.46)", " Go now!", r"\(5)", ""]
        );
        assert_eq!(doan_te(s), vec!["Hello there.", "Go now!"]);
        assert_eq!(duoi_shadow(u64::MAX), "");
        assert_eq!(
            thu_tu_ngon_ngu("bigfile.dlc.mode.bloodties.000_ENGLISH.000.tiger"),
            1
        );
        assert_eq!(thu_tu_ngon_ngu("bigfile.update2.000.000.tiger"), 0);
    }

    /// Cài thật vào thư mục game (chỉ dùng thư mục giả để thử):
    /// TR=<thư mục> LOAI=rise|tr2013|shadow [KHONG_DAU=1] cargo test -- --ignored cai_that
    #[test]
    #[ignore]
    fn cai_that() {
        let tm = PathBuf::from(std::env::var("TR").unwrap());
        let loai = match std::env::var("LOAI").as_deref() {
            Ok("tr2013") => LoaiTr::Tr2013,
            Ok("shadow") => LoaiTr::Shadow,
            _ => LoaiTr::Rise,
        };
        let dung = AtomicBool::new(false);
        let bao = |s: &str| println!("{s}");
        let t0 = std::time::Instant::now();
        let mut x = TombRaider::nap(&tm, loai, &bao, &dung).unwrap();
        println!(
            "nạp {:?}: thống kê {:?}, phụ đề {:?}, có sẵn {}",
            t0.elapsed(),
            x.thong_ke(),
            x.thong_ke_phu_de(),
            x.co_viet_hoa_san
        );
        if std::env::var("CHI_NAP").is_ok() {
            return;
        }
        if std::env::var("GO").is_ok() {
            x.go().unwrap();
            println!("gỡ xong, còn cài: {}", x.da_cai());
            return;
        }
        let n = x
            .cai(std::env::var("KHONG_DAU").is_ok(), &|_| {}, &dung)
            .unwrap();
        println!("cài {n} dòng trong {:?}\n{}", t0.elapsed(), x.ket_qua_cai);
    }

    /// Lồng tiếng thử vài đoạn đầu game (cần VieNeu GPU, dữ liệu dịch có sẵn):
    /// TR=<thư mục game> LOAI=rise|tr2013 VIET_HOA_GIONG=<thư mục tạm> cargo test --release -- --ignored long_tieng_that
    #[test]
    #[ignore]
    fn long_tieng_that() {
        let tm = PathBuf::from(std::env::var("TR").unwrap());
        let loai = match std::env::var("LOAI").as_deref() {
            Ok("tr2013") => LoaiTr::Tr2013,
            _ => LoaiTr::Rise,
        };
        let dung = AtomicBool::new(false);
        let x = TombRaider::nap(&tm, loai, &|_| {}, &dung).unwrap();
        let kq = x
            .long_tieng(true, 4.0, &dung, &|s| println!("{s}"), &|a, b, s| println!("{a}/{b} {s}"))
            .unwrap();
        println!("{kq:?}");
        assert!(kq.unwrap().file > 0);
        // MP3 ghép ra phải đóng được vào FSB4 (MPEG-1 Layer III 44,1 kHz)
        let mut n = 0;
        for e in std::fs::read_dir(x.thu_muc_giong().join("mp3")).unwrap().flatten() {
            let mp3 = std::fs::read(e.path()).unwrap();
            assert!(!mul::mp3_khung(&mp3).unwrap().is_empty(), "{:?}", e.path());
            mul::tao_fsb4(&mp3, "thu", None).unwrap();
            n += 1;
        }
        assert!(n > 0);
    }
}
