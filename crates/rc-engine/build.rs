fn main() {
    println!("cargo:rerun-if-changed=assets/ratchet-clank.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/ratchet-clank.ico")
            .set("FileDescription", "Ratchet & Clank")
            .set("ProductName", "Ratchet & Clank PC")
            .set("OriginalFilename", "Ratchet & Clank.exe")
            .set("InternalName", "Ratchet & Clank")
            .compile()
            .expect("embedding the Ratchet & Clank Windows icon");
    }
}
