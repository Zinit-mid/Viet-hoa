//! Gõ tiếng Việt kiểu Telex ngay trong ô nhập của app (không cần Unikey). Chỉ xử lý khi gõ thêm 1 ký tự ở cuối.
//! s=sắc f=huyền r=hỏi x=ngã j=nặng z=bỏ dấu, aa/ee/oo=â/ê/ô, aw=ă, ow=ơ, uw=ư, dd=đ. Gõ lặp phím dấu để hủy.

const HANG: [(&str, char, u8); 12] = [
    ("aàáảãạ", 'a', 0),
    ("âầấẩẫậ", 'a', 1),
    ("ăằắẳẵặ", 'a', 2),
    ("eèéẻẽẹ", 'e', 0),
    ("êềếểễệ", 'e', 1),
    ("iìíỉĩị", 'i', 0),
    ("oòóỏõọ", 'o', 0),
    ("ôồốổỗộ", 'o', 1),
    ("ơờớởỡợ", 'o', 3),
    ("uùúủũụ", 'u', 0),
    ("ưừứửữự", 'u', 3),
    ("yỳýỷỹỵ", 'y', 0),
];

/// (chỉ số hàng, dấu thanh 0..5)
fn giai(c: char) -> Option<(usize, usize)> {
    let l = c.to_lowercase().next()?;
    for (i, h) in HANG.iter().enumerate() {
        if let Some(t) = h.0.chars().position(|x| x == l) {
            return Some((i, t));
        }
    }
    None
}

fn ghep(c: char, hang: usize, thanh: usize) -> char {
    let m = HANG[hang].0.chars().nth(thanh).unwrap_or(c);
    if c.is_uppercase() {
        m.to_uppercase().next().unwrap_or(m)
    } else {
        m
    }
}

fn la_ban(w: &[char], i: usize, base: char) -> bool {
    giai(w[i]).is_some_and(|(h, _)| HANG[h].1 == base)
}

