fn main() {
    // Embed the app icon and version info into the Windows executable.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Spotty");
        res.set("FileDescription", "Spotty");
        res.compile().expect("failed to embed Windows resources");
    }
}
