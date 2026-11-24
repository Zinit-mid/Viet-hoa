//! OCR của Windows (Windows.Media.Ocr, gói en-US) – xử lý giống app cũ:
//! phóng to mịn tới 2 lần (cạnh dài ≤ 2500), lọc chữ trắng, gom dòng, chia ô chồng mép.

use anyhow::{anyhow, Result};
use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;

use crate::chup::Anh;

const CANH_TOI_DA: f32 = 2500.0;

#[derive(Clone, Debug)]
pub struct Dong {
    /// Chữ (đoạn nhiều dòng thì nối bằng '\n')
    pub chu: String,
    /// [x, y, w, h] theo toạ độ ảnh đưa vào
    pub hop: [i32; 4],
    /// Chiều cao một dòng chữ
    pub cao_dong: i32,
}

pub struct Ocr {
    may: OcrEngine,
}

impl Ocr {
    pub fn moi() -> Result<Self> {
        let ngon_ngu = Language::CreateLanguage(&HSTRING::from("en-US"))?;
        let may = OcrEngine::TryCreateFromLanguage(&ngon_ngu)
            .map_err(|_| anyhow!("Chưa cài gói OCR tiếng Anh (Language.OCR~~~en-US)"))?;
        Ok(Self { may })
    }

    /// Đọc từng dòng (giống ocr_tho của app cũ).
    pub fn doc(&self, anh: &Anh) -> Result<Vec<Dong>> {
        let he_so = (CANH_TOI_DA / anh.w.max(anh.h) as f32).min(2.0);
        let (anh_doc, he_so) = if (he_so - 1.0).abs() > 0.05 {
            (
                anh.doi_co((anh.w as f32 * he_so) as u32, (anh.h as f32 * he_so) as u32),
                he_so,
            )
        } else {
            (anh.clone(), 1.0)
        };
        let viet = DataWriter::new()?;
        viet.WriteBytes(&anh_doc.bgra)?;
        let buf = viet.DetachBuffer()?;
        let bmp = SoftwareBitmap::CreateCopyFromBuffer(
            &buf,
            BitmapPixelFormat::Bgra8,
            anh_doc.w as i32,
            anh_doc.h as i32,
        )?;
        let kq = self.may.RecognizeAsync(&bmp)?.get()?;
        let mut ra = Vec::new();
        for dong in kq.Lines()? {
            let chu = dong.Text()?.to_string_lossy();
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for tu in dong.Words()? {
                let r = tu.BoundingRect()?;
                x0 = x0.min(r.X);
                y0 = y0.min(r.Y);
                x1 = x1.max(r.X + r.Width);
                y1 = y1.max(r.Y + r.Height);
            }
            if x1 <= x0 {
                continue;
            }
            let cao = ((y1 - y0) / he_so) as i32;
            ra.push(Dong {
                chu,
                hop: [
                    (x0 / he_so) as i32,
                    (y0 / he_so) as i32,
                    ((x1 - x0) / he_so) as i32,
                    cao,
                ],
                cao_dong: cao,
            });
        }
        Ok(ra)
    }

    /// Đọc cả màn hình: ô rộng 2400 px chồng mép 200 px, chỉ lấy dòng có tâm
    /// nằm giữa ô (giống ocr_toan_man của app cũ).
    pub fn doc_toan_man(&self, anh: &Anh) -> Vec<Dong> {
        let (o, o_rong) = (200i32, 2400i32);
        let w = anh.w as i32;
        let mut bat_dau = vec![0];
        while bat_dau.last().unwrap() + o_rong < w {
            let s = bat_dau.last().unwrap() + o_rong - o;
            bat_dau.push(s);
        }
        let mut ra = Vec::new();
        for &s in &bat_dau {
            let e = (s + o_rong).min(w);
            let Some(manh) = anh.cat([s, 0, e - s, anh.h as i32]) else {
                continue;
            };
            let lo = s + if s > 0 { o / 2 } else { 0 };
            let hi = e - if e < w { o / 2 } else { 0 };
            for mut d in self.doc(&manh).unwrap_or_default() {
                let tam = s + d.hop[0] + d.hop[2] / 2;
                if tam >= lo && tam <= hi {
                    d.hop[0] += s;
                    ra.push(d);
                }
            }
        }
        ra
    }
}

/// Dòng có phải chữ thật (giống dong_hop_le của app cũ).
pub fn dong_hop_le(d: &str) -> bool {
    // Từ menu ngắn hay gặp (ON, OFF, NO...) vẫn dịch
    if crate::dich::the::tu_menu(d).is_some() {
        return true;
    }
    let tu: Vec<&str> = d
        .split(|c: char| !(c.is_ascii_alphabetic() || c == '\''))
        .filter(|t| !t.is_empty())
        .collect();
    let so_chu_cai: usize = tu.iter().map(|t| t.len()).sum();
    if so_chu_cai < 3 || !tu.iter().any(|t| t.len() >= 3) {
        return false;
    }
    let tong = d.chars().filter(|c| *c != ' ').count().max(1);
    so_chu_cai as f32 / tong as f32 >= 0.6
}

