//! File .mul của Foundation (luồng đa kênh: âm thanh + phụ đề + chuyển động phim) – chuyển từ rottr_mul.py.
//! Định dạng theo TR Reboot Mod Tools (MultiplexStream):
//!   header cố định 7388 byte (đệm tới 0x2000) → chuỗi gói [kiểu i32][cỡ i32][8 byte] (+nội dung, căn 0x10):
//!     0 = âm thanh: mỗi kênh [cỡ i32][kênh i32][8 byte][dữ liệu] – nối lại = 1 file FSB4 (MP3) / kênh
//!     1 = phim (cine): gói đầu có header "CINE", rồi [cỡ khung][số khung][giá trị kênh][phụ đề]
//!   phụ đề: [cỡ i32] rồi lặp "ngôn_ngữ\r" "chữ\r" (UTF-8).

use anyhow::{bail, Result};

pub const CO_HDR: usize = 7388;
const CINE: i32 = 0x4349_4E45;

fn can(n: usize, a: usize) -> usize {
    n.div_ceil(a) * a
}

fn i32_at(b: &[u8], o: usize) -> Result<i32> {
    match b.get(o..o + 4) {
        Some(x) => Ok(i32::from_le_bytes(x.try_into().unwrap())),
        None => bail!("file .mul hỏng"),
    }
}

#[derive(Clone, Default)]
pub struct GoiPhim {
    pub header: Option<Vec<u8>>,
    pub khung: i32,
    pub kenh: Vec<u8>,
    /// ngôn ngữ → chữ (giữ thứ tự)
    pub phu_de: Option<Vec<(String, String)>>,
    pub duoi: Vec<u8>,
}

#[derive(Clone)]
pub struct GoiAm {
    pub du_lieu: Vec<Vec<u8>>,
    pub dem: Vec<Vec<u8>>,
}

#[derive(Clone)]
pub enum Goi {
    Am(GoiAm),
    Phim(GoiPhim),
    Khac(i32, Vec<u8>),
}

pub struct Mul {
    dau: Vec<u8>,
    pub goi: Vec<Goi>,
    da_thay: bool,
}

impl Mul {
    pub fn so_kenh(&self) -> usize {
        i32_at(&self.dau, 12).unwrap_or(0).max(0) as usize
    }
    pub fn co_phim(&self) -> bool {
        i32_at(&self.dau, 32).unwrap_or(0) != 0
    }
    /// Loại từng kênh âm thanh (mảng 12 byte cuối header).
    pub fn loai_kenh(&self) -> Vec<u8> {
        self.dau[7376..7376 + self.so_kenh().min(12)].to_vec()
    }

    /// Đầu file có phải .mul có phụ đề không (đọc nhanh 7388 byte đầu).
    pub fn la_mul_phu_de(dau: &[u8]) -> bool {
        dau.len() >= CO_HDR
            && i32_at(dau, 0).ok() == Some(44100)
            && i32_at(dau, 36).unwrap_or(0) != 0
    }

    pub fn doc(b: &[u8]) -> Result<Self> {
        if b.len() < CO_HDR {
            bail!("file .mul quá ngắn");
        }
        let so_kenh = i32_at(b, 12)?;
        if !(0..=12).contains(&so_kenh) {
            bail!("số kênh âm thanh sai");
        }
        let so_kenh = so_kenh as usize;
        let mut dau = b[..0x2000.min(b.len())].to_vec();
        dau.resize(0x2000, 0);
        let mut x = Mul {
            dau,
            goi: Vec::new(),
            da_thay: false,
        };
        let mut vt = 0x2000;
        let mut kenh: Option<Vec<(i32, bool)>> = None;
        while vt + 16 <= b.len() {
            let kieu = i32_at(b, vt)?;
            let co = i32_at(b, vt + 4)?.max(0) as usize;
            let nd = vt + 16;
            match kieu {
                0 => {
                    let mut ds = Vec::new();
                    let mut dem = Vec::new();
                    let mut p = nd;
                    for i in 0..so_kenh {
                        let n = i32_at(b, p)?.max(0) as usize;
                        if i32_at(b, p + 4)? != i as i32 {
                            bail!("kênh âm thanh sai thứ tự");
                        }
                        let Some(d) = b.get(p + 16..p + 16 + n) else {
                            bail!("file .mul hỏng")
                        };
                        ds.push(d.to_vec());
                        dem.push(b[p + 8..p + 16].to_vec());
                        p += 16 + n;
                    }
                    x.goi.push(Goi::Am(GoiAm { du_lieu: ds, dem }));
                }
                1 => x.goi.push(Goi::Phim(doc_phim(b, nd, &mut kenh)?)),
                _ => x
                    .goi
                    .push(Goi::Khac(kieu, b.get(nd..nd + co).unwrap_or(&[]).to_vec())),
            }
            vt = can(nd + co, 0x10);
        }
        Ok(x)
    }

