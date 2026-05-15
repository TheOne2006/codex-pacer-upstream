fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=src/macos_menu_bar_popup.m");
        cc::Build::new()
            .file("src/macos_menu_bar_popup.m")
            .flag("-fobjc-arc")
            .compile("codex_pacer_macos_menu_bar_popup");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=QuartzCore");
    }

    tauri_build::build()
}
