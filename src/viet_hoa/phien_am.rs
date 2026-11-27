//! Phiên âm tên riêng nước ngoài (Kliff, Konstantin…) thành cách đọc tiếng Việt để VieNeu đọc được.
//! Chỉ dùng cho CHỮ ĐƯA VÀO MÁY ĐỌC – phụ đề vẫn giữ tên gốc. Chuyển từ phien_am.py.
//! Thứ tự: từ điển cố định → từ điển học sẵn (`du_lieu\phien_am.json` nếu có) → quy tắc tự động.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

const CO_DINH: &[(&str, &str)] = &[
    ("kliff", "Cờ-líp"),
    ("oongka", "Un-ca"),
    ("yann", "Dan"),
    ("damian", "Đây-mi-ần"),
    ("nairah", "Nai-ra"),
    ("naira", "Nai-ra"),
    ("greymane", "Grây-mên"),
    ("greymanes", "Grây-mên"),
    ("pywel", "Pai-oeo"),
    ("hernand", "Hơ-nan"),
    ("demeniss", "Đê-mơ-nít"),
    ("pailune", "Pai-lun"),
    ("delesyia", "Đê-lê-xi-a"),
    ("kweiden", "Quây-đần"),
    ("calphade", "Can-phết"),
    ("witch", "Phù thủy"),
    ("marius", "Ma-ri-út"),
    ("dwayne", "Đuây-n"),
    ("andrew", "An-đru"),
    ("tolstein", "Tôn-xtai"),
    ("shay", "Sây"),
    ("russo", "Ru-xô"),
    ("valgash", "Van-gát"),
    ("alustin", "A-lu-xtin"),
    ("shakatu", "Sa-ca-tu"),
    ("diederik", "Đi-đơ-ríc"),
    ("azureian", "A-du-rê-an"),
    ("drake", "Đrây-c"),
    ("myurdin", "Mi-ua-đin"),
    ("kamu", "Ca-mu"),
    ("matthias", "Ma-thi-át"),
    ("crimson", "Crim-xơn"),
    ("abyss", "A-bít"),
    ("silver", "Xin-vơ"),
    ("desert", "Đe-dợt"),
    ("lara", "La-ra"),
    ("croft", "Cờ-róp"),
    ("jonah", "Giô-na"),
    ("jacob", "Gia-cốp"),
    ("nadia", "Na-đi-a"),
    ("sofia", "Xô-phi-a"),
    ("ana", "A-na"),
    ("konstantin", "Côn-xtan-tin"),
    ("trinity", "Tri-ni-ti"),
    ("remnant", "Rem-nần"),
    ("kitezh", "Ki-tét"),
    ("siberia", "Xi-bê-ri-a"),
    ("syria", "Xi-ri-a"),
    ("baba", "Ba-ba"),
    ("yaga", "Gia-ga"),
    ("atlas", "Át-lát"),
    ("richard", "Ri-chát"),
    ("lord", "Lo"),
    ("prophet", "Pro-phít"),
    ("divine", "Đi-vai"),
    ("source", "Xót"),
    ("soviet", "Xô-viết"),
    ("gulag", "Gu-lát"),
    ("constantinople", "Côn-xtan-ti-nốp"),
    ("byzantine", "By-dăng"),
    ("ivan", "I-van"),
    ("jonathan", "Giô-na-than"),
];

