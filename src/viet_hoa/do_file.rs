//! Dò file chữ của MỌI game (giống do_file_chu.py của app cũ):
//! - Quét thư mục game → file nhiều chữ tiếng Anh (loại, số câu, % tiếng Anh, câu mẫu).
//! - Đọc / ghi: JSON (cả RPG Maker), CSV/TSV, XML/RESX, INI/LANG/PROPERTIES/STRINGS, PO,
//!   YAML, TXT, Ren'Py .rpy, Unreal .locres. Gói nén riêng (.pak, .assets...) chỉ báo "chưa hỗ trợ".
//! - Dịch bằng lõi Việt hóa (Claude theo lô / Offline) hoặc tự dịch bằng CSV (Excel).
//! - Ghi vào game: sao lưu bản gốc lần đầu, luôn dịch từ bản gốc; có trả lại bản gốc.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use anyhow::{anyhow, Result};
use fancy_regex::Regex as FRegex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::duong_dan;
use crate::viet_hoa::viec_dich::{BangTen, ViecDich};

pub const PHIEN_BAN_QUET: u32 = 1;

pub fn loai_theo_duoi(duoi: &str) -> Option<&'static str> {
    Some(match duoi {
        ".json" => "json",
        ".csv" | ".tsv" => "csv",
        ".xml" | ".resx" | ".xliff" | ".xlf" => "xml",
        ".ini" | ".lang" | ".properties" | ".cfg" | ".strings" | ".loc" => "ini",
        ".po" | ".pot" => "po",
        ".yml" | ".yaml" => "yaml",
        ".txt" => "txt",
        ".rpy" => "rpy",
        ".locres" => "locres",
        _ => return None,
    })
}

pub fn ten_loai(loai: &str) -> &'static str {
    match loai {
        "json" => "JSON",
        "csv" => "Bảng CSV",
        "xml" => "XML",
        "ini" => "Danh sách khóa=chữ",
        "po" => "Gettext PO",
        "yaml" => "YAML",
        "txt" => "Văn bản",
        "rpy" => "Kịch bản Ren'Py",
        "locres" => "Unreal .locres",
        _ => "?",
    }
}

fn goi_nen(duoi: &str) -> Option<&'static str> {
    Some(match duoi {
        ".pak" => "gói Unreal Engine (.pak)",
        ".utoc" | ".ucas" => "gói Unreal Engine (.utoc/.ucas)",
        ".assets" => "gói Unity (.assets)",
        ".bundle" => "gói Unity (.bundle)",
        ".resource" => "gói Unity (.resource)",
        ".rpa" => "gói Ren'Py (.rpa)",
        ".pck" => "gói Godot (.pck)",
        ".tiger" => "gói Crystal Dynamics (.tiger)",
        ".paz" => "gói Pearl Abyss (.paz)",
        ".forge" => "gói Ubisoft (.forge)",
        ".cpk" => "gói CRI (.cpk)",
        ".vpk" => "gói Valve (.vpk)",
        ".big" => "gói .big",
        ".wad" => "gói .wad",
        ".arc" => "gói .arc",
        ".rgssad" => "gói RPG Maker XP/VX",
        ".rgss2a" => "gói RPG Maker VX",
        ".rgss3a" => "gói RPG Maker VX Ace",
        _ => return None,
    })
}

const BO_THU_MUC: &[&str] = &[
    "__pycache__",
    ".git",
    "_commonredist",
    "directx",
    "redist",
    "redistributables",
    "vcredist",
    "dotnetfx",
    "physx",
    "easyanticheat",
    "battleye",
    "steam_settings",
    "__installer",
    "_redist",
    "binaries",
    "plugins",
    "crashreportclient",
    "mono",
    "monobleedingedge",
    "il2cpp_data",
];
const TU_ANH: &[&str] = &[
    "the", "and", "you", "to", "of", "is", "a", "in", "it", "your", "for", "that", "this", "with",
    "on", "are", "be", "have", "not", "we", "can", "will", "my", "what", "i", "me", "do", "an",
    "at", "if", "from",
];
const KHOA_BO: &[&str] = &[
    "id",
    "key",
    "type",
    "icon",
    "iconindex",
    "image",
    "file",
    "filename",
    "path",
    "sound",
    "se",
    "bgm",
    "bgs",
    "me",
    "script",
    "class",
    "tag",
    "tags",
    "url",
    "font",
    "animation",
    "charactername",
    "facename",
    "battlername",
    "picture",
    "texture",
    "sprite",
    "model",
    "prefab",
    "material",
    "shader",
    "guid",
    "uuid",
    "hash",
    "version",
    "color",
    "colour",
    "format",
    "locale",
    "lang",
    "language",
    "code",
    "ref",
    "asset",
    "assetpath",
    "event",
    "trigger",
    "command",
    "anim",
    "audio",
    "music",
    "voice",
    "tileset",
    "parallaxname",
    "title1name",
    "title2name",
    "battleback1name",
    "battleback2name",
    "note",
    "plugin",
    "parameters_raw",
];
const MA_RPG_BO: &[i64] = &[
    355, 655, 356, 357, 108, 408, 111, 122, 231, 241, 245, 249, 250, 132, 133, 139, 283, 284,
];

fn re(p: &'static str, o: &'static OnceLock<Regex>) -> &'static Regex {
    o.get_or_init(|| Regex::new(p).unwrap())
}

/// Thẻ / mã cần giữ nguyên (kiểm tra bản dịch).
fn re_the() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(
        r"<[^<>]*>|\{[^{}]*\}|\[[^\[\]]*\]|%[-+ 0#]*\d*(?:\.\d+)?[sdifxXuc@]|%\d+\$?[sd]?|\\[nrt]|\$[A-Za-z_][A-Za-z0-9_]*|\\[A-Za-z]+\[[^\]]*\]|\\[A-Za-z{}\\|.!<>^$]",
        &R,
    )
}

pub fn the_cua(s: &str) -> Vec<String> {
    let mut v: Vec<String> = re_the()
        .find_iter(s)
        .map(|m| m.as_str().to_string())
        .collect();
    v.sort();
    v
}

/// Bỏ dấu tiếng Việt (chế độ "không dấu" cho game không có font tiếng Việt).
pub fn bo_dau(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.replace('đ', "d")
        .replace('Đ', "D")
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

/// Chuỗi có vẻ là chữ tiếng Anh hiện lên màn hình (không phải mã, đường dẫn, tên file...).
pub fn la_chu_anh(s: &str) -> bool {
    let n = s.chars().count();
    if !(2..=5000).contains(&n) {
        return false;
    }
    let t = re_the().replace_all(s, " ");
    let t = t.trim();
    static R2: OnceLock<Regex> = OnceLock::new();
    if t.chars().count() < 2 || !re(r"[A-Za-z]{2,}", &R2).is_match(t) {
        return false;
    }
    static R3: OnceLock<Regex> = OnceLock::new();
    static R4: OnceLock<Regex> = OnceLock::new();
    if t.contains("://")
        || re(r"^[\w\-./\\]+\.[A-Za-z0-9]{2,4}$", &R3).is_match(t)
        || re(r"^[\w\-]+[/\\][\w\-./\\]*$", &R4).is_match(t)
    {
        return false;
    }
    let chu: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    if !chu.is_empty()
        && chu.iter().filter(|c| c.is_ascii()).count() as f64 / (chu.len() as f64) < 0.9
    {
        return false; // ngôn ngữ khác (Trung, Nga, Việt...)
    }
    if !t.contains(' ') {
        static R5: OnceLock<Regex> = OnceLock::new();
        static R6: OnceLock<Regex> = OnceLock::new();
        if t.contains('_')
            || re(r"[a-z][A-Z]", &R5).is_match(t)
            || re(r"[A-Za-z]\d|\d[A-Za-z]", &R6).is_match(t)
        {
            return false; // mã nội bộ: ten_bien, TenBien, abc123
        }
        let co_hoa = t.chars().any(|c| c.is_uppercase());
        let co_thuong = t.chars().any(|c| c.is_lowercase());
        if (!co_hoa && co_thuong) || (co_hoa && !co_thuong && t.chars().count() > 12) {
            return false; // true, normal, CONSTANT_NAME
        }
    }
    let ma = t
        .chars()
        .filter(|c| "{}();=<>|&*^~`\\".contains(*c))
        .count();
    if ma as f64 > (2f64).max(t.chars().count() as f64 * 0.12) {
        return false;
    }
    true
}

pub fn la_cau(s: &str) -> bool {
    re_the().replace_all(s, " ").split_whitespace().count() >= 3
}

pub fn ty_le_anh(cac: &[&str]) -> f64 {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = re(r"[A-Za-z']+", &R);
    let tu: Vec<String> = cac
        .iter()
        .take(3000)
        .flat_map(|s| r.find_iter(s).map(|m| m.as_str().to_lowercase()))
        .collect();
    if tu.is_empty() {
        return 0.0;
    }
    tu.iter().filter(|w| TU_ANH.contains(&w.as_str())).count() as f64 / tu.len() as f64
}

// ---------------- mã hóa chữ ----------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MaHoa {
    Utf8,
    Utf16Le,
    Utf16Be,
    Cp1252,
}

