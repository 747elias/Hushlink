fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/hushlink.ico");
        res.set_manifest_file("assets/hushlink.manifest");
        res.set("ProductName", "Hushlink");
        res.set(
            "FileDescription",
            "Hushlink – verschlüsselter Bluetooth-Chat",
        );
        res.set("LegalCopyright", "Hushlink contributors");
        res.compile()
            .unwrap_or_else(|e| panic!("Windows resources: {e}"));
    }
}
