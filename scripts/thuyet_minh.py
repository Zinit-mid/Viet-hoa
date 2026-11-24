# Thuyet minh phim bang VieNeu (chay bang Python cua VieNeu). Doc argv[1] = viec.json.
#   che_do "tao_bo": tao bo giong cam xuc cho cac giong dung san: <ra>\<giong>\{binh_thuong,gap_rut,buon}.wav
#   che_do "doc"   : doc tung dong phu de -> <ra>\<id>.wav (chon giong mau theo cam xuc, nhan nha, ep nhip)
# In: TONG n / XONG a b / HET
import json, os, sys
import numpy as np
import soundfile as sf

viec = json.load(open(sys.argv[1], encoding="utf-8"))
from vieneu import Vieneu
try:
    import librosa
except Exception:
    librosa = None

tts = Vieneu(mode="v3turbo", device="cuda" if viec.get("gpu", True) else "cpu")
SR = 48000


def ghi(p, w):
    sf.write(p + ".tam", np.asarray(w, dtype=np.float32), SR, subtype="PCM_16", format="WAV")
    os.replace(p + ".tam", p)


if viec["che_do"] == "tao_bo":
    cau = {"binh_thuong": "Thành phố này đã yên bình suốt hàng trăm năm, cho đến một đêm mưa.",
           "gap_rut": "Chạy mau! Chúng đang đuổi theo chúng ta, đừng quay đầu lại!",
           "buon": "Anh ấy nhìn theo bóng cô khuất dần, rồi lặng lẽ cúi đầu, nước mắt rơi xuống."}
    ds = viec["giong"]
    print("TONG", len(ds), flush=True)
    for i, g in enumerate(ds, 1):
        thu = os.path.join(viec["ra"], g.replace(" ", "_"))
        os.makedirs(thu, exist_ok=True)
        for k, c in cau.items():
            p = os.path.join(thu, k + ".wav")
            if not os.path.isfile(p):
                ghi(p, np.asarray(tts.infer(c, voice=g), dtype=np.float32).reshape(-1))
        print("XONG", i, len(ds), flush=True)
    print("HET", flush=True)
    sys.exit(0)

# ---- che do doc
# nhan nha theo cam xuc: (gain dB, toc do, cao do nua cung)
NHAN_NHA = {"binh_thuong": (0.0, 1.0, 0.0), "nhan_manh": (3.0, 1.06, 1.0), "gap_rut": (4.0, 1.12, 1.5),
            "buon": (-3.0, 0.92, -1.0), "hoi": (1.0, 1.0, 0.5)}
MAU_THEO = {"binh_thuong": "binh_thuong", "hoi": "binh_thuong", "nhan_manh": "gap_rut", "gap_rut": "gap_rut",
            "buon": "buon"}
cuong_do = float(viec.get("cuong_do", 0.7))
toi_da = float(viec.get("toi_da", 1.35))
rms_dich = float(viec.get("rms", 0.09))
ra = viec["ra"]
os.makedirs(ra, exist_ok=True)
ds = [d for d in viec["ds"] if not os.path.isfile(os.path.join(ra, d["id"] + ".wav"))]
print("TONG", len(viec["ds"]), len(ds), flush=True)

nhom = {}
for d in ds:
    mau = None
    if viec.get("bo"):
        p = os.path.join(viec["bo"], MAU_THEO.get(d.get("cam_xuc", "binh_thuong"), "binh_thuong") + ".wav")
        if os.path.isfile(p):
            mau = p
    nhom.setdefault(mau or "", []).append(d)


def xu_ly(w, d):
    w = np.nan_to_num(np.asarray(w, dtype=np.float32).reshape(-1))
    if librosa is not None and len(w) > SR // 10:
        try:
            w, _ = librosa.effects.trim(w, top_db=38)
        except Exception:
            pass
    r = float(np.sqrt(np.mean(w ** 2))) if len(w) else 0
    if r > 1e-4:
        w = w * (rms_dich / r)
    gain, toc, cao = NHAN_NHA.get(d.get("cam_xuc", "binh_thuong"), NHAN_NHA["binh_thuong"])
    if viec.get("nhan_nha", True) and librosa is not None and len(w) > SR // 5:
        try:
            if abs(cao) > 1e-3:
                w = librosa.effects.pitch_shift(w, sr=SR, n_steps=cao * cuong_do)
            t = 1.0 + (toc - 1.0) * cuong_do
            if abs(t - 1.0) > 0.01:
                w = librosa.effects.time_stretch(w, rate=t)
        except Exception as e:
            print("khong nhan nha duoc:", e, flush=True)
        w = w * (10 ** (gain * cuong_do / 20.0))
    # ep nhip: dong dai hon khung thoi gian thi nen lai (toi da toi_da)
    khung = float(d.get("giay", 0))
    if khung > 0.3 and librosa is not None and len(w) / SR > khung:
        try:
            w = librosa.effects.time_stretch(w, rate=min(len(w) / SR / khung, toi_da))
        except Exception as e:
            print("khong ep duoc nhip:", e, flush=True)
    dinh = float(np.max(np.abs(w))) if len(w) else 0
    if dinh > 0.97:
        w = w * (0.97 / dinh)
    return w


xong = 0
tong = len(ds)
LO = 8
for mau, lst in nhom.items():
    lst.sort(key=lambda d: len(d["text"]))
    for i in range(0, len(lst), LO):
        lo = lst[i:i + LO]
        kw = {"ref_audio": mau, "denoise": False} if mau else {"voice": viec.get("giong") or None}
        try:
            wavs = tts.infer_batch([d["text"] for d in lo], **kw)
        except Exception as e:
            print("lo loi, doc tung dong:", e, flush=True)
            wavs = []
            for d in lo:
                try:
                    wavs.append(tts.infer(d["text"], **kw))
                except Exception as e2:
                    print("LOI", d["id"], e2, flush=True)
                    wavs.append(None)
        for d, w in zip(lo, wavs):
            if w is None or len(w) == 0:
                continue
            try:
                ghi(os.path.join(ra, d["id"] + ".wav"), xu_ly(w, d))
            except Exception as e:
                print("LOI xu ly", d["id"], e, flush=True)
        xong += len(lo)
        print("XONG", xong, tong, flush=True)
print("HET", flush=True)
