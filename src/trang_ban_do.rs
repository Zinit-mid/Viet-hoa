//! Trang "Bản đồ" Crimson Desert: vị trí thú cưỡi huyền thoại, rương / đồ xịn (chuyển từ ban_do_cd.py của app cũ)
//! và công cụ xuất bảng thú cưỡi (chỉ đọc game) để làm mod bắt ngựa.
//! Tên địa danh giữ tiếng Anh như trong game. Dữ liệu tổng hợp từ hướng dẫn công khai (11/2026), có thể lệch sau cập nhật game.

use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::viet_hoa::tim_game::{self, LoaiGame};

const BAN_DO_ONLINE: &[(&str, &str, &str)] = &[
    (
        "Bản đồ MapMaster",
        "Lọc rương, thú cưỡi, trang bị",
        "https://mapmaster.io/games/crimson-desert/maps/crimson-desert",
    ),
    (
        "Bản đồ th.gl",
        "Có app overlay đè lên game",
        "https://crimsondesert.th.gl/",
    ),
    (
        "Danh sách rương",
        "Rương + két sắt và phần thưởng",
        "https://thegameswiki.com/crimson-desert/wiki/treasure-chest-locations",
    ),
];

const NHOM: &[(&str, &str)] = &[
    ("tat_ca", "Tất cả"),
    ("ngua", "Ngựa huyền thoại"),
    ("bay", "Rồng / bay"),
    ("thu", "Thú huyền thoại"),
    ("ruong", "Rương đồ xịn"),
    ("ket", "Két sắt (Strongbox)"),
];

