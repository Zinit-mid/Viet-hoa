//! Thêm chữ tiếng Việt có dấu vào font Scaleform (DefineFont3) của game – chuyển từ font_viet.py.
//! Giữ kiểu chữ của game: chữ Việt GHÉP từ chữ cái gốc + dấu của chính font (sắc, huyền, ngã, mũ,
//! trăng, chấm); dấu hỏi và râu (ơ, ư) lấy từ font Be Vietnam Pro (OFL), co lại cho vừa.
//! Râu / gạch ngang chữ "đ" được HỢP (union) với nét chữ.
//! Các phép tính giữ đúng thứ tự như bản Python để kết quả giống từng byte.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read;

use anyhow::{anyhow, bail, Result};
use unicode_normalization::UnicodeNormalization;

// ---------------- đọc / ghi bit SWF ----------------

struct Br<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Br<'a> {
    fn moi(b: &'a [u8], byte: usize) -> Self {
        Br { b, p: byte * 8 }
    }
    fn ub(&mut self, n: u32) -> Result<i64> {
        let mut v = 0i64;
        for _ in 0..n {
            let byte = *self
                .b
                .get(self.p >> 3)
                .ok_or_else(|| anyhow!("hình hỏng"))?;
            v = (v << 1) | ((byte >> (7 - (self.p & 7))) & 1) as i64;
            self.p += 1;
        }
        Ok(v)
    }
    fn sb(&mut self, n: u32) -> Result<i64> {
        let mut v = self.ub(n)?;
        if n > 0 && v & (1 << (n - 1)) != 0 {
            v -= 1 << n;
        }
        Ok(v)
    }
    fn align(&mut self) {
        self.p = (self.p + 7) & !7;
    }
}

#[derive(Default)]
struct Bw {
    o: Vec<u8>,
    cur: u32,
    n: u32,
}

impl Bw {
    fn ub(&mut self, n: u32, v: i64) {
        for i in (0..n).rev() {
            self.cur = (self.cur << 1) | ((v >> i) & 1) as u32;
            self.n += 1;
            if self.n == 8 {
                self.o.push(self.cur as u8);
                self.cur = 0;
                self.n = 0;
            }
        }
    }
    fn sb(&mut self, n: u32, v: i64) {
        self.ub(n, v & ((1i64 << n) - 1));
    }
    fn bytes(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.o.push((self.cur << (8 - self.n)) as u8);
        }
        self.o
    }
}

fn bits(vals: &[i64]) -> u32 {
    let mut n = 1u32;
    for &v in vals {
        while !(-(1i64 << (n - 1)) <= v && v < (1i64 << (n - 1))) {
            n += 1;
        }
    }
    n
}

/// Lệnh đường viền
#[derive(Clone, Copy, Debug)]
pub enum Op {
    M(f64, f64),
    L(f64, f64),
    Q(f64, f64, f64, f64),
}

impl Op {
    fn xy(&self) -> (f64, f64) {
        match *self {
            Op::M(x, y) | Op::L(x, y) | Op::Q(x, y, _, _) => (x, y),
        }
    }
}

type Vien = Vec<Op>;
type Diem = (f64, f64);

/// Python int(round(x)): làm tròn nửa về số chẵn.
fn tron(x: f64) -> i64 {
    x.round_ties_even() as i64
}

pub fn doc_hinh(b: &[u8]) -> Result<Vec<Vien>> {
    let mut r = Br::moi(b, 0);
    let fb = r.ub(4)? as u32;
    let lb = r.ub(4)? as u32;
    let (mut x, mut y) = (0i64, 0i64);
    let mut vien: Vec<Vien> = Vec::new();
    loop {
        if r.ub(1)? == 0 {
            let fl = r.ub(5)?;
            if fl == 0 {
                break;
            }
            if fl & 1 != 0 {
                let n = r.ub(5)? as u32;
                x = r.sb(n)?;
                y = r.sb(n)?;
                vien.push(vec![Op::M(x as f64, y as f64)]);
            }
            if fl & 2 != 0 {
                r.ub(fb)?;
            }
            if fl & 4 != 0 {
                r.ub(fb)?;
            }
            if fl & 8 != 0 {
                r.ub(lb)?;
            }
            if fl & 16 != 0 {
                bail!("glyph có kiểu tô mới – không hỗ trợ");
            }
        } else {
            let thang = r.ub(1)?;
            let n = r.ub(4)? as u32 + 2;
            let cur = vien.last_mut().ok_or_else(|| anyhow!("hình hỏng"))?;
            if thang == 1 {
                let (dx, dy) = if r.ub(1)? == 1 {
                    (r.sb(n)?, r.sb(n)?)
                } else if r.ub(1)? == 1 {
                    (0, r.sb(n)?)
                } else {
                    (r.sb(n)?, 0)
                };
                x += dx;
                y += dy;
                cur.push(Op::L(x as f64, y as f64));
            } else {
                let cx = x + r.sb(n)?;
                let cy = y + r.sb(n)?;
                x = cx + r.sb(n)?;
                y = cy + r.sb(n)?;
                cur.push(Op::Q(cx as f64, cy as f64, x as f64, y as f64));
            }
        }
    }
    Ok(vien)
}