fn vi_tri_dau(w: &[char]) -> Option<usize> {
    let mut v: Vec<usize> = (0..w.len()).filter(|&i| giai(w[i]).is_some()).collect();
    let thuong: Vec<char> = w
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    if v.len() >= 2 && v[0] == 1 && thuong[0] == 'q' && thuong[1] == 'u' {
        v.remove(0);
    } else if v.len() >= 2 && v[0] == 1 && thuong[0] == 'g' && thuong[1] == 'i' {
        v.remove(0);
    }
    if v.is_empty() {
        return None;
    }
    if let Some(&p) = v
        .iter()
        .rev()
        .find(|&&i| giai(w[i]).is_some_and(|(h, _)| HANG[h].2 != 0))
    {
        return Some(p);
    }
    Some(match v.len() {
        1 => v[0],
        2 => {
            if v[1] + 1 < w.len() {
                v[1]
            } else {
                v[0]
            }
        }
        _ => v[1],
    })
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kieu {
    Telex,
    Vni,
    Tat,
}

enum Lenh {
    Thanh(usize),
    /// ơ ư ă; chuỗi = các nguyên âm gốc được phép
    Moc(&'static str),
    /// â ê ô; Some(c) = (Telex) nguyên âm phải đứng ngay cuối từ
    Mu(Option<char>),
    D,
}

fn lenh(kieu: Kieu, c: char) -> Option<Lenh> {
    let lc = c.to_lowercase().next()?;
    match kieu {
        Kieu::Telex => Some(match lc {
            's' => Lenh::Thanh(2),
            'f' => Lenh::Thanh(1),
            'r' => Lenh::Thanh(3),
            'x' => Lenh::Thanh(4),
            'j' => Lenh::Thanh(5),
            'z' => Lenh::Thanh(0),
            'w' => Lenh::Moc("aou"),
            'a' | 'e' | 'o' => Lenh::Mu(Some(lc)),
            'd' => Lenh::D,
            _ => return None,
        }),
        Kieu::Vni => Some(match lc {
            '1' => Lenh::Thanh(2),
            '2' => Lenh::Thanh(1),
            '3' => Lenh::Thanh(3),
            '4' => Lenh::Thanh(4),
            '5' => Lenh::Thanh(5),
            '0' => Lenh::Thanh(0),
            '6' => Lenh::Mu(None),
            '7' => Lenh::Moc("ou"),
            '8' => Lenh::Moc("a"),
            '9' => Lenh::D,
            _ => return None,
        }),
        Kieu::Tat => None,
    }
}

/// `truoc`: văn bản trước khi gõ, `c`: ký tự vừa gõ. Trả về văn bản mới nếu có biến đổi.
pub fn them_ky_tu(truoc: &str, c: char, kieu: Kieu) -> Option<String> {
    let chars: Vec<char> = truoc.chars().collect();
    let dau = chars
        .iter()
        .rposition(|x| !x.is_alphabetic())
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut w: Vec<char> = chars[dau..].to_vec();
    let ra = |w: &[char]| -> String { chars[..dau].iter().chain(w.iter()).collect() };
    match lenh(kieu, c)? {
        Lenh::Thanh(t) => {
            let p = vi_tri_dau(&w)?;
            let (h, cu) = giai(w[p])?;
            if cu == t && t != 0 {
                w[p] = ghep(w[p], h, 0);
                w.push(c);
            } else if t == 0 && cu == 0 {
                return None;
            } else {
                w[p] = ghep(w[p], h, t);
            }
            Some(ra(&w))
        }
        Lenh::Moc(cho_phep) => {
            if w.is_empty() {
                return if kieu == Kieu::Telex {
                    Some(ra(&[if c.is_uppercase() { 'Ư' } else { 'ư' }]))
                } else {
                    None
                };
            }
            let p = (0..w.len())
                .rev()
                .find(|&i| giai(w[i]).is_some_and(|(h, _)| cho_phep.contains(HANG[h].1)))?;
            let (h, t) = giai(w[p])?;
            let base = HANG[h].1;
            if HANG[h].2 != 0 {
                // đã có dấu mũ/móc → gõ lần nữa là hủy
                let h0 = HANG.iter().position(|x| x.1 == base && x.2 == 0)?;
                w[p] = ghep(w[p], h0, t);
                w.push(c);
                return Some(ra(&w));
            }
            let moi = HANG
                .iter()
                .position(|x| x.1 == base && x.2 == if base == 'a' { 2 } else { 3 })?;
            w[p] = ghep(w[p], moi, t);
            // "uo" + móc → "ươ"
            if base == 'o' && p > 0 && la_ban(&w, p - 1, 'u') {
                if let Some((hu, tu)) = giai(w[p - 1]) {
                    if HANG[hu].2 == 0 {
                        let hu3 = HANG.iter().position(|x| x.1 == 'u' && x.2 == 3)?;
                        w[p - 1] = ghep(w[p - 1], hu3, tu);
                    }
                }
            }
            Some(ra(&w))
        }
        Lenh::Mu(yeu_cau) => {
            let p = match yeu_cau {
                Some(_) => w.len().checked_sub(1)?,
                None => (0..w.len()).rev().find(|&i| {
                    giai(w[i]).is_some_and(|(h, _)| matches!(HANG[h].1, 'a' | 'e' | 'o'))
                })?,
            };
            let (h, t) = giai(w[p])?;
            let base = HANG[h].1;
            if !matches!(base, 'a' | 'e' | 'o') || yeu_cau.is_some_and(|q| q != base) {
                return None;
            }
            let (loai, moi) = match HANG[h].2 {
                0 => (true, HANG.iter().position(|x| x.1 == base && x.2 == 1)?),
                1 => (false, HANG.iter().position(|x| x.1 == base && x.2 == 0)?),
                _ => return None,
            };
            w[p] = ghep(w[p], moi, t);
            if !loai {
                w.push(c);
            }
            Some(ra(&w))
        }
        Lenh::D => {
            let p = w.len().checked_sub(1)?;
            match w[p] {
                'd' | 'D' => {
                    w[p] = if w[p] == 'D' { 'Đ' } else { 'đ' };
                    Some(ra(&w))
                }
                'đ' | 'Đ' => {
                    w[p] = if w[p] == 'Đ' { 'D' } else { 'd' };
                    w.push(c);
                    Some(ra(&w))
                }
                _ => None,
            }
        }
    }
}

/// Gọi sau mỗi lần ô nhập đổi: nếu chỉ gõ thêm đúng 1 ký tự ở cuối thì áp bộ gõ. `truoc` luôn được cập nhật.
pub fn ap_dung(text: &mut String, truoc: &mut String, kieu: Kieu) {
    if *text != *truoc {
        if let Some(c) = text
            .strip_prefix(truoc.as_str())
            .filter(|r| r.chars().count() == 1)
            .and_then(|r| r.chars().next())
        {
            if let Some(moi) = them_ky_tu(truoc, c, kieu) {
                *text = moi;
            }
        }
    }
    *truoc = text.clone();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn go(s: &str) -> String {
        go_k(s, Kieu::Telex)
    }
    fn go_k(s: &str, k: Kieu) -> String {
        let mut t = String::new();
        for c in s.chars() {
            let mut n = t.clone();
            n.push(c);
            let mut p = t.clone();
            ap_dung(&mut n, &mut p, k);
            t = n;
        }
        t
    }
    #[test]
    fn telex() {
        assert_eq!(go("tieengs Vieejt"), "tiếng Việt");
        assert_eq!(go("xin chaof"), "xin chào");
        assert_eq!(go("nguwowif"), "người");
        assert_eq!(go("dduwowcj"), "được");
        assert_eq!(go("hoaf"), "hòa");
        assert_eq!(go("quas"), "quá");
        assert_eq!(go("giaos"), "giáo");
        assert_eq!(go("tooi"), "tôi");
        assert_eq!(go("looix"), "lỗi");
        assert_eq!(go("khoong"), "không");
        assert_eq!(go("ass"), "as");
    }
    #[test]
    fn vni() {
        let v = |s: &str| go_k(s, Kieu::Vni);
        assert_eq!(v("tie6ng1 Vie6t5"), "tiếng Việt");
        assert_eq!(v("ngu7o7i2"), "người");
        assert_eq!(v("d9u7o7c5"), "được");
        assert_eq!(v("hoa2"), "hòa");
        assert_eq!(v("to6i"), "tôi");
        assert_eq!(v("a8"), "ă");
        assert_eq!(v("khong6"), "không");
        assert_eq!(v("thuong7"), "thương");
        assert_eq!(v("a11"), "a1");
    }
}
