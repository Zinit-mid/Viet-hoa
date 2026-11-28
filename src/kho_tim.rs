//! Phần tìm trong Kho kiến thức. Chỉ dùng thư viện chuẩn để thử riêng được:
//! `rustc --edition 2021 --test src/kho_tim.rs`

use std::collections::{BTreeSet, HashMap};

/// Độ dài tối đa (ký tự) của một đoạn gửi cho AI.
const DOAN_TOI_DA: usize = 600;

const TU_BO_QUA: &str = "và là của có cho các một những được không trong với này đó khi thì mà ở ra vào lên xuống làm sao gì nào bị đã sẽ đang rồi tôi bạn nó họ ta cái con người về từ đến như để hay hoặc nếu vì nên cũng chỉ còn rất quá lại đi máy lỗi \
the a an and or of to in on at is are was were be been you your we they he she it this that these those with for from not no yes do does did have has had will would can could should what when where who how why my me him her them our their there here just so if but as by up out all any";

/// Một mục kiến thức đưa vào tìm.
pub struct Nguon<'a> {
    pub tieu_de: &'a str,
    pub noi_dung: &'a str,
    /// mục thuật ngữ: dòng "từ gốc = nghĩa" chỉ gửi khi từ gốc xuất hiện
    pub thuat_ngu: bool,
}

fn tach_tu(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 2)
        .map(String::from)
        .collect()
}

fn tu_khoa(s: &str) -> BTreeSet<String> {
    let bo: BTreeSet<&str> = TU_BO_QUA.split_whitespace().collect();
    tach_tu(s)
        .into_iter()
        .filter(|t| !bo.contains(t.as_str()))
        .collect()
}

/// `cum` xuất hiện trong `van` thành từ riêng (không nằm giữa một từ khác), không phân biệt hoa thường.
fn co_cum(van: &str, cum: &str) -> bool {
    let (van, cum) = (van.to_lowercase(), cum.trim().to_lowercase());
    if cum.is_empty() {
        return false;
    }
    van.match_indices(&cum).any(|(i, m)| {
        let truoc = van[..i].chars().next_back();
        let sau = van[i + m.len()..].chars().next();
        !truoc.is_some_and(char::is_alphanumeric) && !sau.is_some_and(char::is_alphanumeric)
    })
}

/// Chia nội dung thành đoạn: theo dòng trống; đoạn quá dài thì cắt theo từ.
pub fn chia_doan(noi_dung: &str) -> Vec<String> {
    let noi_dung = noi_dung.replace('\r', "");
    let mut ra = Vec::new();
    for khoi in noi_dung.split("\n\n") {
        let mut doan = String::new();
        for dong in khoi.lines() {
            for (i, tu) in dong.split_whitespace().enumerate() {
                // một "từ" dài bất thường thì cắt cứng
                let ky: Vec<char> = tu.chars().collect();
                for manh in ky.chunks(DOAN_TOI_DA) {
                    if !doan.is_empty() && doan.chars().count() + manh.len() + 1 > DOAN_TOI_DA {
                        ra.push(std::mem::take(&mut doan));
                    }
                    if !doan.is_empty() {
                        doan.push(if i == 0 { '\n' } else { ' ' });
                    }
                    doan.extend(manh);
                }
            }
        }
        if !doan.is_empty() {
            ra.push(doan);
        }
    }
    ra
}

