//! Kiểu giao diện VIỆT HÓA (theo GiaoDien_VietHoa_ChoClaudeCode.md): nền tối có vệt sáng, mặt kính, font IBM Plex,
//! nút viên thuốc, NÚT TIẾN ĐỘ đổ màu theo %, công tắc, chip. Chỉ theme tối.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui::{self, epaint, Color32, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, TextStyle, Ui, Vec2};

// ---------------- màu ----------------
pub const NEN: Color32 = Color32::from_rgb(12, 8, 22);
pub const CHU: Color32 = Color32::from_rgb(244, 244, 244);
pub const CHU_PHU: Color32 = Color32::from_rgb(198, 198, 198);
pub const CHU_MO: Color32 = Color32::from_rgb(168, 168, 168);
pub const XANH: Color32 = Color32::from_rgb(69, 137, 255);
pub const XANH_SANG: Color32 = Color32::from_rgb(120, 169, 255);
pub const CYAN: Color32 = Color32::from_rgb(109, 240, 255);
pub const TIM: Color32 = Color32::from_rgb(156, 104, 242);
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
    rgba(30, 16, 54, 175)
}
fn kinh_vien() -> Color32 {
    rgba(140, 96, 220, 96)
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
        s.spacing.button_padding = Vec2::new(22.0, 11.0);
        s.spacing.slider_rail_height = 8.0;
        s.visuals.handle_shape = egui::style::HandleShape::Circle;
        s.spacing.interact_size = Vec2::new(40.0, 36.0);
        s.spacing.combo_width = 220.0;
        s.spacing.scroll.bar_width = 8.0;

        let v = &mut s.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = Color32::TRANSPARENT;
        v.window_fill = Color32::from_rgb(22, 14, 40);
        v.window_stroke = Stroke::new(1.0, kinh_vien());
        v.window_corner_radius = egui::CornerRadius::same(24);
        v.menu_corner_radius = egui::CornerRadius::same(16);
        v.extreme_bg_color = rgba(0, 0, 0, 90); // ô nhập
        v.faint_bg_color = rgba(255, 255, 255, 8);
        v.override_text_color = Some(CHU);
        v.hyperlink_color = CYAN;
        v.selection.bg_fill = Color32::from_rgb(62, 34, 124);
        v.selection.stroke = Stroke::new(1.5, a(TIM, 235));
        let bo = egui::CornerRadius::same(26);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, kinh_vien());
        v.widgets.noninteractive.corner_radius = bo;
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, CHU);
        v.widgets.inactive.bg_fill = rgba(20, 12, 40, 235);
        v.widgets.inactive.weak_bg_fill = rgba(20, 12, 40, 235);
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, rgba(150, 104, 235, 130));
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, CHU);
        v.widgets.inactive.corner_radius = bo;
        v.widgets.hovered.bg_fill = rgba(50, 28, 94, 245);
        v.widgets.hovered.weak_bg_fill = rgba(50, 28, 94, 245);
        v.widgets.hovered.bg_stroke = Stroke::new(1.5, a(TIM, 200));
        v.widgets.hovered.fg_stroke = Stroke::new(1.5, CHU);
        v.widgets.hovered.corner_radius = bo;
        v.widgets.active.bg_fill = rgba(68, 38, 124, 250);
        v.widgets.active.weak_bg_fill = rgba(68, 38, 124, 250);
        v.widgets.active.bg_stroke = Stroke::new(1.5, TIM);
        v.widgets.active.fg_stroke = Stroke::new(1.5, CHU);
        v.widgets.active.corner_radius = bo;
        v.widgets.open.corner_radius = bo;
        v.widgets.open.bg_fill = rgba(255, 255, 255, 30);
    });
}

