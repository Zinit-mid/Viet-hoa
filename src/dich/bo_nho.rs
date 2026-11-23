//! Bộ nhớ dịch {en: vi}: khóa theo câu đã thay số, khớp gần ≥ 93%.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::duong_dan;

pub const NGUONG_GAN: f64 = 0.93;

/// "Got 15 gold" -> ("got # gold", ["15"])
pub fn so_hoa(s: &str) -> (String, Vec<String>) {
    let mut khoa = String::with_capacity(s.len());
    let mut so = Vec::new();
    let mut dang = String::new();
    for c in s.trim().chars() {
        if c.is_ascii_digit() {
            dang.push(c);
        } else {
            if !dang.is_empty() {
                so.push(std::mem::take(&mut dang));
                khoa.push('#');
            }
            khoa.extend(c.to_lowercase());
        }
    }
    if !dang.is_empty() {
        so.push(dang);
        khoa.push('#');
    }
    (khoa.split_whitespace().collect::<Vec<_>>().join(" "), so)
}

/// Thay lần lượt các '#' trong bản dịch mẫu bằng số thật.
pub fn thay_so(mau: &str, so: &[String]) -> Option<String> {
    if mau.matches('#').count() != so.len() {
        return None;
    }
    let mut it = so.iter();
    Some(mau.chars().fold(String::new(), |mut a, c| {
        if c == '#' {
            a.push_str(it.next().map(String::as_str).unwrap_or("#"));
        } else {
            a.push(c);
        }
        a
    }))
}

pub struct BoNhoDich {
    duong: PathBuf,
    /// khóa so_hoa(en) -> bản dịch (số đã thay bằng '#')
    ban: HashMap<String, String>,
    thay_doi: usize,
}

impl BoNhoDich {
    pub fn nap(ten_file: &str) -> Self {
        let duong = duong_dan::file_du_lieu(ten_file);
        let goc: HashMap<String, String> = std::fs::read(&duong)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let mut bn = Self { duong, ban: HashMap::new(), thay_doi: 0 };
        for (en, vi) in goc {
            bn.them_noi_bo(&en, &vi);
        }
        bn.thay_doi = 0;
        bn
    }

    fn them_noi_bo(&mut self, en: &str, vi: &str) {
        let (k, so) = so_hoa(en);
        let (_, so_vi) = so_hoa(vi);
        // Bản dịch giữ nguyên các số -> lưu dạng mẫu '#'
        let mau = if so == so_vi && !so.is_empty() {
            let mut s = vi.to_string();
            for n in &so {
                s = s.replacen(n.as_str(), "\u{0}", 1);
            }
            s.replace('\u{0}', "#")
        } else {
            vi.to_string()
        };
        self.ban.insert(k, mau);
    }

    pub fn them(&mut self, en: &str, vi: &str) {
        if en.trim().is_empty() || vi.trim().is_empty() {
            return;
        }
        self.them_noi_bo(en, vi);
        self.thay_doi += 1;
        if self.thay_doi >= 20 {
            self.luu();
        }
    }

    pub fn tra(&self, en: &str) -> Option<String> {
        let (k, so) = so_hoa(en);
        if let Some(m) = self.ban.get(&k) {
            return thay_so(m, &so).or_else(|| Some(m.clone()));
        }
        if k.chars().count() < 8 || self.ban.len() > 50_000 {
            return None;
        }
        let dai = k.chars().count() as f64;
        let mut tot: Option<(f64, &String)> = None;
        for (kk, m) in &self.ban {
            let d = kk.chars().count() as f64;
            if (d - dai).abs() / dai > 0.08 {
                continue;
            }
            let s = strsim::normalized_levenshtein(&k, kk);
            if s >= NGUONG_GAN && tot.is_none_or(|(t, _)| s > t) {
                tot = Some((s, m));
            }
        }
        tot.and_then(|(_, m)| thay_so(m, &so))
    }

    pub fn luu(&mut self) {
        if self.thay_doi == 0 {
            return;
        }
        if let Ok(b) = serde_json::to_vec(&self.ban) {
            let _ = duong_dan::ghi_file(&self.duong, &b);
            self.thay_doi = 0;
        }
    }
}

impl Drop for BoNhoDich {
    fn drop(&mut self) {
        self.luu();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thay_so_dung() {
        let mut bn = BoNhoDich { duong: PathBuf::from("x"), ban: HashMap::new(), thay_doi: 0 };
        bn.them_noi_bo("You got 15 gold", "Bạn nhận 15 vàng");
        assert_eq!(bn.tra("You got 230 gold").as_deref(), Some("Bạn nhận 230 vàng"));
        bn.thay_doi = 0;
    }
}
