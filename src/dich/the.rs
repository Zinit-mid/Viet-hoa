//! Kiểm tra thẻ: %s {..} [..] <..> \n \c[1]. Bản dịch sai thẻ thì giữ tiếng Anh.

pub fn lay_the(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut ra = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let dong = match ch {
            '{' => Some('}'),
            '[' => Some(']'),
            '<' => Some('>'),
            _ => None,
        };
        if let Some(d) = dong {
            if let Some(j) = c[i + 1..].iter().position(|&x| x == d) {
                let t: String = c[i..=i + 1 + j].iter().collect();
                if t.len() <= 64 {
                    ra.push(t);
                    i += j + 2;
                    continue;
                }
            }
        } else if ch == '%' && i + 1 < c.len() && c[i + 1].is_ascii_alphabetic() {
            ra.push(c[i..i + 2].iter().collect());
            i += 2;
            continue;
        } else if ch == '\\' && i + 1 < c.len() {
            let mut j = i + 2;
            if c[i + 1].is_ascii_alphabetic() && j < c.len() && c[j] == '[' {
                while j < c.len() && c[j] != ']' {
                    j += 1;
                }
                j = (j + 1).min(c.len());
            }
            ra.push(c[i..j].iter().collect());
            i = j;
            continue;
        }
        i += 1;
    }
    ra.sort();
    ra
}

pub fn khop_the(goc: &str, dich: &str) -> bool {
    lay_the(goc) == lay_the(dich)
}

/// Còn chữ tiếng Anh rõ ràng (≥ 2 từ Latin không dấu dài ≥ 4) → nên dịch lại.
pub fn con_chu_anh(s: &str) -> bool {
    const PHO_BIEN: [&str; 12] = [
        "the", "and", "you", "with", "that", "this", "your", "have", "from", "will", "what", "there",
    ];
    s.split(|c: char| !c.is_alphabetic())
        .filter(|w| PHO_BIEN.contains(&w.to_lowercase().as_str()))
        .count()
        >= 2
}

/// Chữ tiếng Việt bị OCR tiếng Anh đọc mất dấu thường ra các ký tự lạ như ä ö ü å é
/// ("Dä chay 2 lénh") → không phải tiếng Anh, không dịch.
pub fn la_viet_mat_dau(s: &str) -> bool {
    const LA: &str = "äöüåéèêëïîìôòûùâàáãõçñðøæœ";
    let la = s.to_lowercase().chars().filter(|c| LA.contains(*c)).count();
    let chu = s.chars().filter(|c| c.is_alphabetic()).count().max(1);
    la >= 2 || (la >= 1 && chu <= 12)
}

/// Bản dịch bị lặp vô cớ ("OFF" → "OFF OFF OFF OFF") hoặc dài bất thường.
pub fn bi_lap(en: &str, vi: &str) -> bool {
    fn lap_dai_nhat(s: &str) -> usize {
        let tu: Vec<String> = s.split_whitespace().map(|w| w.to_lowercase()).collect();
        let (mut tot, mut dem) = (0, 0);
        for i in 0..tu.len() {
            dem = if i > 0 && tu[i] == tu[i - 1] { dem + 1 } else { 1 };
            tot = tot.max(dem);
        }
        tot
    }
    let (so_en, so_vi) = (en.split_whitespace().count(), vi.split_whitespace().count());
    (lap_dai_nhat(vi) >= 3 && lap_dai_nhat(en) < 3) || so_vi > so_en * 4 + 6
}

/// Bản dịch lặp đôi cả câu ("ADVANCED" → "NÂNG CAO NÂNG CAO") → giữ một nửa.
pub fn bo_lap_doi(en: &str, vi: &str) -> String {
    fn la_doi(tu: &[String]) -> bool {
        let n = tu.len();
        n >= 2 && n % 2 == 0 && tu[..n / 2] == tu[n / 2..]
    }
    let thap = |s: &str| s.split_whitespace().map(|w| w.to_lowercase()).collect::<Vec<_>>();
    let tu_vi: Vec<&str> = vi.split_whitespace().collect();
    if la_doi(&thap(vi)) && !la_doi(&thap(en)) {
        tu_vi[..tu_vi.len() / 2].join(" ")
    } else {
        vi.to_string()
    }
}

