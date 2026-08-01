use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let dist = manifest_dir.join("../../web/admin/dist");
    if !dist.exists() {
        fs::create_dir_all(&dist).expect("failed to create web/admin/dist fallback");
        fs::write(
            dist.join("index.html"),
            r#"<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>QimenBot Admin</title><body><div id="root">Admin UI has not been built. Run npm run build in web/admin.</div></body></html>"#,
        )
        .expect("failed to create admin fallback index");
    }
    println!("cargo:rerun-if-changed=../../web/admin/dist");
    println!("cargo:rerun-if-changed=../../web/admin/src");
}
