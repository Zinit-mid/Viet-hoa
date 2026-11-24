//! Kiểu giao diện VIỆT HÓA (theo GiaoDien_VietHoa_ChoClaudeCode.md): nền tối có vệt sáng, mặt kính, font IBM Plex,
//! nút viên thuốc, NÚT TIẾN ĐỘ đổ màu theo %, công tắc, chip. Chỉ theme tối.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui::{self, epaint, Color32, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, TextStyle, Ui, Vec2};

// ---------------- màu ----------------
pub const NEN: Color32 = Color32::from_rgb(7, 9, 26);
pub const CHU: Color32 = Color32::from_rgb(244, 244, 244);
pub const CHU_PHU: Color32 = Color32::from_rgb(198, 198, 198);
pub const CHU_MO: Color32 = Color32::from_rgb(168, 168, 168);
pub const XANH: Color32 = Color32::from_rgb(69, 137, 255);
pub const XANH_SANG: Color32 = Color32::from_rgb(120, 169, 255);
pub const CYAN: Color32 = Color32::from_rgb(109, 240, 255);
pub const TIM: Color32 = Color32::from_rgb(185, 138, 255);
pub const CAM: Color32 = Color32::from_rgb(255, 154, 60);
pub const VANG: Color32 = Color32::from_rgb(245, 197, 66);
pub const XANH_BIEN: Color32 = Color32::from_rgb(79, 195, 255);
pub const OK: Color32 = Color32::from_rgb(66, 190, 101);
pub const LOI: Color32 = Color32::from_rgb(250, 77, 86);
pub const LOI_CHU: Color32 = Color32::from_rgb(255, 131, 137);

pub fn a(c: Color32, alpha: u8) -> Color32 {
    rgba(c.r(), c.g(), c.b(), alpha)
}

fn kinh_nen() -> Color32 {
    rgba(150, 120, 255, 15)
}
fn kinh_vien() -> Color32 {
    rgba(190, 150, 255, 70)
}

// ---------------- cài đặt giao diện ----------------
static ANH_SANG: AtomicBool = AtomicBool::new(true);

fn tep_cai_dat() -> std::path::PathBuf {
    crate::duong_dan::file_du_lieu("giao_dien.json")
}

pub fn nap_cai_dat() {
    if let Some(v) = std::fs::read(tep_cai_dat()).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
        ANH_SANG.store(v["anh_sang"].as_bool().unwrap_or(true), Ordering::Relaxed);
    }
}

pub fn anh_sang_bat() -> bool {
    ANH_SANG.load(Ordering::Relaxed)
}

pub fn dat_anh_sang(b: bool) {
    ANH_SANG.store(b, Ordering::Relaxed);
    let _ = std::fs::write(tep_cai_dat(), serde_json::json!({"anh_sang": b}).to_string());
}

// ---------------- font + kiểu chung ----------------
pub fn dam_ho() -> FontFamily {
    FontFamily::Name("dam".into())
}
pub fn nhe_ho() -> FontFamily {
    FontFamily::Name("nhe".into())
}

/// Chữ đậm (IBM Plex Sans SemiBold).
pub fn dam(s: impl Into<String>) -> RichText {
    RichText::new(s).family(dam_ho())
}

