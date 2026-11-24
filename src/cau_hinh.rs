//! Cấu hình app (du_lieu\cau_hinh.json). Dùng CÙNG tên khóa với app Dịch Game cũ
//! để đọc thẳng cấu hình cũ; khóa lạ được giữ nguyên khi lưu.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::duong_dan;

pub const SO_VUNG_TOI_DA: usize = 6;

/// Tinh chỉnh theo loại vùng (giống app cũ).
pub struct TuyLoai {
    /// ẩn khung sau N lần đọc không thấy chữ
    pub an_trong: u32,
    /// ẩn sau N lần chữ khác hẳn bản đang hiện
    pub an_lech: u32,
    /// giống câu đang hiện từ mức này → không dịch lại
    pub bo_qua: f64,
    /// dưới mức này coi là đổi chữ
    pub lech: f64,
}

pub fn tuy_loai(loai: &str) -> TuyLoai {
    if loai == "co_dinh" {
        TuyLoai {
            an_trong: 30,
            an_lech: u32::MAX,
            bo_qua: 0.85,
            lech: 0.7,
        }
    } else {
        TuyLoai {
            an_trong: 3,
            an_lech: 5,
            bo_qua: 0.92,
            lech: 0.8,
        }
    }
}

/// Vùng cố định đọc lại mỗi N vòng (~0,25 giây).
pub const VONG_CO_DINH: u64 = 2;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Vung {
    #[serde(deserialize_with = "doc_id")]
    pub id: String,
    pub ten: String,
    /// "phu_de" (hội thoại, đọc liên tục) | "co_dinh" (nhiệm vụ, menu – giữ bản dịch)
    pub loai: String,
    /// Vùng chữ cần đọc [x, y, w, h] (pixel màn hình); None = chưa chọn
    pub vung: Option<[i32; 4]>,
    /// Khung hiện bản dịch [x, y, w]; None = đặt ngay ở vùng chữ
    pub khung: Option<[i32; 3]>,
    pub co_chu: i32,
    pub loc_trang: bool,
    #[serde(flatten)]
    pub khac: Map<String, Value>,
}

impl Default for Vung {
    fn default() -> Self {
        vung_moi("vung", "Vùng", "phu_de")
    }
}

pub fn vung_moi(id: &str, ten: &str, loai: &str) -> Vung {
    Vung {
        id: id.into(),
        ten: ten.into(),
        loai: loai.into(),
        vung: None,
        khung: None,
        co_chu: if loai == "phu_de" { 18 } else { 15 },
        loc_trang: loai == "phu_de",
        khac: Map::new(),
    }
}

impl Vung {
    /// Khung hiện bản dịch: khung đã chỉnh, nếu chưa thì góc trên vùng chữ.
    pub fn khung_dich(&self) -> Option<[i32; 3]> {
        self.khung.or_else(|| self.vung.map(|v| [v[0], v[1], v[2]]))
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
    pub cac_vung: Vec<Vung>,
    /// Ngưỡng lọc chữ trắng (độ sáng ≥ ngưỡng = chữ)
    pub nguong_chu_trang: u8,
    /// "phu_de" (chữ viền, nền trong suốt) | "khung" (hộp nền tối)
    pub kieu_hien: String,
    pub che_chu_goc: bool,
    pub vien_chu: i32,
    pub do_dam_chu: f32,
    pub do_mo: f32,
    /// offline | offline_claude | claude | google
    pub che_do_hoi_thoai: String,
    pub claude_bat: bool,
    pub thay_bang_claude: bool,
    pub co_dinh_thay_claude: bool,
    pub hien_google_truoc: bool,
    pub tu_dong_dich: bool,
    pub giong_doc: bool,
    pub giong: String,
    pub chu_ky_giay: f32,
    pub co_chu_toan_man: i32,
    pub dung_viet_hoa: bool,
    #[serde(flatten)]
    pub khac: Map<String, Value>,
}

impl Default for CauHinh {
    fn default() -> Self {
        Self {
            cac_vung: vec![
                vung_moi("hoi_thoai", "Hội thoại", "phu_de"),
                vung_moi("nhiem_vu", "Nhiệm vụ", "co_dinh"),
            ],
            nguong_chu_trang: 185,
            kieu_hien: "phu_de".into(),
            che_chu_goc: true,
            vien_chu: 2,
            do_dam_chu: 1.0,
            do_mo: 0.85,
            che_do_hoi_thoai: "offline_claude".into(),
            claude_bat: true,
            thay_bang_claude: false,
            co_dinh_thay_claude: true,
            hien_google_truoc: true,
            tu_dong_dich: true,
            giong_doc: false,
            giong: "vi-VN-NamMinhNeural".into(),
            chu_ky_giay: 0.12,
            co_chu_toan_man: 0,
            dung_viet_hoa: true,
            khac: Map::new(),
        }
    }
}

fn doc_file(p: &std::path::Path) -> Option<CauHinh> {
    let b = std::fs::read(p).ok()?;
    match serde_json::from_slice(b.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&b)) {
        Ok(c) => Some(c),
        Err(_) => {
            // Không đọc được: giữ bản cũ để không mất cấu hình
            let _ = std::fs::copy(p, p.with_extension("json.loi"));
            None
        }
    }
}