pub fn ghi_hinh(vien: &[Vien]) -> Vec<u8> {
    let mut w = Bw::default();
    w.ub(4, 1);
    w.ub(4, 0);
    let (mut x, mut y) = (0i64, 0i64);
    let mut dau = true;
    for c in vien {
        for op in c {
            match *op {
                Op::M(px, py) => {
                    let (nx, ny) = (tron(px), tron(py));
                    w.ub(1, 0);
                    w.ub(5, 1 | if dau { 2 } else { 0 });
                    let n = bits(&[nx, ny]);
                    w.ub(5, n as i64);
                    w.sb(n, nx);
                    w.sb(n, ny);
                    if dau {
                        w.ub(1, 1);
                    }
                    dau = false;
                    x = nx;
                    y = ny;
                }
                Op::L(px, py) => {
                    let (nx, ny) = (tron(px), tron(py));
                    let (dx, dy) = (nx - x, ny - y);
                    x = nx;
                    y = ny;
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    w.ub(1, 1);
                    w.ub(1, 1);
                    if dx != 0 && dy != 0 {
                        let n = bits(&[dx, dy]).max(2);
                        w.ub(4, n as i64 - 2);
                        w.ub(1, 1);
                        w.sb(n, dx);
                        w.sb(n, dy);
                    } else if dx == 0 {
                        let n = bits(&[dy]).max(2);
                        w.ub(4, n as i64 - 2);
                        w.ub(1, 0);
                        w.ub(1, 1);
                        w.sb(n, dy);
                    } else {
                        let n = bits(&[dx]).max(2);
                        w.ub(4, n as i64 - 2);
                        w.ub(1, 0);
                        w.ub(1, 0);
                        w.sb(n, dx);
                    }
                }
                Op::Q(qx, qy, px, py) => {
                    let (cx, cy) = (tron(qx), tron(qy));
                    let (ax, ay) = (tron(px), tron(py));
                    let v = [cx - x, cy - y, ax - cx, ay - cy];
                    x = ax;
                    y = ay;
                    let n = bits(&v).max(2);
                    w.ub(1, 1);
                    w.ub(1, 0);
                    w.ub(4, n as i64 - 2);
                    for a in v {
                        w.sb(n, a);
                    }
                }
            }
        }
    }
    w.ub(1, 0);
    w.ub(5, 0);
    w.bytes()
}

fn rect0() -> Vec<u8> {
    let mut w = Bw::default();
    let n = bits(&[0, 0, 0, 0]);
    w.ub(5, n as i64);
    for _ in 0..4 {
        w.sb(n, 0);
    }
    w.bytes()
}

// ---------------- DefineFont3 ----------------

struct BoCuc {
    asc: u16,
    desc: u16,
    lead: i16,
    adv: Vec<i16>,
    ker: Vec<(u16, u16, i16)>,
}

struct Font3 {
    fid: u16,
    co: u8,
    lang: u8,
    ten: Vec<u8>,
    hinh: Vec<Vec<u8>>,
    ma: Vec<u32>,
    bo_cuc: Option<BoCuc>,
    con_lai: Vec<u8>,
}

fn u16_at(d: &[u8], o: usize) -> Result<u16> {
    d.get(o..o + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| anyhow!("font hỏng"))
}
fn u32_at(d: &[u8], o: usize) -> Result<u32> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| anyhow!("font hỏng"))
}

impl Font3 {
    fn doc(d: &[u8]) -> Result<Self> {
        let fid = u16_at(d, 0)?;
        let (co, lang, nl) = (d[2], d[3], d[4] as usize);
        let ten = d
            .get(5..5 + nl)
            .ok_or_else(|| anyhow!("font hỏng"))?
            .to_vec();
        let mut p = 5 + nl;
        let n = u16_at(d, p)? as usize;
        p += 2;
        let rong = co & 8 != 0;
        let mut offs = Vec::with_capacity(n + 1);
        for i in 0..=n {
            offs.push(if rong {
                u32_at(d, p + 4 * i)? as usize
            } else {
                u16_at(d, p + 2 * i)? as usize
            });
        }
        let mut hinh = Vec::with_capacity(n);
        for i in 0..n {
            hinh.push(
                d.get(p + offs[i]..p + offs[i + 1])
                    .ok_or_else(|| anyhow!("font hỏng"))?
                    .to_vec(),
            );
        }
        p += offs[n];
        let mut ma = Vec::with_capacity(n);
        if co & 4 != 0 {
            for i in 0..n {
                ma.push(u16_at(d, p + 2 * i)? as u32);
            }
            p += 2 * n;
        } else {
            for i in 0..n {
                ma.push(*d.get(p + i).ok_or_else(|| anyhow!("font hỏng"))? as u32);
            }
            p += n;
        }
        let mut bo_cuc = None;
        if co & 0x80 != 0 {
            let asc = u16_at(d, p)?;
            let desc = u16_at(d, p + 2)?;
            let lead = u16_at(d, p + 4)? as i16;
            p += 6;
            let mut adv = Vec::with_capacity(n);
            for i in 0..n {
                adv.push(u16_at(d, p + 2 * i)? as i16);
            }
            p += 2 * n;
            let mut r = Br::moi(d, p);
            for _ in 0..n {
                let k = r.ub(5)? as u32;
                for _ in 0..4 {
                    r.sb(k)?;
                }
                r.align();
            }
            p = r.p >> 3;
            let kc = u16_at(d, p)? as usize;
            p += 2;
            let mut ker = Vec::with_capacity(kc);
            for _ in 0..kc {
                if co & 4 != 0 {
                    ker.push((u16_at(d, p)?, u16_at(d, p + 2)?, u16_at(d, p + 4)? as i16));
                    p += 6;
                } else {
                    ker.push((d[p] as u16, d[p + 1] as u16, u16_at(d, p + 2)? as i16));
                    p += 4;
                }
            }
            bo_cuc = Some(BoCuc {
                asc,
                desc,
                lead,
                adv,
                ker,
            });
        }
        Ok(Font3 {
            fid,
            co,
            lang,
            ten,
            hinh,
            ma,
            bo_cuc,
            con_lai: d.get(p..).unwrap_or(&[]).to_vec(),
        })
    }