/// bytes → (chữ, mã hóa, BOM)
pub fn giai_ma(b: &[u8]) -> (String, MaHoa, Vec<u8>) {
    if let Some(r) = b.strip_prefix(b"\xEF\xBB\xBF") {
        return (
            String::from_utf8_lossy(r).into_owned(),
            MaHoa::Utf8,
            b"\xEF\xBB\xBF".to_vec(),
        );
    }
    if let Some(r) = b.strip_prefix(b"\xFF\xFE") {
        return (
            encoding_rs::UTF_16LE
                .decode_without_bom_handling(r)
                .0
                .into_owned(),
            MaHoa::Utf16Le,
            b"\xFF\xFE".to_vec(),
        );
    }
    if let Some(r) = b.strip_prefix(b"\xFE\xFF") {
        return (
            encoding_rs::UTF_16BE
                .decode_without_bom_handling(r)
                .0
                .into_owned(),
            MaHoa::Utf16Be,
            b"\xFE\xFF".to_vec(),
        );
    }
    if b.len() >= 4 && b[1] == 0 && b[3] == 0 {
        return (
            encoding_rs::UTF_16LE
                .decode_without_bom_handling(b)
                .0
                .into_owned(),
            MaHoa::Utf16Le,
            vec![],
        );
    }
    match std::str::from_utf8(b) {
        Ok(s) => (s.to_string(), MaHoa::Utf8, vec![]),
        Err(_) => (
            encoding_rs::WINDOWS_1252
                .decode_without_bom_handling(b)
                .0
                .into_owned(),
            MaHoa::Cp1252,
            vec![],
        ),
    }
}

pub fn ma_hoa(chu: &str, mh: MaHoa, bom: &[u8]) -> Vec<u8> {
    let mut ra = bom.to_vec();
    match mh {
        MaHoa::Utf8 => ra.extend_from_slice(chu.as_bytes()),
        MaHoa::Utf16Le => chu
            .encode_utf16()
            .for_each(|u| ra.extend_from_slice(&u.to_le_bytes())),
        MaHoa::Utf16Be => chu
            .encode_utf16()
            .for_each(|u| ra.extend_from_slice(&u.to_be_bytes())),
        MaHoa::Cp1252 => {
            let (b, _, loi) = encoding_rs::WINDOWS_1252.encode(chu);
            if loi {
                // file cũ không chứa được tiếng Việt → ghi UTF-8 có BOM
                let mut r = b"\xEF\xBB\xBF".to_vec();
                r.extend_from_slice(chu.as_bytes());
                return r;
            }
            ra.extend_from_slice(&b);
        }
    }
    ra
}

/// Thêm \ trước dấu nháy chưa được thoát; xuống dòng → \n (chữ đọc ra giữ nguyên mã thoát gốc).
fn thoat(vi: &str, q: char) -> String {
    let vi = vi.replace("\r\n", "\\n").replace('\n', "\\n");
    let mut ra = String::with_capacity(vi.len() + 4);
    let mut truoc = '\0';
    for c in vi.chars() {
        if c == q && truoc != '\\' {
            ra.push('\\');
        }
        ra.push(c);
        truoc = c;
    }
    ra
}

fn nhay_mo(v: &str) -> String {
    let v = v.trim();
    let c: Vec<char> = v.chars().collect();
    if c.len() >= 2 && c[0] == c[c.len() - 1] && (c[0] == '"' || c[0] == '\'') {
        return c[1..c.len() - 1].iter().collect();
    }
    v.to_string()
}

pub type HamDich<'a> = dyn FnMut(&str) -> Option<String> + 'a;

/// Một định dạng: đọc → [(khóa, chữ Anh)], ghi(bytes, dịch) → bytes.
pub trait Dang: Sync {
    fn doc(&self, b: &[u8]) -> Result<Vec<(String, String)>>;
    fn ghi(&self, b: &[u8], dich: &mut HamDich) -> Result<Vec<u8>>;
}

// ---------------- dạng đọc bằng biểu thức (INI, YAML, TXT, Ren'Py) ----------------

#[derive(Clone, Copy, PartialEq)]
enum KieuRegex {
    Ini,
    Yaml,
    Txt,
    Rpy,
}

struct DangRegex {
    kieu: KieuRegex,
    mau: FRegex,
}

impl DangRegex {
    fn moi(kieu: KieuRegex) -> Self {
        let p = match kieu {
            // key = value | "key" = "value"; (Apple .strings) – có \r? để đọc được file CRLF
            KieuRegex::Ini => {
                r#"(?m)^(?P<k>[ \t]*(?:"(?:[^"\\]|\\.)*"|[^\s=#;\[/][^=\r\n]*?)[ \t]*=[ \t]*)(?P<v>"(?:[^"\\]|\\.)*"|[^\r\n]*?)[ \t]*;?[ \t]*\r?$"#
            }
            KieuRegex::Yaml => {
                r#"(?m)^(?P<k>[ \t]*(?:- )?[\w.\-"' ]+:[ \t]+)(?P<v>(?!["']?\s*\r?$)[^|>&*!{\[\r\n#][^\r\n]*?)[ \t]*\r?$"#
            }
            KieuRegex::Txt => r"(?m)^(?P<l>[ \t]*)(?P<v>\S[^\r\n]*?)[ \t]*\r?$",
            KieuRegex::Rpy => {
                r#"(?m)^(?P<l>[ \t]*)(?:(?P<n>[A-Za-z_][\w.]*)[ \t]+)?"(?P<v>(?:[^"\\\r\n]|\\.)*)"(?P<s>[ \t]*(?:with[ \t]+\w+)?[ \t]*:?)[ \t]*\r?$"#
            }
        };
        Self {
            kieu,
            mau: FRegex::new(p).unwrap(),
        }
    }

    /// Giá trị chữ của nhóm "v" (None = bỏ).
    fn mo(&self, v: &str) -> Option<String> {
        match self.kieu {
            KieuRegex::Ini => Some(nhay_mo(v)),
            KieuRegex::Yaml => {
                let v = v.trim();
                if ["true", "false", "null", "~"].contains(&v) || v.parse::<f64>().is_ok() {
                    return None;
                }
                let c: Vec<char> = v.chars().collect();
                if c.len() >= 2 && c[0] == '\'' && c[c.len() - 1] == '\'' {
                    return Some(
                        c[1..c.len() - 1]
                            .iter()
                            .collect::<String>()
                            .replace("''", "'"),
                    );
                }
                if c.len() >= 2 && c[0] == '"' && c[c.len() - 1] == '"' {
                    return Some(c[1..c.len() - 1].iter().collect());
                }
                Some(v.to_string())
            }
            _ => Some(v.to_string()),
        }
    }

    fn dong(&self, vi: &str, g: &str) -> String {
        match self.kieu {
            KieuRegex::Ini => {
                let g = g.trim();
                let c: Vec<char> = g.chars().collect();
                if c.len() >= 2 && c[0] == c[c.len() - 1] && (c[0] == '"' || c[0] == '\'') {
                    format!("{q}{}{q}", thoat(vi, c[0]), q = c[0])
                } else {
                    vi.replace('\n', "\\n")
                }
            }
            KieuRegex::Yaml => {
                if g.trim().starts_with('\'') {
                    format!("'{}'", vi.replace('\'', "''"))
                } else {
                    format!("\"{}\"", thoat(vi, '"'))
                }
            }
            KieuRegex::Rpy => thoat(vi, '"'),
            KieuRegex::Txt => vi.to_string(),
        }
    }

    fn loc(&self, v: &str, ten: Option<&str>) -> bool {
        const BO_RPY: &[&str] = &[
            "old",
            "new",
            "voice",
            "play",
            "queue",
            "image",
            "show",
            "call",
            "jump",
            "style",
            "define",
            "default",
            "scene",
            "hide",
            "font",
            "translate",
            "label",
            "screen",
            "action",
            "text_font",
            "renpy",
        ];
        match self.kieu {
            KieuRegex::Txt => la_chu_anh(v) && v.split_whitespace().count() >= 2,
            KieuRegex::Rpy => !BO_RPY.contains(&ten.unwrap_or("")) && la_chu_anh(v),
            _ => la_chu_anh(v),
        }
    }
}

impl Dang for DangRegex {
    fn doc(&self, b: &[u8]) -> Result<Vec<(String, String)>> {
        let (chu, _, _) = giai_ma(b);
        let mut kq = Vec::new();
        for m in self.mau.captures_iter(&chu) {
            let m = m?;
            let g = m.name("v").unwrap();
            if let Some(v) = self.mo(g.as_str()) {
                if self.loc(&v, m.name("n").map(|x| x.as_str())) {
                    kq.push((g.start().to_string(), v));
                }
            }
        }
        Ok(kq)
    }