/// Đoạn hợp lệ khi đọc cả màn hình (loại cảnh vật bị đọc nhầm thành chữ).
pub fn hop_le_toan_man(chu: &str) -> bool {
    let tu: Vec<&str> = chu
        .split(|c: char| !(c.is_ascii_alphabetic() || c == '\''))
        .filter(|t| !t.is_empty())
        .collect();
    let so_chu_cai: usize = tu.iter().map(|t| t.len()).sum();
    let tong = chu.chars().filter(|c| !c.is_whitespace()).count().max(1);
    so_chu_cai >= 4 && tu.iter().any(|t| t.len() >= 3) && so_chu_cai as f32 / tong as f32 >= 0.65
}

fn het_cau_dong(s: &str) -> bool {
    s.trim_end()
        .ends_with(['.', '!', '?', ':', ';', '"', '”', '’', ')', ']', '…'])
}

/// Làm sạch các dòng OCR (giống lam_sach): bỏ rác, nối dòng bị ngắt (dòng sau viết
/// thường và dòng trước chưa hết câu), còn lại giữ xuống dòng.
pub fn lam_sach(cac_dong: &[String]) -> String {
    let mut kq: Vec<String> = Vec::new();
    for d in cac_dong {
        let d = d.split_whitespace().collect::<Vec<_>>().join(" ");
        if !dong_hop_le(&d) {
            continue;
        }
        let viet_thuong = d.chars().next().is_some_and(|c| c.is_lowercase());
        match kq.last_mut() {
            Some(cuoi) if viet_thuong && !het_cau_dong(cuoi) => {
                cuoi.push(' ');
                cuoi.push_str(&d);
            }
            _ => kq.push(d),
        }
    }
    kq.join("\n")
}

/// Gom các dòng gần nhau thành đoạn (giống gom_doan của app cũ). Mỗi đoạn giữ
/// xuống dòng giữa các dòng (menu vẫn dịch từng mục).
pub fn gom_doan(dong: Vec<Dong>) -> Vec<Dong> {
    struct D {
        dong: Vec<String>,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        h: i32,
    }
    let mut ds: Vec<Dong> = dong.into_iter().filter(|d| dong_hop_le(&d.chu)).collect();
    ds.sort_by_key(|d| (d.hop[1], d.hop[0]));
    let mut doan: Vec<D> = Vec::new();
    'ngoai: for d in ds {
        let [x, y, w, h] = d.hop;
        for p in doan.iter_mut().rev() {
            let khoang = y - p.y1;
            let ph = p.h as f32;
            if (khoang as f32) >= -0.3 * h as f32
                && (khoang as f32) <= 0.8 * ph
                && ((h - p.h).abs() as f32) <= 0.45 * ph
                && x < p.x1
                && x + w > p.x0
                && ((x - p.x0).abs() as f32) < 6.0 * ph
            {
                p.dong.push(d.chu);
                p.x0 = p.x0.min(x);
                p.x1 = p.x1.max(x + w);
                p.y1 = y + h;
                continue 'ngoai;
            }
        }
        doan.push(D {
            dong: vec![d.chu],
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
            h,
        });
    }
    doan.into_iter()
        .filter_map(|p| {
            let chu = lam_sach(&p.dong);
            (!chu.is_empty()).then_some(Dong {
                chu,
                hop: [p.x0, p.y0, p.x1 - p.x0, p.y1 - p.y0],
                cao_dong: p.h,
            })
        })
        .take(80)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(chu: &str, y: i32, w: i32) -> Dong {
        Dong {
            chu: chu.into(),
            hop: [100, y, w, 20],
            cao_dong: 20,
        }
    }
    #[test]
    fn gom_dong_giu_xuong_dong() {
        let ra = gom_doan(vec![
            d("Play", 100, 60),
            d("Options", 124, 90),
            d("Quit game", 148, 100),
        ]);
        assert_eq!(ra.len(), 1);
        assert_eq!(ra[0].chu, "Play\nOptions\nQuit game");
        let ra = gom_doan(vec![
            d("The village is under attack and we", 100, 400),
            d("need to leave now!", 124, 220),
        ]);
        assert_eq!(
            ra[0].chu,
            "The village is under attack and we need to leave now!"
        );
    }
    #[test]
    fn loc_rac() {
        assert!(dong_hop_le("NEW GAME"));
        assert!(dong_hop_le("ON"));
        assert!(!dong_hop_le("Zq"));
        assert!(!dong_hop_le("|| 12 ::"));
    }
}
