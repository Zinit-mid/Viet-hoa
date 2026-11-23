//! Bộ nhớ dịch – CÙNG định dạng với app Dịch Game cũ (ban_dich_da_luu.json):
//!   "<máy>#|<câu đã thay số bằng #>" → {"src": câu gốc, "vi": bản dịch}
//!   "<máy>|<câu>"                    → "bản dịch" (kiểu cũ hơn)
//! Tra: khớp đúng → gần giống ≥ 93% trong 400 câu mới nhất, rồi thay số.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::duong_dan;

pub const NGUONG_GAN: f64 = 0.93;

/// "Got 15 gold" → "Got # gold"
pub fn so_hoa(s: &str) -> String {
    let mut ra = String::with_capacity(s.len());
    let mut trong_so = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !trong_so {
                ra.push('#');
            }
            trong_so = true;
        } else {
            ra.push(c);
            trong_so = false;
        }
    }
    ra
}

fn cac_so(s: &str) -> Vec<String> {
    let mut ra = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            ra.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        ra.push(cur);
    }
    ra
}

/// Thay số trong bản dịch cũ bằng số mới (giống app cũ).
pub fn thay_so(ban_dich: &str, so_cu: &[String], so_moi: &[String]) -> Option<String> {
    if so_cu == so_moi {
        return Some(ban_dich.to_string());
    }
    if so_cu.len() != so_moi.len() || cac_so(ban_dich) != so_cu {
        return None;
    }
    let mut it = so_moi.iter();
    let mut ra = String::new();
    let mut trong_so = false;
    for c in ban_dich.chars() {
        if c.is_ascii_digit() {
            if !trong_so {
                ra.push_str(it.next().map(String::as_str).unwrap_or(""));
            }
            trong_so = true;
        } else {
            ra.push(c);
            trong_so = false;
        }
    }
    Some(ra)
}

pub fn chu_giong(a: &str, b: &str, ty_le: f64) -> bool {
    a == b || (!a.is_empty() && !b.is_empty() && strsim::normalized_levenshtein(a, b) >= ty_le)
}

pub struct BoNhoDich {
    duong: PathBuf,
    m: Map<String, Value>,
    gan_day: Vec<String>,
    thay_doi: usize,
}

impl BoNhoDich {
    pub fn nap(ten_file: &str) -> Self {
        let duong = duong_dan::file_du_lieu(ten_file);
        let mut m = doc(&duong);
        // Bản Rust cũ lưu {câu thường: bản dịch} và đã ghi đè bản của app cũ →
        // đổi sang định dạng mới, rồi gộp lại bộ nhớ gốc của app cũ trong %APPDATA%.
        let la_kieu_rust = !m.is_empty() && m.iter().all(|(k, v)| v.is_string() && !k.contains('|'));
        if la_kieu_rust {
            m = m.into_iter().map(|(k, v)| (format!("claude#|{k}"), json!({"src": k, "vi": v}))).collect();
        }
        let mut thay_doi = 0;
        if let Ok(appdata) = std::env::var("APPDATA") {
            let cu = doc(&PathBuf::from(appdata).join("DichGame").join("ban_dich_da_luu.json"));
            for (k, v) in cu {
                if !m.contains_key(&k) {
                    m.insert(k, v);
                    thay_doi += 1;
                }
            }
        }
        if la_kieu_rust {
            thay_doi += 1;
        }
        let gan_day: Vec<String> = m.keys().filter(|k| k.contains("#|")).cloned().collect();
        let gan_day = gan_day[gan_day.len().saturating_sub(400)..].to_vec();
        let mut bn = Self { duong, m, gan_day, thay_doi };
        if thay_doi > 0 {
            duong_dan::nhat_ky(&format!("Bộ nhớ dịch: gộp thêm {thay_doi} mục từ app cũ, tổng {}", bn.m.len()));
            bn.luu();
        }
        bn
    }

    pub fn so_cau(&self) -> usize {
        self.m.len()
    }

    /// Tra bản dịch đã lưu của một máy dịch ("claude", "google", ...).
    pub fn tim(&self, may: &str, van_ban: &str) -> Option<String> {
        if let Some(Value::String(s)) = self.m.get(&format!("{may}|{van_ban}")) {
            return Some(s.clone());
        }
        let mau = so_hoa(van_ban);
        let so_moi = cac_so(van_ban);
        let khoa_dung = format!("{may}#|{mau}");
        let tien_to = format!("{may}#|");
        let ung_vien = std::iter::once(&khoa_dung).chain(self.gan_day.iter().rev().filter(|k| k.starts_with(&tien_to)));
        for (i, k) in ung_vien.enumerate() {
            let Some(Value::Object(e)) = self.m.get(k) else { continue };
            if i > 0 && !chu_giong(&k[tien_to.len()..], &mau, NGUONG_GAN) {
                continue;
            }
            let (Some(src), Some(vi)) = (e.get("src").and_then(Value::as_str), e.get("vi").and_then(Value::as_str)) else {
                continue;
            };
            if let Some(kq) = thay_so(vi, &cac_so(src), &so_moi) {
                return Some(kq);
            }
        }
        None
    }

    pub fn them(&mut self, may: &str, van_ban: &str, vi: &str) {
        if van_ban.trim().is_empty() || vi.trim().is_empty() {
            return;
        }
        let k = format!("{may}#|{}", so_hoa(van_ban));
        self.m.insert(k.clone(), json!({"src": van_ban, "vi": vi}));
        self.gan_day.retain(|x| x != &k);
        self.gan_day.push(k);
        if self.gan_day.len() > 400 {
            self.gan_day.remove(0);
        }
        self.thay_doi += 1;
        if self.thay_doi >= 10 {
            self.luu();
        }
    }