    pub fn ghi(&self) -> Vec<u8> {
        let mut ra = self.dau_moi();
        let pack =
            |ra: &mut Vec<u8>, o: usize, v: i32| ra[o..o + 4].copy_from_slice(&v.to_le_bytes());
        for g in &self.goi {
            let kieu = match g {
                Goi::Am(_) => 0,
                Goi::Phim(_) => 1,
                Goi::Khac(k, _) => *k,
            };
            ra.extend(kieu.to_le_bytes());
            ra.extend([0u8; 12]);
            let nd = ra.len();
            match g {
                Goi::Am(a) => {
                    for (i, d) in a.du_lieu.iter().enumerate() {
                        ra.extend((d.len() as i32).to_le_bytes());
                        ra.extend((i as i32).to_le_bytes());
                        match a.dem.get(i) {
                            Some(x) => ra.extend(x),
                            None => ra.extend([0u8; 8]),
                        }
                        ra.extend(d);
                    }
                }
                Goi::Phim(p) => {
                    if let Some(h) = &p.header {
                        ra.extend(h);
                    }
                    let pk = ra.len();
                    ra.extend(0i32.to_le_bytes());
                    ra.extend(p.khung.to_le_bytes());
                    if p.khung >= 0 {
                        ra.extend(&p.kenh);
                        let pd = ra.len();
                        ra.extend([0u8; 4]);
                        for (k, v) in p.phu_de.iter().flatten() {
                            ra.extend(k.as_bytes());
                            ra.push(b'\r');
                            ra.extend(v.as_bytes());
                            ra.push(b'\r');
                        }
                        let n = (ra.len() - pd - 4) as i32;
                        pack(&mut ra, pd, n);
                        ra.extend(&p.duoi);
                    }
                    let n = (ra.len() - pk - 4) as i32;
                    pack(&mut ra, pk, n);
                    ra.resize(can(ra.len(), 0x10), 0);
                }
                Goi::Khac(_, d) => ra.extend(d),
            }
            let n = (ra.len() - nd) as i32;
            pack(&mut ra, nd - 12, n);
            ra.resize(can(ra.len(), 0x10), 0);
        }
        ra
    }

    /// Header: giữ nguyên; nếu đã thay âm thanh thì cập nhật số mẫu và cỡ dữ liệu từng kênh.
    fn dau_moi(&self) -> Vec<u8> {
        let mut h = self.dau.clone();
        if !self.da_thay {
            return h;
        }
        let fsb = self.fsb_kenh();
        let h2 = i32_at(&h, 8).unwrap_or(0);
        let so_mau = fsb
            .iter()
            .filter(|f| f.len() >= 0x54)
            .map(|f| i32_at(f, 0x50).unwrap_or(h2))
            .min()
            .unwrap_or(h2);
        if 0 < h2 && h2 < 0x7FFF_FFFF {
            h[8..12].copy_from_slice(&h2.min(so_mau).to_le_bytes());
        }
        for (i, f) in fsb.iter().enumerate() {
            let o = 208 + 4 * (1023 - i);
            h[o..o + 4].copy_from_slice(&(f.len() as i32).to_le_bytes());
        }
        h
    }

    /// Nối các gói âm thanh → mỗi kênh 1 file FSB.
    pub fn fsb_kenh(&self) -> Vec<Vec<u8>> {
        let mut ra = vec![Vec::new(); self.so_kenh()];
        for g in &self.goi {
            if let Goi::Am(a) = g {
                for (i, d) in a.du_lieu.iter().enumerate() {
                    if d.as_slice() != b"\xff\xfb" {
                        if let Some(r) = ra.get_mut(i) {
                            r.extend(d);
                        }
                    }
                }
            }
        }
        ra
    }

