//! Việt hóa Crimson Desert (chuyển từ viet_hoa_cd.py): đọc chữ tiếng Anh (nhóm 0020) ngay trong
//! thư mục game, dịch theo đợt (Claude / offline) vào bộ nhớ, xuất gói zip cho DMM;
//! thêm mod tăng ô chứa đồ (inventory.staticinfobody, nhóm 0008).
//! Không sửa file nào trong thư mục game.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use super::cd_dinh_dang::{self as cd, Paloc};
use super::viec_dich::{khong_can_dich, la_cau, BangTen, ViecDich};
use crate::duong_dan;

pub struct Dot {
    pub ten: &'static str,
    pub file: &'static [&'static str],
    /// file lẫn tên và mô tả: chỉ dịch CÂU (mô tả), giữ nguyên tên / nhãn
    pub chi_cau: &'static [&'static str],
}

pub const DOT: [Dot; 3] = [
    Dot {
        ten: "Đợt 1 – menu, nhiệm vụ, hội thoại cốt truyện, cắt cảnh",
        file: &[
            "ui",
            "localstringinfo",
            "quest",
            "questdialog",
            "sequencer",
            "narrationdialog",
            "talktree",
            "stageclosedialog",
            "failmessage",
            "gameadvice",
            "interaction",
            "inventory",
            "equiptype",
            "symboltext",
            "minigame",
            "platformservice",
            "uisocialaction",
            "lookat",
            "board",
            "bank",
        ],
        chi_cau: &[],
    },
    Dot {
        ten: "Đợt 2 – lời thoại chiến đấu / đi đường, mô tả vật phẩm, phe phái, kỹ năng, sự kiện",
        file: &[
            "aidialog",
            "item",
            "multichange",
            "gimmick",
            "character",
            "npc",
            "faction",
            "patterndescription",
            "managementbuff",
            "managementneedssynergy",
            "globalgameevent",
            "entitlement",
            "housingobject",
            "sublevel",
            "house",
            "housingroom",
            "housingobjectgroup",
            "socket",
        ],
        chi_cau: &[
            "item",
            "multichange",
            "gimmick",
            "character",
            "npc",
            "faction",
            "housingobject",
        ],
    },
    Dot {
        ten: "Đợt 3 – bách khoa (knowledge): nhân vật, vùng đất, sinh vật, lịch sử",
        file: &["knowledge"],
        chi_cau: &["knowledge"],
    },
];

/// File chỉ chứa tên → bảng tên riêng (giữ nguyên, không dịch)
const FILE_TEN: &[&str] = &[
    "npcname",
    "character",
    "region",
    "faction",
    "factionoperationgroup",
    "item",
    "itemgroup",
    "housingobject",
    "dyecolorgroup",
    "personality",
];

const HUONG_DAN: &str = "Bạn là dịch giả chuyên Việt hóa game. Game: Crimson Desert (nhập vai thế giới mở, trung cổ kỳ ảo; \
nhân vật chính Kliff, thủ lĩnh nhóm lính đánh thuê Greymanes; lục địa Pywel).
Bạn nhận một mảng JSON các chuỗi tiếng Anh lấy từ file chữ của game và trả về mảng JSON các bản dịch tiếng Việt.