    fn ghi(&self, b: &[u8], dich: &mut HamDich) -> Result<Vec<u8>> {
        let (chu, mh, bom) = giai_ma(b);
        let mut ra = String::with_capacity(chu.len());
        let mut cuoi = 0;
        for m in self.mau.captures_iter(&chu) {
            let m = m?;
            let g = m.name("v").unwrap();
            let Some(v) = self.mo(g.as_str()) else {
                continue;
            };
            if !self.loc(&v, m.name("n").map(|x| x.as_str())) {
                continue;
            }
            let Some(vi) = dich(&v) else { continue };
            ra.push_str(&chu[cuoi..g.start()]);
            ra.push_str(&self.dong(&vi, g.as_str()));
            cuoi = g.end();
        }
        ra.push_str(&chu[cuoi..]);
        Ok(ma_hoa(&ra, mh, &bom))
    }
}

// ---------------- XML / RESX ----------------

struct DangXml {
    mau: FRegex,
}

fn xml_mo(v: &str) -> String {
    v.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#10;", "\n")
        .replace("&amp;", "&")
}

fn xml_dong(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl DangXml {
    fn moi() -> Self {
        Self {
            mau: FRegex::new(
                r#"(?s)<!\[CDATA\[(?P<c>.*?)\]\]>|(?<=>)(?P<t>[^<>]+)(?=<)|\b(?i:text|value|string|label|title|caption|desc|description|tooltip|content|message|displayname|display_name|hint|header|body)\s*=\s*"(?P<a>[^"]*)""#,
            )
            .unwrap(),
        }
    }
}

impl Dang for DangXml {
    fn doc(&self, b: &[u8]) -> Result<Vec<(String, String)>> {
        let (chu, _, _) = giai_ma(b);
        let mut kq = Vec::new();
        for m in self.mau.captures_iter(&chu) {
            let m = m?;
            let v = match (m.name("c"), m.name("t"), m.name("a")) {
                (Some(c), _, _) => c.as_str().to_string(),
                (_, Some(t), _) => xml_mo(t.as_str()),
                (_, _, Some(a)) => xml_mo(a.as_str()),
                _ => continue,
            };
            let s = v.trim();
            if !s.is_empty() && la_chu_anh(s) {
                kq.push((m.get(0).unwrap().start().to_string(), s.to_string()));
            }
        }
        Ok(kq)
    }

    fn ghi(&self, b: &[u8], dich: &mut HamDich) -> Result<Vec<u8>> {
        let (chu, mh, bom) = giai_ma(b);
        let mut ra = String::with_capacity(chu.len());
        let mut cuoi = 0;
        for m in self.mau.captures_iter(&chu) {
            let m = m?;
            let (g, ten) = match (m.name("c"), m.name("t"), m.name("a")) {
                (Some(c), _, _) => (c, 'c'),
                (_, Some(t), _) => (t, 't'),
                (_, _, Some(a)) => (a, 'a'),
                _ => continue,
            };
            let v = if ten == 'c' {
                g.as_str().to_string()
            } else {
                xml_mo(g.as_str())
            };
            let s = v.trim();
            if s.is_empty() || !la_chu_anh(s) {
                continue;
            }
            let Some(vi) = dich(s) else { continue };
            let moi = v.replacen(s, &vi, 1);
            let moi = match ten {
                'c' => moi,
                'a' => xml_dong(&moi),
                _ => xml_dong(&moi).replace("&quot;", "\""),
            };
            ra.push_str(&chu[cuoi..g.start()]);
            ra.push_str(&moi);
            cuoi = g.end();
        }
        ra.push_str(&chu[cuoi..]);
        Ok(ma_hoa(&ra, mh, &bom))
    }
}

// ---------------- Gettext PO ----------------

struct DangPo;

impl DangPo {
    /// → (các dòng, [(dòng msgid, dòng msgstr, dòng hết, chữ)])
    fn muc(chu: &str) -> (Vec<String>, Vec<(usize, usize, usize, String)>) {
        let dong: Vec<String> = chu.split_inclusive('\n').map(String::from).collect();
        static R: OnceLock<Regex> = OnceLock::new();
        let r = re(r#"^msgid\s+"(.*)"\s*$"#, &R);
        let mut kq = Vec::new();
        let mut i = 0;
        while i < dong.len() {
            if dong[i].starts_with("msgid ") {
                let a = i;
                let mut id = vec![r
                    .captures(dong[i].trim_end())
                    .map(|c| c[1].to_string())
                    .unwrap_or_default()];
                i += 1;
                while i < dong.len() && dong[i].trim_start().starts_with('"') {
                    let t = dong[i].trim();
                    id.push(t[1..t.len().saturating_sub(1).max(1)].to_string());
                    i += 1;
                }
                if i < dong.len() && dong[i].starts_with("msgstr") {
                    let s = i;
                    i += 1;
                    while i < dong.len() && dong[i].trim_start().starts_with('"') {
                        i += 1;
                    }
                    kq.push((a, s, i, id.concat()));
                }
                continue;
            }
            i += 1;
        }
        (dong, kq)
    }
}

impl Dang for DangPo {
    fn doc(&self, b: &[u8]) -> Result<Vec<(String, String)>> {
        let (chu, _, _) = giai_ma(b);
        Ok(DangPo::muc(&chu)
            .1
            .into_iter()
            .filter(|(_, _, _, v)| !v.is_empty() && la_chu_anh(v))
            .map(|(a, _, _, v)| (a.to_string(), v))
            .collect())
    }

    fn ghi(&self, b: &[u8], dich: &mut HamDich) -> Result<Vec<u8>> {
        let (chu, mh, bom) = giai_ma(b);
        let (mut dong, ds) = DangPo::muc(&chu);
        for (_, s, e, v) in ds.into_iter().rev() {
            if v.is_empty() || !la_chu_anh(&v) {
                continue;
            }
            let Some(vi) = dich(&v) else { continue };
            let xd = if dong[s].ends_with("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            dong.splice(s..e, [format!("msgstr \"{}\"{xd}", thoat(&vi, '"'))]);
        }
        Ok(ma_hoa(&dong.concat(), mh, &bom))
    }
}

// ---------------- CSV / TSV ----------------

struct DangCsv;

const COT_ANH: &[&str] = &[
    "en",
    "eng",
    "english",
    "en-us",
    "en_us",
    "en-gb",
    "en_gb",
    "text",
    "source",
    "value",
    "string",
    "english(en)",
    "english (en)",
    "en_text",
    "text_en",
    "englishtext",
];

struct BangCsv {
    chu: String,
    mh: MaHoa,
    bom: Vec<u8>,
    dl: u8,
    hang: Vec<Vec<String>>,
    xd: &'static str,
    cot: Option<usize>,
}

fn doan_dau_phan_cach(mau: &str) -> u8 {
    [b',', b';', b'\t', b'|']
        .into_iter()
        .max_by_key(|d| mau.matches(*d as char).count())
        .unwrap_or(b',')
}

impl DangCsv {
    fn nap(b: &[u8]) -> Result<BangCsv> {
        let (chu, mh, bom) = giai_ma(b);
        let mau: String = chu.chars().take(20000).collect();
        let dl = doan_dau_phan_cach(&mau);
        let mut rd = csv::ReaderBuilder::new()
            .delimiter(dl)
            .has_headers(false)
            .flexible(true)
            .from_reader(chu.as_bytes());
        let mut hang = Vec::new();
        for r in rd.records() {
            hang.push(r?.iter().map(String::from).collect::<Vec<_>>());
        }
        let xd = if chu.chars().take(5000).collect::<String>().contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let cot = hang.first().and_then(|dau| {
            dau.iter().position(|c| {
                let c = c.trim().to_lowercase();
                COT_ANH.contains(&c.as_str()) || c.contains("english")
            })
        });
        Ok(BangCsv {
            chu,
            mh,
            bom,
            dl,
            hang,
            xd,
            cot,
        })
    }

    fn o(b: &BangCsv) -> Vec<(usize, usize)> {
        let mut ra = Vec::new();
        for (r, h) in b.hang.iter().enumerate() {
            match b.cot {
                Some(c) => {
                    if r > 0 && c < h.len() {
                        ra.push((r, c));
                    }
                }
                None => ra.extend((0..h.len()).map(|c| (r, c))),
            }
        }
        ra
    }

    fn lay(b: &BangCsv, v: &str) -> bool {
        la_chu_anh(v) && (b.cot.is_some() || la_cau(v))
    }
}