    pub fn luu(&mut self) {
        if self.thay_doi == 0 {
            return;
        }
        if let Ok(b) = serde_json::to_vec(&self.m) {
            if duong_dan::ghi_file(&self.duong, &b).is_ok() {
                self.thay_doi = 0;
            }
        }
    }
}

impl Drop for BoNhoDich {
    fn drop(&mut self) {
        self.luu();
    }
}

fn doc(p: &std::path::Path) -> Map<String, Value> {
    let Ok(b) = std::fs::read(p) else { return Map::new() };
    match serde_json::from_slice::<Value>(b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&b)) {
        Ok(Value::Object(m)) => m,
        _ => {
            // Đọc lỗi: sao lưu file, KHÔNG ghi đè
            let _ = std::fs::copy(p, p.with_extension("json.loi"));
            Map::new()
        }
    }
}

// ---------------- bản Việt hóa game (giống app cũ: BoNhoVietHoa) ----------------

/// Khóa so khớp giữa chữ OCR trên màn hình và câu gốc trong file game.
pub fn chuan_hoa_vh(s: &str) -> String {
    let mut t = String::new();
    let mut sau = 0;
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let dong = match c[i] {
            '<' => Some('>'),
            '{' => Some('}'),
            _ => None,
        };
        if let Some(d) = dong {
            if let Some(j) = c[i + 1..].iter().position(|&x| x == d) {
                t.push(' ');
                i += j + 2;
                continue;
            }
        }
        t.push(c[i]);
        i += 1;
        sau += 1;
    }
    let _ = sau;
    let t = t.replace("[EMPTY]", " ").replace(['’', '‘'], "'");
    let t: String = t.chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '\'' || ch == ' ' { ch } else { ' ' }).collect();
    t.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Default)]
pub struct BoNhoVietHoa {
    m: HashMap<String, String>,
    mtime: Option<std::time::SystemTime>,
    lan_xem: Option<std::time::Instant>,
}

impl BoNhoVietHoa {
    fn duong() -> PathBuf {
        duong_dan::viet_hoa_game().join("crimson_desert").join("bo_nho_dich.json")
    }

    fn nap(&mut self) {
        if self.lan_xem.is_some_and(|t| t.elapsed().as_secs() < 30) {
            return;
        }
        self.lan_xem = Some(std::time::Instant::now());
        let p = Self::duong();
        let mt = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
        if mt.is_none() {
            self.m.clear();
            self.mtime = None;
            return;
        }
        if mt == self.mtime {
            return;
        }
        let mut m = HashMap::new();
        for (en, vi) in doc(&p) {
            let k = chuan_hoa_vh(&en);
            if let (true, Some(vi)) = (k.len() >= 4, vi.as_str()) {
                let vi = vi.replace("<br/>", "\n").replace("<br>", "\n");
                let vi = chuan_hoa_vh_giu_dau(&vi);
                m.insert(k, vi);
            }
        }
        self.m = m;
        self.mtime = mt;
    }

    pub fn tim(&mut self, van_ban: &str) -> Option<String> {
        self.nap();
        if self.m.is_empty() {
            return None;
        }
        if let Some(kq) = self.m.get(&chuan_hoa_vh(van_ban)) {
            return Some(kq.clone());
        }
        if van_ban.contains('\n') {
            let ds: Vec<Option<&String>> =
                van_ban.lines().filter(|x| !x.trim().is_empty()).map(|x| self.m.get(&chuan_hoa_vh(x))).collect();
            if !ds.is_empty() && ds.iter().all(Option::is_some) {
                return Some(ds.into_iter().flatten().cloned().collect::<Vec<_>>().join("\n"));
            }
        }
        None
    }
}

/// Bỏ thẻ <..> {..} [EMPTY], gọn khoảng trắng từng dòng (giữ dấu tiếng Việt).
fn chuan_hoa_vh_giu_dau(s: &str) -> String {
    let mut t = String::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let dong = match c[i] {
            '<' => Some('>'),
            '{' => Some('}'),
            _ => None,
        };
        if let Some(d) = dong {
            if let Some(j) = c[i + 1..].iter().position(|&x| x == d) {
                i += j + 2;
                continue;
            }
        }
        t.push(c[i]);
        i += 1;
    }
    t.replace("[EMPTY]", "").lines().map(|d| d.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dinh_dang_app_cu() {
        let mut bn = BoNhoDich { duong: PathBuf::from("x"), m: Map::new(), gan_day: vec![], thay_doi: 0 };
        bn.m.insert("claude#|You got # gold".into(), json!({"src": "You got 15 gold", "vi": "Bạn nhận 15 vàng"}));
        bn.gan_day.push("claude#|You got # gold".into());
        assert_eq!(bn.tim("claude", "You got 230 gold").as_deref(), Some("Bạn nhận 230 vàng"));
        assert_eq!(bn.tim("google", "You got 230 gold"), None);
        bn.them("google", "Off", "Tắt");
        assert_eq!(bn.tim("google", "Off").as_deref(), Some("Tắt"));
        bn.thay_doi = 0;
    }
    #[test]
    fn khoa_viet_hoa() {
        assert_eq!(chuan_hoa_vh("Press <b>E</b> to {Key:open}!"), "press e to");
    }
}
