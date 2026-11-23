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
        assert!(la_viet_mat_dau("Xéa"));
        assert!(!la_viet_mat_dau("Please note that the Square Enix Membership"));
        assert!(!la_viet_mat_dau("NEW GAME"));
        assert_eq!(lay_the(r"a \c[1]b\n"), vec![r"\c[1]".to_string(), r"\n".to_string()]);
    }
}
