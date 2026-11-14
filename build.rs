fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            // Optional camera support must not prevent chat startup on Windows N
            // without Media Feature Pack. Only the disposable camera helper uses
            // these APIs; the parent reports its failure without loading them.
            println!("cargo:rustc-link-lib=delayimp");
            for dll in ["mf.dll", "mfplat.dll", "mfreadwrite.dll"] {
                println!("cargo:rustc-link-arg=/DELAYLOAD:{dll}");
            }
        }
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
