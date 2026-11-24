//! Quy trình MỘT NÚT cho game: nạp chữ → dịch (offline mặc định, Claude tùy chọn) → lồng tiếng → cài.
//! Mỗi bước làm tiếp được khi dừng (bộ nhớ dịch, file giọng đã tạo được giữ lại).
//!   Crimson Desert: dịch 3 đợt → gói chữ ra Desktop → lồng tiếng A, B (gói zip ra Desktop cho DMM).
//!   Rise / Tomb Raider 2013: dịch → lồng tiếng → cài vào game.   Shadow: dịch → cài.
//!   Game khác: dò file chữ tự nhận → dịch → ghi vào game (có sao lưu).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};

use crate::viet_hoa::crimson_desert::{self as vcd, CrimsonDesert};
use crate::viet_hoa::do_file::ViecFile;
use crate::viet_hoa::tim_game::{Game, LoaiGame};
use crate::viet_hoa::tomb_raider::{thu_muc_lam_viec, LoaiTr, TombRaider};
use crate::viet_hoa::viec_dich::{BaoCao, ViecDich};
use crate::viet_hoa::{giong_vieneu, wwise};
use crate::{dich, duong_dan};

#[derive(Clone)]
pub struct TuyChon {
    pub claude: bool,
    pub gioi_han: f64,
    pub dich: bool,
    pub long_tieng: bool,
    pub cai: bool,
    pub khong_dau: bool,
}

pub struct Bc<'a> {
    pub bao: &'a (dyn Fn(&str) + Sync),
    pub tien: &'a (dyn Fn(usize, usize, f64) + Sync),
}

impl BaoCao for Bc<'_> {
    fn bao(&self, s: &str) {
        (self.bao)(s)
    }
    fn tien_do(&self, xong: usize, tong: usize, usd: f64) {
        (self.tien)(xong, tong, usd)
    }
}

/// Dịch `ds` bằng máy đã chọn. → true nếu dịch xong hết, false nếu dừng / chạm giới hạn / còn câu lỗi.
fn dich_cau(viec: &mut ViecDich, ds: Vec<String>, t: &TuyChon, bc: &Bc) -> Result<bool> {
    if ds.is_empty() {
        return Ok(true);
    }
    if t.claude && dich::claude::co_khoa() {
        viec.chay_claude(ds, t.gioi_han, bc)
    } else {
        viec.chay_offline(ds, bc)
    }
}

fn chua_xong(dung: &AtomicBool, viec: &ViecDich) -> String {
    if dung.load(Ordering::Relaxed) {
        "Đã dừng – bấm lại để làm tiếp.".into()
    } else {
        format!("Chưa dịch xong ({} câu lỗi) – bấm lại để dịch tiếp.", viec.loi)
    }
}

