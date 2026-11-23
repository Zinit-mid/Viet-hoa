//! Máy dịch dùng chung: bộ nhớ dịch → Offline / Claude / Google.

pub mod bo_nho;
pub mod claude;
pub mod google;
pub mod offline;
pub mod the;

use std::sync::Mutex;

use bo_nho::BoNhoDich;

pub static BO_NHO: Mutex<Option<BoNhoDich>> = Mutex::new(None);

/// Nhớ tạm bản dịch offline / Google trong lúc app đang mở (không lưu xuống đĩa,
/// vì bộ nhớ dịch lâu dài chỉ giữ bản Claude).
static NHO_TAM: Mutex<Option<std::collections::HashMap<String, String>>> = Mutex::new(None);

fn nho_tam(en: &str) -> Option<String> {
    NHO_TAM.lock().ok()?.as_ref()?.get(en).cloned()
}

fn ghi_nho_tam(en: &str, vi: &str) {
    if let Ok(mut k) = NHO_TAM.lock() {
        let m = k.get_or_insert_with(Default::default);
        if m.len() > 5000 {
            m.clear();
        }
        m.insert(en.to_string(), vi.to_string());
    }
}

fn bo_nho<R>(f: impl FnOnce(&mut BoNhoDich) -> R) -> R {
    let mut k = BO_NHO.lock().unwrap_or_else(|e| e.into_inner());
    f(k.get_or_insert_with(|| BoNhoDich::nap("ban_dich_da_luu.json")))
}

pub fn tra_bo_nho(en: &str) -> Option<String> {
    bo_nho(|b| b.tra(en))
}

pub fn ghi_bo_nho(en: &str, vi: &str) {
    bo_nho(|b| b.them(en, vi));
}

pub fn luu_bo_nho() {
    bo_nho(|b| b.luu());
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum May {
    BoNho,
    Offline,
    Claude,
    Google,
}

impl May {
    pub fn ten(self) -> &'static str {
        match self {
            May::BoNho => "bộ nhớ",
            May::Offline => "offline",
            May::Claude => "Claude",
            May::Google => "Google",
        }
    }
}

/// Dịch nhanh một câu theo chế độ: trả (bản dịch, máy đã dùng).
/// `che_do`: offline | offline_claude | claude | google.
pub fn dich_nhanh(en: &str, che_do: &str) -> Option<(String, May)> {
    let en = en.trim();
    if en.is_empty() {
        return None;
    }
    if the::la_tieng_viet(en) {
        return Some((en.to_string(), May::BoNho));
    }
    if let Some(vi) = tra_bo_nho(en) {
        return Some((vi, May::BoNho));
    }
    if che_do != "claude" {
        if let Some(vi) = nho_tam(en) {
            return Some((vi, May::BoNho));
        }
    }
    let thu: &[May] = match che_do {
        "claude" => &[May::Claude, May::Google],
        "google" => &[May::Google],
        _ => &[May::Offline, May::Google],
    };
    for &m in thu {
        let kq = match m {
            May::Offline if offline::co_the_dung() => offline::dich(en),
            May::Claude if claude::co_khoa() => claude::dich(en),
            May::Google => google::dich(en),
            _ => continue,
        };
        if let Err(e) = &kq {
            crate::duong_dan::nhat_ky(&format!("Máy dịch {} lỗi: {e}", m.ten()));
        }
        if let Ok(vi) = kq {
            if !vi.trim().is_empty() && the::khop_the(en, &vi) {
                if m == May::Claude {
                    ghi_bo_nho(en, &vi);
                } else {
                    ghi_nho_tam(en, &vi);
                }
                return Some((vi, m));
            }
        }
    }
    None
}

/// Claude dịch ngầm câu để lưu vào bộ nhớ (chế độ offline_claude).
pub fn claude_luu_ngam(en: String) {
    if !claude::co_khoa() || tra_bo_nho(&en).is_some() {
        return;
    }
    std::thread::spawn(move || {
        if let Ok(vi) = claude::dich(&en) {
            if the::khop_the(&en, &vi) && !the::con_chu_anh(&vi) {
                ghi_bo_nho(&en, &vi);
            }
        }
    });
}

/// Dịch nhiều câu một lúc (F12, Việt hóa file): bộ nhớ → Claude lô → Offline → Google.
pub fn dich_nhieu(cac_cau: &[String], dung_claude: bool) -> Vec<Option<String>> {
    let mut ra: Vec<Option<String>> = cac_cau.iter().map(|c| tra_bo_nho(c)).collect();
    let can: Vec<usize> = (0..cac_cau.len()).filter(|&i| ra[i].is_none()).collect();
    if can.is_empty() {
        return ra;
    }
    let cau: Vec<String> = can.iter().map(|&i| cac_cau[i].clone()).collect();
    let mut kq: Option<Vec<String>> = None;
    if dung_claude && claude::co_khoa() {
        kq = claude::dich_lo(&cau, claude::HUONG_DAN).ok();
    }
    if kq.is_none() && offline::co_the_dung() {
        kq = offline::dich_lo(&cau).ok();
    }
    match kq {
        Some(v) => {
            for (k, &i) in can.iter().enumerate() {
                let vi = &v[k];
                if the::khop_the(&cac_cau[i], vi) {
                    if dung_claude {
                        ghi_bo_nho(&cac_cau[i], vi);
                    }
                    ra[i] = Some(vi.clone());
                }
            }
        }
        None => {
            for &i in &can {
                ra[i] = google::dich(&cac_cau[i]).ok();
            }
        }
    }
    ra
}
