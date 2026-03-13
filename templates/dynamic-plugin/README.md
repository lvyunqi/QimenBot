# QimenBot 动态插件模板

## 快速开始

1. 复制此目录到 `plugins/` 下并重命名：
   ```bash
   cp -r templates/dynamic-plugin plugins/qimen-dynamic-plugin-myplugin
   ```

2. 替换所有 `{{name}}` 为你的插件 ID（如 `myplugin`）

3. 编译：
   ```bash
   cd plugins/qimen-dynamic-plugin-myplugin
   cargo build --release
   ```

4. 复制动态库到 `plugins/bin/`：
   ```bash
   # Linux
   cp target/release/libqimen_dynamic_plugin_myplugin.so ../../plugins/bin/
   # macOS
   cp target/release/libqimen_dynamic_plugin_myplugin.dylib ../../plugins/bin/
   # Windows
   cp target/release/qimen_dynamic_plugin_myplugin.dll ../../plugins/bin/
   ```

5. 在 bot 中执行 `/plugins reload` 热加载

## 使用过程宏简化开发

在 `Cargo.toml` 中取消注释 `qimen-dynamic-plugin-derive` 依赖，然后：

```rust
use qimen_dynamic_plugin_derive::dynamic_plugin;
use abi_stable_host_api::{CommandRequest, CommandResponse};

#[dynamic_plugin(id = "myplugin", version = "0.1.0")]
mod myplugin {
    #[command(name = "hello", description = "Say hello", aliases = "hi")]
    fn hello(req: &CommandRequest) -> CommandResponse {
        CommandResponse::text("Hello!")
    }
}
```

## API 参考

### CommandRequest 字段
| 字段 | 类型 | 说明 |
|------|------|------|
| `args` | RString | 命令参数（空格分隔） |
| `command_name` | RString | 匹配的命令名 |
| `sender_id` | RString | 发送者 ID |
| `sender_nickname` | RString | 发送者昵称 |
| `group_id` | RString | 群 ID（私聊为空） |
| `message_id` | RString | 消息 ID |
| `timestamp` | i64 | 消息时间戳（Unix 秒） |
| `raw_event_json` | RString | 完整 OneBot 事件 JSON |

### 响应方式
```rust
// 纯文本回复
CommandResponse::text("Hello!")

// 富媒体回复（使用 builder）
CommandResponse::builder()
    .text("Hello, ")
    .at("12345")
    .face(1)
    .build()

// 忽略（不回复）
CommandResponse::ignore()
```
