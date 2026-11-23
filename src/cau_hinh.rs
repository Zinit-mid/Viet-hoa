//! Cấu hình app (du_lieu\cau_hinh.json). Giữ nguyên các khóa lạ của app cũ.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::duong_dan;

pub const SO_VUNG_TOI_DA: usize = 6;
pub const PHIEN_BAN_CAU_HINH: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Vung {
    /// App Python cũ dùng chữ ("nhiem_vu"), có thể là số → luôn đọc thành chữ
    #[serde(deserialize_with = "doc_id")]
    pub id: String,
    pub ten: String,
    /// "phu_de" (đọc liên tục) hoặc "co_dinh" (giữ bản dịch)
    pub loai: String,
    /// [x, y, w, h] toạ độ màn hình thật (pixel)
    pub vung: [i32; 4],
    pub co_chu: i32,
    /// Cỡ chữ theo chiều cao dòng chữ gốc
    pub co_chu_tu_dong: bool,
    pub loc_trang: bool,
}

impl Default for Vung {
    fn default() -> Self {
        Self {
            id: "vung_1".into(),
            ten: "Vùng 1".into(),
            loai: "phu_de".into(),
            vung: [0, 0, 0, 0],
            co_chu: 22,
            co_chu_tu_dong: true,
            loc_trang: false,
        }
    }
}

fn doc_id<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => s,
        v => v.to_string(),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CauHinh {
    pub phien_ban_rust: u32,
    pub cac_vung: Vec<Vung>,
    /// offline | offline_claude | claude | google
    pub che_do_hoi_thoai: String,
    pub thay_bang_claude: bool,
    pub che_chu_goc: bool,
    pub co_chu_toan_man: i32,
    pub giong_doc: bool,
    pub giong: String,
    pub gioi_han_usd: f64,
    #[serde(flatten)]
    pub khac: Map<String, Value>,
}

impl Default for CauHinh {
    fn default() -> Self {
        Self {
            phien_ban_rust: PHIEN_BAN_CAU_HINH,
            cac_vung: Vec::new(),
            che_do_hoi_thoai: "offline_claude".into(),
            thay_bang_claude: false,
            che_chu_goc: true,
            co_chu_toan_man: 18,
            giong_doc: false,
            giong: "vi-VN-NamMinhNeural".into(),
            gioi_han_usd: 1.0,
            khac: Map::new(),
        }
    }
}

impl CauHinh {
    pub fn nap() -> Self {
        let p = duong_dan::file_du_lieu("cau_hinh.json");
        let mut ch: CauHinh = match std::fs::read(&p) {
            Err(_) => CauHinh::default(),
            Ok(b) => serde_json::from_slice(b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&b)).unwrap_or_else(|_| {
                // Không đọc được: giữ bản cũ để không mất cấu hình
                let _ = std::fs::copy(&p, p.with_extension("json.loi"));
                CauHinh::default()
            }),
        };
        ch.nang_cap();
        ch
    }

    /// Chuyển cấu hình của app Python cũ (giữ vùng người dùng đã chọn).
    fn nang_cap(&mut self) {
        self.cac_vung.retain(|v| v.vung[2] > 0 && v.vung[3] > 0);
        self.cac_vung.truncate(SO_VUNG_TOI_DA);
        if !["offline", "offline_claude", "claude", "google"].contains(&self.che_do_hoi_thoai.as_str()) {
            self.che_do_hoi_thoai = "offline_claude".into();
        }
        self.phien_ban_rust = PHIEN_BAN_CAU_HINH;
    }

    pub fn luu(&self) {
        if let Ok(b) = serde_json::to_vec_pretty(self) {
            let _ = duong_dan::ghi_file(&duong_dan::file_du_lieu("cau_hinh.json"), &b);
        }
    }
}

/// Đọc mã API từ file (không bao giờ ghi vào code / nhật ký).
pub fn doc_khoa(ten_file: &str) -> Option<String> {
    let s = std::fs::read_to_string(duong_dan::file_du_lieu(ten_file)).ok()?;
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

pub fn luu_khoa(ten_file: &str, khoa: &str) -> std::io::Result<()> {
    duong_dan::ghi_file(&duong_dan::file_du_lieu(ten_file), khoa.trim().as_bytes())
}
