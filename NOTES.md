# Việt Hóa – ghi chú dự án

App Rust viết lại app Python "Dịch Game" (repo cũ Zinit-mid/DichGame, mã tham khảo ở `tham_khao\DichGame`).
Chuyển hết chức năng app cũ nhưng gọn hơn và bỏ các nút thử. Cập nhật lần cuối: 24/11/2026.

## Tiến độ

| GĐ | Nội dung | Trạng thái |
|---|---|---|
| 1 | Dịch màn hình (OCR, khung dịch, F6–F12) | Xong (c876f55) |
| 2 | Lõi `ViecDich` dùng chung (Claude theo lô / offline, bộ nhớ dịch) | Xong (c876f55) |
| 3 | Thư viện game (Steam / Epic / GOG / thêm tay) + Dò file chữ 9 định dạng | Xong (12f183d) |
| 4 | Crimson Desert: chữ (paz / paloc), gói DMM, mod túi đồ | Xong (4450b13) |
| 5 | Rise / TR2013 / Shadow: chữ, phụ đề .mul, lời thoại TextEntry, font Việt (Rise), cài / gỡ | Xong, chưa commit |
| 6 | Lồng tiếng + Clone giọng (VieNeu, Wwise cho CD, FSB4 cho TR; cài giọng TR2013 vào patch2_ENGLISH) | **Đang làm:** lõi VieNeu xong (`giong_vieneu.rs` + `scripts\tao_giong.py`, đã đọc thật 2 câu trên GPU). Wwise xong (`wwise.rs`: WAV→.wem→zip DMM, đã mã hóa thật 1 câu). Lồng tiếng Rise/TR2013 xong (`TombRaider::long_tieng`, `phien_am.rs`, `scripts\long_tieng_tr.py`; thử 16 đoạn đầu Rise, MP3 đóng được FSB4; chưa thử trong game, TR2013 chưa chạy thật). Giao diện lồng tiếng TR có trong thẻ game (chưa xem bằng mắt). Lồng tiếng CD xong phần code (`cd_giong.rs`: kế hoạch A/B đọc đúng 64.813 file .wem, giọng mẫu bằng vgmstream đã thử với Kliff; thẻ giao diện CD chưa xem bằng mắt; chưa chạy trọn A/B vì rất lâu, tự sửa giới tính F0 đã port, khớp 119/119 người với app cũ; chưa có học phiên âm bằng Claude). Clone giọng xong (`clone_giong.rs` + trang `trang_clone_giong.rs`: cắt/bỏ lặng/lọc 70 Hz/chuẩn hóa, nghe thử bằng VieNeu, lưu kho `giong_rieng`, gán `phan_bo.json`; đã thử thật 1 mẫu + 1 câu; chưa thử nút bấm). Xong GĐ6 phần code |
| 7 | Phụ đề phim (Whisper) + giọng đọc | Xong, chưa commit: `phu_de_phim.rs` + trang `trang_phu_de_phim.rs` (yt-dlp, faster-whisper GPU, dịch Claude/offline, .srt, mở VLC); đã chạy thật 60 giây phim (nghe 5 s, ra .srt Việt). Giọng đọc (edge-tts) có từ GĐ1 trong Dịch màn hình. Chưa thử tải link YouTube thật |

**Việc còn lại:**
- **Claude:** đường dịch bằng Claude chưa gọi thật lần nào (tránh tốn tiền).
- **Kiểm tra trong game thật:**
  - Crimson Desert: gói DMM và mod túi đồ.
  - TR2013 và Shadow: bản cài.
- **Rise:** gói mới giống từng byte gói app cũ đã cài, nên coi như đã kiểm.

## Quyết định kiến trúc

**Thư mục và dữ liệu**
- **Mọi thứ nằm trong `D:\Việt hóa`:**
  - `cong_cu` chứa Rust, Build Tools, python, ffmpeg…
  - `du_lieu` chứa cấu hình, bộ nhớ dịch, mục lục.
  - Không ghi gì vào `%APPDATA%` hay ổ C.
- **Tìm thư mục gốc** (`duong_dan::goc()`), theo thứ tự:
  1. Thư mục có `Cargo.toml`.
  2. Thư mục chứa exe.
  3. Nếu cạnh exe không có `cong_cu` thì dùng `D:\Việt hóa`, hoặc biến `VIET_HOA_GOC`.
  - Biến `VIET_HOA_DU_LIEU` đổi riêng thư mục dữ liệu, dùng khi kiểm thử.
- **Dữ liệu tương thích app cũ:**
  - Giữ đúng định dạng: bộ nhớ dịch, `cau_hinh.json`, mục lục `.mul` / TextEntry, `font_rottr.json`.
  - Giữ đúng tên gói mod (`viethoa_dichgame`), để hai app dùng chung dữ liệu và gỡ được gói của nhau.
- **Khóa API** chỉ nằm trong `du_lieu\khoa_api.txt`, không bao giờ đưa vào code hay repo.