const VAN: &[(&str, &str)] = &[
    ("ee", "i"),
    ("ea", "i"),
    ("oo", "u"),
    ("ou", "ao"),
    ("ow", "ao"),
    ("au", "o"),
    ("aw", "o"),
    ("ai", "ây"),
    ("ay", "ây"),
    ("ei", "ây"),
    ("ey", "ây"),
    ("oi", "oi"),
    ("oy", "oi"),
    ("ie", "i"),
    ("ue", "u"),
    ("ui", "ui"),
    ("ia", "ia"),
    ("io", "iô"),
    ("eu", "iu"),
    ("a", "a"),
    ("e", "e"),
    ("i", "i"),
    ("o", "ô"),
    ("u", "u"),
    ("y", "i"),
];
const DAU: &[(&str, &str)] = &[
    ("sch", "s"),
    ("tch", "ch"),
    ("ch", "ch"),
    ("sh", "s"),
    ("th", "th"),
    ("ph", "ph"),
    ("kh", "kh"),
    ("gh", "g"),
    ("wh", "u"),
    ("qu", "qu"),
    ("ck", "c"),
    ("b", "b"),
    ("c", "c"),
    ("d", "đ"),
    ("f", "ph"),
    ("g", "g"),
    ("h", "h"),
    ("j", "gi"),
    ("k", "c"),
    ("l", "l"),
    ("m", "m"),
    ("n", "n"),
    ("p", "p"),
    ("q", "c"),
    ("r", "r"),
    ("s", "x"),
    ("t", "t"),
    ("v", "v"),
    ("w", "u"),
    ("x", "x"),
    ("z", "d"),
];
const CUOI: &[(&str, &str)] = &[
    ("c", "c"),
    ("k", "c"),
    ("ck", "c"),
    ("g", "c"),
    ("q", "c"),
    ("d", "t"),
    ("t", "t"),
    ("th", "t"),
    ("b", "p"),
    ("p", "p"),
    ("f", "p"),
    ("ff", "p"),
    ("ph", "p"),
    ("v", "p"),
    ("n", "n"),
    ("nn", "n"),
    ("m", "m"),
    ("mm", "m"),
    ("ng", "ng"),
    ("nk", "ng"),
    ("l", "n"),
    ("ll", "n"),
    ("r", ""),
    ("rr", ""),
    ("s", ""),
    ("ss", "t"),
    ("z", ""),
    ("x", "c"),
    ("ch", "ch"),
    ("sh", ""),
    ("tch", "ch"),
    ("h", ""),
    ("gh", ""),
    ("w", ""),
    ("y", ""),
];

fn tra<'a>(b: &'a [(&str, &str)], k: &str) -> Option<&'a str> {
    b.iter().find(|x| x.0 == k).map(|x| x.1)
}

fn re_am_tiet() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^(?:ngh|ng|gh|gi|kh|nh|ph|qu|th|tr|ch|b|c|d|g|h|k|l|m|n|p|r|s|t|v|x)?(?:uye|uya|uyu|uy|ieu|yeu|uoi|uou|oai|oay|oeo|uay|uao|ai|ao|au|ay|eo|eu|ia|ie|iu|oa|oe|oi|oo|ua|ue|ui|uo|uu|ye|a|e|i|o|u|y)(?:ch|ng|nh|c|m|n|p|t|i|o|u|y)?$",
        )
        .unwrap()
    })
}

fn re_tu() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[A-Za-zÀ-ỹĐđ][A-Za-zÀ-ỹĐđ'’]*").unwrap())
}

/// Từ thuần chữ Latin không dấu và không phải âm tiết tiếng Việt → từ nước ngoài.
pub fn la_ngoai(w: &str) -> bool {
    let w = w.replace('’', "'");
    w.is_ascii() && !re_am_tiet().is_match(&w.replace('\'', "").to_lowercase())
}

fn phu_am(s: &str) -> Vec<&'static str> {
    let (mut ra, mut i) = (Vec::new(), 0);
    while i < s.len() {
        match DAU.iter().find(|(a, _)| s[i..].starts_with(a)) {
            Some((a, b)) => {
                ra.push(*b);
                i += a.len();
            }
            None => i += 1,
        }
    }
    ra
}

fn van(s: &str) -> &'static str {
    VAN.iter()
        .find(|(a, _)| s.starts_with(a))
        .map(|x| x.1)
        .unwrap_or("")
}

fn gan_cuoi(mut am: String, cuoi: &str) -> String {
    let c: Vec<char> = am.chars().collect();
    if c.len() >= 2 && "yiuo".contains(c[c.len() - 1]) && "âaôoeu".contains(c[c.len() - 2]) {
        am.pop();
    }
    am + cuoi
}

fn la_nguyen_am(c: char) -> bool {
    "aeiouy".contains(c)
}

/// Cắt thành [phụ âm đầu][nguyên âm] / phụ âm cuối: tương đương regex `[^aeiouy]*[aeiouy]+|[^aeiouy]+$`.
fn cat_khuc(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let (mut ra, mut i) = (Vec::new(), 0);
    while i < c.len() {
        let mut j = i;
        while j < c.len() && !la_nguyen_am(c[j]) {
            j += 1;
        }
        let k = j;
        while j < c.len() && la_nguyen_am(c[j]) {
            j += 1;
        }
        ra.push(c[i..j].iter().collect());
        if j == k && j >= c.len() {
            break;
        }
        i = j;
    }
    ra
}

