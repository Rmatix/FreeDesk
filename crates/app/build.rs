fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "FreeDesk Remote Desktop");
        res.set("FileDescription", "FreeDesk — Ultra-Performance Open-Source Remote Desktop");
        res.set("LegalCopyright", "Copyright (c) 2026 FreeDesk Contributors");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile windows resource: {:?}", e);
        }
    }
}
