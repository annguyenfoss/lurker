fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_style("material".into())
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);

    slint_build::compile_with_config("ui/app-window.slint", config)
        .expect("failed to compile Slint UI");
}