    fn ghi(&self) -> Vec<u8> {
        let n = self.hinh.len();
        let mut thu_tu: Vec<usize> = (0..n).collect();
        thu_tu.sort_by_key(|&i| self.ma[i]);
        let tong: usize = self.hinh.iter().map(Vec::len).sum();
        let rong = self.co & 8 != 0 || (2 * (n + 1) + tong) > 0xFFFF;
        let co = (self.co & !0x08) | if rong { 0x08 } else { 0 } | 0x04;
        let mut ra = Vec::new();
        ra.extend(self.fid.to_le_bytes());
        ra.extend([co, self.lang, self.ten.len() as u8]);
        ra.extend(&self.ten);
        ra.extend((n as u16).to_le_bytes());
        let k = if rong { 4 } else { 2 };
        let mut o = k * (n + 1);
        let mut offs = Vec::new();
        for &i in &thu_tu {
            offs.push(o);
            o += self.hinh[i].len();
        }
        offs.push(o);
        for v in offs {
            if rong {
                ra.extend((v as u32).to_le_bytes());
            } else {
                ra.extend((v as u16).to_le_bytes());
            }
        }
        for &i in &thu_tu {
            ra.extend(&self.hinh[i]);
        }
        for &i in &thu_tu {
            ra.extend((self.ma[i] as u16).to_le_bytes());
        }
        if let Some(l) = &self.bo_cuc {
            ra.extend(l.asc.to_le_bytes());
            ra.extend(l.desc.to_le_bytes());
            ra.extend(l.lead.to_le_bytes());
            for &i in &thu_tu {
                ra.extend(l.adv[i].to_le_bytes());
            }
            let r = rect0();
            for _ in 0..n {
                ra.extend(&r);
            }
            ra.extend((l.ker.len() as u16).to_le_bytes());
            for (a, b, v) in &l.ker {
                ra.extend(a.to_le_bytes());
                ra.extend(b.to_le_bytes());
                ra.extend(v.to_le_bytes());
            }
        }
        ra.extend(&self.con_lai);
        ra
    }

    fn la_latin(&self) -> bool {
        if self.ma.len() < 150
            || !self.ma.contains(&('a' as u32))
            || !self.ma.contains(&('à' as u32))
        {
            return false;
        }
        let la = self
            .ma
            .iter()
            .filter(|&&c| (0x400..0x700).contains(&c) || (0x3000..0xE000).contains(&c))
            .count();
        la <= 10
    }
}

/// GFX/SWF (không nén) → (đầu, [(mã thẻ, dữ liệu)])
pub fn tach_the(g: &[u8]) -> Result<(Vec<u8>, Vec<(u16, Vec<u8>)>)> {
    let mut r = Br::moi(g, 8);
    let nb = r.ub(5)? as usize;
    r.p += 4 * nb;
    r.align();
    let mut p = (r.p >> 3) + 4;
    let dau = g.get(..p).ok_or_else(|| anyhow!("GFX hỏng"))?.to_vec();
    let mut the = Vec::new();
    while p + 2 <= g.len() {
        let cl = u16_at(g, p)?;
        let (c, mut n) = (cl >> 6, (cl & 63) as usize);
        p += 2;
        if n == 63 {
            n = u32_at(g, p)? as usize;
            p += 4;
        }
        the.push((c, g.get(p..(p + n).min(g.len())).unwrap_or(&[]).to_vec()));
        p += n;
        if c == 0 {
            break;
        }
    }
    Ok((dau, the))
}

pub fn gop_the(dau: &[u8], the: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let mut ra = dau.to_vec();
    for (c, d) in the {
        if d.len() >= 63 || [75, 48, 1005].contains(c) {
            ra.extend(((c << 6) | 63).to_le_bytes());
            ra.extend((d.len() as u32).to_le_bytes());
        } else {
            ra.extend(((c << 6) | d.len() as u16).to_le_bytes());
        }
        ra.extend(d);
    }
    let n = ra.len() as u32;
    ra[4..8].copy_from_slice(&n.to_le_bytes());
    ra
}

/// GFX/CFX/FWS/CWS → bản không nén.
pub fn giai_gfx(b: &[u8]) -> Result<Vec<u8>> {
    if b.starts_with(b"CFX") || b.starts_with(b"CWS") {
        let mut d = Vec::new();
        flate2::read::ZlibDecoder::new(&b[8..]).read_to_end(&mut d)?;
        let mut ra = if b.starts_with(b"CFX") {
            b"GFX".to_vec()
        } else {
            b"FWS".to_vec()
        };
        ra.extend(&b[3..8]);
        ra.extend(d);
        return Ok(ra);
    }
    Ok(b.to_vec())
}

/// File GFX có chứa font Latin (DefineFont3 ≥ 150 chữ, có a / à, không phải Nga / Ả Rập / CJK)?
pub fn co_font_latin(gfx: &[u8]) -> bool {
    let Ok(g) = giai_gfx(gfx) else { return false };
    let Ok((_, the)) = tach_the(&g) else {
        return false;
    };
    the.iter()
        .any(|(c, d)| *c == 75 && Font3::doc(d).is_ok_and(|f| f.la_latin()))
}

// ---------------- hình học ----------------

fn phang(vien: &[Op], buoc: usize) -> Vec<Diem> {
    let mut pts = Vec::new();
    let (mut x, mut y) = (0.0, 0.0);
    for op in vien {
        match *op {
            Op::M(a, b) | Op::L(a, b) => {
                x = a;
                y = b;
                pts.push((x, y));
            }
            Op::Q(cx, cy, ax, ay) => {
                for k in 1..=buoc {
                    let t = k as f64 / buoc as f64;
                    let (a, b, c) = ((1.0 - t).powf(2.0), 2.0 * (1.0 - t) * t, t * t);
                    pts.push((a * x + b * cx + c * ax, a * y + b * cy + c * ay));
                }
                x = ax;
                y = ay;
            }
        }
    }
    if pts.len() > 1 {
        let (f, l) = (pts[0], pts[pts.len() - 1]);
        if (f.0 - l.0).abs() < 1e-6 && (f.1 - l.1).abs() < 1e-6 {
            pts.pop();
        }
    }
    pts
}

fn dien_tich(pts: &[Diem]) -> f64 {
    let n = pts.len();
    let mut s = 0.0;
    for i in 0..n {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[(i + 1) % n];
        s += x0 * y1 - x1 * y0;
    }
    s / 2.0
}