pub fn ap_dung(ctx: &egui::Context) {
    let mut f = egui::FontDefinitions::default();
    let nhung: [(&str, &[u8]); 5] = [
        ("plex", include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf")),
        ("plex_dam", include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf")),
        ("plex_nhe", include_bytes!("../assets/fonts/IBMPlexSans-Light.ttf")),
        ("plex_mono", include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")),
        ("viet", include_bytes!("../assets/fonts/BeVietnamPro-Regular.ttf")),
    ];
    for (ten, b) in nhung {
        f.font_data.insert(ten.into(), Arc::new(egui::FontData::from_static(b)));
    }
    let chen = |f: &mut egui::FontDefinitions, ho: FontFamily, ds: &[&str]| {
        let e = f.families.entry(ho).or_default();
        for (i, t) in ds.iter().enumerate() {
            e.insert(i, t.to_string());
        }
    };
    chen(&mut f, FontFamily::Proportional, &["plex", "viet"]);
    chen(&mut f, FontFamily::Monospace, &["plex_mono", "plex", "viet"]);
    chen(&mut f, dam_ho(), &["plex_dam", "plex", "viet"]);
    chen(&mut f, nhe_ho(), &["plex_nhe", "plex", "viet"]);
    // font dự phòng của egui (emoji, ký hiệu) nằm sau
    for ho in [dam_ho(), nhe_ho()] {
        let e = f.families.get_mut(&ho).unwrap();
        for t in ["Hack", "NotoEmoji-Regular", "emoji-icon-font"] {
            if f.font_data.contains_key(t) && !e.contains(&t.to_string()) {
                e.push(t.to_string());
            }
        }
    }
    ctx.set_fonts(f);

    ctx.style_mut(|s| {
        // cỡ chữ ≥ 14 (mục 2 của thiết kế)
        s.text_styles.insert(TextStyle::Body, FontId::new(16.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(17.0, dam_ho()));
        s.text_styles.insert(TextStyle::Small, FontId::new(14.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Monospace, FontId::new(15.0, FontFamily::Monospace));
        s.text_styles.insert(TextStyle::Heading, FontId::new(38.0, nhe_ho()));
        s.spacing.item_spacing = Vec2::new(10.0, 10.0);
        s.spacing.button_padding = Vec2::new(18.0, 9.0);
        s.spacing.interact_size = Vec2::new(40.0, 36.0);
        s.spacing.combo_width = 220.0;
        s.spacing.scroll.bar_width = 8.0;

        let v = &mut s.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = Color32::TRANSPARENT;
        v.window_fill = Color32::from_rgb(28, 20, 58);
        v.window_stroke = Stroke::new(1.0, kinh_vien());
        v.window_corner_radius = egui::CornerRadius::same(24);
        v.menu_corner_radius = egui::CornerRadius::same(16);
        v.extreme_bg_color = rgba(0, 0, 0, 90); // ô nhập
        v.faint_bg_color = rgba(255, 255, 255, 8);
        v.override_text_color = Some(CHU);
        v.hyperlink_color = CYAN;
        v.selection.bg_fill = Color32::from_rgb(70, 46, 160);
        v.selection.stroke = Stroke::new(1.5, a(TIM, 235));
        let bo = egui::CornerRadius::same(20);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, kinh_vien());
        v.widgets.noninteractive.corner_radius = bo;
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, CHU);
        v.widgets.inactive.bg_fill = rgba(175, 135, 255, 38);
        v.widgets.inactive.weak_bg_fill = rgba(175, 135, 255, 38);
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, rgba(200, 160, 255, 90));
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, CHU);
        v.widgets.inactive.corner_radius = bo;
        v.widgets.hovered.bg_fill = rgba(190, 150, 255, 60);
        v.widgets.hovered.weak_bg_fill = rgba(190, 150, 255, 60);
        v.widgets.hovered.bg_stroke = Stroke::new(1.5, a(TIM, 200));
        v.widgets.hovered.fg_stroke = Stroke::new(1.5, CHU);
        v.widgets.hovered.corner_radius = bo;
        v.widgets.active.bg_fill = rgba(190, 150, 255, 84);
        v.widgets.active.weak_bg_fill = rgba(190, 150, 255, 84);
        v.widgets.active.bg_stroke = Stroke::new(1.5, TIM);
        v.widgets.active.fg_stroke = Stroke::new(1.5, CHU);
        v.widgets.active.corner_radius = bo;
        v.widgets.open.corner_radius = bo;
        v.widgets.open.bg_fill = rgba(255, 255, 255, 30);
    });
}

/// Màu nền cửa sổ (đằng sau mọi thứ).
pub fn mau_nen() -> [f32; 4] {
    [7.0 / 255.0, 9.0 / 255.0, 26.0 / 255.0, 1.0]
}

/// Nền như ảnh mẫu HARIRI: đêm xanh đen, các mảng sáng màu (xanh ngọc, tím, xanh dương, vàng cam), bụi sao li ti,
/// vài tia sáng chéo. Vẽ ở lớp nền; tắt được ở Cài đặt > Giao diện. Gọi đầu mỗi khung hình.
pub fn nen_anh_sang(ctx: &egui::Context) {
    if !anh_sang_bat() {
        return;
    }
    let r = ctx.screen_rect();
    let p = ctx.layer_painter(egui::LayerId::background());
    let (w, h) = (r.width(), r.height());
    let vet = |tam: Pos2, ban_kinh: f32, mau: Color32| {
        let mut m = epaint::Mesh::default();
        m.colored_vertex(tam, mau);
        let n = 48;
        for i in 0..=n {
            let g = i as f32 / n as f32 * std::f32::consts::TAU;
            m.colored_vertex(tam + Vec2::angled(g) * ban_kinh, Color32::TRANSPARENT);
        }
        for i in 1..=n {
            m.add_triangle(0, i as u32, i as u32 + 1);
        }
        p.add(epaint::Shape::mesh(m));
    };
    let at = |fx: f32, fy: f32| Pos2::new(r.left() + w * fx, r.top() + h * fy);
    let b = w.max(h);
    // xanh dương phía trên giữa, tím trái trên, tím hồng bên phải, xanh ngọc dưới trái, vàng cam dưới phải
    vet(at(0.45, 0.0), b * 0.42, rgba(60, 110, 255, 56));
    vet(at(0.06, 0.08), b * 0.36, rgba(150, 80, 255, 72));
    vet(at(0.96, 0.32), b * 0.38, rgba(190, 90, 255, 64));
    vet(at(0.04, 0.96), b * 0.30, rgba(30, 205, 225, 56));
    vet(at(1.0, 0.88), b * 0.30, rgba(255, 170, 70, 46));
    vet(at(0.55, 1.0), b * 0.28, rgba(255, 120, 90, 22));
    vet(at(0.30, 0.55), b * 0.26, rgba(120, 70, 255, 22));

    // tia sáng chéo mảnh (góc trên phải, góc dưới trái)
    let tia = |a1: Pos2, a2: Pos2, al: u8| {
        p.line_segment([a1, a2], Stroke::new(1.0, rgba(190, 220, 255, al)));
    };
    for (i, (x2, y2)) in [(1.0, 0.18), (1.0, 0.26), (0.97, 0.34), (1.0, 0.10)].iter().enumerate() {
        tia(at(0.74 + i as f32 * 0.02, 0.0), at(*x2, *y2), 26 - i as u8 * 4);
    }
    for (i, x2) in [0.20f32, 0.24, 0.28].iter().enumerate() {
        tia(at(0.17 + i as f32 * 0.015, 1.0), at(*x2, 0.80 - i as f32 * 0.02), 22);
    }

    // bụi sao: vị trí cố định theo chỉ số (không chớp, không tốn CPU)
    let mut x: u32 = 0x9E37_79B9;
    let mut ngau = || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x % 10_000) as f32 / 10_000.0
    };
    for _ in 0..150 {
        let (fx, fy, k, d) = (ngau(), ngau(), ngau(), ngau());
        let mau = if k < 0.12 {
            rgba(120, 230, 255, 60 + (d * 110.0) as u8)
        } else if k < 0.2 {
            rgba(255, 200, 120, 60 + (d * 90.0) as u8)
        } else {
            rgba(235, 225, 255, 30 + (d * 120.0) as u8)
        };
        p.circle_filled(at(fx, fy), 0.7 + d * 1.0, mau);
    }
}

