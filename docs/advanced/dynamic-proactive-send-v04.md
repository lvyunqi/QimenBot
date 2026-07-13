# 动态插件 API 0.4+ 实时主动推送

QimenBot v0.1.10 在动态插件 API 0.4 中引入实时主动发送。API 0.5 完整包含该能力，并增加 Webhook Gateway。插件后台线程可显式选择 Bot，无需命令、事件或 Heartbeat 即可向宿主提交发送请求。v0.1.12 增加按稳定账号标识选择 Bot 的接口，OneBot 部署可使用 Bot QQ / `self_id`，避免插件绑定可变的部署别名。

API 0.1 至 0.3 仍然兼容。旧的 BotApi::send_group_msg、BotApi::send_private_msg 和 SendBuilder::send 会继续进入插件本地队列，并在当前 FFI 回调结束后由宿主 flush。

## 启用 API 0.4+

新建插件应显式声明 API 0.5，以同时使用实时主动发送和 Webhook。已有的 `api = "0.4"` 插件可继续使用本页列出的主动发送接口，但不能声明 `#[webhook]`。未声明 `api` 时，过程宏生成 API 0.3 插件。

~~~rust
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(id = "my-plugin", version = "0.1.0", api = "0.5")]
mod plugin {
    // commands, routes, init and shutdown hooks
}
~~~

独立插件的依赖：

~~~toml
[lib]
crate-type = ["cdylib"]

[dependencies]
abi-stable-host-api = "0.1.12"
qimen-dynamic-plugin-derive = "0.1.12"
abi_stable = "0.11"
serde_json = "1"
~~~

`0.1.12` 已发布到 crates.io；只使用旧版按实例别名 `for_bot` / `bot` 接口的插件仍可继续依赖 `0.1.11`。

## 宿主队列配置

~~~toml
[official_host.proactive_send]
queue_capacity = 256
offline_ttl_secs = 60
~~~

- queue_capacity 是每个启用 Bot 的独立有界队列容量，必须大于 0。
- offline_ttl_secs 是请求在首次网络发送前等待 Bot 上线的最长时间。
- TTL 为 0 时，Bot 离线会立即丢弃请求。
- 同一 Bot 严格保序；不同 Bot 的队列和执行器互不影响。
- 网络发送一旦开始，失败后不会自动重试，避免服务端已收到但响应丢失时重复发送。

## 配置稳定账号标识

`id` 是部署实例别名，可随部署调整；`account_id` 是插件可长期引用的稳定外部账号标识。OneBot 11/12 通常填写事件中的 `self_id`，也就是 Bot QQ 号：

~~~toml
[[bots]]
id = "qq-reverse"
account_id = "2733944636"
protocol = "onebot11"
transport = "ws-reverse"
bind = "0.0.0.0:6710"
path = "/onebot/qimenbot"
enabled = true
~~~

`account_id` 可选，因此旧配置完全兼容。多个启用 Bot 不能声明相同的 `account_id`；一个启用实例和一个禁用的备用传输实例可以声明同一账号。框架不会依赖“最近一次事件的 `self_id`”自动学习账号，因为插件可能在连接前或 `init` 阶段就发送消息。

## 最简实时发送

推荐按稳定账号发送。即使部署人员把 `id = "qq-reverse"` 改成其他名称，插件代码也不需要修改：

~~~rust
use abi_stable_host_api::{BotApi, SendEnqueueStatus};

let status = BotApi::for_account("2733944636")
    .send_group_msg("123456", "后台采集完成");

match status {
    SendEnqueueStatus::Accepted => {}
    other => eprintln!("send was not accepted: {other:?}"),
}
~~~

如果插件明确需要区分同一账号的不同部署实例，也可以继续按实例别名选择：

~~~rust
let status = BotApi::for_bot("qq-main")
    .send_group_msg("123456", "仅从指定传输实例发送");
~~~

SendBuilder 适合富消息、频道上下文和发送选项：

~~~rust
use abi_stable_host_api::SendBuilder;

let status = SendBuilder::channel("channel-100")
    .guild_id("guild-200")
    .bot_account("2733944636")
    .text("频道通知")
    .try_send();
~~~

