//! Gắn icon (assets/icon.ico) vào VietHoa.exe. Thiếu rc.exe thì chỉ cảnh báo, không làm hỏng build.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("assets/icon.ico");
        r.set("ProductName", "VIỆT HÓA");
        r.set("FileDescription", "Việt Hóa");
        if let Err(e) = r.compile() {
            println!("cargo:warning=không gắn được icon vào exe: {e}");
        }
    }
}