Quy tắc bắt buộc:
- CHỈ trả về một mảng JSON hợp lệ, CÙNG SỐ PHẦN TỬ, CÙNG THỨ TỰ. Không giải thích, không ```.
- GIỮ NGUYÊN tiếng Anh: tên nhân vật, tên người, địa danh, phe phái, tổ chức, tên vật phẩm, tên trang bị, \
tên kỹ năng riêng, và mọi từ trong danh sách \"Giữ nguyên\" nếu có.
- Giữ NGUYÊN từng ký tự các thẻ: <br/>, <...>, {...} (VD {Key:Key_Jump}, {Param1}), [EMPTY]. Không dịch, không thêm, không bớt thẻ.
- Giữ nguyên số, ký hiệu, khoảng trắng đầu/cuối và cách xuống dòng.
- Văn phong tự nhiên như phụ đề phim kỳ ảo/cổ trang. Xưng hô hợp ngữ cảnh \
(ta – ngươi, tôi – ngài, anh – em, ta – các ngươi...).
- Menu, nút bấm, tiêu đề nhiệm vụ: ngắn gọn, rõ nghĩa như game Việt hóa chính thức.
- Chuỗi chỉ gồm tên riêng hoặc không có gì để dịch: trả lại y nguyên.";

pub const TEN_GOI: &str = "VietHoa_CrimsonDesert.zip";

pub struct CrimsonDesert {
    pub thu_muc_game: PathBuf,
    pub(super) nhom: BTreeMap<String, Paloc>,
    pub viec: ViecDich,
}

impl CrimsonDesert {
    pub fn thu_muc_du_lieu() -> PathBuf {
        duong_dan::viet_hoa_game().join("crimson_desert")
    }

    /// Đọc chữ tiếng Anh của game + bộ nhớ dịch đã có.
    pub fn nap(thu_muc_game: &Path) -> Result<Self> {
        let tm = thu_muc_game.join("0020");
        if !tm.join("0.pamt").is_file() {
            return Err(anyhow!(
                "không thấy thư mục chữ tiếng Anh (0020) trong thư mục game"
            ));
        }
        let nhom = cd::doc_nhom(&tm)?;
        let bang_ten = BangTen::tao(
            FILE_TEN
                .iter()
                .filter_map(|f| nhom.get(*f))
                .flat_map(|p| p.ds.iter().map(|x| x.2.as_str())),
        );
        let du_lieu = Self::thu_muc_du_lieu();
        std::fs::create_dir_all(&du_lieu)?;
        Ok(Self {
            thu_muc_game: thu_muc_game.to_path_buf(),
            nhom,
            viec: ViecDich::moi(du_lieu, HUONG_DAN, bang_ten),
        })
    }

    /// Chuỗi v trong file f có thuộc diện dịch không (luật của đợt chứa file f).
    fn nen_dich(&self, f: &str, v: &str, dot: Option<usize>) -> bool {
        if khong_can_dich(v) || self.viec.bang_ten.la_ten(v) {
            return false;
        }
        let Some(d) = dot.or_else(|| DOT.iter().position(|x| x.file.contains(&f))) else {
            return false;
        };
        !(DOT[d].chi_cau.contains(&f) && !la_cau(v))
    }

    fn chuoi_cua_dot(&self, dot: usize) -> Vec<&str> {
        DOT[dot]
            .file
            .iter()
            .filter_map(|f| self.nhom.get(*f).map(|p| (f, p)))
            .flat_map(|(f, p)| {
                p.ds.iter()
                    .map(|x| x.2.as_str())
                    .filter(move |v| self.nen_dich(f, v, Some(dot)))
            })
            .collect()
    }

    /// Các câu (không trùng) của đợt chưa có trong bộ nhớ.
    pub fn cau_can_dich(&self, dot: usize) -> Vec<String> {
        self.viec.cau_can_dich(self.chuoi_cua_dot(dot))
    }

    /// (tổng câu khác nhau, đã dịch, còn, ký tự còn)
    pub fn thong_ke(&self, dot: usize) -> (usize, usize, usize, usize) {
        let tat_ca: std::collections::HashSet<&str> = self.chuoi_cua_dot(dot).into_iter().collect();
        let con = self.cau_can_dich(dot);
        let da = {
            let bn = self.viec.bo_nho.lock().unwrap_or_else(|e| e.into_inner());
            tat_ca.iter().filter(|v| bn.contains_key(**v)).count()
        };
        (
            tat_ca.len(),
            da,
            con.len(),
            con.iter().map(|v| v.chars().count()).sum(),
        )
    }

    /// Ghi gói zip cho DMM: VietHoa_CrimsonDesert/0020/gamedata/stringtable/binary__/eng/*.paloc
    /// Trả số câu tiếng Việt đã đưa vào.
    pub fn xuat_goi(&self, duong_zip: &Path) -> Result<usize> {
        let bn = self
            .viec
            .bo_nho
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let tam = duong_zip.with_extension("zip.tam");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&tam)?);
        let tuy = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut tong = 0;
        let mut cac_file: Vec<&str> = DOT.iter().flat_map(|d| d.file.iter().copied()).collect();
        cac_file.sort();
        cac_file.dedup();
        for f in cac_file {
            let Some(p) = self.nhom.get(f) else { continue };
            let dich: HashMap<String, String> =
                p.ds.iter()
                    .filter_map(|(_, k, v)| {
                        let vi = bn.get(v)?;
                        (vi != v && self.nen_dich(f, v, None)).then(|| (k.clone(), vi.clone()))
                    })
                    .collect();
            if dich.is_empty() {
                continue;
            }
            let (b, so) = cd::dong_goi_paloc(p, &dich);
            z.start_file(
                format!("VietHoa_CrimsonDesert/0020/gamedata/stringtable/binary__/eng/{f}.paloc"),
                tuy,
            )?;
            z.write_all(&b)?;
            tong += so;
        }
        z.finish()?;
        std::fs::rename(&tam, duong_zip).context("ghi gói zip")?;
        Ok(tong)
    }

    // ---------------- mod tăng ô chứa đồ ----------------

    /// Đọc inventory.staticinfobody của bản game đang cài, tạo 2 mod DMM (JSON vá byte):
    /// TangO_1440_AnToan.json (balo / kho thành phố 732, còn lại 1440) và TangO_1440_TatCa.json.
    pub fn tao_mod_tui_do(&self, thu_ra: &Path) -> Result<Vec<String>> {
        let nhom = self.thu_muc_game.join("0008");
        let ds = cd::doc_pamt(&nhom.join("0.pamt"))?;
        let f = ds
            .iter()
            .find(|f| f.ten == "inventory" && f.duoi == ".staticinfobody")
            .ok_or_else(|| anyhow!("không thấy file inventory.staticinfobody trong nhóm 0008"))?;
        let b = cd::doc_tep(&nhom, f)?;
        let dau = ds
            .iter()
            .find(|f| f.ten == "inventory" && f.duoi == ".staticinfoheader")
            .and_then(|f| cd::doc_tep(&nhom, f).ok());
        let (kq, bao_cao) = tim_o_chua(&b, dau.as_deref());
        if kq.is_empty() {
            return Err(anyhow!(
                "không tìm thấy ô chứa đồ nào – {}",
                bao_cao.join("; ")
            ));
        }
        std::fs::create_dir_all(thu_ra)?;
        for (tui, ten, mo_ta) in [
            (
                732,
                "TangO_1440_AnToan.json",
                "balo 732 ô (mức an toàn), kho / tủ / sưu tầm 1440 ô",
            ),
            (
                1440,
                "TangO_1440_TatCa.json",
                "TẤT CẢ 1440 ô kể cả balo (vượt mức an toàn)",
            ),
        ] {
            let j = json_mod_tui_do(&kq, tui, mo_ta);
            duong_dan::ghi_file(
                &thu_ra.join(ten),
                serde_json::to_string_pretty(&j)?.as_bytes(),
            )?;
        }
        Ok(bao_cao)
    }
}

/// Bảng chỉ mục của file .staticinfoheader: (tên bản ghi, vị trí, cỡ). Thử các độ rộng số đếm / mã khác nhau
/// (mỗi bảng một kiểu) và nhận kiểu khớp đúng hết file.
fn doc_chi_muc(h: &[u8]) -> Option<Vec<(String, usize, usize)>> {
    let rd = |o: usize, n: usize| -> Option<usize> {
        let s = h.get(o..o + n)?;
        Some(
            s.iter()
                .enumerate()
                .fold(0usize, |v, (i, b)| v | ((*b as usize) << (8 * i))),
        )
    };
    for cw in [2usize, 4] {
        for kw in [2usize, 4] {
            let Some(n) = rd(0, cw) else { continue };
            let (mut p, mut ra, mut ok) = (cw, Vec::new(), true);
            for _ in 0..n {
                let Some(l) = rd(p + kw, 4).filter(|l| *l < 300) else {
                    ok = false;
                    break;
                };
                let (Some(nm), Some(o), Some(sz)) = (
                    h.get(p + kw + 4..p + kw + 4 + l),
                    rd(p + kw + 4 + l, 4),
                    rd(p + kw + 8 + l, 4),
                ) else {
                    ok = false;
                    break;
                };
                ra.push((String::from_utf8_lossy(nm).into_owned(), o, sz));
                p += kw + 12 + l;
            }
            if ok && p == h.len() {
                return Some(ra);
            }
        }
    }
    None
}

/// Tạo mod "hiện rương trên bản đồ" cho đúng bản game đang cài (9 mức: 15…3000 m), JSON vá byte cho DMM.
/// Tìm bản ghi theo TÊN nên game cập nhật chỉ cần bấm tạo lại. Bật MỘT mức duy nhất trong DMM.
pub fn tao_mod_ruong(thu_muc_game: &Path, thu_ra: &Path) -> Result<Vec<String>> {
    let nhom = thu_muc_game.join("0008");
    let ds = cd::doc_pamt(&nhom.join("0.pamt"))?;
    let doc = |ten: &str| -> Result<(Vec<u8>, Vec<(String, usize, usize)>)> {
        let tim = |duoi: &str| {
            ds.iter()
                .find(|f| f.ten == ten && f.duoi == duoi)
                .ok_or_else(|| anyhow!("không thấy {ten}{duoi} trong nhóm 0008"))
        };
        let b = cd::doc_tep(&nhom, tim(".staticinfobody")?)?;
        let h = cd::doc_tep(&nhom, tim(".staticinfoheader")?)?;
        let ci = doc_chi_muc(&h)
            .ok_or_else(|| anyhow!("không đọc được chỉ mục {ten} (game đổi cấu trúc)"))?;
        Ok((b, ci))
    };
    let mut bao = Vec::new();
    // 1. rương: số thực "bán kính hiện" (mặc định 2.0) trong bản ghi TreasureBox / TreasureBox_Sector
    let (lg, ci) = doc("levelgimmicksceneobjectinfo")?;
    let mut vt_kc: Vec<usize> = Vec::new();
    for ten in ["TreasureBox", "TreasureBox_Sector"] {
        let (_, o, sz) = ci
            .iter()
            .find(|x| x.0 == ten)
            .ok_or_else(|| anyhow!("không thấy bản ghi {ten}"))?;
        let r = lg
            .get(*o..*o + *sz)
            .ok_or_else(|| anyhow!("bản ghi {ten} hỏng"))?;
        let i = (30..r.len().saturating_sub(4).min(90))
            .find(|&i| r[i..i + 4] == [0, 0, 0, 0x40])
            .ok_or_else(|| anyhow!("không thấy bán kính hiện trong {ten} (game đổi cấu trúc)"))?;
        vt_kc.push(*o + i);
        bao.push(format!("+ {ten}: bán kính hiện ở byte {}", *o + i));
    }
    // 2. biểu tượng bản đồ: lớp bản đồ (byte 36) và cờ tự chuyển lớp vực sâu (byte 38)
    let (um, ci) = doc("uimaptextureinfo")?;
    let mut lop: Vec<(usize, String, u8, u8)> = Vec::new();
    for ten in ["Gimmick_LevelTreasureBox", "Gimmick_LevelTreasureBoxOpened"] {
        let (_, o, sz) = ci
            .iter()
            .find(|x| x.0 == ten)
            .ok_or_else(|| anyhow!("không thấy biểu tượng {ten}"))?;
        if *sz < 40 || um.len() < *o + 40 {
            return Err(anyhow!("biểu tượng {ten} lạ"));
        }
        let (l, c) = (um[*o + 36], um[*o + 38]);
        if l > 5 || c > 1 {
            return Err(anyhow!(
                "biểu tượng {ten}: giá trị lạ ({l}/{c}) – game đổi cấu trúc"
            ));
        }
        lop.push((*o, ten.to_string(), l, c));
        bao.push(format!("+ {ten}: lớp {l}, tự chuyển {c}"));
    }
    std::fs::create_dir_all(thu_ra)?;
    for m in [15u32, 30, 50, 100, 200, 500, 1000, 2000, 3000] {
        let f = hex_bytes(&(m as f32).to_le_bytes());
        let c1: Vec<serde_json::Value> = vt_kc
                .iter()
                .map(|o| serde_json::json!({"offset": o, "label": format!("Rương: khoảng hiện trên bản đồ -> {m} m"), "original": "00000040", "patched": f}))
                .collect();
        let mut c2: Vec<serde_json::Value> = Vec::new();
        for (o, n, l, c) in &lop {
            c2.push(serde_json::json!({"offset": o + 36, "label": format!("{n}: lớp bản đồ {l} -> 5"), "original": format!("{l:02X}"), "patched": "05"}));
            c2.push(serde_json::json!({"offset": o + 38, "label": format!("{n}: tắt tự chuyển lớp vực sâu"), "original": format!("{c:02X}"), "patched": "00"}));
        }
        let j = serde_json::json!({
            "name": format!("Hiện rương trên bản đồ – {m}m"), "version": "CD2.06", "author": "Việt Hóa",
            "description": format!("Rương chưa mở hiện trong bán kính {m} m trên bản đồ. Chỉ bật MỘT bản."),
            "dmm_v3upgrade_disable": "true",
            "patches": [
                {"game_file": "gamedata/levelgimmicksceneobjectinfo.staticinfobody", "changes": c1},
                {"game_file": "gamedata/uimaptextureinfo.staticinfobody", "changes": c2}
            ]
        });
        duong_dan::ghi_file(
            &thu_ra.join(format!("RuongHien_{m}m.json")),
            serde_json::to_string_pretty(&j)?.as_bytes(),
        )?;
    }
    Ok(bao)
}

fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

/// Ô chứa đồ: (tên Việt, tên bản ghi, loại "tui" = tối đa an toàn 732 | "kho" = 1440)
const O_CHUA: &[(&str, &str, &str)] = &[
    ("Túi đồ (balo)", "Character", "tui"),
    ("Kho riêng ở trại", "CampWareHouse", "kho"),
    ("Kho thành phố / nồi Kuku", "WareHouse", "tui"),
    ("Tủ quần áo", "Housing_Dresser", "kho"),
    ("Tủ lạnh Kuku", "Housing_Refrigerator", "kho"),
    ("Biểu tượng", "Housing_Symbol", "kho"),
    ("Đồ sưu tầm", "Housing_Collecting", "kho"),
    ("Đồ hái lượm", "Housing_GatheredMaterials", "kho"),
];

pub struct OChua {
    ten_viet: &'static str,
    vt_mac_dinh: usize,
    mac_dinh: u16,
    vt_toi_da: usize,
    toi_da: u16,
    loai: &'static str,
}

/// Bản ghi: [u32 độ dài][tên][u32 mã][3 byte][u16 mặc định][4 byte][u16 tối đa]
/// Bố cục game 2.06: tên bản ghi nằm trong file header
/// [u16 số bản ghi]{[u16 mã][u32 dài][tên][u32 vị trí][u32 cỡ]}; bản ghi = [u16 mã][...][3 byte cờ][u16 mặc định][4 byte][u16 tối đa].
fn tim_theo_header(b: &[u8], h: &[u8], ten: &str) -> Option<(usize, usize)> {
    let rd32 = |o: usize| {
        h.get(o..o + 4)
            .map(|x| u32::from_le_bytes(x.try_into().unwrap()) as usize)
    };
    let n = u16::from_le_bytes([*h.first()?, *h.get(1)?]) as usize;
    let mut p = 2;
    for _ in 0..n {
        let l = rd32(p + 2)?;
        let nm = h.get(p + 6..p + 6 + l)?;
        let (o, c) = (rd32(p + 6 + l)?, rd32(p + 10 + l)?);
        p += 14 + l;
        if nm == ten.as_bytes() {
            let r = b.get(o..o + c)?;
            let i = (6..16.min(r.len().saturating_sub(3)))
                .find(|&i| r[i..i + 3] == [1, 1, 1] || r[i..i + 3] == [1, 0, 1])?;
            let a = o + i + 3;
            return (a + 8 <= b.len()).then_some((a, a + 6));
        }
    }
    None
}

pub fn tim_o_chua(b: &[u8], header: Option<&[u8]>) -> (Vec<OChua>, Vec<String>) {
    let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    let mut kq = Vec::new();
    let mut bao_cao = Vec::new();
    for &(ten_viet, ten, loai) in O_CHUA {
        let ma = ten.as_bytes();
        if let Some((vd, vm)) = header.and_then(|h| tim_theo_header(b, h, ten)) {
            let (md, td) = (u16_at(vd), u16_at(vm));
            if !(1 <= md && md <= td && td <= 5000) {
                bao_cao.push(format!(
                    "- {ten_viet}: số ô lạ ({md}/{td}) – game đổi cấu trúc"
                ));
                continue;
            }
            bao_cao.push(format!("+ {ten_viet}: {md} ô (tối đa {td})"));
            kq.push(OChua {
                ten_viet,
                vt_mac_dinh: vd,
                mac_dinh: md,
                vt_toi_da: vm,
                toi_da: td,
                loai,
            });
            continue;
        }
        let vt = b.windows(ma.len()).enumerate().find_map(|(i, w)| {
            (w == ma
                && i >= 4
                && u32::from_le_bytes(b[i - 4..i].try_into().unwrap()) as usize == ma.len())
            .then_some(i)
        });
        let Some(vt) = vt else {
            bao_cao.push(format!("- {ten_viet}: không thấy bản ghi {ten}"));
            continue;
        };
        let e = vt + ma.len();
        let (vd, vm) = (e + 7, e + 13);
        if vm + 2 > b.len() {
            bao_cao.push(format!("- {ten_viet}: bản ghi lạ"));
            continue;
        }
        let (md, td) = (u16_at(vd), u16_at(vm));
        if !(1 <= md && md <= td && td <= 5000) {
            bao_cao.push(format!(
                "- {ten_viet}: số ô lạ ({md}/{td}) – game đổi cấu trúc"
            ));
            continue;
        }
        bao_cao.push(format!("+ {ten_viet}: {md} ô (tối đa {td})"));
        kq.push(OChua {
            ten_viet,
            vt_mac_dinh: vd,
            mac_dinh: md,
            vt_toi_da: vm,
            toi_da: td,
            loai,
        });
    }
    (kq, bao_cao)
}

fn hex16(v: u16) -> String {
    v.to_le_bytes().iter().map(|b| format!("{b:02X}")).collect()
}

pub fn json_mod_tui_do(kq: &[OChua], tui_max: u16, mo_ta: &str) -> serde_json::Value {
    let mut doi = Vec::new();
    for o in kq {
        let moi = if o.loai == "tui" { tui_max } else { 1440 };
        doi.push(serde_json::json!({"offset": o.vt_mac_dinh,
            "label": format!("{} {}->{moi}", o.ten_viet, o.mac_dinh),
            "original": hex16(o.mac_dinh), "patched": hex16(moi)}));
        doi.push(serde_json::json!({"offset": o.vt_toi_da,
            "label": format!("{} max {}->{moi}", o.ten_viet, o.toi_da),
            "original": hex16(o.toi_da), "patched": hex16(moi)}));
    }
    serde_json::json!({
        "name": format!("Tăng ô chứa đồ – {mo_ta}"), "version": "CD2.06", "author": "Việt Hóa",
        "description": mo_ta, "dmm_v3upgrade_disable": "true",
        "patches": [{"game_file": "gamedata/inventory.staticinfobody", "changes": doi}]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn o_chua() {
        // bản ghi giả: [len][Character][u32][3 byte][u16 240][4 byte][u16 732]
        let mut b = vec![0u8; 8];
        b.extend(9u32.to_le_bytes());
        b.extend(b"Character");
        b.extend([0u8; 7]);
        b.extend(240u16.to_le_bytes());
        b.extend([0u8; 4]);
        b.extend(732u16.to_le_bytes());
        let (kq, bc) = tim_o_chua(&b, None);
        assert_eq!(kq.len(), 1, "{bc:?}");
        assert_eq!((kq[0].mac_dinh, kq[0].toi_da), (240, 732));
        let j = json_mod_tui_do(&kq, 732, "thử");
        let c = &j["patches"][0]["changes"];
        assert_eq!(c[0]["original"], "F000");
        assert_eq!(c[0]["patched"], "DC02");
        assert_eq!(c[1]["offset"], kq[0].vt_toi_da);
    }

    /// Thử thật (chỉ đọc game, ghi ra thư mục tạm): CD=<thư mục game> cargo test -- --ignored cd_that
    #[test]
    #[ignore]
    fn cd_that() {
        let g = PathBuf::from(std::env::var("CD").unwrap());
        let v = CrimsonDesert::nap(&g).unwrap();
        for d in 0..3 {
            println!("{} => {:?}", DOT[d].ten, v.thong_ke(d));
        }
        let ra = std::env::temp_dir().join("viet_hoa_thu_cd");
        let _ = std::fs::remove_dir_all(&ra);
        std::fs::create_dir_all(&ra).unwrap();
        let n = v.xuat_goi(&ra.join(TEN_GOI)).unwrap();
        println!("gói: {n} câu");
        // đọc lại gói: giải nén từng .paloc, đếm câu tiếng Việt
        let mut z = zip::ZipArchive::new(std::fs::File::open(ra.join(TEN_GOI)).unwrap()).unwrap();
        let mut viet = 0;
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let mut b = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut b).unwrap();
            let (_, nd) = cd::mo_boc(&b).unwrap();
            let p = cd::tach(&nd).unwrap();
            viet +=
                p.ds.iter()
                    .filter(|x| crate::dich::the::la_tieng_viet(&x.2))
                    .count();
            if i == 0 {
                println!("{:?}: {} câu", f.name(), p.ds.len());
            }
        }
        println!("câu có dấu Việt trong gói: {viet}");
        assert!(viet > 0);
        for s in v.tao_mod_tui_do(&ra).unwrap() {
            println!("{s}");
        }
        println!(
            "{}",
            std::fs::read_to_string(ra.join("TangO_1440_AnToan.json")).unwrap()
        );
    }
}

/// Chỉ ĐỌC game: giải nén bảng thú cưỡi / minigame thuần hóa ngựa ra `thu_ra` để phân tích
/// (nhóm 0008: bảng tĩnh; 0014: kịch bản rodeo; 0012: giao diện minigame ngựa).
pub fn xuat_bang_thu_cuoi(
    thu_muc_game: &Path,
    thu_ra: &Path,
    bao: &dyn Fn(&str),
) -> Result<Vec<String>> {
    const BANG: [&str; 4] = [
        "vehicleinfo",
        "minigamedatainfo",
        "characterinfo",
        "knowledgeinfo",
    ];
    std::fs::create_dir_all(thu_ra)?;
    let mut ra = Vec::new();
    let mut ghi =
        |nhom: &Path, f: &cd::TepPamt, tien_to: &str, ra: &mut Vec<String>| -> Result<()> {
            bao(&format!("Đang xuất {}{}...", f.ten, f.duoi));
            let ten = format!("{tien_to}{}{}", f.ten, f.duoi);
            std::fs::write(thu_ra.join(&ten), cd::doc_tep(nhom, f)?)?;
            ra.push(ten);
            Ok(())
        };
    let n8 = thu_muc_game.join("0008");
    for f in cd::doc_pamt(&n8.join("0.pamt"))? {
        let ten = f.ten.strip_prefix("bin").unwrap_or(&f.ten);
        if BANG.contains(&ten) && (f.duoi == ".staticinfobody" || f.duoi == ".staticinfoheader") {
            ghi(&n8, &f, "", &mut ra)?;
        }
    }
    let n14 = thu_muc_game.join("0014");
    match cd::doc_pamt(&n14.join("0.pamt")) {
        Ok(ds) => {
            for f in ds {
                if f.ten.contains("minigame_rodeo")
                    && [".paseq", ".paseqc", ".pastage"].contains(&f.duoi.as_str())
                {
                    let t = if f.thu_muc.contains("binary__") {
                        "bin_"
                    } else {
                        "goc_"
                    };
                    ghi(&n14, &f, t, &mut ra)?;
                }
            }
        }
        Err(e) => bao(&format!("(bỏ qua kịch bản rodeo: {e})")),
    }
    let n12 = thu_muc_game.join("0012");
    match cd::doc_pamt(&n12.join("0.pamt")) {
        Ok(ds) => {
            for f in ds {
                if (f.ten.contains("rodeominigameview") || f.ten.contains("minigame_rodeo"))
                    && [".html", ".css", ".uibinary", ".xml"].contains(&f.duoi.as_str())
                {
                    ghi(&n12, &f, "ui_", &mut ra)?;
                }
            }
        }
        Err(e) => bao(&format!("(bỏ qua giao diện rodeo: {e})")),
    }
    // bảng rương / biểu tượng bản đồ (làm mod kiểu TreasureSeeker): tìm ở mọi nhóm
    if let Ok(rd) = std::fs::read_dir(thu_muc_game) {
        let mut nhom: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.join("0.pamt").is_file())
            .collect();
        nhom.sort();
        for n in nhom {
            let Ok(ds) = cd::doc_pamt(&n.join("0.pamt")) else {
                continue;
            };
            for f in ds {
                let co = [
                    "levelgimmicksceneobjectinfo",
                    "uimaptextureinfo",
                    "inventory",
                ]
                .contains(&f.ten.as_str());
                if co
                    && (f.duoi == ".staticinfobody"
                        || f.duoi == ".staticinfoheader"
                        || f.duoi == ".pabgb"
                        || f.duoi == ".pabgh")
                {
                    ghi(&n, &f, "ruong_", &mut ra)?;
                }
            }
        }
    }
    if ra.is_empty() {
        return Err(anyhow!("không thấy bảng thú cưỡi trong nhóm 0008"));
    }
    Ok(ra)
}