    /// Thay âm thanh, GIỮ NGUYÊN bố cục gói của game (ranh giới gói mới = ranh giới gốc nhân tỉ lệ).
    pub fn thay_am_thanh(&mut self, fsb_moi: &[Vec<u8>]) -> Result<()> {
        if fsb_moi.len() != self.so_kenh() {
            bail!("số kênh không khớp");
        }
        let vi_tri: Vec<usize> = self
            .goi
            .iter()
            .enumerate()
            .filter(|(_, g)| matches!(g, Goi::Am(_)))
            .map(|(i, _)| i)
            .collect();
        if vi_tri.is_empty() {
            bail!("file .mul không có âm thanh");
        }
        for (kenh, moi) in fsb_moi.iter().enumerate() {
            let goc: Vec<usize> = vi_tri
                .iter()
                .map(|&i| match &self.goi[i] {
                    Goi::Am(a) => {
                        let d = &a.du_lieu[kenh];
                        if d.as_slice() == b"\xff\xfb" {
                            0
                        } else {
                            d.len()
                        }
                    }
                    _ => 0,
                })
                .collect();
            let tong_goc = goc.iter().sum::<usize>().max(1);
            let ti_le = moi.len() as f64 / tong_goc as f64;
            let mut cat = Vec::new();
            let (mut truoc, mut cong) = (0usize, 0usize);
            for d in &goc[..goc.len() - 1] {
                cong += d;
                let vt = ((cong as f64 * ti_le).round_ties_even() as usize)
                    .max(truoc)
                    .min(moi.len());
                cat.push((truoc, vt));
                truoc = vt;
            }
            cat.push((truoc, moi.len()));
            for (&i, (a, b)) in vi_tri.iter().zip(cat) {
                if let Goi::Am(g) = &mut self.goi[i] {
                    g.du_lieu[kenh] = if b > a {
                        moi[a..b].to_vec()
                    } else {
                        b"\xff\xfb".to_vec()
                    };
                }
            }
        }
        self.da_thay = true;
        Ok(())
    }

    /// Các gói phim có phụ đề: (chỉ số gói, số khung).
    pub fn cac_phu_de(&self) -> Vec<(usize, i32)> {
        self.goi
            .iter()
            .enumerate()
            .filter_map(|(i, g)| match g {
                Goi::Phim(p) if p.phu_de.as_ref().is_some_and(|x| !x.is_empty()) => {
                    Some((i, p.khung))
                }
                _ => None,
            })
            .collect()
    }

    pub fn phu_de_mut(&mut self, i: usize) -> Option<&mut Vec<(String, String)>> {
        match &mut self.goi[i] {
            Goi::Phim(p) => p.phu_de.as_mut(),
            _ => None,
        }
    }

    pub fn phu_de(&self, i: usize) -> Option<&Vec<(String, String)>> {
        match &self.goi[i] {
            Goi::Phim(p) => p.phu_de.as_ref(),
            _ => None,
        }
    }
}

/// Lấy giá trị phụ đề theo khóa ngôn ngữ ("0" = tiếng Anh).
pub fn lay<'a>(pd: &'a [(String, String)], k: &str) -> Option<&'a str> {
    pd.iter().find(|x| x.0 == k).map(|x| x.1.as_str())
}

