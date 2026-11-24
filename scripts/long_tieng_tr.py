# Chay bang Python cua app VieNeu. che_do: phan_tich (gan cau->kenh giong) / mau (giong mau) / ghep (ghep vao MP3 44,1 kHz).
import json, os, sys
import numpy as np
import soundfile as sf
viec = json.load(open(sys.argv[1], encoding="utf-8"))
SR = 44100

def doc(p):
    y, sr = sf.read(p, dtype="float32", always_2d=True)
    y = y.mean(axis=1)
    if sr != SR:
        try:
            import soxr
            y = soxr.resample(y, sr, SR)
        except Exception:
            import librosa
            y = librosa.resample(y, orig_sr=sr, target_sr=SR)
    return np.nan_to_num(y.astype(np.float32))

def rms(y):
    return float(np.sqrt(np.mean(y ** 2))) if len(y) else 0.0

def chi_giong(y):
    # bo doan im lang (giu cac khung 50 ms co tieng)
    n = int(0.05 * SR)
    k = len(y) // n
    if k == 0:
        return y
    kh = y[:k * n].reshape(k, n)
    e = np.sqrt((kh ** 2).mean(axis=1))
    giu = kh[e > max(0.01, e.max() * 0.08)]
    return giu.reshape(-1) if len(giu) else y

ds = viec["ds"]
print("TONG", len(ds), flush=True)
if viec["che_do"] == "phan_tich":
    kq = {}
    for i, v in enumerate(ds, 1):
        am = []
        for p in v["kenh"]:
            try:
                am.append(doc(p))
            except Exception as e:
                print("LOI", p, e, flush=True)
                am.append(np.zeros(1, np.float32))
        gan = []
        for a, b in v["cau"]:
            e = [rms(y[a:b]) for y in am]
            j = int(np.argmax(e)) if e else -1
            gan.append(j if e and e[j] > 0.004 else -1)
        kq[v["id"]] = gan
        print("XONG", i, len(ds), flush=True)
    json.dump(kq, open(viec["ra"], "w", encoding="utf-8"))
elif viec["che_do"] == "mau":
    for i, v in enumerate(ds, 1):
        phan = []
        nguon = v["nguon"]
        if v.get("chon"):
            # chon cac cau noi TO va CHAC nhat (RMS phan co tieng cao, it khoang lang)
            diem = []
            for p in nguon:
                try:
                    y = doc(p)
                    gy = chi_giong(y)
                    diem.append((rms(gy) * min(1.0, len(gy) / max(len(y), 1) * 1.4), p))
                except Exception as e:
                    print("LOI", p, e, flush=True)
            diem.sort(reverse=True)
            nguon = [p for _, p in diem[: v["chon"]]]
            print("CHON", [os.path.basename(p) for p in nguon], flush=True)
        for p in nguon:
            try:
                phan.append(chi_giong(doc(p)))
                phan.append(np.zeros(int(0.3 * SR), np.float32))
            except Exception as e:
                print("LOI", p, e, flush=True)
        if phan:
            y = np.concatenate(phan)[: 15 * SR]
            sf.write(v["ra"] + ".tam.wav", y, SR, subtype="PCM_16")
            os.replace(v["ra"] + ".tam.wav", v["ra"])
        print("XONG", i, len(ds), flush=True)
else:
    try:
        import librosa
    except Exception:
        librosa = None
    tang = 10 ** (float(viec.get("tang_db", 0)) / 20)
    san = float(viec.get("san", 0))

    def dam(y):
        # giong chac hon: bo am tram duoi 100 Hz, tang nhe dai 2-6 kHz (ro chu), nen dinh mem
        try:
            from scipy.signal import butter, sosfilt
            y = sosfilt(butter(2, 100, "highpass", fs=SR, output="sos"), y).astype(np.float32)
            y = (y + 0.35 * sosfilt(butter(2, 2000, "highpass", fs=SR, output="sos"), y)).astype(np.float32)
        except Exception:
            pass
        return y

    for i, v in enumerate(ds, 1):
        try:
            goc = doc(v["goc"])
        except Exception as e:
            print("LOI doc goc", v["goc"], e, flush=True)
            continue
        n = int(v["so_mau"])
        goc = np.pad(goc, (0, max(0, n - len(goc))))[:n]
        ra = goc.copy()                       # giu nguyen am thanh goc ngoai cac cau da long tieng
        for a, b, w in v["cau"]:
            a, b = max(0, min(a, n)), max(0, min(b, n))
            if b <= a or not w:
                continue
            y = doc(w)
            cho = b - a
            if len(y) > cho and librosa is not None:
                try:
                    y = librosa.effects.time_stretch(y, rate=min(len(y) / cho, 1.35))
                except Exception:
                    pass
            if len(y) > cho:
                y = y[:cho]
                m = min(len(y), int(0.03 * SR))
                y[-m:] *= np.linspace(1, 0, m, dtype=np.float32)
            y = dam(y)
            g = goc[a:b]
            r0, r1 = rms(g[np.abs(g) > 0.01]), rms(y[np.abs(y) > 0.01])
            r0 = max(r0, san)                 # goc noi tham -> giong Viet van du ro
            if r0 > 1e-3 and r1 > 1e-3:
                y = y * min(r0 / r1, 6.0)
            y = y * tang
            y = (np.tanh(1.5 * y) / 1.5).astype(np.float32)      # nen dinh: to ma khong vo tieng
            ra[a:b] = 0
            ra[a:a + len(y)] = y
        dinh = float(np.max(np.abs(ra))) if n else 0
        if dinh > 0.98:
            ra *= 0.98 / dinh
        sf.write(v["ra"] + ".tam", ra, SR, format="MP3", subtype="MPEG_LAYER_III",
                 bitrate_mode="CONSTANT", compression_level=0.7)
        os.replace(v["ra"] + ".tam", v["ra"])
        print("XONG", i, len(ds), flush=True)
print("HET", flush=True)
