#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icon_app.ico");
    res.set_icon_with_id("assets/icon_app.ico", "100");
    // Embed tray icons as resources (IDs 101-104)
    res.set_icon_with_id("assets/icon_green.ico", "101");
    res.set_icon_with_id("assets/icon_yellow.ico", "102");
    res.set_icon_with_id("assets/icon_red.ico", "103");
    res.set_icon_with_id("assets/icon_gray.ico", "104");
    // Complete VERSIONINFO metadata: sparse or missing fields are a common
    // heuristic signal for antivirus ML classifiers on unsigned binaries.
    let version = env!("CARGO_PKG_VERSION");
    res.set("ProductName", "ClaudeMeter");
    res.set(
        "FileDescription",
        "ClaudeMeter - Claude AI usage monitor for the system tray",
    );
    res.set("CompanyName", "klivak (open source)");
    res.set("OriginalFilename", "claudemeter.exe");
    res.set("InternalName", "claudemeter");
    res.set("ProductVersion", version);
    res.set("FileVersion", version);
    res.set("LegalCopyright", "MIT License - klivak");
    res.set(
        "Comments",
        "Open source: https://github.com/klivak/claudemeter",
    );
    res.set_manifest_file("app.manifest");
    if let Err(e) = res.compile() {
        eprintln!("Warning: could not compile Windows resources: {e}");
        eprintln!("This is expected on non-Windows or without Windows SDK.");
    }
}

#[cfg(not(windows))]
fn main() {}