/// (nhóm, tên, vùng, vị trí, cách lấy)
const DU_LIEU: &[(&str, &str, &str, &str, &str)] = &[
    ("ngua", "Royler – ngựa trắng", "Pailune", "Silver Wolf Mountain, gần Snowgrass Shelter, cạnh một hồ băng hẻo lánh.", "Đuổi theo cho kịp rồi bấm E thuần ngựa (căn đuôi). Nên có thể lực (Stamina) cấp 8 trở lên."),
    ("ngua", "Rokade – ngựa đen", "Hernand (đông nam)", "Phía trái Pinnacle of Perception, trong lùm cây khô nhỏ ở tận đông nam Hernand.", "Đuổi theo và thuần giống Royler."),
    ("ngua", "Camora – ngựa đỏ", "Crimson Desert", "Đi về nam dọc Red River, trong khu rừng có Cây Đỏ (Red Tree).", "Đuổi theo và thuần giống Royler."),
    ("bay", "Blackstar – rồng", "(Không nằm trên bản đồ)", "Mở khóa theo cốt truyện, không đi tìm được.", "Làm xong cốt truyện chính hết Chương 11 (nhiệm vụ \"Whispers in the Wind\"). Bay tối đa 15 phút, hồi 50 phút, không bay qua một số khu đông dân."),
    ("bay", "Wyvern (rồng con)", "Delesyia", "4 quả trứng gần Three Brothers Cliff.", "Cho ăn thịt sống khoảng 3 ngày trong game tới cấp lớn 3, rồi chọn Grow. Bay không giới hạn thời gian."),
    ("bay", "Kuku Bing (chim lượn)", "Demeniss (phía nam)", "Tòa tháp phía nam Demeniss, gần trại Bloodsteel.", "Dùng Force Palm phá bức tường bí mật, lấy quả trứng lớn, ấp ở tổ phía tây trại Bloodsteel."),
    ("bay", "Gà con Kuku (chim lượn)", "Nhiều nơi", "Chỗ có chim Kuku hoang và chim con.", "Cho ăn ngũ cốc (ngô / lúa mạch) rồi chọn Grow để đăng ký. Chỉ lượn, không bay lên được."),
    ("thu", "Gấu trắng (White Bear)", "Pailune (tây bắc)", "Gần Silver Wolf Mountain, phía nam White Wastes Sanctuary. Gấu ngủ trên sườn núi cùng 2 gấu con.", "Hạ cả đàn, lột da con lớn lấy White Bear Claws, mang tới Phù thủy (Witch) làm ấn triệu hồi."),
    ("thu", "Sói Silver Fang", "Hernand", "Hang đá ở Howling Hollow gần Hernand.", "Làm các nhiệm vụ phụ ở North Gate, hạ sói trong hang, mang chiến lợi phẩm tới Phù thủy."),
    ("thu", "Hươu Snowwhite Deer", "Pailune", "Phía trên chữ \"A\" của chữ Pailune trên bản đồ vùng.", "Hươu không đánh lại mà bỏ chạy – săn rồi mang chiến lợi phẩm tới Phù thủy."),
    ("thu", "Lợn rừng Rock Tusk", "Crimson Desert", "Trại Bordig, đông nam Tashkalp.", "Làm nhiệm vụ Savage Fang, hạ Ravok, mang Rock Tusk Boar's Tusk tới Phù thủy."),
    ("thu", "Dê núi Icicle Edge Alpine Ibex", "Pailune", "Wayward Woods, phía tây chữ \"I\" của Pailune.", "Làm nhiệm vụ The Sports Hunter, hạ Moren, mang sừng tới Phù thủy."),
    ("thu", "Lợn thần núi (Mountain God Boar)", "Pailune", "Rừng ở Pailune (theo nhiệm vụ).", "Nhận Lumberjack's Witness Report ở bảng thông báo, chặt cây tới khi boss hiện ra rồi hạ nó."),
    ("thu", "Hổ (Tiger)", "Demeniss (phía bắc)", "Rừng mưa phía bắc Demeniss, phía trên hồ nước tròn và tháp canh.", "Hạ hổ trắng trước. Dùng Force Palm khống chế hổ thường, cho ăn thịt sống tới khi tin tưởng 100%."),
    ("thu", "Sư tử (Lion)", "Phía bắc (gần Varnia)", "Tận phía bắc bản đồ, dưới Varnia, trên tảng đá cát phẳng. Cẩn thận mép vực.", "Hạ sư tử trắng trước, rồi khống chế + cho ăn thịt sống giống hổ."),
    ("thu", "Lạc đà Sandwalk", "Crimson Desert", "Theo nhiệm vụ của Redwind Merchant Guild.", "Làm nhiệm vụ Outlaws of the Desert, hạ thủ lĩnh Sandfang Marauder, nói chuyện với Canion."),
    ("ruong", "Shield of Conviction (khiên)", "Hernand", "Rương trên tháp chuông nhà thờ Church of Calphade (Calphadean Territory).", "Leo lên tháp chuông."),
    ("ruong", "Survivor's Solitude", "Hernand", "Rương sau thác nước trong Death's Grip Cavern (Everfrost Basin).", "Đi xuyên qua thác nước."),
    ("ruong", "Hollow Visage", "Hernand", "Rương sau thác nước trong Dawn Cave (Mountain of Frozen Souls).", "Đi xuyên qua thác nước."),
    ("ruong", "Soul Spear (giáo 2 tay)", "Hernand", "Một căn phòng trong Antumbra Ritual Grounds (The Argent Peaks).", ""),
    ("ruong", "Frozen Anguish", "Demeniss", "Rương dưới cầu thang ngay lối vào Spire of Frost (Eye of Ice).", ""),
    ("ruong", "Sydmon Bow (cung)", "Hernand", "Nằm giữa bụi hoa ở Hernand Highlands, ngay phía bắc chữ \"S\" của \"Highlands\".", "Nhặt dưới đất."),
    ("ruong", "Golden-Knotted Ancestral Bow (cung)", "Pailune", "Căn phòng cuối đường bẫy trong Pailune Ancestors' Ruins (Silver Wolf Mountain).", "Vượt qua dãy bẫy."),
    ("ruong", "Kylus Dagger (dao găm)", "Hernand", "Oakenshield Manor.", "Phần thưởng nhiệm vụ 'Woman Who Lost Her Husband'."),
    ("ruong", "Darkbringer", "Silver Mountains (bắc)", "White Wastes Sanctuary.", "Một trong những vũ khí mạnh nhất đầu game, phải đi xa lên phía bắc."),
    ("ruong", "Bộ Fallen Kingdom (áo choàng + kiếm Rapier)", "Pailune", "Icemoor Castle Ruins: áo choàng ở lối vào hang (dưới bên trái), kiếm cạnh thùng gỗ.", ""),
    ("ruong", "Frozen Heart Plate Armor (giáp)", "Delesyia", "Lâu đài Delesyia Castle (tận phía đông Pywel).", ""),
    ("ruong", "Rhonid Large Shield (khiên lớn)", "Hernand", "Lioncrest Manor, sau cánh cửa khóa.", ""),
    ("ruong", "Lifsoth Large Shield (khiên lớn)", "Witchwoods", "Nhà Phù thủy (Witch's House), trên ban công.", ""),
    ("ruong", "Sior Blaster", "Bowsprit Cape", "Hang Bowsprit Cape Cave.", ""),
    ("ruong", "Elite Vanguard Spear (giáo)", "Bowsprit Cape", "Hang gần Abyss Nexus cạnh Bowsprit Cape.", ""),
    ("ruong", "Odeck's Protector Plate Boots (giày)", "Sanctum", "Sanctum of Temperance – rương nằm sau hốc bị cỏ che.", "Nên đi sớm, đồ rất mạnh đầu game."),
    ("ruong", "Mirror of Night Shield (khiên)", "Sanctum", "Sanctum of Benediction.", "Nên đi sớm."),
    ("ruong", "Abyss Gear + bản vẽ Kuku Pack", "Sanctum", "Sanctum of Penitence – nhảy xuống hố gần con chó nâu.", "Nên đi sớm."),
    ("ruong", "Goblin King's Treasure Dagger", "Pororin Forest", "Mudridge Cabin – rương giấu dưới sàn gỗ.", ""),
    ("ruong", "Bản vẽ Palmar Pills (thuốc tăng chỉ số)", "Hernand", "Three Saints Falls Cave.", ""),
    ("ket", "Oath of Darkness", "Hernand", "Bluemont Manor – phòng ngủ Leon Roberts, tầng 2.", "Giải câu đố két sắt."),
    ("ket", "Saint's Necklace (dây chuyền)", "Hernand", "Hillside Manor – phòng đông nam.", "Giải câu đố két sắt."),
    ("ket", "Engraved Gold Earring (bông tai)", "Hernand", "Lioncrest Manor – phòng phía đông.", "Giải câu đố két sắt."),
    ("ket", "Dagger of Radiance (dao găm)", "Demeniss", "Church of West Demeniss (Byron Estate).", "Giải câu đố két sắt."),
    ("ket", "Witch's Earring (bông tai)", "Demeniss", "Southern Court – tầng 2.", "Giải câu đố két sắt."),
    ("ket", "Ancient Earring (bông tai)", "", "Azerian Manor – tầng 2, sau bức tường xoay.", "Giải câu đố két sắt."),
    ("ket", "Karanda's Necklace (dây chuyền)", "", "Western Court – tầng hầm.", "Giải câu đố két sắt."),
    ("ket", "Finely Crafted Gold Ring (nhẫn)", "", "Stellen Manor – trên tháp.", "Giải câu đố két sắt."),
    ("ket", "Flower Petal Earring (bông tai)", "Pororin Forest", "Mudridge Cabin.", "Giải câu đố két sắt."),
    ("ket", "Công thức Lavish Meal", "Hernand", "Hernand Castle – phòng khóa tầng trên cùng.", "Giải câu đố két sắt."),
];

