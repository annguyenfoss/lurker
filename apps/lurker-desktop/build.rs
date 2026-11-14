fn main() {
    // Winit loads X11/xkbcommon at runtime. NetBSD's loader does not search
    // the standard pkgsrc and native X11 directories unless the app records them.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("netbsd") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/pkg/lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/X11R7/lib");
    }

    let config = slint_build::CompilerConfiguration::new().with_style("material".into());

    slint_build::compile_with_config("ui/app-window.slint", config)
        .expect("failed to compile Slint UI");
}