/// Chọn phần liên quan tới `hoi`, tổng không quá `toi_da` ký tự.
/// `gui_quy_tac`: dòng không có dấu "=" trong mục thuật ngữ được gửi luôn (dùng khi dịch).
/// → (các dòng (số thứ tự nguồn, chữ), theo thứ tự ưu tiên).
pub fn chon(nguon: &[Nguon], hoi: &str, toi_da: usize, gui_quy_tac: bool) -> Vec<(usize, String)> {
    let mut quy_tac = Vec::new();
    let mut thuat_ngu = Vec::new();
    let mut doan: Vec<(usize, String, BTreeSet<String>)> = Vec::new();
    for (i, n) in nguon.iter().enumerate() {
        if n.thuat_ngu {
            let mut con_lai = String::new();
            for dong in n.noi_dung.lines().map(str::trim).filter(|d| !d.is_empty()) {
                match dong.split_once('=') {
                    Some((goc, _)) => {
                        if co_cum(hoi, goc) {
                            thuat_ngu.push((i, dong.to_string()));
                        }
                    }
                    None if gui_quy_tac => quy_tac.push((i, dong.to_string())),
                    None => {
                        con_lai.push_str(dong);
                        con_lai.push('\n');
                    }
                }
            }
            for d in chia_doan(&con_lai) {
                let tu = tu_khoa(&format!("{} {d}", n.tieu_de));
                doan.push((i, d, tu));
            }
        } else {
            for d in chia_doan(n.noi_dung) {
                let tu = tu_khoa(&format!("{} {d}", n.tieu_de));
                doan.push((i, d, tu));
            }
        }
    }

    let hoi_tu = tu_khoa(hoi);
    let mut so_doan: HashMap<&str, usize> = HashMap::new();
    for (_, _, tu) in &doan {
        for t in tu.intersection(&hoi_tu) {
            *so_doan.entry(t.as_str()).or_default() += 1;
        }
    }
    let tong = doan.len() as f64;
    let mut diem: Vec<(f64, usize)> = doan
        .iter()
        .enumerate()
        .filter_map(|(k, (_, _, tu))| {
            let trung: Vec<&String> = tu.intersection(&hoi_tu).collect();
            // cần ít nhất 2 từ trùng, hoặc 1 từ đủ đặc trưng (tên riêng, mã lỗi)
            let du = trung.len() >= 2 || trung.iter().any(|t| t.chars().count() >= 4);
            du.then(|| {
                let d: f64 = trung
                    .iter()
                    .map(|t| (1.0 + tong / so_doan[t.as_str()] as f64).ln())
                    .sum();
                (d, k)
            })
        })
        .collect();
    diem.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));

    let mut ra = Vec::new();
    let mut dai = 0;
    let ung_vien = thuat_ngu.into_iter().chain(quy_tac).chain(
        diem.into_iter()
            .map(|(_, k)| (doan[k].0, doan[k].1.clone())),
    );
    for (i, chu) in ung_vien {
        let n = chu.chars().count();
        if dai + n <= toi_da {
            dai += n;
            ra.push((i, chu));
        }
    }
    ra
}

#[cfg(test)]
mod test {
    use super::*;

    fn n<'a>(tieu_de: &'a str, noi_dung: &'a str, thuat_ngu: bool) -> Nguon<'a> {
        Nguon {
            tieu_de,
            noi_dung,
            thuat_ngu,
        }
    }

    #[test]
    fn thuat_ngu_chi_gui_tu_co_mat() {
        let ds = [n(
            "Từ",
            "Greymanes = Bờm Xám\nOre = quặng\nKliff xưng ta",
            true,
        )];
        let r = chon(&ds, "The Greymanes need more.", 1000, true);
        let chu: Vec<&str> = r.iter().map(|x| x.1.as_str()).collect();
        // "Ore" nằm trong "more" thì không tính
        assert_eq!(chu, ["Greymanes = Bờm Xám", "Kliff xưng ta"]);
        assert_eq!(chon(&ds, "iron ore", 1000, false).len(), 1);
    }

    #[test]
    fn tim_dung_muc() {
        let ds = [
            n("Nhân vật Kliff", "Kliff là thủ lĩnh nhóm Greymanes.", false),
            n(
                "Ổ đĩa đầy",
                "Dung lượng ổ C còn ít thì Windows Update thất bại.",
                false,
            ),
        ];
        let r = chon(&ds, "Windows Update thất bại thì làm sao?", 1000, false);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, 1);
        assert_eq!(chon(&ds, "Kliff, wait for me!", 1000, true)[0].0, 0);
        assert!(chon(&ds, "hôm nay trời đẹp", 1000, false).is_empty());
        assert!(chon(&ds, "", 1000, false).is_empty());
    }

    #[test]
    fn ma_loi() {
        let ds = [n("Cập nhật", "Lỗi 0x80070005 là bị từ chối quyền.", false)];
        assert_eq!(chon(&ds, "máy báo 0x80070005", 1000, false).len(), 1);
    }

    #[test]
    fn khong_vuot_gioi_han() {
        let dai = "Kliff chiến đấu. ".repeat(500);
        let dac = "x".repeat(2000);
        let ds = [n("A", &dai, false), n("B", &dac, false)];
        assert!(chia_doan(&dai)
            .iter()
            .all(|d| d.chars().count() <= DOAN_TOI_DA));
        assert!(chia_doan(&dac)
            .iter()
            .all(|d| d.chars().count() <= DOAN_TOI_DA));
        let r = chon(&ds, "Kliff", 1500, false);
        assert!(!r.is_empty());
        assert!(r.iter().map(|x| x.1.chars().count()).sum::<usize>() <= 1500);
    }
}
