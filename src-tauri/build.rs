fn main() {
    // 嵌入自定义 app.manifest（requireAdministrator 提权 + UTF-8 代码页）
    let manifest = std::path::Path::new("app.manifest");
    if manifest.exists() {
        println!("cargo:rerun-if-changed=app.manifest");
        let _ = embed_resource::compile(manifest, embed_resource::NONE);
    }
    tauri_build::build()
}
