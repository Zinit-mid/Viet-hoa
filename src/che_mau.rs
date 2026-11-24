//! Che chữ gốc: màu nền = trung vị viền quanh ô chữ; màu chữ = trung vị 7% điểm
//! khác nền nhất (tương phản yếu → trắng/đen). Nền lộn xộn → None (hộp tối).

use crate::chup::Anh;

fn trung_vi(v: &mut [[u8; 3]]) -> [u8; 3] {
    let mut ra = [0u8; 3];
    for k in 0..3 {
        let mut kenh: Vec<u8> = v.iter().map(|p| p[k]).collect();
        kenh.sort_unstable();
        ra[k] = kenh[kenh.len() / 2];
    }
    ra
}

fn khoang_cach(a: [u8; 3], b: [u8; 3]) -> u32 {
    (0..3)
        .map(|k| (a[k] as i32 - b[k] as i32).unsigned_abs())
        .sum()
}

/// `hop` = [x, y, w, h] theo toạ độ trong ảnh `anh`.
/// Trả (màu nền, màu chữ), hoặc None khi nền quá lộn xộn để che cho đẹp.
pub fn mau_che(anh: &Anh, hop: [i32; 4]) -> Option<([u8; 3], [u8; 3])> {
    // Lấy viền cách chữ 6 px để không dính nét chữ / viền mờ
    let le = 6;
    let gioi_han = |v: i32, max: u32| v.clamp(0, max.saturating_sub(1) as i32) as u32;
    let x0 = gioi_han(hop[0] - le, anh.w);
    let y0 = gioi_han(hop[1] - le, anh.h);
    let x1 = gioi_han(hop[0] + hop[2] + le, anh.w);
    let y1 = gioi_han(hop[1] + hop[3] + le, anh.h);
    if x1 <= x0 + 2 || y1 <= y0 + 2 {
        return None;
    }
    let mut vien = Vec::new();
    for x in x0..=x1 {
        vien.push(anh.diem(x, y0));
        vien.push(anh.diem(x, y1));
    }
    for y in y0..=y1 {
        vien.push(anh.diem(x0, y));
        vien.push(anh.diem(x1, y));
    }
    let nen = trung_vi(&mut vien);
    // Nền lộn xộn: hơn 35% điểm viền khác hẳn màu trung vị
    let khac = vien.iter().filter(|p| khoang_cach(**p, nen) > 90).count();
    if khac * 100 > vien.len() * 35 {
        return None;
    }

    let bx0 = hop[0].clamp(0, anh.w as i32) as u32;
    let by0 = hop[1].clamp(0, anh.h as i32) as u32;
    let bx1 = (hop[0] + hop[2]).clamp(0, anh.w as i32) as u32;
    let by1 = (hop[1] + hop[3]).clamp(0, anh.h as i32) as u32;
    let buoc = (((bx1 - bx0) * (by1 - by0)) / 6000).max(1);
    let mut trong: Vec<[u8; 3]> = Vec::new();
    let mut i = 0u32;
    for y in by0..by1 {
        for x in bx0..bx1 {
            i += 1;
            if i % buoc == 0 {
                trong.push(anh.diem(x, y));
            }
        }
    }
    if trong.is_empty() {
        return Some((nen, tuong_phan(nen)));
    }
    trong.sort_by_key(|p| std::cmp::Reverse(khoang_cach(*p, nen)));
    let n = (trong.len() * 7 / 100).max(1);
    let chu = trung_vi(&mut trong[..n]);
    if khoang_cach(chu, nen) < 120 {
        Some((nen, tuong_phan(nen)))
    } else {
        Some((nen, chu))
    }
}

fn tuong_phan(nen: [u8; 3]) -> [u8; 3] {
    let sang = nen[0] as u32 * 299 + nen[1] as u32 * 587 + nen[2] as u32 * 114;
    if sang > 128_000 {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}