/// Màu nền cửa sổ (đằng sau mọi thứ).
pub fn mau_nen() -> [f32; 4] {
    [12.0 / 255.0, 8.0 / 255.0, 22.0 / 255.0, 1.0]
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
    vet(at(0.45, 0.0), b * 0.42, rgba(70, 70, 210, 40));
    vet(at(0.06, 0.08), b * 0.36, rgba(120, 56, 215, 58));
    vet(at(0.96, 0.32), b * 0.38, rgba(150, 64, 215, 50));
    vet(at(0.04, 0.96), b * 0.30, rgba(24, 150, 170, 38));
    vet(at(1.0, 0.88), b * 0.30, rgba(210, 130, 60, 30));
    vet(at(0.55, 1.0), b * 0.28, rgba(200, 80, 90, 16));
    vet(at(0.30, 0.55), b * 0.26, rgba(100, 50, 220, 16));

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
        .shadow(epaint::Shadow { offset: [0, 0], blur: 22, spread: 0, color: a(m, 30) })
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

// ---------------- nút kính neon ----------------
/// Biểu tượng (glyph) hợp với nhãn nút.
pub fn icon_cho(nhan: &str) -> &'static str {
    let n = nhan.to_lowercase();
    let co = |k: &[&str]| k.iter().any(|x| n.contains(x));
    if co(&["dừng", "tắt"]) {
        "■"
    } else if co(&["lồng", "giọng", "nghe"]) {
        "🔊"
    } else if co(&["dịch"]) {
        "🌐"
    } else if co(&["cài", "tải", "nhập", "nạp"]) {
        "⬇"
    } else if co(&["xuất", "ghi", "lưu"]) {
        "💾"
    } else if co(&["quét", "tìm", "dò"]) {
        "🔍"
    } else if co(&["tạo", "thuyết", "phụ đề"]) {
        "✨"
    } else if co(&["xử lý", "thêm", "gán"]) {
        "⚙"
    } else if co(&["xóa", "gỡ", "trả"]) {
        "✖"
    } else {
        "▶"
    }
}

/// Nền nút kính đen có quầng sáng màu (viền neon như ảnh mẫu). `cuong` 0..1.5 = độ sáng.
fn ve_neon(p: &egui::Painter, rect: Rect, mau: Color32, cuong: f32) {
    let br = rect.height() / 2.0;
    for (e, al) in [(9.0f32, 9.0f32), (5.0, 15.0), (2.5, 26.0)] {
        p.rect_filled(rect.expand(e), br + e, a(mau, (al * cuong).min(120.0) as u8));
    }
    p.rect_filled(rect, br, Color32::from_rgb(10, 6, 20));
    // ánh sáng màu ở nửa dưới
    for i in 0..7 {
        let hh = rect.height() * 0.5 * (1.0 - i as f32 / 7.0);
        let r = Rect::from_min_max(Pos2::new(rect.left() + br * 0.55, rect.bottom() - 3.0 - hh), Pos2::new(rect.right() - br * 0.55, rect.bottom() - 3.0));
        p.rect_filled(r, (br * 0.7).min(hh / 2.0 + 2.0), a(mau, (11.0 * cuong).min(60.0) as u8));
    }
    // bóng kính phía trên + viền trong mờ
    let g = Rect::from_min_max(Pos2::new(rect.left() + br * 0.7, rect.top() + 4.0), Pos2::new(rect.right() - br * 0.7, rect.center().y - 2.0));
    p.rect_filled(g, 10.0, rgba(255, 255, 255, 16));
    p.rect_stroke(rect.shrink(4.5), br - 4.5, Stroke::new(1.0, rgba(255, 255, 255, 34)), egui::StrokeKind::Inside);
}

/// Màu phụ đi cùng màu chính để viền đổi màu dần (trái = phụ → phải = chính), như nút mẫu.
fn mau_phu(m: Color32) -> Color32 {
    if m == TIM {
        Color32::from_rgb(70, 130, 255)
    } else if m == CAM {
        Color32::from_rgb(255, 70, 70)
    } else if m == VANG {
        Color32::from_rgb(255, 140, 50)
    } else if m == XANH_BIEN {
        Color32::from_rgb(150, 100, 255)
    } else if m == OK {
        Color32::from_rgb(60, 220, 255)
    } else if m == LOI {
        Color32::from_rgb(255, 150, 60)
    } else if m == CYAN {
        Color32::from_rgb(110, 140, 255)
    } else {
        Color32::from_rgb(150, 100, 255)
    }
}