`try_send` 必须先调用 `.bot(...)` 或 `.bot_account(...)`。未指定 Bot 会返回 `InvalidRequest`，不会选择最近事件 Bot 或任意默认 Bot。两个方法都调用时最后一次调用生效。

## 目标映射

| kind | OneBot 11 | QQ 官方 | target_id | guild_id |
|---|---|---|---|---|
| private | send_private_msg | C2C send_private_msg | user_id / openid | 不需要 |
| group | send_group_msg | 群 send_group_msg | group_id / group_openid | 不需要 |
| channel | send_guild_channel_msg | 频道 send_channel_msg | channel_id | OneBot 必填，QQ 官方可选上下文 |
| channel_private | send_guild_private_msg | 频道私信 send_dms | OneBot user_id；QQ 官方 guild_id | OneBot 必填 |

OneBot 频道私信示例：

~~~rust
let status = SendBuilder::channel_private("user-300")
    .guild_id("guild-200")
    .bot("qq-reverse")
    .text("频道私信")
    .try_send();
~~~

QQ 官方频道私信示例：

~~~rust
let status = SendBuilder::channel_private("guild-200")
    .bot("qq-official")
    .text("频道私信")
    .try_send();
~~~

## 返回状态

实时接口只确认“宿主是否接受入队”，不等待网络响应。

| 状态 | 含义 |
|---|---|
| Accepted | 请求已复制到宿主持有的 Bot 队列 |
| HostUnavailable | 插件尚未绑定 Host API，或绑定不可用 |
| InvalidRequest | 字段、目标类型或 JSON 无效 |
| BotNotFound | `bot_id` 或 `account_id` 没有匹配的 Bot |
| BotDisabled | Bot 已配置但被禁用 |
| QueueFull | 对应 Bot 的有界队列已满 |
| HostShuttingDown | Runtime 正在关闭，不再接受请求 |

## 后台线程与安全卸载

API 0.4/0.5 的 Host API 都在插件 init 前完成绑定，因此 init 启动的线程可以立即发送。插件必须在 shutdown 中通知线程停止并 join；只有 shutdown 完成后，宿主才会 unbind Host API 并卸载动态库。

~~~rust
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use abi_stable_host_api::{BotApi, PluginInitConfig, PluginInitResult};

static STOP: AtomicBool = AtomicBool::new(false);
static WORKER: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

#[init]
fn init(_config: PluginInitConfig) -> PluginInitResult {
    STOP.store(false, Ordering::Release);
    let handle = thread::spawn(|| {
        while !STOP.load(Ordering::Acquire) {
            let _ = BotApi::for_account("2733944636")
                .send_group_msg("123456", "实时后台通知");
            thread::park_timeout(Duration::from_secs(60));
        }
    });
    *WORKER.lock().unwrap() = Some(handle);
    PluginInitResult::ok()
}

#[shutdown]
fn shutdown() {
    STOP.store(true, Ordering::Release);
    if let Some(handle) = WORKER.lock().unwrap().take() {
        handle.thread().unpark();
        let _ = handle.join();
    }
}
~~~

如果 shutdown panic 或 Host API unbind 失败，宿主会保留动态库和绑定，不冒险释放仍可能被线程使用的代码或上下文。

## FFI 生命周期

- 插件导出 qimen_plugin_bind_host_api_v1 和 qimen_plugin_unbind_host_api_v1。
- 宿主回调进入后会复制请求中的全部字符串和 JSON，回调返回后不再引用插件内存。
- 插件发送持有绑定读锁直到宿主回调返回；unbind 持有写锁并等待在途发送完成。
- Host API 只返回稳定整数状态码，不跨动态库边界传递需要另一侧释放的错误字符串。
- Runtime 关闭时先拒绝新请求，允许当前网络发送完成，丢弃未开始请求，然后执行插件 shutdown、unbind 和动态库卸载。

## 示例工程

仓库中的 plugins/qimen-dynamic-plugin-example 展示了：

- API 0.5 声明方式及 API 0.4 主动发送兼容语义；
- init 阶段启动后台实时发送线程；
- shutdown 停止并 join 线程；
- BotApi::for_bot / BotApi::for_account 和 SendBuilder::bot(...) / bot_account(...).try_send()；
- 私聊、群聊、频道和频道私信目标；
- 旧 callback-flush 发送路径继续兼容。
