//! OCR của Windows (Windows.Media.Ocr, gói ngôn ngữ en-US).

use anyhow::{anyhow, Result};
use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;

use crate::chup::Anh;

#[derive(Clone, Debug)]
pub struct Dong {
    pub chu: String,
    /// [x, y, w, h] theo toạ độ ảnh đưa vào
    pub hop: [i32; 4],
    /// Chiều cao một dòng chữ (để chọn cỡ chữ Việt ổn định)
    pub cao_dong: i32,
    /// Số ký tự của dòng cuối (để biết có phải câu bị xuống dòng)
    pub dai_dong_cuoi: usize,
}

pub struct Ocr {
    may: OcrEngine,
    pub toi_da: u32,
}

impl Ocr {
    pub fn moi() -> Result<Self> {
        let ngon_ngu = Language::CreateLanguage(&HSTRING::from("en-US"))?;
        let may = OcrEngine::TryCreateFromLanguage(&ngon_ngu)
            .map_err(|_| anyhow!("Chưa cài gói OCR tiếng Anh (Language.OCR~~~en-US)"))?;
        let toi_da = OcrEngine::MaxImageDimension().unwrap_or(2600);
        Ok(Self { may, toi_da })
    }

    pub fn doc(&self, anh: &Anh) -> Result<Vec<Dong>> {
        // Ảnh nhỏ → phóng to cho OCR đọc chuẩn hơn
        let k = if anh.h < 60 { 3 } else if anh.h < 140 { 2 } else { 1 };
        let k = k.min((self.toi_da / anh.w.max(1)).max(1));
        let anh_doc = anh.phong(k);
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
            let k = k as f32;
            let cao = ((y1 - y0) / k) as i32;
            ra.push(Dong {
                dai_dong_cuoi: chu.chars().count(),
                chu,
                hop: [(x0 / k) as i32, (y0 / k) as i32, ((x1 - x0) / k) as i32, cao],
                cao_dong: cao,
            });
        }
        Ok(ra)
    }

    /// Đọc cả màn hình: chia phần rộng ≤ 2400 px.
    pub fn doc_toan_man(&self, anh: &Anh) -> Vec<Dong> {
        let rong = 2400.min(self.toi_da) as i32;
        let so = (anh.w as i32 + rong - 1) / rong;
        let phan = (anh.w as i32 + so - 1) / so;
        let mut ra = Vec::new();
        for i in 0..so {
            let Some(manh) = anh.cat([i * phan, 0, phan, anh.h.min(self.toi_da) as i32]) else { continue };
            let x = manh.x0 - anh.x0;
            if let Ok(dong) = self.doc(&manh) {
                ra.extend(dong.into_iter().map(|mut d| {
                    d.hop[0] += x;
                    d
                }));
            }
        }
        ra
    }
}

/// Dòng có phải chữ thật (lọc rác OCR).
pub fn dong_hop_le(s: &str) -> bool {
    let chu = s.chars().filter(|c| c.is_alphabetic()).count();
    let tong = s.chars().filter(|c| !c.is_whitespace()).count();
    chu >= 2 && chu * 2 >= tong
}

/// Gom các dòng gần nhau thành đoạn (F12).
pub fn gom_doan(mut dong: Vec<Dong>) -> Vec<Dong> {
    dong.retain(|d| dong_hop_le(&d.chu));
    dong.sort_by_key(|d| (d.hop[1], d.hop[0]));
    let mut doan: Vec<Dong> = Vec::new();
    'ngoai: for d in dong {
        for p in doan.iter_mut().rev() {
            let cao = d.hop[3].max(1);
            let day_p = p.hop[1] + p.hop[3];
            let khoang = d.hop[1] - day_p;
            let le_trai = (d.hop[0] - p.hop[0]).abs();
            let chong = d.hop[0] < p.hop[0] + p.hop[2] && p.hop[0] < d.hop[0] + d.hop[2];
            let cao_p = p.hop[3].max(1) as f32;
            // Chỉ gộp khi dòng trên là câu dài bị xuống dòng, hoặc dòng dưới viết thường
            // (menu: "Play" / "Options" là các mục riêng, không gộp)
            let noi_tiep = p.dai_dong_cuoi >= 25 || d.chu.chars().next().is_some_and(|c| c.is_lowercase());
            if noi_tiep
                && khoang > -cao / 2
                && khoang < cao * 8 / 10
                && chong
                && le_trai < cao * 3
                && (cao as f32) < cao_p * 1.6
            {
                p.chu.push(' ');
                p.chu.push_str(&d.chu);
                p.cao_dong = (p.cao_dong + d.cao_dong) / 2;
                p.dai_dong_cuoi = d.dai_dong_cuoi;
                let x1 = (p.hop[0] + p.hop[2]).max(d.hop[0] + d.hop[2]);
                let y1 = day_p.max(d.hop[1] + d.hop[3]);
                p.hop[0] = p.hop[0].min(d.hop[0]);
                p.hop[2] = x1 - p.hop[0];
                p.hop[3] = y1 - p.hop[1];
                continue 'ngoai;
            }
        }
        doan.push(d);
    }
    doan
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(chu: &str, y: i32, w: i32) -> Dong {
        Dong { chu: chu.into(), hop: [100, y, w, 20], cao_dong: 20, dai_dong_cuoi: chu.chars().count() }
    }
    #[test]
    fn menu_khong_gop_cau_dai_thi_gop() {
        let ra = gom_doan(vec![d("Play", 100, 60), d("Options", 124, 90), d("Quit", 148, 50)]);
        assert_eq!(ra.len(), 3);
        let ra = gom_doan(vec![d("The village is under attack and we", 100, 400), d("need to leave now!", 124, 220)]);
        assert_eq!(ra.len(), 1);
        assert_eq!(ra[0].chu, "The village is under attack and we need to leave now!");
    }
}
