use qimen_error::Result;
use qimen_official_host::run_official_host;

// 强制链接包含 inventory 注册项的静态插件目标文件。
// Windows/MSVC 可能丢弃只有 inventory 构造器、没有具体符号引用的目标文件。
extern crate qimen_plugin_example;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--version")) {
        println!("qimenbotd {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let _ = dotenvy::dotenv();

    // 引用每个静态插件的具体符号，确保 inventory 注册项进入最终二进制。
    std::hint::black_box(qimen_plugin_example::BasicModule::__QIMEN_MODULE_ID);

    let config_path =
        std::env::var("QIMEN_CONFIG_PATH").unwrap_or_else(|_| "config/base.toml".to_string());
    run_official_host(&config_path).await
}