fn doc_phim(b: &[u8], mut p: usize, kenh: &mut Option<Vec<(i32, bool)>>) -> Result<GoiPhim> {
    let mut g = GoiPhim::default();
    if kenh.is_none() {
        let p0 = p;
        if i32_at(b, p)? != CINE {
            bail!("gói phim sai");
        }
        let ver = i32_at(b, p + 4)?;
        p += 16 + 0x40 + 4;
        let mut n: i64 = 0;
        let rd = |p: usize| -> Result<i64> { Ok(i32_at(b, p)? as i64) };
        let na = rd(p)?;
        p = (p as i64 + 4 + 0xC * na) as usize;
        n += 9 * na;
        let ns = rd(p)?;
        p += 4;
        for _ in 0..ns.max(0) {
            p += 4;
            if ver == 13 {
                p += 4;
            }
            let nb = rd(p)?;
            p = (p as i64 + 8 + 0x40 * nb) as usize;
            n += 3 + 14 * nb;
            let nl = rd(p)?;
            p = (p as i64 + 8 + 0x18 * nl) as usize;
            n += 10 * nl;
            let nc = rd(p)?;
            p += 8;
            n += nc;
            if ver == 14 {
                let nbs = rd(p)?;
                p += 8;
                n += nbs;
            }
        }
        let ncam = rd(p)?;
        p = (p as i64 + 4 + 4 * ncam) as usize;
        n += 16 * ncam;
        p += 4;
        let nt = rd(p)?;
        p = (p as i64 + 4 + 8 * nt) as usize;
        n += nt;
        p += 4;
        p = (p as i64 + 0x1C * ns) as usize;
        if ver == 14 {
            p = (p as i64 + 0x1C * ncam) as usize;
            if ncam > 0 {
                let ncc = rd(p)?;
                p = (p as i64 + 4 + 4 * ncc) as usize;
            }
        }
        if !(0..10_000_000).contains(&n) || p > b.len() {
            bail!("gói phim hỏng");
        }
        g.header = Some(b[p0..p].to_vec());
        *kenh = Some(vec![(0, false); n as usize]);
    }
    let co_khung = i32_at(b, p)?;
    g.khung = i32_at(b, p + 4)?;
    let het_khung = (p as i64 + 4 + co_khung as i64) as usize;
    p += 8;
    if g.khung < 0 {
        return Ok(g);
    }
    let p0 = p;
    for c in kenh.as_mut().unwrap().iter_mut() {
        if c.0 == 0 {
            let run = i32_at(b, p)?;
            p += 4;
            c.0 = run & 0xFF_FFFF;
            c.1 = ((run >> 24) & 0xFF) != 0;
            p += 4;
        } else if c.1 {
            p += 4;
        }
        c.0 -= 1;
    }
    g.kenh = b.get(p0..p).unwrap_or(&[]).to_vec();
    let n = i32_at(b, p)?.max(0) as usize;
    p += 4;
    g.duoi = b
        .get((p + n).min(b.len())..het_khung.min(b.len()).max((p + n).min(b.len())))
        .unwrap_or(&[])
        .to_vec();
    if n > 0 {
        let Some(d) = b.get(p..p + n) else {
            bail!("phụ đề hỏng")
        };
        let ds: Vec<&[u8]> = d.split(|&c| c == b'\r').collect();
        let mut pd: Vec<(String, String)> = Vec::new();
        let mut i = 0;
        while i + 1 < ds.len() {
            let k = String::from_utf8_lossy(ds[i]).into_owned();
            let v = String::from_utf8_lossy(ds[i + 1]).into_owned();
            match pd.iter_mut().find(|x| x.0 == k) {
                Some(x) => x.1 = v,
                None => pd.push((k, v)),
            }
            i += 2;
        }
        g.phu_de = Some(pd);
    }
    Ok(g)
}

// ---------------- FSB4 (MP3) ----------------