pub struct TrangBanDo {
    nhom: &'static str,
    tim: String,
    tb: Arc<Mutex<String>>,
    dang_xuat: Arc<Mutex<bool>>,
}

impl Default for TrangBanDo {
    fn default() -> Self {
        Self {
            nhom: "tat_ca",
            tim: String::new(),
            tb: Arc::new(Mutex::new(String::new())),
            dang_xuat: Arc::new(Mutex::new(false)),
        }
    }
}

fn mo_web(url: &str) {
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
}

impl TrangBanDo {
    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("Bản đồ Crimson Desert");
        ui.label("Vị trí ngựa / thú cưỡi huyền thoại, rương và két sắt đồ xịn. Tên địa danh giữ tiếng Anh như trong game để dễ tìm trên bản đồ.");
        ui.add_space(8.0);
        crate::kieu::khoi(ui, "BẢN ĐỒ TRỰC TUYẾN", |ui| {
            for (ten, mo_ta, url) in BAN_DO_ONLINE {
                ui.horizontal(|ui| {
                    if ui.button(*ten).clicked() {
                        mo_web(url);
                    }
                    ui.label(RichText::new(*mo_ta).color(crate::kieu::CHU_MO));
                });
            }
        });
        crate::kieu::khoi(ui, "DANH SÁCH VỊ TRÍ", |ui| {
            ui.horizontal_wrapped(|ui| {
                for (ma, ten) in NHOM {
                    if crate::kieu::chip(ui, self.nhom == *ma, ten).clicked() {
                        self.nhom = ma;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Tìm:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.tim)
                        .desired_width(260.0)
                        .hint_text("tên, vùng, vị trí..."),
                );
            });
            ui.add_space(6.0);
            let tim = self.tim.trim().to_lowercase();
            let mut dem = 0;
            for (nhom, ten, vung, vi_tri, cach) in DU_LIEU {
                if self.nhom != "tat_ca" && self.nhom != *nhom {
                    continue;
                }
                if !tim.is_empty()
                    && ![ten, vung, vi_tri, cach]
                        .iter()
                        .any(|s| s.to_lowercase().contains(&tim))
                {
                    continue;
                }
                dem += 1;
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(*ten).strong().size(16.0));
                    if !vung.is_empty() {
                        ui.label(RichText::new(format!("Vùng: {vung}")).color(Color32::LIGHT_BLUE));
                    }
                    ui.label(format!("Vị trí: {vi_tri}"));
                    if !cach.is_empty() {
                        ui.label(
                            RichText::new(format!("Cách lấy: {cach}")).color(crate::kieu::CHU_MO),
                        );
                    }
                });
            }
            if dem == 0 {
                ui.label("Không có mục nào khớp.");
            }
        });
        crate::kieu::khoi(ui, "MOD HIỆN RƯƠNG TRÊN BẢN ĐỒ", |ui| {
            ui.label("Tạo mod cho đúng bản game đang cài (game cập nhật thì bấm tạo lại). Ra thư mục Mod_HienRuong trên Desktop gồm 9 mức 15…3000 m: chép cả thư mục vào mods của DMM, bật MỘT mức, tắt TreasureSeeker, thoát game rồi Mount Mods.");
            let ban = *self.dang_xuat.lock().unwrap();
            if ui
                .add_enabled(
                    !ban,
                    egui::Button::new(if ban {
                        "Đang tạo..."
                    } else {
                        "Tạo mod hiện rương"
                    }),
                )
                .clicked()
            {
                let (tb, dx) = (self.tb.clone(), self.dang_xuat.clone());
                *dx.lock().unwrap() = true;
                *tb.lock().unwrap() = "Đang đọc game...".into();
                std::thread::spawn(move || {
                    let kq = (|| -> anyhow::Result<String> {
                        let game = tim_game::tim_tat_ca()
                            .into_iter()
                            .find(|g| g.loai() == LoaiGame::CrimsonDesert)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "không tìm thấy Crimson Desert (thêm game trong Thư viện game)"
                                )
                            })?;
                        let ra = crate::duong_dan::desktop().join("Mod_HienRuong");
                        let bc =
                            crate::viet_hoa::crimson_desert::tao_mod_ruong(&game.thu_muc, &ra)?;
                        crate::duong_dan::mo_explorer(&ra);
                        Ok(format!(
                            "Đã tạo 9 mod ở {}\n{}",
                            ra.display(),
                            bc.join("\n")
                        ))
                    })();
                    *tb.lock().unwrap() = kq.unwrap_or_else(|e| format!("Lỗi: {e}"));
                    *dx.lock().unwrap() = false;
                });
            }
        });
        crate::kieu::khoi(ui, "XUẤT BẢNG THÚ CƯỠI + RƯƠNG (LÀM MOD)", |ui| {
            ui.label("Chỉ ĐỌC game, không sửa gì. Xuất bảng thú cưỡi, giao diện minigame bắt ngựa và bảng rương / biểu tượng bản đồ ra thư mục BangThuCuoi_CrimsonDesert trên Desktop. Có thư mục đó rồi, nhắn Claude để làm mod \"dễ bắt ngựa\" và mod rương cho đúng bản game của bạn.");
            let ban = *self.dang_xuat.lock().unwrap();
            if ui
                .add_enabled(
                    !ban,
                    egui::Button::new(if ban {
                        "Đang xuất..."
                    } else {
                        "Xuất bảng thú cưỡi + rương"
                    }),
                )
                .clicked()
            {
                let (tb, dx) = (self.tb.clone(), self.dang_xuat.clone());
                *dx.lock().unwrap() = true;
                *tb.lock().unwrap() = "Đang tìm game và xuất...".into();
                std::thread::spawn(move || {
                    let kq = (|| -> anyhow::Result<String> {
                        let game = tim_game::tim_tat_ca()
                            .into_iter()
                            .find(|g| g.loai() == LoaiGame::CrimsonDesert)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "không tìm thấy Crimson Desert (thêm game trong Thư viện game)"
                                )
                            })?;
                        let ra = crate::duong_dan::desktop().join("BangThuCuoi_CrimsonDesert");
                        let tb2 = tb.clone();
                        let ds = crate::viet_hoa::crimson_desert::xuat_bang_thu_cuoi(
                            &game.thu_muc,
                            &ra,
                            &|s| *tb2.lock().unwrap() = s.to_string(),
                        )?;
                        Ok(format!("Đã xuất {} file ra {}", ds.len(), ra.display()))
                    })();
                    *tb.lock().unwrap() = kq.unwrap_or_else(|e| format!("Lỗi: {e}"));
                    *dx.lock().unwrap() = false;
                });
            }
            let t = self.tb.lock().unwrap().clone();
            if !t.is_empty() {
                ui.label(t);
            }
        });
    }
}
