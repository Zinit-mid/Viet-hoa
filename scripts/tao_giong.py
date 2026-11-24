# Chay bang Python cua app VieNeu: doc cac cau thanh WAV 48 kHz theo giong mau.
# Giao thuc: argv[1] = viec.json; stdout: TONG / NHOM / XONG / HET.
import json, os, sys, time
viec = json.load(open(sys.argv[1], encoding="utf-8"))
import numpy as np
import soundfile as sf
try:
    import librosa
except Exception:
    librosa = None
from vieneu import Vieneu
tts = Vieneu(mode="v3turbo", device="cuda" if viec.get("gpu") else "cpu")
SR = 48000
nhom = viec["nhom"]
viec_lam = []
for g in nhom:
    os.makedirs(g["ra"], exist_ok=True)
    can = [d for d in g["ds"] if not os.path.isfile(os.path.join(g["ra"], d[0] + ".wav"))]
    can.sort(key=lambda d: len(d[1]))
    viec_lam.append((g, can))
tong = sum(len(c) for _, c in viec_lam)
print("TONG", sum(len(g["ds"]) for g in nhom), tong, flush=True)

def keo_gian(w, rate):
    dem = np.zeros(int(0.25 * SR), np.float32)
    y = librosa.effects.time_stretch(np.concatenate([dem, w, dem]), rate=rate)
    d = int(len(dem) / rate)
    return y[d:len(y) - d] if len(y) > 2 * d else y

def xu_ly(w, goc, rms):
    w = np.nan_to_num(np.asarray(w, dtype=np.float32).reshape(-1))
    if librosa is not None and len(w) > SR // 10:
        try:
            w, _ = librosa.effects.trim(w, top_db=38)
        except Exception:
            pass
    if goc > 0.3 and librosa is not None and len(w) > SR // 20:
        dai, muc = len(w) / SR, goc * 0.97
        if dai > muc:
            try:
                w = keo_gian(w, min(dai / muc, viec.get("toi_da", 1.4)))
            except Exception as e:
                print("khong ep duoc thoi gian:", e, flush=True)
    w = np.nan_to_num(w)
    r = float(np.sqrt(np.mean(w ** 2))) if len(w) else 0
    if r > 1e-4:
        w = w * (rms / r)
    dinh = float(np.max(np.abs(w))) if len(w) else 0
    if dinh > 0.97:
        w = w * (0.97 / dinh)
    return np.concatenate([np.zeros(int(0.05 * SR), np.float32), w, np.zeros(int(0.1 * SR), np.float32)])

LO, xong = 16, 0
for so, (g, can) in enumerate(viec_lam, 1):
    print("NHOM", so, len(viec_lam), g["ten"], len(can), flush=True)
    if not can:
        continue
    nk = open(os.path.join(g["ra"], "_da_tao.txt"), "a", encoding="utf-8")
    for i in range(0, len(can), LO):
        lo = can[i:i + LO]
        try:
            wavs = tts.infer_batch([d[1] for d in lo], ref_audio=g["mau"])
        except Exception as e:
            print("lo loi, doc tung cau:", e, flush=True)
            wavs = []
            for d in lo:
                try:
                    wavs.append(tts.infer(d[1], ref_audio=g["mau"]))
                except Exception as e2:
                    print("LOI", d[0], e2, flush=True)
                    wavs.append(None)
        for d, w in zip(lo, wavs):
            if w is None or len(w) == 0:
                continue
            p = os.path.join(g["ra"], d[0] + ".wav")
            try:
                y = xu_ly(w, float(d[2] or 0), g.get("rms", 0.08))
            except Exception as e:
                print("LOI xu ly", d[0], e, flush=True)
                y = np.nan_to_num(np.asarray(w, dtype=np.float32).reshape(-1))
            sf.write(p + ".tam", y, SR, subtype="PCM_16", format="WAV")
            os.replace(p + ".tam", p)
            nk.write(d[0] + "\n")
            nk.flush()
        xong += len(lo)
        print("XONG", xong, tong, flush=True)
    nk.close()
print("HET", flush=True)