// ---------------- khối kính ----------------
static MAU_VIEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0xFFB98AFF);

/// Màu viền sáng của khối kính (theo chức năng của trang đang mở).
pub fn dat_mau_vien(c: Color32) {
    MAU_VIEN.store(u32::from_be_bytes([c.a(), c.r(), c.g(), c.b()]), Ordering::Relaxed);
}

fn mau_vien() -> Color32 {
    let [_, r, g, b] = MAU_VIEN.load(Ordering::Relaxed).to_be_bytes();
    Color32::from_rgb(r, g, b)
}

pub fn kinh_khung() -> egui::Frame {
    let m = mau_vien();
    egui::Frame::NONE
        .fill(kinh_nen())
        .stroke(Stroke::new(1.5, a(m, 175)))
        .corner_radius(egui::CornerRadius::same(24))
        .inner_margin(egui::Margin::same(20))
        // quầng sáng màu quanh khối (như viền neon của ảnh mẫu)
        .shadow(epaint::Shadow { offset: [0, 0], blur: 24, spread: 1, color: a(m, 42) })
}

/// Khối kính có tiêu đề (thẻ). Nếu thanh bên yêu cầu cuộn tới khối này thì cuộn tới.
pub fn khoi(ui: &mut Ui, tieu_de: &str, noi: impl FnOnce(&mut Ui)) {
    let r = kinh_khung().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(dam(tieu_de).size(20.0).color(CHU));
        ui.add_space(4.0);
        noi(ui);
    });
    cuon_neu_can(ui, tieu_de, &r.response);
    ui.add_space(14.0);
}