/// Từ menu game hay gặp: dịch thẳng, giữ kiểu chữ (IN HOA / Viết hoa đầu / thường).
pub fn tu_menu(en: &str) -> Option<String> {
    const BANG: &[(&str, &str)] = &[
        ("on", "bật"), ("off", "tắt"), ("yes", "có"), ("no", "không"), ("back", "quay lại"),
        ("apply", "áp dụng"), ("cancel", "hủy"), ("auto", "tự động"), ("low", "thấp"),
        ("medium", "trung bình"), ("high", "cao"), ("ultra", "rất cao"), ("very high", "rất cao"),
        ("enabled", "bật"), ("disabled", "tắt"), ("default", "mặc định"), ("close", "đóng"),
        ("continue", "tiếp tục"), ("resume", "tiếp tục"), ("quit", "thoát"), ("exit", "thoát"),
        ("options", "tùy chọn"), ("settings", "cài đặt"), ("normal", "bình thường"),
        ("none", "không"), ("custom", "tùy chỉnh"), ("accept", "chấp nhận"), ("confirm", "xác nhận"),
        ("save", "lưu"), ("load", "tải"), ("loading", "đang tải"), ("new game", "trò chơi mới"),
        ("load game", "tải game"), ("quit game", "thoát game"), ("graphics", "đồ họa"),
        ("audio", "âm thanh"), ("controls", "điều khiển"), ("advanced", "nâng cao"), ("basic", "cơ bản"),
        ("brightness", "độ sáng"), ("fullscreen", "toàn màn hình"), ("windowed", "cửa sổ"),
        ("borderless", "không viền"), ("subtitles", "phụ đề"), ("language", "ngôn ngữ"),
        ("difficulty", "độ khó"), ("easy", "dễ"), ("hard", "khó"),
    ];
    let khoa: String = en.trim().trim_end_matches(['.', ':', '•']).trim().to_lowercase();
    let vi = BANG.iter().find(|(e, _)| *e == khoa)?.1;
    let chu: Vec<char> = en.chars().filter(|c| c.is_alphabetic()).collect();
    Some(if chu.len() >= 2 && chu.iter().all(|c| c.is_uppercase()) {
        vi.to_uppercase()
    } else if chu.first().is_some_and(|c| c.is_uppercase()) {
        let mut it = vi.chars();
        it.next().map(|f| f.to_uppercase().collect::<String>() + it.as_str()).unwrap_or_default()
    } else {
        vi.to_string()
    })
}

pub fn la_tieng_viet(s: &str) -> bool {
    const DAU: &str = "ăâđêôơưàảãáạằẳẵắặầẩẫấậèẻẽéẹềểễếệìỉĩíịòỏõóọồổỗốộờởỡớợùủũúụừửữứựỳỷỹýỵ";
    s.to_lowercase().chars().filter(|c| DAU.contains(*c)).count() >= 2
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_co_ban() {
        assert!(khop_the("Hello {Param1} <br/> %s", "Xin chào %s {Param1} <br/>"));
        assert!(!khop_the("Hello {Param1}", "Xin chào"));
        assert!(la_viet_mat_dau("Dä chay 2 lénh, dä doc"));
        assert!(la_viet_mat_dau("Cåi dét"));
        assert!(bi_lap("OFF", "OFF OFF OFF OFF OFF"));
        assert_eq!(bo_lap_doi("ADVANCED", "NÂNG CAO NÂNG CAO"), "NÂNG CAO");
        assert_eq!(bo_lap_doi("bye bye", "tạm biệt tạm biệt"), "tạm biệt tạm biệt");
        assert_eq!(tu_menu("ON").as_deref(), Some("BẬT"));
        assert_eq!(tu_menu("Off").as_deref(), Some("Tắt"));
        assert_eq!(tu_menu("QUIT GAME").as_deref(), Some("THOÁT GAME"));
        assert_eq!(tu_menu("Press E"), None);
        assert!(!bi_lap("Press E to open the door.", "Nhấn E để mở cửa."));
        assert!(la_viet_mat_dau("Xéa"));
        assert!(!la_viet_mat_dau("Please note that the Square Enix Membership"));
        assert!(!la_viet_mat_dau("NEW GAME"));
        assert_eq!(lay_the(r"a \c[1]b\n"), vec![r"\c[1]".to_string(), r"\n".to_string()]);
    }
}