/// Điểm + pháp tuyến ngoài dọc đường viền hình viên thuốc (theo chiều kim đồng hồ từ cạnh trên bên trái).
fn duong_vien(rc: Rect) -> Vec<(Pos2, Vec2)> {
    let br = rc.height() / 2.0;
    let (cl, cr, cy) = (rc.left() + br, rc.right() - br, rc.center().y);
    let mut v = Vec::new();
    let (nb, nd) = (22, 14);
    for i in 0..=nd {
        let x = cl + (cr - cl) * i as f32 / nd as f32;
        v.push((Pos2::new(x, rc.top()), Vec2::new(0.0, -1.0)));
    }
    for i in 1..nb {
        let g = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / nb as f32;
        let d = Vec2::new(g.cos(), g.sin());
        v.push((Pos2::new(cr, cy) + d * br, d));
    }
    for i in 0..=nd {
        let x = cr - (cr - cl) * i as f32 / nd as f32;
        v.push((Pos2::new(x, rc.bottom()), Vec2::new(0.0, 1.0)));
    }
    for i in 1..nb {
        let g = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / nb as f32;
        let d = Vec2::new(g.cos(), g.sin());
        v.push((Pos2::new(cl, cy) + d * br, d));
    }
    v
}

/// Viền neon đổi màu dần theo chiều ngang (c_trai → c_phai).
fn gradient_vien(p: &egui::Painter, rect: Rect, c_trai: Color32, c_phai: Color32, dam: f32) {
    let rc = rect.shrink(dam / 2.0);
    let pts = duong_vien(rc);
    let mut m = epaint::Mesh::default();
    let w = rc.width().max(1.0);
    for (pt, n) in &pts {
        let t = ((pt.x - rc.left()) / w).clamp(0.0, 1.0);
        let c = lerp_mau(c_trai, c_phai, t);
        m.colored_vertex(*pt - *n * (dam / 2.0), c);
        m.colored_vertex(*pt + *n * (dam / 2.0), c);
    }
    let k = pts.len() as u32;
    for i in 0..k {
        let (a0, a1) = (i * 2, ((i + 1) % k) * 2);
        m.add_triangle(a0, a0 + 1, a1);
        m.add_triangle(a0 + 1, a1 + 1, a1);
    }
    p.add(epaint::Shape::mesh(m));
}

fn ve_vien(p: &egui::Painter, rect: Rect, mau: Color32, al: u8, focus: bool) {
    if focus {
        p.rect_stroke(rect, rect.height() / 2.0, Stroke::new(2.5, Color32::WHITE), egui::StrokeKind::Inside);
    } else {
        gradient_vien(p, rect, a(mau_phu(mau), al), a(mau, al), 2.2);
    }
}

/// Ô biểu tượng tròn ở đầu nút.
fn ve_icon(p: &egui::Painter, rect: Rect, glyph: &str, mau_chu: Color32) {
    let br = rect.height() / 2.0;
    let tam = Pos2::new(rect.left() + br, rect.center().y);
    p.circle_filled(tam, br - 8.0, rgba(255, 255, 255, 14));
    p.circle_stroke(tam, br - 8.0, Stroke::new(1.2, rgba(255, 255, 255, 85)));
    p.text(tam, egui::Align2::CENTER_CENTER, glyph, FontId::new(19.0, FontFamily::Proportional), mau_chu);
}

fn rong_nut(ui: &Ui, nhan: &str, cao: f32, co_icon: bool) -> f32 {
    let gal = ui.painter().layout_no_wrap(nhan.to_string(), FontId::new(17.0, dam_ho()), CHU);
    (gal.size().x + 34.0 + if co_icon { cao + 4.0 } else { 18.0 }).max(150.0)
}

/// Nút kính neon có biểu tượng, viền / quầng sáng màu `mau`. Cao 52.
pub fn pill(ui: &mut Ui, nhan: &str, mau: Color32, bat: bool) -> Response {
    let size = Vec2::new(rong_nut(ui, nhan, 52.0, true), 52.0);
    let (rect, ph) = ui.allocate_exact_size(size, if bat { Sense::click() } else { Sense::hover() });
    let ph = ph.on_hover_cursor(if bat { egui::CursorIcon::PointingHand } else { egui::CursorIcon::Default });
    let p = ui.painter();
    let cuong = if !bat { 0.3 } else if ph.hovered() { 1.5 } else { 1.0 };
    ve_neon(p, rect, mau, cuong);
    ve_vien(p, rect, mau, if bat { 235 } else { 80 }, ph.has_focus());
    ve_icon(p, rect, icon_cho(nhan), if bat { CHU } else { CHU_MO });
    p.text(Pos2::new(rect.left() + 52.0 + 4.0, rect.center().y), egui::Align2::LEFT_CENTER, nhan, FontId::new(17.0, dam_ho()), if bat { CHU } else { CHU_MO });
    ph
}