impl Dang for DangCsv {
    fn doc(&self, b: &[u8]) -> Result<Vec<(String, String)>> {
        let bang = DangCsv::nap(b)?;
        Ok(DangCsv::o(&bang)
            .into_iter()
            .filter(|&(r, c)| DangCsv::lay(&bang, &bang.hang[r][c]))
            .map(|(r, c)| (format!("{r}:{c}"), bang.hang[r][c].clone()))
            .collect())
    }

    fn ghi(&self, b: &[u8], dich: &mut HamDich) -> Result<Vec<u8>> {
        let mut bang = DangCsv::nap(b)?;
        for (r, c) in DangCsv::o(&bang) {
            let v = bang.hang[r][c].clone();
            if DangCsv::lay(&bang, &v) {
                if let Some(vi) = dich(&v) {
                    bang.hang[r][c] = vi;
                }
            }
        }
        let mut wr = csv::WriterBuilder::new()
            .delimiter(bang.dl)
            .flexible(true)
            .terminator(if bang.xd == "\r\n" {
                csv::Terminator::CRLF
            } else {
                csv::Terminator::Any(b'\n')
            })
            .from_writer(Vec::new());
        for h in &bang.hang {
            wr.write_record(h)?;
        }
        let mut moi = String::from_utf8(wr.into_inner().map_err(|e| anyhow!("{e}"))?)?;
        if !bang.chu.ends_with(['\n', '\r']) {
            moi = moi.trim_end_matches(['\r', '\n']).to_string();
        }
        Ok(ma_hoa(&moi, bang.mh, &bang.bom))
    }
}

// ---------------- JSON (cả RPG Maker MV/MZ) ----------------

struct DangJson;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
enum Khoa {
    Ten(String),
    So(usize),
}

impl DangJson {
    fn duyet(o: &Value, duong: &mut Vec<Khoa>, ra: &mut Vec<(Vec<Khoa>, String)>) {
        match o {
            Value::Object(m) => {
                let am = ["volume", "pitch", "pan"]
                    .iter()
                    .all(|k| m.contains_key(*k));
                let ma = m.get("code").and_then(Value::as_i64);
                for (k, v) in m {
                    let kl = k.to_lowercase();
                    if KHOA_BO.contains(&kl.as_str()) || (am && kl == "name") {
                        continue;
                    }
                    if ma.is_some_and(|c| MA_RPG_BO.contains(&c)) && kl == "parameters" {
                        continue;
                    }
                    duong.push(Khoa::Ten(k.clone()));
                    DangJson::duyet(v, duong, ra);
                    duong.pop();
                }
            }
            Value::Array(a) => {
                for (i, v) in a.iter().enumerate() {
                    duong.push(Khoa::So(i));
                    DangJson::duyet(v, duong, ra);
                    duong.pop();
                }
            }
            Value::String(s) if la_chu_anh(s) => ra.push((duong.clone(), s.clone())),
            _ => {}
        }
    }

    fn nap(b: &[u8]) -> Result<(String, MaHoa, Vec<u8>, Value)> {
        let (chu, mh, bom) = giai_ma(b);
        let o: Value = serde_json::from_str(&chu)?;
        Ok((chu, mh, bom, o))
    }
}

/// Định dạng ", " / ": " (giống json.dumps mặc định của Python).
struct CachRong;
impl serde_json::ser::Formatter for CachRong {
    fn begin_array_value<W: ?Sized + std::io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> std::io::Result<()> {
        if first {
            Ok(())
        } else {
            w.write_all(b", ")
        }
    }
    fn begin_object_key<W: ?Sized + std::io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> std::io::Result<()> {
        if first {
            Ok(())
        } else {
            w.write_all(b", ")
        }
    }
    fn begin_object_value<W: ?Sized + std::io::Write>(&mut self, w: &mut W) -> std::io::Result<()> {
        w.write_all(b": ")
    }
}

impl Dang for DangJson {
    fn doc(&self, b: &[u8]) -> Result<Vec<(String, String)>> {
        let (_, _, _, o) = DangJson::nap(b)?;
        let mut ra = Vec::new();
        DangJson::duyet(&o, &mut Vec::new(), &mut ra);
        Ok(ra
            .into_iter()
            .map(|(d, v)| (serde_json::to_string(&d).unwrap_or_default(), v))
            .collect())
    }

    fn ghi(&self, b: &[u8], dich: &mut HamDich) -> Result<Vec<u8>> {
        let (chu, mh, bom, mut o) = DangJson::nap(b)?;
        let mut ra = Vec::new();
        DangJson::duyet(&o, &mut Vec::new(), &mut ra);
        let mut doi = 0;
        for (d, v) in ra {
            let Some(vi) = dich(&v) else { continue };
            let mut x = &mut o;
            for k in &d {
                x = match k {
                    Khoa::Ten(t) => &mut x[t.as_str()],
                    Khoa::So(i) => &mut x[*i],
                };
            }
            *x = Value::String(vi);
            doi += 1;
        }
        if doi == 0 {
            return Ok(b.to_vec());
        }
        static R: OnceLock<Regex> = OnceLock::new();
        let mut moi = if let Some(m) = re(r#"\n([ \t]+)["{\[]"#, &R).captures(&chu) {
            let thut = m[1].to_string();
            let thut = if thut.starts_with('\t') {
                thut
            } else {
                " ".repeat(thut.len())
            };
            let mut buf = Vec::new();
            let mut ser = serde_json::Serializer::with_formatter(
                &mut buf,
                serde_json::ser::PrettyFormatter::with_indent(thut.as_bytes()),
            );
            o.serialize(&mut ser)?;
            let s = String::from_utf8(buf)?;
            if chu.chars().take(2000).collect::<String>().contains("\r\n") {
                s.replace('\n', "\r\n")
            } else {
                s
            }
        } else if chu.matches("\":").count() > chu.matches("\": ").count() {
            serde_json::to_string(&o)?
        } else {
            let mut buf = Vec::new();
            let mut ser = serde_json::Serializer::with_formatter(&mut buf, CachRong);
            o.serialize(&mut ser)?;
            String::from_utf8(buf)?
        };
        if chu.ends_with('\n') {
            moi.push_str(if chu.ends_with("\r\n") { "\r\n" } else { "\n" });
        }
        Ok(ma_hoa(&moi, mh, &bom))
    }
}

// ---------------- Unreal .locres ----------------

const MAGIC_LOCRES: [u8; 16] = [
    0x0E, 0x14, 0x74, 0x75, 0x67, 0x4A, 0x03, 0xFC, 0x4A, 0x15, 0x90, 0x9D, 0xC3, 0x37, 0x7F, 0x1B,
];

struct Doc<'a> {
    b: &'a [u8],
    p: usize,
}

impl Doc<'_> {
    fn lay<const N: usize>(&mut self) -> Result<[u8; N]> {
        let s = self
            .b
            .get(self.p..self.p + N)
            .ok_or_else(|| anyhow!("file .locres bị cụt"))?;
        self.p += N;
        Ok(s.try_into().unwrap())
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.lay()?))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.lay()?))
    }
    fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.lay()?))
    }
    fn fstr(&mut self) -> Result<String> {
        let n = self.i32()?;
        if n == 0 {
            return Ok(String::new());
        }
        let s = if n < 0 {
            let dai = (-n as usize) * 2;
            let b = self
                .b
                .get(self.p..self.p + dai)
                .ok_or_else(|| anyhow!("file .locres bị cụt"))?;
            self.p += dai;
            encoding_rs::UTF_16LE
                .decode_without_bom_handling(b)
                .0
                .into_owned()
        } else {
            let b = self
                .b
                .get(self.p..self.p + n as usize)
                .ok_or_else(|| anyhow!("file .locres bị cụt"))?;
            self.p += n as usize;
            encoding_rs::WINDOWS_1252
                .decode_without_bom_handling(b)
                .0
                .into_owned()
        };
        Ok(s.strip_suffix('\0').map(String::from).unwrap_or(s))
    }
}

fn ghi_fstr(s: &str) -> Vec<u8> {
    if s.is_empty() {
        return 0i32.to_le_bytes().to_vec();
    }
    if s.is_ascii() {
        let mut d = s.as_bytes().to_vec();
        d.push(0);
        let mut ra = (d.len() as i32).to_le_bytes().to_vec();
        ra.extend(d);
        return ra;
    }
    let mut d: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
    d.extend([0, 0]);
    let mut ra = (-((d.len() / 2) as i32)).to_le_bytes().to_vec();
    ra.extend(d);
    ra
}

struct DangLocres;

enum Locres {
    /// bản cũ: giá trị nằm ngay trong bảng – [(đầu, cuối, chữ)]
    Cu(Vec<(usize, usize, String)>),
    Moi {
        ban: u8,
        vt_mang: usize,
        mang: Vec<(String, Option<i32>)>,
        het: usize,
    },
}