impl CauHinh {
    pub fn nap() -> Self {
        let p = duong_dan::file_du_lieu("cau_hinh.json");
        let mut ch = doc_file(&p).unwrap_or_default();
        let lay_lai = ch.lay_lai_khung_cu();
        ch.nang_cap();
        if lay_lai {
            duong_dan::nhat_ky("Cấu hình: đã lấy lại vị trí khung dịch từ app cũ");
            ch.luu();
        }
        ch
    }

    /// Bản Rust trước đây làm mất "khung" của từng vùng → lấy lại từ cấu hình app cũ.
    fn lay_lai_khung_cu(&mut self) -> bool {
        if self.cac_vung.iter().all(|v| v.khung.is_some()) {
            return false;
        }
        let Ok(appdata) = std::env::var("APPDATA") else {
            return false;
        };
        let Ok(b) = std::fs::read(
            std::path::Path::new(&appdata)
                .join("DichGame")
                .join("cau_hinh.json"),
        ) else {
            return false;
        };
        let Ok(cu) = serde_json::from_slice::<CauHinh>(&b) else {
            return false;
        };
        let mut co = false;
        for v in &mut self.cac_vung {
            if let Some(c) = cu.cac_vung.iter().find(|c| c.id == v.id) {
                if v.khung.is_none() && c.khung.is_some() && v.vung == c.vung {
                    v.khung = c.khung;
                    co = true;
                }
            }
        }
        co
    }

    fn nang_cap(&mut self) {
        if self.cac_vung.is_empty() {
            self.cac_vung = CauHinh::default().cac_vung;
        }
        self.cac_vung.truncate(SO_VUNG_TOI_DA);
        for v in &mut self.cac_vung {
            if v.loai != "co_dinh" {
                v.loai = "phu_de".into();
            }
            if v.vung.is_some_and(|r| r[2] < 10 || r[3] < 5) {
                v.vung = None;
            }
        }
        if !["offline", "offline_claude", "claude", "google"]
            .contains(&self.che_do_hoi_thoai.as_str())
        {
            self.che_do_hoi_thoai = "offline_claude".into();
        }
        if self.kieu_hien != "khung" {
            self.kieu_hien = "phu_de".into();
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn doc_cau_hinh_app_cu() {
        let cu = r#"{"cac_vung":[{"id":"nhiem_vu","ten":"Nhiệm vụ","loai":"co_dinh","vung":[27,767,687,221],
            "khung":[27,707,624],"co_chu":14,"loc_trang":false},{"id":"x","ten":"T","loai":"phu_de","vung":null,
            "khung":null,"co_chu":15,"loc_trang":true}],"che_do_hoi_thoai":"offline","phim_tat":{"a":"f6"}}"#;
        let ch: CauHinh = serde_json::from_str(cu).unwrap();
        assert_eq!(ch.cac_vung[0].khung, Some([27, 707, 624]));
        assert_eq!(ch.cac_vung[1].vung, None);
        assert!(ch.khac.contains_key("phim_tat"));
        let lai = serde_json::to_string(&ch).unwrap();
        assert!(lai.contains("\"khung\":[27,707,624]") && lai.contains("phim_tat"));
    }
}