/// Nút phụ: kính đen, viền mảnh tím nhạt (không quầng sáng).
pub fn pill_phu(ui: &mut Ui, nhan: &str, bat: bool) -> Response {
    let gal = ui.painter().layout_no_wrap(nhan.to_string(), FontId::new(16.0, dam_ho()), CHU);
    let size = Vec2::new((gal.size().x + 40.0).max(96.0), 44.0);
    let (rect, ph) = ui.allocate_exact_size(size, if bat { Sense::click() } else { Sense::hover() });
    let br = rect.height() / 2.0;
    let p = ui.painter();
    p.rect_filled(rect, br, Color32::from_rgb(if ph.hovered() && bat { 22 } else { 12 }, 10, if ph.hovered() && bat { 44 } else { 30 }));
    p.rect_stroke(rect, br, Stroke::new(1.4, a(TIM, if bat { 150 } else { 60 })), egui::StrokeKind::Inside);
    p.rect_stroke(rect.shrink(3.5), br - 3.5, Stroke::new(1.0, rgba(255, 255, 255, 26)), egui::StrokeKind::Inside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, nhan, FontId::new(16.0, dam_ho()), if bat { CHU } else { CHU_MO });
    ph.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Nút nguy hiểm (neon đỏ).
pub fn pill_nguy_hiem(ui: &mut Ui, nhan: &str, bat: bool) -> Response {
    let size = Vec2::new(rong_nut(ui, nhan, 48.0, true), 48.0);
    let (rect, ph) = ui.allocate_exact_size(size, if bat { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    ve_neon(p, rect, LOI, if !bat { 0.3 } else if ph.hovered() { 1.4 } else { 0.9 });
    ve_vien(p, rect, LOI, 230, false);
    ve_icon(p, rect, "✖", LOI_CHU);
    p.text(Pos2::new(rect.left() + 48.0 + 4.0, rect.center().y), egui::Align2::LEFT_CENTER, nhan, FontId::new(16.0, dam_ho()), LOI_CHU);
    ph.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// ---------------- NÚT TIẾN ĐỘ ----------------
/// Nút việc dài (kính neon): màu đổ đầy từ trái sang phải theo % THẬT (từ thanh tiến trình toàn cục), không hiệu ứng chạy vòng.
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
    let glyph = match tt {
        Tt::Chay(_) => "■",
        Tt::Xong => "✔",
        Tt::Loi => "✖",
        _ => icon_cho(nhan),
    };
    let size = Vec2::new(rong_nut(ui, &chu, 56.0, true).max(rong_nut(ui, nhan, 56.0, true)), 56.0);
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
    let hoat = bat || !matches!(tt, Tt::Cho);
    let p = ui.painter();
    ve_neon(p, rect, mau_hien, if sang || matches!(tt, Tt::Xong | Tt::Loi) { 1.5 } else if !hoat { 0.3 } else if ph.hovered() { 1.5 } else { 1.0 });

    // phần đổ màu theo % (nằm trong viền)
    let phan = match tt {
        Tt::Chay(Some(x)) => x.clamp(0.0, 1.0),
        Tt::Xong => 1.0,
        Tt::Loi => th.as_ref().and_then(|t| t.tien).unwrap_or(0.0).clamp(0.0, 1.0),
        _ => 0.0,
    };
    if phan > 0.0 {
        let trong = rect.shrink(3.0);
        let br = trong.height() / 2.0;
        let w = (trong.width() * phan).max(br * 2.0).min(trong.width());
        let r2 = Rect::from_min_size(trong.min, Vec2::new(w, trong.height()));
        let mut m = epaint::Mesh::default();
        let (c1, c2) = (a(mau_hien, 0x44), a(mau_hien, 0xA0));
        let n = 24;
        let mut tren = Vec::new();
        let mut duoi = Vec::new();
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let x = r2.left() + t * r2.width();
            let dx = if x < trong.left() + br {
                trong.left() + br - x
            } else if x > trong.right() - br {
                x - (trong.right() - br)
            } else {
                0.0
            };
            let dy = (br * br - dx * dx).max(0.0).sqrt();
            let mx = lerp_mau(c1, c2, t);
            tren.push((Pos2::new(x, trong.center().y - dy), mx));
            duoi.push((Pos2::new(x, trong.center().y + dy), mx));
        }
        for i in 0..=n {
            m.colored_vertex(tren[i].0, tren[i].1);
            m.colored_vertex(duoi[i].0, duoi[i].1);
        }
        for i in 0..n as u32 {
            let b = i * 2;
            m.add_triangle(b, b + 1, b + 2);
            m.add_triangle(b + 1, b + 3, b + 2);
        }
        p.add(epaint::Shape::mesh(m));
        if phan < 0.999 {
            p.line_segment([Pos2::new(r2.right(), trong.top() + 6.0), Pos2::new(r2.right(), trong.bottom() - 6.0)], Stroke::new(2.0, mau_hien));
        }
    }
    ve_vien(p, rect, mau_hien, if sang || matches!(tt, Tt::Xong | Tt::Loi) { 255 } else if hoat { 235 } else { 80 }, ph.has_focus());
    ve_icon(p, rect, glyph, if hoat { CHU } else { CHU_MO });
    p.text(
        Pos2::new(rect.left() + rect.height() + 4.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        chu,
        FontId::new(17.0, dam_ho()),
        if hoat { CHU } else { CHU_MO },
    );
    let _ = ban_kinh;

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
/// Công tắc dạng hàng kính neon: [biểu tượng] nhãn ........ [gạt]. Bật = viền tím sáng + gạt xanh chàm.
pub fn cong_tac(ui: &mut Ui, bat: &mut bool, nhan: &str) -> Response {
    let cao = 52.0;
    let khung_khac = cao + 4.0 + 72.0 + 40.0;
    let toi_da = ui.available_width().max(300.0);
    let font = FontId::new(16.0, FontFamily::Proportional);
    // cắt nhãn quá dài bằng "…" cho vừa hàng
    let mut chu = nhan.to_string();
    let rong_chu = |s: &str| ui.painter().layout_no_wrap(s.to_string(), font.clone(), CHU).size().x;
    while rong_chu(&chu) + khung_khac > toi_da && chu.chars().count() > 8 {
        chu = chu.chars().take(chu.chars().count() - 2).collect::<String>().trim_end().to_string();
        if !chu.ends_with('…') {
            chu.push('…');
        } else {
            chu.pop();
            chu.pop();
            chu.push('…');
        }
    }
    let rong = (rong_chu(&chu) + khung_khac).clamp(240.0, toi_da.max(240.0));
    let (rect, mut ph) = ui.allocate_exact_size(Vec2::new(rong, cao), Sense::click());
    if chu != nhan {
        ph = ph.on_hover_text(nhan);
    }
    if ph.clicked() {
        *bat = !*bat;
        ph.mark_changed();
    }
    ph.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *bat, nhan));
    let t = ui.ctx().animate_bool_with_time(ph.id, *bat, 0.12);
    let p = ui.painter();
    ve_neon(p, rect, TIM, if *bat { 1.2 } else if ph.hovered() { 0.7 } else { 0.4 });
    ve_vien(p, rect, TIM, if *bat { 235 } else { 110 }, ph.has_focus());
    ve_icon(p, rect, if *bat { "✔" } else { "○" }, if *bat { CHU } else { CHU_PHU });
    p.text(Pos2::new(rect.left() + cao + 4.0, rect.center().y), egui::Align2::LEFT_CENTER, chu, font, CHU);
    // gạt
    let g = Rect::from_min_size(Pos2::new(rect.right() - 16.0 - 64.0, rect.center().y - 15.0), Vec2::new(64.0, 30.0));
    if *bat {
        p.rect_filled(g, 15.0, Color32::from_rgb(40, 40, 150));
        p.rect_filled(Rect::from_min_size(g.min + Vec2::new(32.0, 0.0), Vec2::new(32.0, 30.0)), 15.0, Color32::from_rgb(120, 74, 238));
        p.rect_filled(Rect::from_min_max(g.min + Vec2::new(15.0, 0.0), g.max - Vec2::new(15.0, 0.0)), 0.0, Color32::from_rgb(78, 56, 195));
    } else {
        p.rect_filled(g, 15.0, Color32::from_rgb(24, 16, 52));
    }
    p.rect_stroke(g, 15.0, Stroke::new(1.2, rgba(255, 255, 255, 70)), egui::StrokeKind::Inside);
    p.circle_filled(Pos2::new(g.left() + 15.0 + t * 34.0, g.center().y), 12.0, Color32::WHITE);
    ph.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Công tắc nhỏ (không biểu tượng) dùng được trong hàng ngang tự xuống dòng.
pub fn cong_tac_nho(ui: &mut Ui, bat: &mut bool, nhan: &str) -> Response {
    cong_tac(ui, bat, nhan)
}

// ---------------- chip ----------------
/// Chip chọn: được chọn = kính neon viền tím + chữ đậm; chưa chọn = kính đen viền mờ.
pub fn chip(ui: &mut Ui, chon: bool, nhan: &str) -> Response {
    let gal = ui.painter().layout_no_wrap(nhan.to_string(), FontId::new(16.0, if chon { dam_ho() } else { FontFamily::Proportional }), CHU);
    let size = Vec2::new((gal.size().x + 40.0).max(76.0), 44.0);
    let (rect, ph) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let br = rect.height() / 2.0;
    if chon {
        ve_neon(p, rect, TIM, 1.0);
        ve_vien(p, rect, TIM, 240, false);
    } else {
        p.rect_filled(rect, br, Color32::from_rgb(if ph.hovered() { 26 } else { 14 }, 10, if ph.hovered() { 50 } else { 32 }));
        p.rect_stroke(rect, br, Stroke::new(1.2, a(TIM, if ph.hovered() { 130 } else { 70 })), egui::StrokeKind::Inside);
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


/// Thanh trượt dạng viên kính neon (như "Brightness" của ảnh mẫu): nhãn + giá trị ở trên, rãnh + núm trắng ở dưới.
/// `dp`: số chữ số thập phân hiển thị. → Response (changed khi kéo).
pub fn thanh_truot(ui: &mut Ui, gia_tri: &mut f64, khoang: std::ops::RangeInclusive<f64>, nhan: &str, dp: usize) -> Response {
    let rong = ui.available_width().clamp(260.0, 460.0);
    let cao = 68.0;
    let (rect, mut ph) = ui.allocate_exact_size(Vec2::new(rong, cao), Sense::click_and_drag());
    let (lo, hi) = (*khoang.start(), *khoang.end());
    let rail_l = rect.left() + 26.0;
    let rail_r = rect.right() - 26.0;
    let rail_y = rect.bottom() - 20.0;
    if (ph.dragged() || ph.clicked()) && ph.interact_pointer_pos().is_some() {
        let x = ph.interact_pointer_pos().unwrap().x;
        let t = ((x - rail_l) / (rail_r - rail_l)).clamp(0.0, 1.0) as f64;
        let moi = lo + (hi - lo) * t;
        if (moi - *gia_tri).abs() > f64::EPSILON {
            *gia_tri = moi;
            ph.mark_changed();
        }
    }
    let t = ((*gia_tri - lo) / (hi - lo)).clamp(0.0, 1.0) as f32;
    let p = ui.painter();
    ve_neon(p, rect, TIM, if ph.hovered() || ph.dragged() { 1.2 } else { 0.8 });
    ve_vien(p, rect, TIM, 230, false);
    p.text(Pos2::new(rect.left() + 28.0, rect.top() + 22.0), egui::Align2::LEFT_CENTER, nhan, FontId::new(16.0, dam_ho()), CHU);
    p.text(Pos2::new(rect.right() - 28.0, rect.top() + 22.0), egui::Align2::RIGHT_CENTER, format!("{:.*}", dp, gia_tri), FontId::new(16.0, FontFamily::Monospace), CHU_PHU);
    let rail = Rect::from_min_max(Pos2::new(rail_l, rail_y - 4.0), Pos2::new(rail_r, rail_y + 4.0));
    p.rect_filled(rail, 4.0, rgba(255, 255, 255, 36));
    let xn = rail_l + (rail_r - rail_l) * t;
    let day = Rect::from_min_max(rail.min, Pos2::new(xn.max(rail_l + 8.0), rail.max.y));
    p.rect_filled(day, 4.0, lerp_mau(mau_phu(TIM), TIM, t));
    p.circle_filled(Pos2::new(xn, rail_y + 1.5), 13.0, rgba(0, 0, 0, 90));
    p.circle_filled(Pos2::new(xn, rail_y), 12.0, Color32::WHITE);
    ph.on_hover_cursor(egui::CursorIcon::PointingHand)
}