fn cuon_neu_can(ui: &Ui, tieu_de: &str, r: &Response) {
    let id = egui::Id::new("cuon_den");
    let can: Option<String> = ui.ctx().data(|d| d.get_temp(id));
    if let Some(t) = can {
        if tieu_de.to_lowercase().starts_with(&t.to_lowercase()) {
            r.scroll_to_me(Some(egui::Align::Min));
            ui.ctx().data_mut(|d| d.remove::<String>(id));
        }
    }
}

/// Yêu cầu cuộn tới khối có tiêu đề bắt đầu bằng `t` ở khung hình kế tiếp.
pub fn yeu_cau_cuon(ctx: &egui::Context, t: &str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("cuon_den"), t.to_string()));
}

// ---------------- nút viên thuốc ----------------
fn ve_pill(ui: &mut Ui, rect: Rect, ph: &Response, nhan: &str, mau: Color32, bat: bool) {
    let p = ui.painter();
    let ban_kinh = rect.height() / 2.0;
    let sang = if ph.hovered() && bat { 1.25 } else { 1.0 };
    let nen = Color32::from_rgb((34.0 * sang) as u8, (24.0 * sang) as u8, (70.0 * sang) as u8);
    p.rect_filled(rect.translate(Vec2::new(0.0, 6.0)).shrink(6.0), ban_kinh, a(mau, if bat { 60 } else { 15 })); // bóng màu
    p.rect_filled(rect, ban_kinh, nen);
    p.rect_stroke(rect, ban_kinh, Stroke::new(if ph.has_focus() { 2.0 } else { 1.2 }, if ph.has_focus() { Color32::WHITE } else { a(mau, if bat { 190 } else { 70 }) }), egui::StrokeKind::Inside);
    let m = if bat { CHU } else { CHU_MO };
    p.text(rect.center(), egui::Align2::CENTER_CENTER, nhan, FontId::new(17.0, dam_ho()), m);
}