**Dịch màn hình**
- Chụp màn hình bằng DXGI Desktop Duplication. GDI là dự phòng, kèm thời gian chờ khi lỗi (`quan_ly_chup`).
- Lớp phủ là cửa sổ layered, xuyên chuột và loại khỏi ảnh chụp (`WDA_EXCLUDEFROMCAPTURE`).
- Cửa sổ app được ẩn khỏi ảnh chụp khi đang dịch.
- Mỗi vùng chỉ dịch câu mới nhất.
- Thứ tự tra bản dịch:
  1. Bản Việt hóa
  2. Bản Claude đã lưu
  3. Từ điển menu
  4. Offline (quickmt qua `scripts\dich_offline.py`)
  5. Google

**Lõi dịch và game**
- **`viet_hoa::viec_dich::ViecDich`** là lõi dùng chung cho mọi game:
  - Claude chạy 4 luồng; mỗi lô tối đa 40 câu / 6.000 ký tự; có giới hạn USD mỗi lần.
  - Dịch offline giữ nguyên thẻ.
  - Hàm kiểm thẻ cắm vào được (`ViecDich::the`); Tomb Raider dùng `the_tr`.
- **Mỗi game có công cụ riêng** trong `src/viet_hoa/`:
  - Crimson Desert: `cd_dinh_dang`, `crimson_desert`.
  - Tomb Raider: `tiger`, `mul`, `font_viet`, `font_rottr`, `tomb_raider`.
  - Mọi game khác: `do_file` (Dò file chữ).
- **Không sửa file gốc của game:**
  - Crimson Desert: xuất zip cho DMM.
  - Rise / Shadow: gói mod riêng; gỡ = xóa gói.
  - TR2013: buộc phải tạo lại `patch2`, nhưng giữ bản gốc `bigfile._orig.patch2.000.tiger`.
  - Dò file chữ: sao lưu trước khi ghi và luôn dịch từ bản gốc.
- **Giọng đã lồng của app cũ** (`<ổ>:\DichGame_LongTieng\RotTR|TR2013\mp3`) được cài kèm khi cài Rise, để không mất lồng tiếng.

**Giao diện**
- egui (`eframe` 0.31), tắt lưu trạng thái cửa sổ.
- Việc nặng chạy nền qua `chay_nen`, có thanh tiến trình và nút Dừng.
- Font giao diện không có ký tự ✓ → ✗; dùng chữ thường thay thế.

## Cách kiểm thử

**Lệnh chung**
- `cargo test --release` chạy test thường.
- Các test `#[ignore]` chạy với dữ liệu thật, qua biến môi trường. Ví dụ:
  - `CD=<thư mục game> cargo test -- --ignored doc_that`
  - `TR=<thư mục> LOAI=rise|tr2013|shadow cargo test -- --ignored cai_that` (thêm `CHI_NAP=1` để chỉ đọc, `GO=1` để thử gỡ)

**Cài vào game**
- **Không cài thử vào thư mục game thật.** Tạo thư mục giả trên cùng ổ bằng liên kết cứng (`New-Item -ItemType HardLink`).
- **Ngoại lệ:** file bị ghi đè trực tiếp (vd `patch2.000.tiger`) phải chép thật, không dùng liên kết cứng.
- **So với app cũ:** chạy code Python trong `tham_khao\DichGame` bằng `cong_cu\python`, với `APPDATA` trỏ vào dữ liệu thử, rồi so kết quả từng byte. GĐ4–5 đã khớp.

**Giao diện**
- Phím giả lập từ shell không tới được bộ bắt phím của app. Dùng tùy chọn khởi động `--dich` / `--f12`, và bấm chuột bằng `SendInput`.

## Dùng độc lập (GĐ8, chưa commit)

Mục tiêu: thao tác hoàn toàn trên app, không cần Claude Code; Claude API chỉ là tùy chọn (mặc định dịch offline).
- **Sửa bản dịch** (`trang_sua_dich.rs`): sửa từng câu, tìm & thay hàng loạt, từ điển `tu_dien.json` (tên riêng + sửa bản dịch) áp cho mọi lần dịch sau (`dich::ap_dung_sua` trong `viec_dich`).
- **Làm tự động** (`tu_dong.rs` + `trang_tu_dong.rs`): một nút dịch → lồng tiếng → cài cho CD / Rise / TR2013 / Shadow / game khác (dò file chữ). Đã chạy thật trên game giả (10 câu dịch offline rồi ghi).
- **Lồng tiếng chung** (`long_tieng_chung.rs` + `trang_long_tieng.rs`): game có file thoại rời wav/ogg/mp3/flac/opus; ghép chữ theo tên file hoặc CSV, giọng nhái gốc hoặc từ kho, mã hóa lại bằng ffmpeg, cài/gỡ có sao lưu. Đã thử thật 3 file ogg, gỡ trả đúng từng byte. Chưa thử trên game thật.
- **Đóng gói**: `scripts\dong_goi.ps1` → `dist\VietHoa_<ver>.zip` (exe + scripts + hướng dẫn, 5,6 MB). Script nhúng trong exe, tự chép ra `scripts\` khi chạy ở máy khác (thư mục không có Cargo.toml). VieNeu fallback sang `cong_cu\python` nếu máy không có app VieNeu. Chưa có tự cập nhật.