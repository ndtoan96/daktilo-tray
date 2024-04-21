fn main() {
    // Create icon for Windows
    #[cfg(target_os = "windows")]
    winres::WindowsResource::new()
        .set_icon("assets/typewritter_icon_enabled.ico")
        .compile()
        .unwrap();
}