/// Nút viên thuốc có viền / bóng màu `mau`. Cao 48.
pub fn pill(ui: &mut Ui, nhan: &str, mau: Color32, bat: bool) -> Response {
    let gal = ui.painter().layout_no_wrap(nhan.to_string(), FontId::new(17.0, dam_ho()), CHU);
    let size = Vec2::new((gal.size().x + 48.0).max(120.0), 48.0);
    let (rect, mut ph) = ui.allocate_exact_size(size, if bat { Sense::click() } else { Sense::hover() });
    ph = ph.on_hover_cursor(if bat { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Default });
    ve_pill(ui, rect, &ph, nhan, mau, bat);
    ph
}

/// Nút phụ (nền mờ, không bóng màu).
pub fn pill_phu(ui: &mut Ui, nhan: &str, bat: bool) -> Response {
    let gal = ui.painter().layout_no_wrap(nhan.to_string(), FontId::new(16.0, dam_ho()), CHU);
    let size = Vec2::new((gal.size().x + 40.0).max(96.0), 44.0);
    let (rect, ph) = ui.allocate_exact_size(size, if bat { Sense::click() } else { Sense::hover() });
    let ban_kinh = rect.height() / 2.0;
    let p = ui.painter();
    p.rect_filled(rect, ban_kinh, rgba(255, 255, 255, if ph.hovered() && bat { 34 } else { 20 }));
    p.rect_stroke(rect, ban_kinh, Stroke::new(1.0, rgba(255, 255, 255, 60)), egui::StrokeKind::Inside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, nhan, FontId::new(16.0, dam_ho()), if bat { CHU } else { CHU_MO });
    ph.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Nút nguy hiểm (viền đỏ).
pub fn pill_nguy_hiem(ui: &mut Ui, nhan: &str, bat: bool) -> Response {
    let gal = ui.painter().layout_no_wrap(nhan.to_string(), FontId::new(16.0, dam_ho()), CHU);
    let size = Vec2::new((gal.size().x + 40.0).max(96.0), 44.0);
    let (rect, ph) = ui.allocate_exact_size(size, if bat { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    p.rect_filled(rect, rect.height() / 2.0, a(LOI, if ph.hovered() { 40 } else { 25 }));
    p.rect_stroke(rect, rect.height() / 2.0, Stroke::new(1.2, LOI), egui::StrokeKind::Inside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, nhan, FontId::new(16.0, dam_ho()), LOI_CHU);
    ph.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------- NÚT TIẾN ĐỘ ----------------
/// Nút việc dài: màu đổ đầy từ trái sang phải theo % THẬT (từ thanh tiến trình toàn cục), không hiệu ứng chạy vòng.
/// Chờ → "Đang … 37%" → "Xong 100%" (xanh lá ~2,2 s) / "Lỗi – bấm để xem". Bấm khi đang chạy = dừng.
/// `khoa`: khóa nguồn tiến trình đã đăng ký (xem `tien_trinh::dang_ky`). → true khi người dùng bấm BẮT ĐẦU việc.
pub fn nut_tien_do(ui: &mut Ui, khoa: &'static str, nhan: &str, dong_tu: &str, mau: Color32, bat: bool) -> bool {
    let id_nut = egui::Id::new(("nut_viec", khoa));
    let dang_cua_nut: Option<String> = ui.ctx().data(|d| d.get_temp(id_nut));
    let la_nut_nay = dang_cua_nut.as_deref() == Some(nhan);
    let th = crate::tien_trinh::tinh_hinh(khoa);
    let ban = th.as_ref().is_some_and(|t| t.dang_ban);
    let hien_xong = th
        .as_ref()
        .and_then(|t| t.xong.as_ref())
        .filter(|x| x.2.elapsed().as_millis() < if x.1 { 6000 } else { 2200 })
        .cloned();

    // trạng thái nút này
    enum Tt {
        Cho,
        Chay(Option<f32>),
        Xong,
        Loi,
        Khoa,
    }
    let tt = if ban && la_nut_nay {
        Tt::Chay(th.as_ref().and_then(|t| t.tien))
    } else if ban {
        Tt::Khoa
    } else if la_nut_nay && hien_xong.as_ref().is_some_and(|x| x.1) {
        Tt::Loi
    } else if la_nut_nay && hien_xong.is_some() {
        Tt::Xong
    } else {
        Tt::Cho
    };

    let chu = match &tt {
        Tt::Cho | Tt::Khoa => nhan.to_string(),
        Tt::Chay(Some(p)) => format!("{dong_tu}… {:.0}%", p * 100.0),
        Tt::Chay(None) => format!("{dong_tu}… {}", dung_mm(th.as_ref().map(|t| t.giay).unwrap_or(0))),
        Tt::Xong => "Xong 100%".to_string(),
        Tt::Loi => "Lỗi – bấm để xem".to_string(),
    };
    let gal = ui.painter().layout_no_wrap(chu.clone(), FontId::new(17.0, dam_ho()), CHU);
    let size = Vec2::new((gal.size().x + 56.0).max(190.0), 52.0);
    let sense = match tt {
        Tt::Khoa => Sense::hover(),
        _ => Sense::click(),
    };
    let (rect, ph) = ui.allocate_exact_size(size, if bat || matches!(tt, Tt::Chay(_) | Tt::Loi) { sense } else { Sense::hover() });
    let ban_kinh = rect.height() / 2.0;
    let mau_hien = match tt {
        Tt::Xong => OK,
        Tt::Loi => LOI,
        _ => mau,
    };
    let sang = matches!(tt, Tt::Chay(_));
    let p = ui.painter();
    p.rect_filled(rect.translate(Vec2::new(0.0, 6.0)).shrink(8.0), ban_kinh, a(mau_hien, if sang { 100 } else if bat { 50 } else { 12 }));
    p.rect_filled(rect, ban_kinh, Color32::from_rgb(28, 20, 60));
    // phần đổ màu
    let phan = match tt {
        Tt::Chay(Some(x)) => x.clamp(0.0, 1.0),
        Tt::Xong => 1.0,
        Tt::Loi => th.as_ref().and_then(|t| t.tien).unwrap_or(0.0).clamp(0.0, 1.0),
        _ => 0.0,
    };
    if phan > 0.0 {
        let w = (rect.width() * phan).max(ban_kinh * 2.0).min(rect.width());
        let r2 = Rect::from_min_size(rect.min, Vec2::new(w, rect.height()));
        let mut m = epaint::Mesh::default();
        // gradient ngang <màu>33 → <màu>88 trong hình viên thuốc (xấp xỉ: dải chữ nhật bo ở hai đầu)
        let (c1, c2) = (a(mau_hien, 0x33), a(mau_hien, 0x88));
        let n = 24;
        let mut diem_tren = Vec::new();
        let mut diem_duoi = Vec::new();
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let x = r2.left() + t * r2.width();
            // bán kính cắt theo hình viên thuốc ở hai đầu
            let dx = if x < rect.left() + ban_kinh { rect.left() + ban_kinh - x } else if x > r2.right() - ban_kinh && w < rect.width() - 0.5 { 0.0 } else if x > rect.right() - ban_kinh { x - (rect.right() - ban_kinh) } else { 0.0 };
            let dy = (ban_kinh * ban_kinh - dx * dx).max(0.0).sqrt();
            let mau_x = lerp_mau(c1, c2, t);
            diem_tren.push((Pos2::new(x, rect.center().y - dy), mau_x));
            diem_duoi.push((Pos2::new(x, rect.center().y + dy), mau_x));
        }
        for i in 0..=n {
            m.colored_vertex(diem_tren[i].0, diem_tren[i].1);
            m.colored_vertex(diem_duoi[i].0, diem_duoi[i].1);
        }
        for i in 0..n as u32 {
            let b = i * 2;
            m.add_triangle(b, b + 1, b + 2);
            m.add_triangle(b + 1, b + 3, b + 2);
        }
        p.add(epaint::Shape::mesh(m));
        if phan < 0.999 {
            p.line_segment([Pos2::new(r2.right(), rect.top() + 6.0), Pos2::new(r2.right(), rect.bottom() - 6.0)], Stroke::new(2.0, mau_hien));
        }
    }
    let vien = if sang { a(mau_hien, 0x99) } else if bat || matches!(tt, Tt::Xong | Tt::Loi) { a(mau_hien, 170) } else { a(mau_hien, 60) };
    p.rect_stroke(rect, ban_kinh, Stroke::new(if ph.has_focus() { 2.0 } else { 1.5 }, if ph.has_focus() { Color32::WHITE } else { vien }), egui::StrokeKind::Inside);
    let mau_chu = if bat || !matches!(tt, Tt::Cho) { CHU } else { CHU_MO };
    p.text(rect.center(), egui::Align2::CENTER_CENTER, chu, FontId::new(17.0, dam_ho()), mau_chu);

    let mut bam = false;
    if ph.clicked() {
        match tt {
            Tt::Chay(_) => crate::tien_trinh::dung_viec(khoa),
            Tt::Khoa => {}
            _ if bat => {
                ui.ctx().data_mut(|d| d.insert_temp(id_nut, nhan.to_string()));
                bam = true;
            }
            _ => {}
        }
    }
    if matches!(tt, Tt::Loi) {
        ph.on_hover_text(th.as_ref().map(|t| t.thong_bao.clone()).unwrap_or_default());
    }
    bam
}

fn lerp_mau(a1: Color32, a2: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    rgba(l(a1.r(), a2.r()), l(a1.g(), a2.g()), l(a1.b(), a2.b()), l(a1.a(), a2.a()))
}

pub fn dung_mm(giay: u64) -> String {
    format!("{}:{:02}", giay / 60, giay % 60)
}

// ---------------- công tắc ----------------
/// Công tắc 72×36: bật = nền xanh chàm + bóng; tắt = nền tối. `role=switch` qua widget_info.
pub fn cong_tac(ui: &mut Ui, bat: &mut bool, nhan: &str) -> Response {
    // trong hàng ngang / hàng tự xuống dòng thì vẽ thẳng (để còn xuống dòng được), ngoài ra bọc một hàng ngang
    if ui.layout().main_dir().is_horizontal() {
        return cong_tac_trong(ui, bat, nhan);
    }
    ui.horizontal(|ui| cong_tac_trong(ui, bat, nhan)).inner
}

fn cong_tac_trong(ui: &mut Ui, bat: &mut bool, nhan: &str) -> Response {
    let (rect, mut ph) = ui.allocate_exact_size(Vec2::new(72.0, 36.0), Sense::click());
    if ph.clicked() {
        *bat = !*bat;
        ph.mark_changed();
    }
    ph.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *bat, nhan));
    let t = ui.ctx().animate_bool_with_time(ph.id, *bat, 0.12);
    let p = ui.painter();
    if *bat {
        p.rect_filled(rect.translate(Vec2::new(0.0, 6.0)).shrink(6.0), 18.0, rgba(140, 80, 255, 120));
        let (c1, c2) = (Color32::from_rgb(44, 40, 150), Color32::from_rgb(130, 76, 240));
        p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(36.0, 36.0)), 18.0, c1);
        p.rect_filled(Rect::from_min_size(rect.min + Vec2::new(36.0, 0.0), Vec2::new(36.0, 36.0)), 18.0, c2);
        let mut m = epaint::Mesh::default();
        m.colored_vertex(rect.left_top() + Vec2::new(18.0, 0.0), c1);
        m.colored_vertex(rect.left_bottom() + Vec2::new(18.0, 0.0), c1);
        m.colored_vertex(rect.right_top() - Vec2::new(18.0, 0.0), c2);
        m.colored_vertex(rect.right_bottom() - Vec2::new(18.0, 0.0), c2);
        m.add_triangle(0, 1, 2);
        m.add_triangle(1, 3, 2);
        p.add(epaint::Shape::mesh(m));
    } else {
        p.rect_filled(rect, 18.0, Color32::from_rgb(24, 16, 52));
    }
    p.rect_stroke(rect, 18.0, Stroke::new(1.2, rgba(255, 255, 255, 70)), egui::StrokeKind::Inside);
    let x = rect.left() + 18.0 + t * 34.0;
    p.circle_filled(Pos2::new(x, rect.center().y), 14.0, Color32::WHITE);
    ui.label(nhan);
    ph.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------- chip ----------------