impl DangLocres {
    fn nap(b: &[u8]) -> Result<Locres> {
        let mut r = Doc { b, p: 0 };
        let mut ban = 0u8;
        if b.len() > 17 && b[..16] == MAGIC_LOCRES {
            ban = b[16];
            r.p = 17;
        }
        if ban == 0 {
            let mut kq = Vec::new();
            let so_ns = r.u32()?;
            for _ in 0..so_ns {
                r.fstr()?;
                let so = r.u32()?;
                for _ in 0..so {
                    r.fstr()?;
                    r.u32()?;
                    let vt = r.p;
                    let v = r.fstr()?;
                    kq.push((vt, r.p, v));
                }
            }
            return Ok(Locres::Cu(kq));
        }
        let vt_mang = r.i64()? as usize;
        let mut r2 = Doc { b, p: vt_mang };
        let so = r2.i32()?;
        let mut mang = Vec::new();
        for _ in 0..so {
            let v = r2.fstr()?;
            let dem = if ban >= 2 { Some(r2.i32()?) } else { None };
            mang.push((v, dem));
        }
        Ok(Locres::Moi {
            ban,
            vt_mang,
            mang,
            het: r2.p,
        })
    }
}

impl Dang for DangLocres {
    fn doc(&self, b: &[u8]) -> Result<Vec<(String, String)>> {
        Ok(match DangLocres::nap(b)? {
            Locres::Cu(o) => o
                .into_iter()
                .enumerate()
                .filter(|(_, x)| la_chu_anh(&x.2))
                .map(|(i, x)| (i.to_string(), x.2))
                .collect(),
            Locres::Moi { mang, .. } => mang
                .into_iter()
                .enumerate()
                .filter(|(_, x)| la_chu_anh(&x.0))
                .map(|(i, x)| (i.to_string(), x.0))
                .collect(),
        })
    }

    fn ghi(&self, b: &[u8], dich: &mut HamDich) -> Result<Vec<u8>> {
        let mut doi = |v: &str| -> String {
            if !la_chu_anh(v) {
                return v.to_string();
            }
            dich(v).unwrap_or_else(|| v.to_string())
        };
        Ok(match DangLocres::nap(b)? {
            Locres::Cu(o) => {
                let mut ra = Vec::with_capacity(b.len());
                let mut cu = 0;
                for (a, z, v) in o {
                    ra.extend_from_slice(&b[cu..a]);
                    ra.extend(ghi_fstr(&doi(&v)));
                    cu = z;
                }
                ra.extend_from_slice(&b[cu..]);
                ra
            }
            Locres::Moi {
                vt_mang,
                mang,
                het,
                ban: _,
            } => {
                let mut m = (mang.len() as i32).to_le_bytes().to_vec();
                for (v, dem) in mang {
                    m.extend(ghi_fstr(&doi(&v)));
                    if let Some(d) = dem {
                        m.extend(d.to_le_bytes());
                    }
                }
                let mut ra = b[..vt_mang].to_vec();
                ra.extend(m);
                ra.extend_from_slice(&b[het..]);
                ra
            }
        })
    }
}

/// Định dạng theo loại.
pub fn dang(loai: &str) -> Option<&'static dyn Dang> {
    struct Tat {
        ini: DangRegex,
        yaml: DangRegex,
        txt: DangRegex,
        rpy: DangRegex,
        xml: DangXml,
    }
    static T: OnceLock<Tat> = OnceLock::new();
    let t = T.get_or_init(|| Tat {
        ini: DangRegex::moi(KieuRegex::Ini),
        yaml: DangRegex::moi(KieuRegex::Yaml),
        txt: DangRegex::moi(KieuRegex::Txt),
        rpy: DangRegex::moi(KieuRegex::Rpy),
        xml: DangXml::moi(),
    });
    Some(match loai {
        "json" => &DangJson,
        "csv" => &DangCsv,
        "xml" => &t.xml,
        "ini" => &t.ini,
        "po" => &DangPo,
        "yaml" => &t.yaml,
        "txt" => &t.txt,
        "rpy" => &t.rpy,
        "locres" => &DangLocres,
        _ => return None,
    })
}

// ---------------- quét thư mục game ----------------