/// Tách khung MP3 (bỏ ID3, khung Xing/Info). Chỉ nhận MPEG-1 Layer III 44.1 kHz.
pub fn mp3_khung(mp3: &[u8]) -> Result<Vec<&[u8]>> {
    let mut d = mp3;
    if d.starts_with(b"ID3") && d.len() >= 10 {
        let n = ((d[6] as usize) << 21)
            | ((d[7] as usize) << 14)
            | ((d[8] as usize) << 7)
            | d[9] as usize;
        d = &d[(10 + n).min(d.len())..];
    }
    if d.len() >= 128 && &d[d.len() - 128..d.len() - 125] == b"TAG" {
        d = &d[..d.len() - 128];
    }
    const BR: [usize; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    let mut ra = Vec::new();
    let (mut vt, mut dau) = (0, true);
    while vt + 4 <= d.len() {
        let h = &d[vt..vt + 4];
        let br = if h[0] == 0xFF && (h[1] & 0xFE) == 0xFA {
            BR[(h[2] >> 4) as usize]
        } else {
            0
        };
        if br == 0 || (h[2] >> 2) & 3 != 0 {
            match d[vt + 1..].windows(2).position(|w| w == b"\xff\xfb") {
                Some(j) => {
                    vt = vt + 1 + j;
                    continue;
                }
                None => break,
            }
        }
        let dai = 144000 * br / 44100 + ((h[2] >> 1) & 1) as usize;
        if vt + dai > d.len() {
            break;
        }
        let khung = &d[vt..vt + dai];
        vt += dai;
        if dau {
            dau = false;
            let k = &khung[..64.min(khung.len())];
            if k.windows(4).any(|w| w == b"Xing" || w == b"Info") {
                continue;
            }
        }
        ra.push(khung);
    }
    if ra.is_empty() {
        bail!("file MP3 không đúng định dạng MPEG-1 Layer III 44.1 kHz");
    }
    Ok(ra)
}

/// Bọc MP3 thành FSB4 giống file game (1 mẫu). mau_goc: FSB gốc để chép header.
pub fn tao_fsb4(mp3: &[u8], ten: &str, mau_goc: Option<&[u8]>) -> Result<Vec<u8>> {
    let ds = mp3_khung(mp3)?;
    let so_mau = (ds.len() * 1152) as u32;
    let mut khung = Vec::new();
    for k in &ds {
        khung.extend(*k);
        khung.resize(khung.len() + (4 - k.len() % 4) % 4, 0);
    }
    let mut du_lieu = khung.clone();
    du_lieu.resize(can(du_lieu.len(), 32), 0);
    let mut dau = match mau_goc {
        Some(m) if m.len() >= 0x80 => m[..0x80].to_vec(),
        _ => {
            let mut d = vec![0u8; 0x80];
            d[0..4].copy_from_slice(b"FSB4");
            for (i, v) in [1u32, 0x50, 0, 0x40000, 0x40].iter().enumerate() {
                d[4 + 4 * i..8 + 4 * i].copy_from_slice(&v.to_le_bytes());
            }
            d[0x30..0x32].copy_from_slice(&0x50u16.to_le_bytes());
            let mut o = 0x50;
            for v in [0u32, 0, 0, 0, 0x1000_0200] {
                d[o..o + 4].copy_from_slice(&v.to_le_bytes());
                o += 4;
            }
            d[o..o + 4].copy_from_slice(&44100i32.to_le_bytes());
            o += 4;
            for v in [255u16, 128, 128, 1] {
                d[o..o + 2].copy_from_slice(&v.to_le_bytes());
                o += 2;
            }
            d[0x70..0x74].copy_from_slice(&1.0f32.to_le_bytes());
            d[0x74..0x78].copy_from_slice(&10000.0f32.to_le_bytes());
            d
        }
    };
    dau[0x0C..0x10].copy_from_slice(&(du_lieu.len() as u32).to_le_bytes());
    let ten_b: Vec<u8> = ten
        .chars()
        .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
        .take(29)
        .collect();
    let mut t = ten_b.clone();
    t.resize(30, 0);
    dau[0x32..0x50].copy_from_slice(&t);
    for (i, v) in [so_mau, khung.len() as u32, 0, so_mau.wrapping_sub(1)]
        .iter()
        .enumerate()
    {
        dau[0x50 + 4 * i..0x54 + 4 * i].copy_from_slice(&v.to_le_bytes());
    }
    dau.extend(du_lieu);
    Ok(dau)
}

/// FSB4 → (tên mẫu, số mẫu).
pub fn fsb_thong_tin(fsb: &[u8]) -> Result<(String, u32)> {
    if !fsb.starts_with(b"FSB4") || fsb.len() < 0x54 {
        bail!("không phải FSB4");
    }
    let ten = &fsb[0x32..0x50];
    let ten = &ten[..ten.iter().position(|&c| c == 0).unwrap_or(ten.len())];
    Ok((
        String::from_utf8_lossy(ten).into_owned(),
        u32::from_le_bytes(fsb[0x50..0x54].try_into().unwrap()),
    ))
}

/// Dữ liệu MP3 bên trong FSB4 (sau header + bảng mẫu).
pub fn fsb_mp3(fsb: &[u8]) -> Result<&[u8]> {
    if !fsb.starts_with(b"FSB4") || fsb.len() < 0x54 {
        bail!("không phải FSB4");
    }
    let sh = u32::from_le_bytes(fsb[8..12].try_into().unwrap()) as usize;
    fsb.get(0x30 + sh..)
        .ok_or_else(|| anyhow::anyhow!("FSB4 hỏng"))
}