fn trong(p: Diem, poly: &[Diem]) -> bool {
    let (x, y) = p;
    let mut ok = false;
    let n = poly.len();
    for i in 0..n {
        let (x0, y0) = poly[i];
        let (x1, y1) = poly[(i + 1) % n];
        if (y0 > y) != (y1 > y) && x < x0 + (y - y0) * (x1 - x0) / (y1 - y0) {
            ok = !ok;
        }
    }
    ok
}

struct Nut {
    x: f64,
    y: f64,
    next: usize,
    prev: usize,
    giao: bool,
    vao: bool,
    ban: Option<usize>,
    a: f64,
    tham: bool,
}

fn vong(nut: &mut Vec<Nut>, pts: &[Diem]) -> Vec<usize> {
    let dau = nut.len();
    let n = pts.len();
    for (i, &(x, y)) in pts.iter().enumerate() {
        nut.push(Nut {
            x,
            y,
            next: dau + (i + 1) % n,
            prev: dau + (i + n - 1) % n,
            giao: false,
            vao: false,
            ban: None,
            a: 0.0,
            tham: false,
        });
    }
    (dau..dau + n).collect()
}

/// Hợp (union) 2 đa giác đơn (Greiner-Hormann) → chỉ giữ đa giác lớn nhất.
fn hop(a_pts: &[Diem], b_goc: &[Diem]) -> Result<Vec<Vec<Diem>>> {
    let b_pts: Vec<Diem> = b_goc
        .iter()
        .map(|&(x, y)| (x + 0.0137, y + 0.0291))
        .collect();
    let mut nut: Vec<Nut> = Vec::new();
    let sa = vong(&mut nut, a_pts);
    let sb = vong(&mut nut, &b_pts);
    let mut giao = Vec::new();
    for i in 0..sa.len() {
        let p1 = (nut[sa[i]].x, nut[sa[i]].y);
        let k = sa[(i + 1) % sa.len()];
        let p2 = (nut[k].x, nut[k].y);
        for j in 0..sb.len() {
            let q1 = (nut[sb[j]].x, nut[sb[j]].y);
            let k = sb[(j + 1) % sb.len()];
            let q2 = (nut[k].x, nut[k].y);
            let d = (p2.0 - p1.0) * (q2.1 - q1.1) - (p2.1 - p1.1) * (q2.0 - q1.0);
            if d.abs() < 1e-12 {
                continue;
            }
            let ta = ((q1.0 - p1.0) * (q2.1 - q1.1) - (q1.1 - p1.1) * (q2.0 - q1.0)) / d;
            let tb = ((q1.0 - p1.0) * (p2.1 - p1.1) - (q1.1 - p1.1) * (p2.0 - p1.0)) / d;
            if 0.0 < ta && ta < 1.0 && 0.0 < tb && tb < 1.0 {
                let x = p1.0 + ta * (p2.0 - p1.0);
                let y = p1.1 + ta * (p2.1 - p1.1);
                giao.push((i, ta, j, tb, x, y));
            }
        }
    }
    if giao.is_empty() {
        if trong(a_pts[0], &b_pts) {
            return Ok(vec![b_pts]);
        }
        if trong(b_pts[0], a_pts) {
            return Ok(vec![a_pts.to_vec()]);
        }
        return Ok(vec![a_pts.to_vec(), b_pts]);
    }
    // chèn điểm giao vào 2 vòng (giữ thứ tự gặp đầu tiên như dict của Python)
    let mut theo_a: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut theo_b: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut cap = Vec::new();
    let them = |ds: &mut Vec<(usize, Vec<usize>)>, k: usize, v: usize| match ds
        .iter_mut()
        .find(|x| x.0 == k)
    {
        Some(x) => x.1.push(v),
        None => ds.push((k, vec![v])),
    };
    for &(i, ta, j, tb, x, y) in &giao {
        let na = nut.len();
        let nb = na + 1;
        for (a, ban) in [(ta, nb), (tb, na)] {
            nut.push(Nut {
                x,
                y,
                next: 0,
                prev: 0,
                giao: true,
                vao: false,
                ban: Some(ban),
                a,
                tham: false,
            });
        }
        them(&mut theo_a, i, na);
        them(&mut theo_b, j, nb);
        cap.push(na);
    }
    for (ds, theo) in [(&sa, &mut theo_a), (&sb, &mut theo_b)] {
        for (i, lst) in theo.iter_mut() {
            lst.sort_by(|&p, &q| {
                nut[p]
                    .a
                    .partial_cmp(&nut[q].a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut cur = ds[*i];
            let nxt = nut[cur].next;
            for &n in lst.iter() {
                nut[cur].next = n;
                nut[n].prev = cur;
                cur = n;
            }
            nut[cur].next = nxt;
            nut[nxt].prev = cur;
        }
    }
    let danh_dau = |nut: &mut Vec<Nut>, dau: usize, khac: &[Diem]| {
        let mut tr = trong((nut[dau].x, nut[dau].y), khac);
        let mut n = dau;
        loop {
            if nut[n].giao {
                nut[n].vao = !tr;
                tr = !tr;
            }
            n = nut[n].next;
            if n == dau {
                break;
            }
        }
    };
    danh_dau(&mut nut, sa[0], &b_pts);
    danh_dau(&mut nut, sb[0], a_pts);
    for v in [sa[0], sb[0]] {
        let mut n = v;
        loop {
            if nut[n].giao {
                nut[n].vao = !nut[n].vao;
            }
            n = nut[n].next;
            if n == v {
                break;
            }
        }
    }
    let mut kq: Vec<Vec<Diem>> = Vec::new();
    for &c in &cap {
        if nut[c].tham {
            continue;
        }
        let mut poly = Vec::new();
        let mut n = c;
        loop {
            nut[n].tham = true;
            if let Some(b) = nut[n].ban {
                nut[b].tham = true;
            }
            if nut[n].vao {
                loop {
                    poly.push((nut[n].x, nut[n].y));
                    n = nut[n].next;
                    if nut[n].giao {
                        break;
                    }
                }
            } else {
                loop {
                    poly.push((nut[n].x, nut[n].y));
                    n = nut[n].prev;
                    if nut[n].giao {
                        break;
                    }
                }
            }
            n = nut[n].ban.ok_or_else(|| anyhow!("lỗi hợp hình"))?;
            if nut[n].tham && (n == c || nut[n].ban == Some(c)) {
                break;
            }
            if poly.len() > 100_000 {
                bail!("lỗi hợp hình");
            }
        }
        kq.push(poly);
    }
    // giữ đa giác lớn nhất (sắp ổn định như Python)
    kq.sort_by(|p, q| {
        let (a, b) = (-dien_tich(p).abs(), -dien_tich(q).abs());
        a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
    });
    kq.truncate(1);
    Ok(kq)
}

fn thanh_vien(poly: &[Diem], chieu: f64) -> Vien {
    let mut p = poly.to_vec();
    if (dien_tich(&p) < 0.0) != (chieu < 0.0) {
        p.reverse();
    }
    let mut v = vec![Op::M(p[0].0, p[0].1)];
    v.extend(p[1..].iter().map(|&(x, y)| Op::L(x, y)));
    v.push(Op::L(p[0].0, p[0].1));
    v
}

/// Hợp đa giác `them` vào viền ngoài đầu tiên chạm vào nó.
fn hop_vien(vien_goc: &[Vien], them: &[Diem], chieu: f64) -> Result<Vec<Vien>> {
    let mut ra = vien_goc.to_vec();
    for i in 0..ra.len() {
        let pts = phang(&ra[i], 10);
        if (dien_tich(&pts) < 0.0) != (chieu < 0.0) {
            continue; // lỗ (vd lòng chữ o)
        }
        let u = hop(&pts, them)?;
        if u.len() == 1 && dien_tich(&u[0]).abs() > dien_tich(&pts).abs() + 1.0 {
            ra[i] = thanh_vien(&u[0], chieu);
            return Ok(ra);
        }
    }
    ra.push(thanh_vien(them, chieu));
    Ok(ra)
}

fn bien(vien_ds: &[Vien]) -> (f64, f64, f64, f64) {
    let (mut x0, mut x1, mut y0, mut y1) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    let mut cap = |x: f64, y: f64| {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    };
    for c in vien_ds {
        for op in c {
            match *op {
                Op::M(x, y) | Op::L(x, y) => cap(x, y),
                Op::Q(cx, cy, x, y) => {
                    cap(cx, cy);
                    cap(x, y);
                }
            }
        }
    }
    (x0, x1, y0, y1)
}

/// Co dãn quanh `tam` rồi dịch chuyển.
fn bien_doi(vien_ds: &[Vien], sx: f64, sy: f64, dx: f64, dy: f64, tam: Diem) -> Vec<Vien> {
    let (cx, cy) = tam;
    let f = |x: f64, y: f64| ((x - cx) * sx + cx + dx, (y - cy) * sy + cy + dy);
    vien_ds
        .iter()
        .map(|c| {
            c.iter()
                .map(|op| match *op {
                    Op::M(x, y) => {
                        let (a, b) = f(x, y);
                        Op::M(a, b)
                    }
                    Op::L(x, y) => {
                        let (a, b) = f(x, y);
                        Op::L(a, b)
                    }
                    Op::Q(qx, qy, x, y) => {
                        let (a, b) = f(qx, qy);
                        let (c2, d) = f(x, y);
                        Op::Q(a, b, c2, d)
                    }
                })
                .collect()
        })
        .collect()
}

fn dich(v: &[Vien], dx: f64, dy: f64) -> Vec<Vien> {
    bien_doi(v, 1.0, 1.0, dx, dy, (0.0, 0.0))
}

// ---------------- TrueType (lấy dấu hỏi, râu) ----------------

fn be16(b: &[u8], o: usize) -> Result<u16> {
    b.get(o..o + 2)
        .map(|x| u16::from_be_bytes([x[0], x[1]]))
        .ok_or_else(|| anyhow!("font TTF hỏng"))
}
fn be32(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|x| u32::from_be_bytes(x.try_into().unwrap()))
        .ok_or_else(|| anyhow!("font TTF hỏng"))
}

fn ttf_vien(b: &[u8], ma: u32) -> Result<(Vec<Vien>, f64)> {
    let n = be16(b, 4)? as usize;
    let mut t: HashMap<String, usize> = HashMap::new();
    for i in 0..n {
        let o = 12 + 16 * i;
        let tag = String::from_utf8_lossy(&b[o..o + 4]).into_owned();
        t.insert(tag, be32(b, o + 8)? as usize);
    }
    let bang = |k: &str| {
        t.get(k)
            .copied()
            .ok_or_else(|| anyhow!("font TTF thiếu bảng {k}"))
    };
    let head = bang("head")?;
    let upem = be16(b, head + 18)? as f64;
    let fmt = be16(b, head + 50)? as i16;
    let ng = be16(b, bang("maxp")? + 4)? as u32;
    let loca_o = bang("loca")?;
    let loca = |i: u32| -> Result<usize> {
        Ok(if fmt == 0 {
            be16(b, loca_o + 2 * i as usize)? as usize * 2
        } else {
            be32(b, loca_o + 4 * i as usize)? as usize
        })
    };
    let o = bang("cmap")?;
    let mut gid = 0u32;
    for i in 0..be16(b, o + 2)? as usize {
        let off = be32(b, o + 4 + 8 * i + 4)? as usize;
        let q = o + off;
        if be16(b, q)? != 4 {
            continue;
        }
        let seg = be16(b, q + 6)? as usize / 2;
        let ro = q + 16 + 6 * seg;
        for k in 0..seg {
            let end = be16(b, q + 14 + 2 * k)? as u32;
            let st = be16(b, q + 16 + 2 * seg + 2 * k)? as u32;
            let dt = be16(b, q + 16 + 4 * seg + 2 * k)? as i16 as i32;
            let rof = be16(b, ro + 2 * k)? as usize;
            if st <= ma && ma <= end {
                gid = if rof == 0 {
                    ((ma as i32 + dt) & 0xFFFF) as u32
                } else {
                    let g = be16(b, ro + 2 * k + rof + 2 * (ma - st) as usize)? as i32;
                    if g != 0 {
                        ((g + dt) & 0xFFFF) as u32
                    } else {
                        0
                    }
                };
                break;
            }
        }
        if gid != 0 {
            break;
        }
    }
    if gid == 0 || gid >= ng {
        bail!("font thiếu ký tự U+{ma:04X}");
    }
    let mut p = bang("glyf")? + loca(gid)?;
    let nc = be16(b, p)? as i16;
    if nc < 0 {
        bail!("glyph ghép – không hỗ trợ");
    }
    let nc = nc as usize;
    p += 10;
    let mut ends = Vec::with_capacity(nc);
    for i in 0..nc {
        ends.push(be16(b, p + 2 * i)? as usize);
    }
    p += 2 * nc;
    p += 2 + be16(b, p)? as usize;
    let npt = ends.last().map_or(0, |e| e + 1);
    let mut fl: Vec<u8> = Vec::new();
    while fl.len() < npt {
        let f = b[p];
        p += 1;
        fl.push(f);
        if f & 8 != 0 {
            let r = b[p] as usize;
            fl.extend(std::iter::repeat_n(f, r));
            p += 1;
        }
    }
    let mut toa = |bit_ngan: u8, bit_giu: u8| -> Result<Vec<i64>> {
        let mut ra = Vec::with_capacity(fl.len());
        let mut v = 0i64;
        for &f in &fl {
            if f & bit_ngan != 0 {
                v += if f & bit_giu != 0 {
                    b[p] as i64
                } else {
                    -(b[p] as i64)
                };
                p += 1;
            } else if f & bit_giu == 0 {
                v += be16(b, p)? as i16 as i64;
                p += 2;
            }
            ra.push(v);
        }
        Ok(ra)
    };
    let xs = toa(2, 16)?;
    let ys = toa(4, 32)?;
    let mut vien = Vec::new();
    let mut st = 0;
    for &en in &ends {
        let c: Vec<(f64, f64, bool)> = (st..=en)
            .map(|i| (xs[i] as f64, -(ys[i] as f64), fl[i] & 1 != 0))
            .collect();
        st = en + 1;
        let (dau, ds): (Diem, Vec<(f64, f64, bool)>) = match c.iter().position(|q| q.2) {
            None => {
                let d = ((c[0].0 + c[1].0) / 2.0, (c[0].1 + c[1].1) / 2.0);
                (d, c[1..].iter().chain(&c[..1]).copied().collect())
            }
            Some(k) => (
                (c[k].0, c[k].1),
                c[k + 1..].iter().chain(&c[..k + 1]).copied().collect(),
            ),
        };
        let mut ops = vec![Op::M(dau.0, dau.1)];
        let mut ctrl: Option<Diem> = None;
        for (x, y, on) in ds {
            if on {
                ops.push(match ctrl {
                    Some(c) => Op::Q(c.0, c.1, x, y),
                    None => Op::L(x, y),
                });
                ctrl = None;
            } else {
                if let Some(c) = ctrl {
                    ops.push(Op::Q(c.0, c.1, (c.0 + x) / 2.0, (c.1 + y) / 2.0));
                }
                ctrl = Some((x, y));
            }
        }
        if let Some(c) = ctrl {
            ops.push(Op::Q(c.0, c.1, dau.0, dau.1));
        }
        vien.push(ops);
    }
    Ok((vien, upem))
}

// ---------------- ghép chữ Việt ----------------

/// Các chữ Việt cần có (theo thứ tự như bản Python).
pub fn chu_viet() -> Vec<char> {
    let mut ds: Vec<char> = Vec::new();
    for goc in "aăâeêioôơuưyAĂÂEÊIOÔƠUƯY".chars() {
        for dau in ["", "\u{300}", "\u{301}", "\u{303}", "\u{309}", "\u{323}"] {
            let s: String = format!("{goc}{dau}").nfc().collect();
            let mut it = s.chars();
            if let (Some(c), None) = (it.next(), it.next()) {
                if !ds.contains(&c) {
                    ds.push(c);
                }
            }
        }
    }
    for c in ['đ', 'Đ'] {
        if !ds.contains(&c) {
            ds.push(c);
        }
    }
    ds
}

/// gfx: file GFX/SWF chứa DefineFont3. ttf: font TrueType có dấu hỏi + râu. → (gfx mới, số chữ thêm)
pub fn them_tieng_viet(gfx: &[u8], ttf: &[u8]) -> Result<(Vec<u8>, usize)> {
    let g = giai_gfx(gfx)?;
    let (dau, mut the) = tach_the(&g)?;
    let mut tong = 0;
    for k in 0..the.len() {
        if the[k].0 != 75 {
            continue;
        }
        let mut f = Font3::doc(&the[k].1)?;
        if f.bo_cuc.is_none() || !f.ma.contains(&('a' as u32)) || !f.ma.contains(&('à' as u32)) {
            continue;
        }
        let so = them_vao_font(&mut f, ttf)?;
        if so > 0 {
            the[k].1 = f.ghi();
            tong += so;
        }
    }
    Ok((gop_the(&dau, &the), tong))
}

struct Dau {
    vs: Vec<Vien>,
    /// (rộng, cao)
    co: Diem,
}

fn chuan(vs: &[Vien]) -> Dau {
    let (x0, x1, _y0, y1) = bien(vs);
    let y0 = _y0;
    Dau {
        vs: dich(vs, -(x0 + x1) / 2.0, -y1),
        co: (x1 - x0, y1 - y0),
    }
}

fn dat(d: &Dau, tam_x: f64, day_y: f64, ty_le: f64) -> Vec<Vien> {
    bien_doi(&d.vs, ty_le, ty_le, tam_x, day_y, (0.0, 0.0))
}

fn nfc1(s: &str) -> Option<char> {
    let t: String = s.nfc().collect();
    let mut it = t.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

fn them_vao_font(f: &mut Font3, ttf: &[u8]) -> Result<usize> {
    let vi_tri: HashMap<u32, usize> = f.ma.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    let co_san: HashSet<u32> = f.ma.iter().copied().collect();
    let hinh = f.hinh.clone();
    let adv_goc = f.bo_cuc.as_ref().map(|b| b.adv.clone()).unwrap_or_default();
    let vien = |ch: char| -> Result<Vec<Vien>> {
        let i = *vi_tri
            .get(&(ch as u32))
            .ok_or_else(|| anyhow!("font thiếu chữ {ch}"))?;
        doc_hinh(&hinh[i])
    };
    let adv = |ch: char| -> Result<i64> {
        let i = *vi_tri
            .get(&(ch as u32))
            .ok_or_else(|| anyhow!("font thiếu chữ {ch}"))?;
        Ok(adv_goc[i] as i64)
    };
    let co = |ch: char| co_san.contains(&(ch as u32));
    let thieu: Vec<char> = chu_viet().into_iter().filter(|&c| !co(c)).collect();
    if thieu.is_empty() {
        return Ok(0);
    }
    let chieu = dien_tich(&phang(&vien('o')?[0], 10));
    let dau_tu = |ghep: char, goc: char| -> Result<Vec<Vien>> {
        let a = vien(ghep)?;
        let b: BTreeSet<(i64, i64)> = vien(goc)?
            .iter()
            .map(|c| {
                let (x, y) = c[0].xy();
                (x as i64, y as i64)
            })
            .collect();
        Ok(a.into_iter()
            .filter(|c| {
                let (x, y) = c[0].xy();
                !b.contains(&(x as i64, y as i64))
            })
            .collect())
    };
    let mut dau_thuong: HashMap<&str, Dau> = HashMap::new();
    let mut dau_hoa: HashMap<&str, Dau> = HashMap::new();
    for (ten, t, h) in [
        ("sac", 'á', 'Á'),
        ("huyen", 'à', 'À'),
        ("mu", 'â', 'Â'),
        ("nga", 'ã', 'Ã'),
    ] {
        dau_thuong.insert(ten, chuan(&dau_tu(t, 'a')?));
        dau_hoa.insert(ten, chuan(&dau_tu(h, 'A')?));
    }
    let mut lech: HashMap<&str, f64> = HashMap::new();
    for (ten, gh) in [("sac", 'á'), ("huyen", 'à'), ("mu", 'â')] {
        let (x0, x1, _, _) = bien(&dau_tu(gh, 'a')?);
        let (b0, b1, _, _) = bien(&vien('a')?);
        lech.insert(ten, (x0 + x1) / 2.0 - (b0 + b1) / 2.0);
    }
    lech.insert("nga", lech["mu"]);
    let day_thuong = bien(&dau_tu('á', 'a')?).3;
    let day_hoa = bien(&dau_tu('Á', 'A')?).3;
    let trang = if co('˘') {
        Some(chuan(&vien('˘')?))
    } else {
        None
    };
    let cham = if co('˙') {
        chuan(&vien('˙')?)
    } else {
        chuan(&vien('.')?)
    };
    let x_cao = -bien(&vien('x')?).2;
    let (hoi_v, upem) = ttf_vien(ttf, 0x0309)?;
    let (rau_v, _) = ttf_vien(ttf, 0x031B)?;
    let ty = x_cao / 540.0 * (upem / 1000.0).powf(-1.0);
    let rau_v = bien_doi(&rau_v, ty, ty, 0.0, 0.0, (0.0, 0.0));
    let bh = bien(&hoi_v);
    let sx_ = dau_thuong["sac"].co.1 * 0.95 / (bh.3 - bh.2);
    let hoi = chuan(&bien_doi(&hoi_v, sx_, sx_, 0.0, 0.0, (0.0, 0.0)));
    dau_thuong.insert(
        "hoi",
        Dau {
            vs: hoi.vs.clone(),
            co: hoi.co,
        },
    );
    dau_hoa.insert("hoi", hoi);
    lech.insert("hoi", 0.0);

    let mut so = 0;
    for ch in thieu {
        if ch == 'đ' || ch == 'Đ' {
            let (vs, a) = if ch == 'Đ' && co('Ð') {
                (vien('Ð')?, adv('Ð')?)
            } else if ch == 'đ' {
                (d_gach(&vien('d')?, chieu)?, adv('d')?)
            } else {
                continue;
            };
            them(f, ch as u32, &vs, a);
            so += 1;
            continue;
        }
        let d: Vec<char> = ch.to_string().nfd().collect();
        let (goc, dau) = (d[0], &d[1..]);
        let hoa = goc.is_uppercase();
        let mu = dau.iter().find_map(|c| match c {
            '\u{302}' => Some("mu"),
            '\u{306}' => Some("trang"),
            _ => None,
        });
        let rau = dau.contains(&'\u{31B}');
        let thanh = dau.iter().find_map(|c| match c {
            '\u{300}' => Some("huyen"),
            '\u{301}' => Some("sac"),
            '\u{303}' => Some("nga"),
            '\u{309}' => Some("hoi"),
            _ => None,
        });
        let nang = dau.contains(&'\u{323}');
        let mut nen_ch = goc;
        if goc == 'i' && (thanh.is_some() || mu.is_some()) {
            nen_ch = 'ı';
        }
        let mut con_thanh = thanh;
        if mu == Some("mu") && thanh.is_none() {
            nen_ch = nfc1(&format!("{goc}\u{302}")).unwrap_or(goc);
        } else if mu.is_none() && matches!(thanh, Some("sac" | "huyen" | "nga")) && !rau {
            let m = match thanh {
                Some("sac") => '\u{301}',
                Some("huyen") => '\u{300}',
                _ => '\u{303}',
            };
            if let Some(c2) = nfc1(&format!("{goc}{m}")) {
                if co(c2) {
                    nen_ch = c2;
                    con_thanh = None;
                }
            }
        }
        if !co(nen_ch) {
            nen_ch = goc;
            con_thanh = thanh;
        }
        let mut vs = vien(nen_ch)?;
        let mut a = adv(nen_ch)?;
        let (b0, b1, b2, _b3) = bien(&vien(if goc != 'i' { goc } else { 'ı' })?);
        let tam = (b0 + b1) / 2.0;
        let bo = if hoa { &dau_hoa } else { &dau_thuong };
        let day = if hoa { day_hoa } else { day_thuong };
        let mut tren: Vec<Vien> = Vec::new();
        let mut dinh = day;
        let duoi_day = |vs: &[Vien]| -> Vec<Vien> {
            vs.iter()
                .filter(|c| bien(std::slice::from_ref(*c)).3 <= day + 1.0)
                .cloned()
                .collect()
        };
        if mu == Some("mu") && (nen_ch == goc || nen_ch == 'ı') {
            let m = dat(&bo["mu"], tam + lech["mu"], day, 1.0);
            dinh = bien(&m).2;
            tren.extend(m);
        } else if mu == Some("mu") {
            let m = duoi_day(&vs);
            dinh = if m.is_empty() { day } else { bien(&m).2 };
        }
        if mu == Some("trang") {
            if let Some(tr) = &trang {
                let m = dat(tr, tam, day, 1.0);
                dinh = bien(&m).2;
                tren.extend(m);
            }
        }
        if rau {
            vs = gan_rau(&vs, &rau_v, goc, chieu, x_cao, b1, b2)?;
            a = a.max((bien(&vs).1 + 300.0) as i64);
        }
        if let Some(ct) = con_thanh {
            let dm = &bo[ct];
            let m = if mu == Some("mu") {
                if ct == "nga" {
                    dat(dm, tam + lech["mu"], dinh - 450.0, 0.8)
                } else {
                    let nguon = if tren.is_empty() {
                        duoi_day(&vs)
                    } else {
                        tren.clone()
                    };
                    let mx1 = bien(&nguon).1;
                    let w = dm.co.0 * 0.78;
                    dat(dm, mx1 - 700.0 + w / 2.0, day - 1500.0, 0.78)
                }
            } else if mu == Some("trang") {
                dat(dm, tam, dinh - 400.0, 0.8)
            } else {
                dat(dm, tam + lech.get(ct).copied().unwrap_or(0.0), day, 1.0)
            };
            tren.extend(m);
        }
        if nang {
            let mut m = dat(&cham, tam, cham.co.1 + 1200.0, 1.0);
            if (goc == 'y' || goc == 'Y') && cham_vao(&vs, &m) {
                for dx in (500..6000).step_by(500) {
                    let m2 = dich(&m, dx as f64, 0.0);
                    if !cham_vao(&vs, &m2) {
                        m = m2;
                        break;
                    }
                }
            }
            tren.extend(m);
        }
        vs.extend(tren);
        them(f, ch as u32, &vs, a);
        so += 1;
    }
    Ok(so)
}

fn cham_vao(vs: &[Vien], m: &[Vien]) -> bool {
    let (x0, x1, y0, y1) = bien(m);
    vs.iter().any(|c| {
        phang(c, 10)
            .iter()
            .any(|&(x, y)| x0 - 150.0 <= x && x <= x1 + 150.0 && y0 - 150.0 <= y && y <= y1 + 150.0)
    })
}

fn them(f: &mut Font3, ma: u32, vs: &[Vien], a: i64) {
    if f.ma.contains(&ma) {
        return;
    }
    f.ma.push(ma);
    f.hinh.push(ghi_hinh(vs));
    if let Some(b) = f.bo_cuc.as_mut() {
        b.adv.push(a as i16);
    }
}

/// Chữ đ = chữ d + gạch ngang qua thân dọc bên phải.
fn d_gach(vs: &[Vien], chieu: f64) -> Result<Vec<Vien>> {
    let (x0, x1, y0, _y1) = bien(vs);
    let mut dinh: Vec<f64> = Vec::new();
    for c in vs {
        for op in c {
            if let Op::L(x, y) = *op {
                if (y - y0).abs() < 1.0 && !dinh.contains(&x) {
                    dinh.push(x);
                }
            }
        }
    }
    dinh.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (trai, phai) = if dinh.len() >= 2 {
        (dinh[0], dinh[dinh.len() - 1])
    } else {
        (x1 - (x1 - x0) * 0.3, x1)
    };
    let day_net = y0 * 0.17;
    let yg = y0 + (-y0) * 0.11;
    let gach = [
        (trai - (phai - trai) * 0.75, yg),
        (phai + (phai - trai) * 0.3, yg),
        (phai + (phai - trai) * 0.3, yg + day_net.abs() * 0.6),
        (trai - (phai - trai) * 0.75, yg + day_net.abs() * 0.6),
    ];
    hop_vien(vs, &gach, chieu)
}

/// Gắn râu (ơ, ư) vào cạnh phải trên của chữ – hợp với nét chữ.
fn gan_rau(
    vs: &[Vien],
    rau_v: &[Vien],
    goc: char,
    chieu: f64,
    x_cao: f64,
    b1: f64,
    b2: f64,
) -> Result<Vec<Vien>> {
    let (_r0, r1, r2, _r3) = bien(rau_v);
    let vuot_dinh = x_cao * (111.0 / 540.0);
    let vuot_phai = x_cao
        * ((if goc == 'o' || goc == 'O' {
            10.0
        } else {
            115.0
        }) / 540.0);
    let dx = (b1 + vuot_phai) - r1;
    let dy = (b2 - vuot_dinh) - r2;
    let rv = dich(rau_v, dx, dy);
    let pts = phang(&rv[0], 12);
    hop_vien(vs, &pts, chieu)
}
