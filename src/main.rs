#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Việt Hóa – dịch màn hình game Anh→Việt, Việt hóa file game, lồng tiếng, phụ đề phim.

mod cau_hinh;
mod che_mau;
mod chup;
mod dich;
mod dich_man_hinh;
mod duong_dan;
mod giao_dien;
mod giong_doc;
mod lop_phu;
mod ocr;

use std::sync::mpsc::channel;

use eframe::egui;

fn main() -> eframe::Result<()> {
    unsafe {
        use windows::Win32::UI::HiDpi::*;
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let bao_chuyen = duong_dan::chuyen_du_lieu_cu();
    let ch = cau_hinh::CauHinh::nap();
    let chung = dich_man_hinh::Chung::moi(ch);
    // Tùy chọn khởi động: --dich (bật dịch vùng), --f12 (bật dịch cả màn hình)
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--dich" => chung.dang_dich.store(true, std::sync::atomic::Ordering::SeqCst),
            "--f12" => chung.toan_man.store(true, std::sync::atomic::Ordering::SeqCst),
            _ => {}
        }
    }

    lop_phu::nap_font();
    let (gui, nhan) = channel();
    // Phím tắt → luồng dịch
    let (gui_phim, nhan_phim) = channel();
    lop_phu::khoi_dong(gui_phim);
    {
        let gui = gui.clone();
        std::thread::spawn(move || {
            for p in nhan_phim {
                if gui.send(dich_man_hinh::Lenh::Phim(p)).is_err() {
                    break;
                }
            }
        });
    }
    {
        let (chung, gui) = (chung.clone(), gui.clone());
        std::thread::spawn(move || dich_man_hinh::chay(chung, nhan, gui));
    }

    let tuy_chon = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Việt Hóa")
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Việt Hóa",
        tuy_chon,
        Box::new(move |cc| Ok(Box::new(giao_dien::App::moi(cc, chung, gui, bao_chuyen)))),
    )
}