/// Chạy quy trình cho `game`. → thông báo kết quả.
pub fn chay(game: &Game, t: &TuyChon, dung: &Arc<AtomicBool>, bc: &Bc) -> Result<String> {
    let _thuc = duong_dan::GiuMayThuc::moi();
    dung.store(false, Ordering::SeqCst);
    let bao = |s: &str| (bc.bao)(s);
    match game.loai() {
        LoaiGame::CrimsonDesert => {
            bao("Đang đọc chữ của game...");
            let mut cd = CrimsonDesert::nap(&game.thu_muc)?;
            cd.viec.dung = dung.clone();
            let mut ra = Vec::new();
            if t.dich {
                for dot in 0..vcd::DOT.len() {
                    bao(&format!("Dịch đợt {}/{}...", dot + 1, vcd::DOT.len()));
                    let ds = cd.cau_can_dich(dot);
                    if !dich_cau(&mut cd.viec, ds, t, bc)? {
                        return Ok(chua_xong(dung, &cd.viec));
                    }
                }
                let p = duong_dan::desktop().join(vcd::TEN_GOI);
                let n = cd.xuat_goi(&p)?;
                ra.push(format!("Gói chữ ({n} câu) ra Desktop: {}", vcd::TEN_GOI));
            }
            if t.long_tieng {
                if giong_vieneu::tim_vieneu().is_none() || wwise::tim_wwise_console().is_none() {
                    bail!("lồng tiếng cần app VieNeu (GPU) và Wwise (WwiseConsole.exe) trên máy");
                }
                let lv = thu_muc_lam_viec(&game.thu_muc);
                for (phan, ten) in [('A', "LongTieng_NhanVat.zip"), ('B', "LongTieng_DanThuong.zip")] {
                    let zip = duong_dan::desktop().join(ten);
                    let kq = cd.long_tieng_phan(phan, &lv, &zip, 6.0, dung, bc.bao, &|a, b, s| {
                        (bc.tien)(a, b, 0.0);
                        (bc.bao)(&format!("Lồng tiếng {phan}: {s} ({a}/{b})"));
                    })?;
                    match kq {
                        None => return Ok("Đã dừng – bấm lại để làm tiếp lồng tiếng.".into()),
                        Some(k) => ra.push(format!("Lồng tiếng {phan}: {} câu → Desktop\\{ten}", k.cau)),
                    }
                }
            }
            ra.push("Trong DMM: tắt gói cũ > Import các file .zip trên Desktop > bật > Mount Mods.".into());
            Ok(ra.join("\n"))
        }
        l @ (LoaiGame::Rise | LoaiGame::Tr2013 | LoaiGame::Shadow) => {
            let loai = match l {
                LoaiGame::Rise => LoaiTr::Rise,
                LoaiGame::Tr2013 => LoaiTr::Tr2013,
                _ => LoaiTr::Shadow,
            };
            let mut x = TombRaider::nap(&game.thu_muc, loai, &|s| bao(s), dung)?;
            x.viec.dung = dung.clone();
            let mut ra = Vec::new();
            if t.dich {
                bao("Đang dịch...");
                let ds = x.cau_can_dich();
                if !dich_cau(&mut x.viec, ds, t, bc)? {
                    return Ok(chua_xong(dung, &x.viec));
                }
            }
            if t.long_tieng && loai != LoaiTr::Shadow {
                let kq = x.long_tieng(false, 4.0, dung, bc.bao, &|a, b, s| {
                    (bc.tien)(a, b, 0.0);
                    (bc.bao)(&format!("{s} ({a}/{b})"));
                })?;
                match kq {
                    None => return Ok("Đã dừng – bấm lại để làm tiếp lồng tiếng.".into()),
                    Some(k) => ra.push(format!("Lồng tiếng: thêm {} file • tổng {}/{} có giọng Việt", k.file, k.da_co, k.tong)),
                }
            }
            if t.cai {
                bao("Đang cài vào game (tắt game trước)...");
                x.cai(t.khong_dau, bc.bao, dung)?;
                ra.push(x.ket_qua_cai.clone());
            }
            Ok(if ra.is_empty() { "Xong.".into() } else { ra.join("\n") })
        }
        LoaiGame::Khac => {
            let mut v = ViecFile::moi(&game.thu_muc, &game.ten);
            v.viec.dung = dung.clone();
            if v.kq_quet.is_none() {
                bao("Đang dò file chữ của game...");
                let d2 = dung.clone();
                v.quet(&|s| bao(s), &move || d2.load(Ordering::Relaxed));
                v.luu_quet();
            }
            if v.chon.is_empty() {
                bail!("không tự nhận ra file chữ nào – mở mục Việt hóa game để chọn file bằng tay");
            }
            v.doc_chon();
            let mut ra = vec![format!("{} file chữ", v.chon.len())];
            if t.dich {
                bao("Đang dịch...");
                let ds = v.viec.cau_can_dich(v.cac_cau().iter().map(String::as_str));
                if !dich_cau(&mut v.viec, ds, t, bc)? {
                    return Ok(chua_xong(dung, &v.viec));
                }
            }
            if t.cai {
                bao("Đang ghi vào game (có sao lưu bản gốc)...");
                let n = v.ghi_vao_game(t.khong_dau, bc.bao)?;
                ra.push(format!("Đã ghi {n} câu tiếng Việt vào game. Gỡ: mục Việt hóa game > Trả lại bản gốc."));
            }
            if t.long_tieng {
                ra.push("(Lồng tiếng cho game này chưa hỗ trợ.)".into());
            }
            Ok(ra.join("\n"))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Game giả (thư mục tạm, file JSON chữ Anh): dò → dịch offline → ghi → kiểm tra có tiếng Việt.
    /// cargo test --release -- --ignored game_khac_that --nocapture
    #[test]
    #[ignore]
    fn game_khac_that() {
        let tm = std::env::temp_dir().join("vh_game_gia");
        let _ = std::fs::remove_dir_all(&tm);
        std::fs::create_dir_all(tm.join("data")).unwrap();
        let cau = [
            "You must find the second shrine before nightfall.",
            "Press the button to open the heavy door.",
            "The merchant sells weapons and armor in the city.",
            "Your health is low, find a safe place to rest.",
            "Welcome back, traveler. The king has been waiting for you.",
            "Collect all the crystals to unlock the ancient gate.",
            "Be careful, the bridge ahead is unstable.",
            "We need to leave before the enemy arrives.",
            "This potion will restore your strength.",
            "Talk to the blacksmith to upgrade your sword.",
        ];
        let json: serde_json::Map<String, serde_json::Value> =
            cau.iter().enumerate().map(|(i, c)| (format!("line_{i}"), (*c).into())).collect();
        std::fs::write(tm.join("data").join("english.json"), serde_json::to_vec_pretty(&json).unwrap()).unwrap();
        let g = Game { nguon: "tay".into(), ma: "gia".into(), ten: "Game gia".into(), thu_muc: tm.clone(), phien_ban: String::new() };
        let t = TuyChon { claude: false, gioi_han: 1.0, dich: true, long_tieng: false, cai: true, khong_dau: false };
        let dung = Arc::new(AtomicBool::new(false));
        let kq = chay(&g, &t, &dung, &Bc { bao: &|s| println!("{s}"), tien: &|a, b, _| println!("{a}/{b}") }).unwrap();
        println!("{kq}");
        let moi = std::fs::read_to_string(tm.join("data").join("english.json")).unwrap();
        println!("{moi}");
        // chỉ dọn thư mục làm việc của game giả này
        let viec = crate::viet_hoa::do_file::thu_muc_viec(&tm);
        if viec.starts_with(duong_dan::du_lieu()) && viec != duong_dan::du_lieu() {
            let _ = std::fs::remove_dir_all(viec);
        }
        assert!(moi.chars().any(|c| "ăâđêôơưạảấầẩẫậắằẳẵặẹẻẽếềểễệ".contains(c)));
    }
}
