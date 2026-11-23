//! Quản lý bộ chụp: mất quyền chụp thì bỏ hẳn bộ cũ, chờ dần (1 s, 2 s, 5 s) rồi
//! mới tạo lại; lỗi 5 lần liền → chụp GDI, cứ 30 s thử lại DXGI một lần.

use std::time::{Duration, Instant};

use crate::chup;
use crate::dich_man_hinh::Chung;

fn nk(s: &str) {
    if !cfg!(test) {
        crate::duong_dan::nhat_ky(s);
    }
}

// ---------------- quản lý bộ chụp ----------------

/// Giữ bộ chụp; mất quyền chụp thì bỏ hẳn bộ cũ, chờ dần (1 s, 2 s, 5 s) rồi mới tạo lại.
/// Lỗi 5 lần liền → chụp GDI, cứ 30 s thử lại DXGI một lần.
#[derive(Default)]
pub struct QuanLyChup {
    pub may: Option<Box<dyn chup::MayChup + Send>>,
    loi_lien: u32,
    cho_den: Option<Instant>,
    gdi_tu: Option<Instant>,
    /// Hàm tạo bộ chụp nhanh (để kiểm thử thay bằng bộ giả)
    tao_nhanh: Option<fn() -> anyhow::Result<Box<dyn chup::MayChup + Send>>>,
}

impl QuanLyChup {
    pub fn tha(&mut self) {
        if self.may.take().is_some() {
            nk("Thả bộ chụp (không dịch)");
        }
        self.loi_lien = 0;
        self.cho_den = None;
        self.gdi_tu = None;
    }

    pub fn lay_may(&mut self, chung: &Chung) -> Option<&mut Box<dyn chup::MayChup + Send>> {
        // Đang dùng GDI quá 30 s → thử lại DXGI
        if let Some(t) = self.gdi_tu {
            if t.elapsed() >= Duration::from_secs(30) {
                self.may = None;
                self.gdi_tu = None;
                self.loi_lien = 4; // lỗi thêm 1 lần là quay lại GDI ngay
            }
        }
        if self.may.is_none() {
            if self.cho_den.is_some_and(|t| Instant::now() < t) {
                return None;
            }
            if self.gdi_tu.is_some() {
                self.may = Some(Box::new(chup::ChupGdi));
            } else {
                let tao = self.tao_nhanh.unwrap_or(|| Ok(Box::new(chup::ChupDxgi::moi()?)));
                match tao() {
                    Ok(m) => self.may = Some(m),
                    Err(e) => {
                        self.loi(chung, &format!("không tạo được DXGI: {e}"));
                        return self.may.as_mut();
                    }
                }
            }
            let ten = self.may.as_ref().map(|m| m.ten()).unwrap_or("");
            nk(&format!("Chụp bằng {ten}"));
            chung.tt().may_chup = ten.to_string();
        }
        self.may.as_mut()
    }

    pub fn thanh_cong(&mut self, chung: &Chung) {
        if self.loi_lien > 0 && self.gdi_tu.is_none() {
            nk("Chụp lại được bình thường");
            chung.tt().loi.clear();
        }
        if self.gdi_tu.is_none() {
            self.loi_lien = 0;
        }
    }

    pub fn loi(&mut self, chung: &Chung, e: &str) {
        self.may = None; // bỏ hẳn bộ cũ trước khi tạo lại
        self.loi_lien += 1;
        if self.loi_lien == 1 {
            nk(&format!("Chụp lỗi: {e} – sẽ thử lại"));
        }
        if self.loi_lien >= 5 {
            if self.gdi_tu.is_none() {
                nk("Lỗi chụp 5 lần liền → chuyển sang chụp GDI, 30 giây sau thử lại DXGI");
            }
            self.gdi_tu = Some(Instant::now());
            self.cho_den = None;
            chung.tt().loi = "Chụp nhanh (DXGI) đang lỗi → tạm dùng GDI. Game nên để Borderless / Windowed.".into();
            return;
        }
        let cho = match self.loi_lien {
            1 => 1,
            2 => 2,
            _ => 5,
        };
        self.cho_den = Some(Instant::now() + Duration::from_secs(cho));
        chung.tt().loi = format!("Chụp lỗi: {e} – thử lại sau {cho} giây");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cau_hinh::CauHinh;
    use crate::chup::Anh;

    struct Hong;
    impl chup::MayChup for Hong {
        fn cap_nhat(&mut self) -> anyhow::Result<bool> {
            Err(anyhow::anyhow!("DXGI mất quyền chụp"))
        }
        fn lay(&mut self, _: [i32; 4]) -> Option<Anh> {
            None
        }
        fn kich_thuoc(&self) -> (u32, u32) {
            (0, 0)
        }
        fn ten(&self) -> &'static str {
            "DXGI"
        }
    }

    #[test]
    fn mat_quyen_chup_cho_dan_roi_sang_gdi() {
        let chung = Chung::moi(CauHinh::default());
        let mut ql = QuanLyChup { tao_nhanh: Some(|| Ok(Box::new(Hong))), ..Default::default() };
        let mut so_lan_tao = 0;
        let mut cho = Vec::new();
        for _ in 0..5 {
            // bỏ qua thời gian chờ cho nhanh
            if let Some(t) = ql.cho_den {
                cho.push((t - Instant::now()).as_secs_f32().round() as u32);
                ql.cho_den = Some(Instant::now());
            }
            let m = ql.lay_may(&chung).expect("phải tạo được bộ chụp");
            so_lan_tao += 1;
            let e = m.cap_nhat().unwrap_err();
            ql.loi(&chung, &e.to_string());
            assert!(ql.may.is_none(), "bộ cũ phải bị bỏ hẳn");
        }
        assert_eq!(so_lan_tao, 5);
        assert_eq!(cho, vec![1, 2, 5, 5]);
        // Trong lúc chờ không được tạo bộ chụp mới
        let mut ql2 = QuanLyChup { tao_nhanh: Some(|| Ok(Box::new(Hong))), ..Default::default() };
        ql2.lay_may(&chung);
        ql2.loi(&chung, "x");
        assert!(ql2.lay_may(&chung).is_none());
        // Sau 5 lần: dùng GDI
        assert!(ql.gdi_tu.is_some());
        assert_eq!(ql.lay_may(&chung).unwrap().ten(), "GDI");
        // 30 giây sau thử lại DXGI; hỏng thêm 1 lần là quay lại GDI ngay
        ql.gdi_tu = Some(Instant::now() - Duration::from_secs(31));
        assert_eq!(ql.lay_may(&chung).unwrap().ten(), "DXGI");
        let e = ql.may.as_mut().unwrap().cap_nhat().unwrap_err();
        ql.loi(&chung, &e.to_string());
        assert_eq!(ql.lay_may(&chung).unwrap().ten(), "GDI");
        // Không dịch: thả bộ chụp
        ql.tha();
        assert!(ql.may.is_none() && ql.gdi_tu.is_none());
    }
}