/// Chip chọn: được chọn = nền xanh chàm + viền sáng + bóng xanh; chưa chọn = nền mờ.
pub fn chip(ui: &mut Ui, chon: bool, nhan: &str) -> Response {
    let gal = ui.painter().layout_no_wrap(nhan.to_string(), FontId::new(16.0, if chon { dam_ho() } else { FontFamily::Proportional }), CHU);
    let size = Vec2::new((gal.size().x + 36.0).max(72.0), 44.0);
    let (rect, ph) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let br = rect.height() / 2.0;
    if chon {
        p.rect_filled(rect.translate(Vec2::new(0.0, 5.0)).shrink(6.0), br, rgba(150, 90, 255, 110));
        p.rect_filled(rect, br, Color32::from_rgb(52, 34, 130));
        p.rect_stroke(rect, br, Stroke::new(1.4, a(TIM, 235)), egui::StrokeKind::Inside);
    } else {
        p.rect_filled(rect, br, rgba(255, 255, 255, if ph.hovered() { 24 } else { 15 }));
        p.rect_stroke(rect, br, Stroke::new(1.0, rgba(255, 255, 255, 50)), egui::StrokeKind::Inside);
    }
    p.text(rect.center(), egui::Align2::CENTER_CENTER, nhan, FontId::new(16.0, if chon { dam_ho() } else { FontFamily::Proportional }), CHU);
    ph.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Nhóm chip chọn một: trả true nếu đổi.
pub fn chip_chon<T: PartialEq + Clone>(ui: &mut Ui, gia_tri: &mut T, ds: &[(T, &str)]) -> bool {
    let mut doi = false;
    ui.horizontal_wrapped(|ui| {
        for (v, nhan) in ds {
            if chip(ui, gia_tri == v, nhan).clicked() && gia_tri != v {
                *gia_tri = v.clone();
                doi = true;
            }
        }
    });
    doi
}

/// Thanh tiến độ nhỏ (cao 8, bo 4) kèm % bên phải; `mau` theo chức năng.
pub fn thanh_nho(ui: &mut Ui, tien: Option<f32>, mau: Color32) {
    let w = ui.available_width();
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new((w - 56.0).max(40.0), 8.0), Sense::hover());
        ui.painter().rect_filled(rect, 4.0, rgba(255, 255, 255, 30));
        if let Some(t) = tien {
            let r = Rect::from_min_size(rect.min, Vec2::new(rect.width() * t.clamp(0.0, 1.0), 8.0));
            ui.painter().rect_filled(r, 4.0, mau);
            ui.label(RichText::new(format!("{:.0}%", t * 100.0)).small());
        } else {
            ui.label(RichText::new("…").small());
        }
    });
}

pub fn bat_dau_chuong() -> Instant {
    Instant::now()
}

/// Màu RGBA (alpha KHÔNG nhân sẵn) → Color32 nhân alpha trong không gian gamma, như CSS.
/// (`Color32::from_rgba_unmultiplied` của egui nhân trong không gian tuyến tính nên màu mờ bị sáng hơn nhiều.)
pub fn rgba(r: u8, g: u8, b: u8, al: u8) -> Color32 {
    let f = |x: u8| ((x as u32 * al as u32 + 127) / 255) as u8;
    Color32::from_rgba_premultiplied(f(r), f(g), f(b), al)
}