/// Phiên âm thô bằng quy tắc: "Hernand" → "Hơ-nan".
pub fn quy_tac(tu: &str) -> String {
    let mut s: String = tu
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase())
        .collect();
    if s.is_empty() {
        return tu.to_string();
    }
    // phụ âm đôi → 1
    let mut gon = String::new();
    for c in s.chars() {
        if gon.chars().last() == Some(c) && !la_nguyen_am(c) {
            continue;
        }
        gon.push(c);
    }
    s = gon;
    if s.len() > 3
        && s.ends_with('e')
        && !la_nguyen_am(s.chars().rev().nth(1).unwrap_or('a'))
        && !s.ends_with("ee")
        && !s.ends_with("le")
    {
        s.pop();
    }
    let mut khuc = cat_khuc(&s);
    let mut am: Vec<String> = Vec::new();
    let mut k = 0;
    while k < khuc.len() {
        let cum = khuc[k].clone();
        let pos = cum.find(la_nguyen_am).unwrap_or(cum.len());
        let (dau, nguyen) = (&cum[..pos], &cum[pos..]);
        if nguyen.is_empty() {
            let mut cuoi = tra(CUOI, dau).map(String::from);
            if cuoi.is_none() && !dau.is_empty() {
                let (a, b) = (&dau[..1], &dau[dau.len() - 1..]);
                cuoi = Some(
                    tra(CUOI, a)
                        .filter(|x| !x.is_empty())
                        .or_else(|| tra(CUOI, b))
                        .unwrap_or("")
                        .to_string(),
                );
            }
            if let Some(c) = cuoi.filter(|c| !c.is_empty()) {
                if let Some(l) = am.pop() {
                    am.push(gan_cuoi(l, &c));
                }
            }
            k += 1;
            continue;
        }
        let pa = phu_am(dau);
        for p in pa.iter().take(pa.len().saturating_sub(1)) {
            am.push(format!("{p}ơ"));
        }
        am.push(format!(
            "{}{}",
            pa.last().copied().unwrap_or(""),
            van(nguyen)
        ));
        if k + 1 < khuc.len() {
            let sau_pos = khuc[k + 1].find(la_nguyen_am).unwrap_or(khuc[k + 1].len());
            let sau = khuc[k + 1][..sau_pos].to_string();
            if sau.len() >= 2 && "mnlr".contains(&sau[..1]) && !khuc[k + 1][sau.len()..].is_empty()
            {
                let cuoi = tra(CUOI, &sau[..1]).unwrap_or("");
                if &sau[..1] == "r" && am.last().is_some_and(|a| a.ends_with('e')) {
                    let l = am.pop().unwrap();
                    am.push(format!("{}ơ", &l[..l.len() - 1]));
                }
                if !cuoi.is_empty() {
                    let l = am.pop().unwrap();
                    am.push(gan_cuoi(l, cuoi));
                }
                khuc[k + 1] = khuc[k + 1][1..].to_string();
            }
        }
        k += 1;
    }
    let am: Vec<String> = am
        .into_iter()
        .filter(|a| !a.is_empty())
        .map(|a| {
            a.replace("cy", "ki")
                .replace("ci", "ki")
                .replace("ce", "ke")
                .replace("ge", "ghe")
                .replace("gi", "ghi")
        })
        .collect();
    if am.is_empty() {
        return tu.to_string();
    }
    let kq = am.join("-");
    let mut c = kq.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or(kq)
}

fn tu_dien_hoc() -> &'static HashMap<String, String> {
    static T: OnceLock<HashMap<String, String>> = OnceLock::new();
    T.get_or_init(|| {
        std::fs::read(crate::duong_dan::du_lieu().join("phien_am.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    })
}

pub fn cach_doc(tu: &str) -> String {
    let k = tu.to_lowercase().replace("'s", "").replace('\'', "");
    if let Some((_, v)) = CO_DINH.iter().find(|(a, _)| *a == k) {
        return v.to_string();
    }
    tu_dien_hoc()
        .get(&k)
        .cloned()
        .unwrap_or_else(|| quy_tac(&k))
}

/// Thay tên nước ngoài bằng cách đọc tiếng Việt (chỉ cho máy đọc; âm tiết cách nhau bằng khoảng trắng).
pub fn doc_duoc(van_ban: &str) -> String {
    re_tu()
        .replace_all(van_ban, |c: &regex::Captures| {
            let w = &c[0];
            if la_ngoai(w) {
                cach_doc(w).replace('-', " ")
            } else {
                w.to_string()
            }
        })
        .into_owned()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn co_dinh_va_quy_tac() {
        assert_eq!(doc_duoc("Kliff nói chuyện"), "Cờ líp nói chuyện");
        assert_eq!(doc_duoc("Tôi đi ngủ"), "Tôi đi ngủ");
        assert!(la_ngoai("Brollam") && !la_ngoai("ban"));
        let r = quy_tac("brollam");
        assert!(r.starts_with('B') && !r.is_empty());
    }
}