pub fn nhan_dien_engine(thu_muc: &Path) -> &'static str {
    let ten: HashSet<String> = std::fs::read_dir(thu_muc)
        .into_iter()
        .flatten()
        .flatten()
        .map(|f| f.file_name().to_string_lossy().to_lowercase())
        .collect();
    if ten.iter().any(|t| t.ends_with("_data")) || ten.contains("unityplayer.dll") {
        return "Unity";
    }
    if ten.contains("engine") || ten.iter().any(|t| t.ends_with(".uproject")) {
        return "Unreal Engine";
    }
    if ten.contains("renpy") || (ten.contains("game") && ten.contains("lib")) {
        return "Ren'Py";
    }
    if ten.contains("www")
        || (ten.contains("data") && ten.contains("js"))
        || ten
            .iter()
            .any(|t| t.ends_with(".rgss3a") || t.ends_with(".rgss2a") || t.ends_with(".rgssad"))
    {
        return "RPG Maker";
    }
    if ten.iter().any(|t| t.ends_with(".pck")) {
        return "Godot";
    }
    ""
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FileChu {
    pub tuong_doi: String,
    pub loai: String,
    pub co: u64,
    pub so: usize,
    pub so_cau: usize,
    pub anh: f64,
    pub mau: Vec<String>,
    #[serde(default)]
    pub mtime: f64,
    #[serde(default)]
    pub diem: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GoiNen {
    pub loai: String,
    pub so: usize,
    pub co: u64,
    pub vi_du: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct KetQuaQuet {
    pub phien_ban: u32,
    pub thu_muc: String,
    pub engine: String,
    pub luc: String,
    pub file: Vec<FileChu>,
    pub goi: Vec<GoiNen>,
    pub so_tep: usize,
    #[serde(default)]
    pub chon: Vec<String>,
}

fn duoi(p: &Path) -> String {
    p.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

fn tuong_doi(p: &Path, goc: &Path) -> String {
    p.strip_prefix(goc).unwrap_or(p).display().to_string()
}

/// Quét thư mục game: các file chữ (xếp theo điểm) và các gói nén chưa hỗ trợ.
pub fn quet(thu_muc_game: &Path, bao: &dyn Fn(&str), dung: &dyn Fn() -> bool) -> KetQuaQuet {
    static BO_TEN: OnceLock<Regex> = OnceLock::new();
    let bo_ten = re(
        r"(?i)(readme|license|licence|eula|credits|changelog|third.?party|copying|notice|patch.?notes|steam_appid|unins|version)",
        &BO_TEN,
    );
    let mut ds: Vec<FileChu> = Vec::new();
    let mut goi: BTreeMap<String, GoiNen> = BTreeMap::new();
    let mut dem = 0usize;
    let mut moc = Instant::now();
    let mut ngan = vec![thu_muc_game.to_path_buf()];
    'het: while let Some(thu) = ngan.pop() {
        let Ok(rd) = std::fs::read_dir(&thu) else {
            continue;
        };
        let mut con = Vec::new();
        for f in rd.flatten() {
            let p = f.path();
            let Ok(kieu) = f.file_type() else { continue };
            if kieu.is_dir() {
                if !BO_THU_MUC.contains(&f.file_name().to_string_lossy().to_lowercase().as_str()) {
                    con.push(p);
                }
                continue;
            }
            dem += 1;
            if dem > 200_000 || dung() {
                break 'het;
            }
            if moc.elapsed().as_millis() > 500 {
                moc = Instant::now();
                bao(&format!(
                    "Đang dò: {dem} file – {}",
                    tuong_doi(&p, thu_muc_game)
                ));
            }
            let d = duoi(&p);
            let co = f.metadata().map(|m| m.len()).unwrap_or(0);
            if let Some(ten_goi) = goi_nen(&d) {
                let g = goi.entry(ten_goi.into()).or_insert_with(|| GoiNen {
                    loai: ten_goi.into(),
                    ..Default::default()
                });
                g.so += 1;
                g.co += co;
                if g.vi_du.len() < 3 {
                    g.vi_du.push(tuong_doi(&p, thu_muc_game));
                }
                continue;
            }
            let Some(loai) = loai_theo_duoi(&d) else {
                continue;
            };
            if bo_ten.is_match(&f.file_name().to_string_lossy()) || !(8..=60 << 20).contains(&co) {
                continue;
            }
            let Ok(b) = std::fs::read(&p) else { continue };
            let Ok(cac) = dang(loai).unwrap().doc(&b) else {
                continue;
            };
            if cac.is_empty() {
                continue;
            }
            let chu: Vec<&str> = cac.iter().map(|(_, v)| v.as_str()).collect();
            let so_cau = chu.iter().filter(|v| la_cau(v)).count();
            let anh = ty_le_anh(&chu);
            if loai == "txt" && (cac.len() < 5 || anh < 0.04) {
                continue;
            }
            let mut mau: Vec<String> = chu
                .iter()
                .filter(|v| la_cau(v))
                .map(|v| v.replace('\n', " ").chars().take(120).collect())
                .collect::<HashSet<String>>()
                .into_iter()
                .collect();
            if mau.is_empty() {
                mau = chu
                    .iter()
                    .map(|v| v.chars().take(120).collect())
                    .collect::<HashSet<String>>()
                    .into_iter()
                    .collect();
            }
            mau.sort_by_key(|v| std::cmp::Reverse(v.chars().count()));
            mau.truncate(3);
            let mtime = f
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            ds.push(FileChu {
                tuong_doi: tuong_doi(&p, thu_muc_game),
                loai: loai.into(),
                co,
                so: cac.len(),
                so_cau,
                anh: (anh * 1000.0).round() / 1000.0,
                mau,
                mtime,
                diem: 0.0,
            });
        }
        ngan.extend(con);
    }
    static EN: OnceLock<Regex> = OnceLock::new();
    let en = re(r"(^|[\\/_.\-])(en|eng|english|en[-_]us)([\\/_.\-]|$)", &EN);
    for f in &mut ds {
        let mut diem = (f.so + 3 * f.so_cau) as f64;
        if en.is_match(&f.tuong_doi.to_lowercase()) {
            diem *= 2.0;
        }
        if f.anh < 0.03 {
            diem *= 0.2;
        }
        f.diem = diem;
    }
    ds.sort_by(|a, b| b.diem.total_cmp(&a.diem));
    let mut goi: Vec<GoiNen> = goi.into_values().collect();
    goi.sort_by_key(|g| std::cmp::Reverse(g.co));
    KetQuaQuet {
        phien_ban: PHIEN_BAN_QUET,
        thu_muc: thu_muc_game.display().to_string(),
        engine: nhan_dien_engine(thu_muc_game).into(),
        luc: duong_dan::gio_dia_phuong(),
        file: ds,
        goi,
        so_tep: dem,
        chon: Vec::new(),
    }
}

// ---------------- việc dịch file của một game ----------------

pub const HUONG_DAN_CHUNG: &str = "Bạn là dịch giả chuyên Việt hóa game. Game: {ten}.
Bạn nhận một mảng JSON các chuỗi tiếng Anh lấy từ file chữ của game và trả về mảng JSON các bản dịch tiếng Việt.

Quy tắc bắt buộc:
- CHỈ trả về một mảng JSON hợp lệ, CÙNG SỐ PHẦN TỬ, CÙNG THỨ TỰ. Không giải thích, không ```.
- GIỮ NGUYÊN tiếng Anh: tên người, địa danh, tổ chức, phe phái, tên riêng của vật phẩm / kỹ năng, và mọi từ trong danh sách \"Giữ nguyên\" nếu có.
- Giữ NGUYÊN từng ký tự mọi thẻ và mã: <...>, {...}, [...], %s, %d, %1, \\n, \\c[1], $..., kể cả khoảng trắng quanh chúng. Không dịch, không thêm, không bớt thẻ.
- Giữ nguyên số, ký hiệu, khoảng trắng đầu/cuối và cách xuống dòng.
- Nếu một chuỗi là mã kỹ thuật, tên file hay từ khóa lập trình thì trả về NGUYÊN VĂN.
- Văn phong tự nhiên như game Việt hóa chính thức; menu / nút bấm ngắn gọn; xưng hô hợp ngữ cảnh.
";

/// Thư mục dữ liệu của game (giống thu_muc_viec của app cũ: tên + md5 đường dẫn).
pub fn thu_muc_viec(thu_muc_game: &Path) -> PathBuf {
    static R: OnceLock<Regex> = OnceLock::new();
    let ten = thu_muc_game
        .file_name()
        .map(|t| t.to_string_lossy().to_string())
        .unwrap_or_default();
    let ten: String = re(r"[^\w\-]+", &R)
        .replace_all(&ten, "_")
        .chars()
        .take(40)
        .collect();
    let chuan = thu_muc_game
        .display()
        .to_string()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase();
    let bam = format!("{:x}", md5::compute(chuan.as_bytes()));
    duong_dan::viet_hoa_game()
        .join("do_file")
        .join(format!("{ten}_{}", &bam[..8]))
}

fn md5_file(p: &Path) -> Result<String> {
    Ok(format!("{:x}", md5::compute(std::fs::read(p)?)))
}

fn doc_json<T: for<'a> Deserialize<'a> + Default>(p: &Path) -> T {
    std::fs::read(p)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn ghi_json<T: Serialize>(p: &Path, d: &T) -> Result<()> {
    duong_dan::ghi_file(p, &serde_json::to_vec(d)?)?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DaGhi {
    pub md5_goc: String,
    pub md5_viet: String,
    pub cau: usize,
    pub luc: String,
    #[serde(default)]
    pub khong_dau: bool,
}

/// Dịch các file chữ người dùng chọn trong một game bất kỳ (giống ViecFile của app cũ).
pub struct ViecFile {
    pub thu_muc_game: PathBuf,
    pub ten_game: String,
    pub tm: PathBuf,
    pub viec: ViecDich,
    pub kq_quet: Option<KetQuaQuet>,
    pub chon: Vec<String>,
    /// tương đối → [(khóa, chữ Anh)]
    pub chu: BTreeMap<String, Vec<(String, String)>>,
    pub loi_doc: Option<String>,
}

impl ViecFile {
    pub fn moi(thu_muc_game: &Path, ten_game: &str) -> Self {
        let tm = thu_muc_viec(thu_muc_game);
        let ten_game = if ten_game.is_empty() {
            thu_muc_game
                .file_name()
                .map(|t| t.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            ten_game.to_string()
        };
        let viec = ViecDich::moi(
            tm.clone(),
            &HUONG_DAN_CHUNG.replace("{ten}", &ten_game),
            BangTen::default(),
        );
        let mut kq_quet: Option<KetQuaQuet> = std::fs::read(tm.join("ket_qua_do.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        if kq_quet
            .as_ref()
            .is_some_and(|k| k.phien_ban != PHIEN_BAN_QUET)
        {
            kq_quet = None;
        }
        let chon = kq_quet.as_ref().map(|k| k.chon.clone()).unwrap_or_default();
        Self {
            thu_muc_game: thu_muc_game.to_path_buf(),
            ten_game,
            tm,
            viec,
            kq_quet,
            chon,
            chu: BTreeMap::new(),
            loi_doc: None,
        }
    }

    pub fn quet(&mut self, bao: &dyn Fn(&str), dung: &dyn Fn() -> bool) -> &KetQuaQuet {
        let kq = quet(&self.thu_muc_game, bao, dung);
        // tự chọn sẵn các file tốt nhất (tiếng Anh rõ ràng), bỏ file mang mã ngôn ngữ khác (BR.ini, fr.json...)
        static KHAC: OnceLock<Regex> = OnceLock::new();
        let khac = re(
            r"(?i)(^|[\\/_.\-])(br|fr|fre|french|de|ger|german|es|spa|spanish|it|ita|pl|pol|ru|rus|cz|cs|hu|tr|kr|ko|jp|ja|cn|zh|tw|pt|nl|sv|ar)([\\/_.\-]|$)",
            &KHAC,
        );
        self.chon = kq
            .file
            .iter()
            .take(40)
            .filter(|f| f.anh >= 0.03 && f.diem >= 10.0 && !khac.is_match(&f.tuong_doi))
            .take(5)
            .map(|f| f.tuong_doi.clone())
            .collect();
        self.kq_quet = Some(kq);
        self.luu_quet();
        self.kq_quet.as_ref().unwrap()
    }

    pub fn luu_quet(&mut self) {
        if let Some(k) = self.kq_quet.as_mut() {
            k.chon = self.chon.clone();
            let _ = ghi_json(&self.tm.join("ket_qua_do.json"), k);
        }
    }

    fn file_ghi(&self) -> PathBuf {
        self.tm.join("da_ghi.json")
    }

    fn sao_luu(&self, td: &str) -> PathBuf {
        self.tm.join("sao_luu").join(td)
    }

    pub fn da_ghi(&self) -> BTreeMap<String, DaGhi> {
        doc_json(&self.file_ghi())
    }

    /// Bản gốc của file (bản sao lưu nếu đã từng ghi Việt hóa; game cập nhật thì file mới là bản gốc).
    pub fn file_goc(&self, td: &str) -> PathBuf {
        let sl = self.sao_luu(td);
        let p = self.thu_muc_game.join(td);
        if let (true, Some(rec)) = (sl.is_file(), self.da_ghi().get(td)) {
            if let Ok(m) = md5_file(&p) {
                if m == rec.md5_viet {
                    return sl; // file trong game vẫn là bản app đã ghi
                }
                if m != rec.md5_goc {
                    return p; // game đã cập nhật → file mới là bản gốc
                }
            }
            return sl;
        }
        p
    }

    pub fn doc_chon(&mut self) -> &BTreeMap<String, Vec<(String, String)>> {
        self.chu.clear();
        self.loi_doc = None;
        for td in self.chon.clone() {
            let Some(loai) = loai_theo_duoi(&duoi(Path::new(&td))) else {
                continue;
            };
            let kq = std::fs::read(self.file_goc(&td))
                .map_err(anyhow::Error::from)
                .and_then(|b| dang(loai).unwrap().doc(&b));
            match kq {
                Ok(v) => {
                    self.chu.insert(td, v);
                }
                Err(e) => {
                    self.loi_doc = Some(format!("{td}: {e}"));
                    self.chu.insert(td, Vec::new());
                }
            }
        }
        &self.chu
    }

    /// Các câu tiếng Anh của các file đã chọn (theo thứ tự, có trùng).
    pub fn cac_cau(&self) -> Vec<String> {
        self.chu
            .values()
            .flatten()
            .map(|(_, v)| v.clone())
            .collect()
    }

    pub fn thong_ke(&self) -> (usize, usize, usize, usize) {
        let tat_ca: HashSet<&str> = self
            .chu
            .values()
            .flatten()
            .map(|(_, v)| v.as_str())
            .collect();
        let bn = self.viec.bo_nho.lock().unwrap_or_else(|e| e.into_inner());
        let da = tat_ca.iter().filter(|v| bn.contains_key(**v)).count();
        let ky_con: usize = tat_ca
            .iter()
            .filter(|v| !bn.contains_key(**v))
            .map(|v| v.chars().count())
            .sum();
        (tat_ca.len(), da, tat_ca.len() - da, ky_con)
    }

    fn ham_dich(&self, khong_dau: bool) -> impl Fn(&str) -> Option<String> + '_ {
        move |en: &str| {
            let vi = self.viec.tim(en)?;
            if vi.is_empty() || vi == en || the_cua(en) != the_cua(&vi) {
                return None;
            }
            Some(if khong_dau { bo_dau(&vi) } else { vi })
        }
    }

    /// Ghi bản dịch vào game (sao lưu bản gốc lần đầu). Trả số câu đã thay.
    pub fn ghi_vao_game(&self, khong_dau: bool, bao: &dyn Fn(&str)) -> Result<usize> {
        let mut rec_all = self.da_ghi();
        let dich = self.ham_dich(khong_dau);
        let mut tong = 0;
        for td in &self.chon {
            let p = self.thu_muc_game.join(td);
            let Some(loai) = loai_theo_duoi(&duoi(Path::new(td))) else {
                continue;
            };
            let nguon = self.file_goc(td);
            let goc = std::fs::read(&nguon)?;
            let sl = self.sao_luu(td);
            if nguon == p || !sl.is_file() {
                std::fs::create_dir_all(sl.parent().unwrap())?;
                std::fs::copy(&p, &sl)?;
            }
            let mut dem = 0;
            let moi = dang(loai).unwrap().ghi(&goc, &mut |en: &str| {
                let vi = dich(en);
                if vi.is_some() {
                    dem += 1;
                }
                vi
            })?;
            dang(loai)
                .unwrap()
                .doc(&moi)
                .map_err(|e| anyhow!("file mới không đọc lại được ({e}) – không ghi {td}"))?;
            bao(&format!("Đang ghi {td} ({dem} câu)..."));
            duong_dan::ghi_file(&p, &moi)?;
            rec_all.insert(
                td.clone(),
                DaGhi {
                    md5_goc: format!("{:x}", md5::compute(&goc)),
                    md5_viet: format!("{:x}", md5::compute(&moi)),
                    cau: dem,
                    luc: duong_dan::gio_dia_phuong(),
                    khong_dau,
                },
            );
            tong += dem;
        }
        ghi_json(&self.file_ghi(), &rec_all)?;
        Ok(tong)
    }

    /// Trả lại bản gốc cho mọi file đã ghi. Trả số file.
    pub fn tra_lai(&self) -> Result<usize> {
        let rec_all = self.da_ghi();
        let mut so = 0;
        for td in rec_all.keys() {
            let sl = self.sao_luu(td);
            if sl.is_file() {
                std::fs::copy(&sl, self.thu_muc_game.join(td))?;
                so += 1;
            }
        }
        ghi_json(&self.file_ghi(), &BTreeMap::<String, DaGhi>::new())?;
        Ok(so)
    }

    /// Xuất CSV (UTF-8 BOM) để tự dịch bằng Excel. Trả số câu.
    pub fn xuat_csv(&self, duong: &Path) -> Result<usize> {
        let mut wr = csv::WriterBuilder::new()
            .terminator(csv::Terminator::CRLF)
            .from_writer(Vec::new());
        wr.write_record(["File", "Khóa", "Tiếng Anh", "Tiếng Việt"])?;
        let mut da = HashSet::new();
        let mut n = 0;
        for (td, ds) in &self.chu {
            for (k, v) in ds {
                if !da.insert(v.clone()) {
                    continue;
                }
                wr.write_record([td.as_str(), k, v, &self.viec.tim(v).unwrap_or_default()])?;
                n += 1;
            }
        }
        let mut b = b"\xEF\xBB\xBF".to_vec();
        b.extend(wr.into_inner().map_err(|e| anyhow!("{e}"))?);
        duong_dan::ghi_file(duong, &b)?;
        Ok(n)
    }

    /// Nạp CSV đã dịch (cột Tiếng Anh / Tiếng Việt). Trả số câu.
    pub fn nap_csv(&self, duong: &Path) -> Result<usize> {
        let (chu, _, _) = giai_ma(&std::fs::read(duong)?);
        let mau: String = chu.chars().take(5000).collect();
        let (tab, phay, cham_phay) = (
            mau.matches('\t').count(),
            mau.matches(',').count(),
            mau.matches(';').count(),
        );
        let dl = if tab > phay.max(cham_phay) {
            b'\t'
        } else if cham_phay > phay {
            b';'
        } else {
            b','
        };
        let mut rd = csv::ReaderBuilder::new()
            .delimiter(dl)
            .has_headers(false)
            .flexible(true)
            .from_reader(chu.as_bytes());
        let hang: Vec<Vec<String>> = rd
            .records()
            .filter_map(|r| r.ok())
            .map(|r| r.iter().map(String::from).collect())
            .collect();
        let Some(dau) = hang.first() else {
            return Ok(0);
        };
        let dau: Vec<String> = dau.iter().map(|c| c.trim().to_lowercase()).collect();
        let ca = dau
            .iter()
            .position(|c| c.contains("anh") || c.contains("english"))
            .unwrap_or(2);
        let cv = dau
            .iter()
            .position(|c| c.contains("việt") || c.contains("viet"))
            .unwrap_or(3);
        let mut so = 0;
        {
            let mut bn = self.viec.bo_nho.lock().unwrap_or_else(|e| e.into_inner());
            for h in &hang[1..] {
                if h.len() > ca.max(cv) && !h[ca].is_empty() && !h[cv].trim().is_empty() {
                    bn.insert(h[ca].clone(), h[cv].clone());
                    so += 1;
                }
            }
        }
        self.viec.luu()?;
        Ok(so)
    }
}

pub fn kiem_tra_loai(td: &str) -> Result<&'static str> {
    loai_theo_duoi(&duoi(Path::new(td))).ok_or_else(|| anyhow!("không đọc được loại file này"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn khu_hoi(loai: &str, goc: &str, cho: &[(&str, &str)]) -> (Vec<String>, String) {
        let d = dang(loai).unwrap();
        let doc: Vec<String> = d
            .doc(goc.as_bytes())
            .unwrap()
            .into_iter()
            .map(|(_, v)| v)
            .collect();
        let bang: HashMap<&str, &str> = cho.iter().cloned().collect();
        let moi = d
            .ghi(goc.as_bytes(), &mut |en: &str| {
                bang.get(en).map(|v| v.to_string())
            })
            .unwrap();
        (doc, String::from_utf8(moi).unwrap())
    }

    #[test]
    fn json_rpg_maker() {
        let goc = "{\n  \"name\": \"Hero\",\n  \"message\": \"Welcome to the village!\",\n  \"icon\": \"Ui Icon\",\n  \"list\": [{\"code\": 401, \"parameters\": [\"You found a sword.\"]}, {\"code\": 355, \"parameters\": [\"Do not touch this script\"]}]\n}\n";
        let (doc, moi) = khu_hoi(
            "json",
            goc,
            &[
                ("Welcome to the village!", "Chào mừng tới làng!"),
                ("You found a sword.", "Bạn tìm thấy kiếm."),
            ],
        );
        assert!(
            doc.contains(&"Welcome to the village!".to_string())
                && doc.contains(&"You found a sword.".to_string())
        );
        assert!(!doc.iter().any(|v| v.contains("script") || v == "Ui Icon"));
        assert!(
            moi.contains("\"message\": \"Chào mừng tới làng!\"")
                && moi.contains("Bạn tìm thấy kiếm.")
                && moi.ends_with("}\n")
        );
    }

    #[test]
    fn ini_po_csv_xml_yaml_rpy() {
        let (doc, moi) = khu_hoi(
            "ini",
            "[UI]\r\nStart=Start the game\r\nQuit = \"Quit to desktop\"\r\nid=menu_main\r\n",
            &[
                ("Start the game", "Bắt đầu"),
                ("Quit to desktop", "Thoát \"ra\" desktop"),
            ],
        );
        assert_eq!(doc, vec!["Start the game", "Quit to desktop"]);
        assert_eq!(
            moi,
            "[UI]\r\nStart=Bắt đầu\r\nQuit = \"Thoát \\\"ra\\\" desktop\"\r\nid=menu_main\r\n"
        );

        let (doc, moi) = khu_hoi(
            "po",
            "msgid \"Open the door\"\nmsgstr \"\"\n",
            &[("Open the door", "Mở cửa")],
        );
        assert_eq!(doc, vec!["Open the door"]);
        assert_eq!(moi, "msgid \"Open the door\"\nmsgstr \"Mở cửa\"\n");

        let (doc, moi) = khu_hoi(
            "csv",
            "key,english\nk1,Hello world\nk2,\"Go, now\"\n",
            &[("Hello world", "Xin chào"), ("Go, now", "Đi, ngay")],
        );
        assert_eq!(doc, vec!["Hello world", "Go, now"]);
        assert_eq!(moi, "key,english\nk1,Xin chào\nk2,\"Đi, ngay\"\n");

        let (doc, moi) = khu_hoi(
            "xml",
            "<root><text>Press any key</text><item label=\"Save &amp; Exit\"/></root>",
            &[
                ("Press any key", "Nhấn phím bất kỳ"),
                ("Save & Exit", "Lưu & Thoát"),
            ],
        );
        assert_eq!(doc, vec!["Press any key", "Save & Exit"]);
        assert_eq!(
            moi,
            "<root><text>Nhấn phím bất kỳ</text><item label=\"Lưu &amp; Thoát\"/></root>"
        );

        let (doc, moi) = khu_hoi(
            "yaml",
            "title: Hello there friend\nenabled: true\ndesc: 'It''s late'\n",
            &[
                ("Hello there friend", "Chào bạn"),
                ("It's late", "Muộn rồi"),
            ],
        );
        assert_eq!(doc, vec!["Hello there friend", "It's late"]);
        assert_eq!(
            moi,
            "title: \"Chào bạn\"\nenabled: true\ndesc: 'Muộn rồi'\n"
        );

        let (doc, moi) = khu_hoi(
            "rpy",
            "    e \"Where are we going?\"\n    play music \"bgm.ogg\"\n",
            &[("Where are we going?", "Mình đi đâu đây?")],
        );
        assert_eq!(doc, vec!["Where are we going?"]);
        assert!(moi.starts_with("    e \"Mình đi đâu đây?\"\n"));
    }

    #[test]
    fn locres_khu_hoi() {
        // bản 2: magic + ver + vị trí mảng + (không namespace) + mảng 2 chuỗi
        let mut b = MAGIC_LOCRES.to_vec();
        b.push(2);
        let vt = (b.len() + 8) as i64;
        b.extend(vt.to_le_bytes());
        b.extend(2i32.to_le_bytes());
        for s in ["Start Game", "Options menu"] {
            b.extend(ghi_fstr(s));
            b.extend(1i32.to_le_bytes());
        }
        let d = dang("locres").unwrap();
        let doc: Vec<String> = d.doc(&b).unwrap().into_iter().map(|(_, v)| v).collect();
        assert_eq!(doc, vec!["Start Game", "Options menu"]);
        let moi = d
            .ghi(&b, &mut |en: &str| {
                (en == "Start Game").then(|| "Bắt đầu".to_string())
            })
            .unwrap();
        let doc2: Vec<String> = DangLocres::nap(&moi)
            .map(|l| match l {
                Locres::Moi { mang, .. } => mang.into_iter().map(|x| x.0).collect(),
                _ => vec![],
            })
            .unwrap();
        assert_eq!(doc2, vec!["Bắt đầu", "Options menu"]);
    }

    /// Thử thật: cargo test -- --ignored quet_that --nocapture (biến QUET = thư mục game)
    #[test]
    #[ignore]
    fn quet_that() {
        let thu = std::env::var("QUET").unwrap();
        let t = Instant::now();
        let kq = quet(Path::new(&thu), &|_| {}, &|| false);
        println!(
            "engine='{}' file đã xem={} thời gian={:?}",
            kq.engine,
            kq.so_tep,
            t.elapsed()
        );
        for f in kq.file.iter().take(6) {
            println!(
                "FILE {} | {} | {} câu ({} câu dài) | Anh {:.0}% | điểm {:.0} | mẫu: {:?}",
                f.tuong_doi,
                f.loai,
                f.so,
                f.so_cau,
                f.anh * 100.0,
                f.diem,
                f.mau.first()
            );
        }
        for g in kq.goi.iter().take(4) {
            println!("GÓI {} | {} file | {} MB", g.loai, g.so, g.co >> 20);
        }
    }

    /// Thử trọn quy trình trên BẢN SAO (biến QUY_TRINH = thư mục game thử, sẽ bị ghi):
    /// dò → chọn → dịch offline 30 câu → ghi → đọc lại → trả lại → giống hệt bản gốc.
    #[test]
    #[ignore]
    fn quy_trinh_that() {
        struct Im;
        impl crate::viet_hoa::viec_dich::BaoCao for Im {
            fn bao(&self, s: &str) {
                println!("  {s}");
            }
            fn tien_do(&self, _: usize, _: usize, _: f64) {}
        }
        let thu = PathBuf::from(std::env::var("QUY_TRINH").unwrap());
        let v = ViecFile::moi(&thu, "Game thử");
        let _ = std::fs::remove_dir_all(&v.tm);
        let mut v = ViecFile::moi(&thu, "Game thử");
        let kq = v.quet(&|_| {}, &|| false);
        println!("dò được {} file, chọn sẵn: {:?}", kq.file.len(), v.chon);
        assert!(!v.chon.is_empty());
        v.chon.truncate(1);
        let td = v.chon[0].clone();
        let goc = std::fs::read(thu.join(&td)).unwrap();
        v.doc_chon();
        let cau: Vec<String> = v.cac_cau();
        let ds = v.viec.cau_can_dich(cau.iter().take(30).map(String::as_str));
        // câu dịch hỏng (vd "Bloom" → "Bloom Bloom Bloom") bị loại → để lần sau / Claude dịch
        v.viec.chay_offline(ds, &Im).unwrap();
        println!("  câu lỗi bị loại: {}", v.viec.loi);
        for en in cau.iter().take(5) {
            println!("  {en}  =>  {:?}", v.viec.tim(en));
        }
        let n = v.ghi_vao_game(false, &|s| println!("  {s}")).unwrap();
        println!("đã ghi {n} câu");
        assert!(n >= 20);
        let moi = std::fs::read(thu.join(&td)).unwrap();
        assert_ne!(moi, goc);
        let doc_lai = dang(kiem_tra_loai(&td).unwrap())
            .unwrap()
            .doc(&moi)
            .unwrap();
        assert_eq!(
            doc_lai.len(),
            dang(kiem_tra_loai(&td).unwrap())
                .unwrap()
                .doc(&goc)
                .unwrap()
                .len()
                .min(doc_lai.len())
        );
        // ghi lần 2 vẫn dịch từ bản gốc (không dịch chồng)
        assert_eq!(v.ghi_vao_game(false, &|_| {}).unwrap(), n);
        assert_eq!(v.file_goc(&td), v.tm.join("sao_luu").join(&td));
        assert_eq!(v.tra_lai().unwrap(), 1);
        assert_eq!(
            std::fs::read(thu.join(&td)).unwrap(),
            goc,
            "trả lại phải giống hệt bản gốc"
        );
        let csv = std::env::temp_dir().join("thu_xuat.csv");
        println!("xuất CSV: {} câu", v.xuat_csv(&csv).unwrap());
        assert!(v.nap_csv(&csv).unwrap() > 0);
    }

    #[test]
    fn loc_chu() {
        assert!(la_chu_anh("Press any key"));
        assert!(la_chu_anh("Inventory"));
        assert!(!la_chu_anh("ui_button_ok"));
        assert!(!la_chu_anh("textures/hero.png"));
        assert!(!la_chu_anh("true"));
        assert!(!la_chu_anh("Ngôi làng đang bị tấn công"));
        assert_eq!(bo_dau("Mở cửa đi"), "Mo cua di");
        assert_eq!(giai_ma(b"\xFF\xFEH\0i\0").0, "Hi");
        assert_eq!(giai_ma(b"caf\xe9").0, "café");
    }
}
