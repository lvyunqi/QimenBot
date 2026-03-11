<div align="center">

<img src="logo.jpg" width="200" alt="QimenBot Logo">

# QimenBot

_✨ Rust で構築された高性能マルチプロトコル Bot フレームワーク ✨_

[![License](https://img.shields.io/github/license/lvyunqi/QimenBot?style=flat-square)](https://github.com/lvyunqi/QimenBot/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![OneBot 11](https://img.shields.io/badge/OneBot-11-black?style=flat-square)](https://github.com/botuniverse/onebot-11)

[简体中文](README.md) | [English](README_EN.md) | **日本語**

</div>

---

QimenBot は Rust で書かれたモジュラーで拡張可能なチャットボットフレームワークです。**再利用可能なフレームワーク層**と**リファレンス Host 実装**を分離しており、公式 Host をそのままデプロイすることも、フレームワーク層をベースに独自の Bot プラットフォームを構築することもできます。

## 特徴

- **マルチプロトコル** — OneBot 11（本番対応）、OneBot 12 / Satori（拡張ポイント予約済み）
- **複数のトランスポート** — 正方向 WebSocket、逆方向 WebSocket、HTTP API、HTTP POST
- **宣言的プラグイン開発** — `#[module]` / `#[commands]` / `#[notice]` マクロで約7行でプラグインを実装
- **インターセプター チェーン** — `pre_handle` / `after_completion` でブラックリスト、権限チェック、ショートカット書き換えなど
- **柔軟なコマンドシステム** — エイリアス、使用例、カテゴリ、権限レベル、メッセージフィルター、`/help` 自動生成
- **システムイベントルーティング** — グループ通知、フレンドリクエスト、メタイベントをアトリビュートルーティングで配信
- **ランタイム保護** — トークンバケット レート制限、メッセージ重複排除、グループイベントフィルタリング、プラグイン ACL
- **ダイナミックプラグイン** — `dlopen` による ABI 安定な共有ライブラリのランタイムロード
- **リクエスト自動化** — ホワイトリスト/ブラックリスト/キーワードフィルターによるフレンド・グループリクエストの自動承認/拒否
- **充実した OneBot 11 API** — メッセージング、グループ管理、ファイル、ギルド、リアクションなど 40+ の操作をラップ

## アーキテクチャ

```
┌─────────────────────────────────────────────────────┐
│               アプリケーション層 (apps/)                │
│         qimenbotd (デーモン)     qimenctl (CLI)        │
├─────────────────────────────────────────────────────┤
│                Official Host 層                       │
│    qimen-official-host · qimen-config · observability  │
├─────────────────────────────────────────────────────┤
│              フレームワーク層（再利用可能）                │
│   runtime · plugin-api · plugin-host · message         │
│   protocol-core · transport-core · command-registry    │
├─────────────────────────────────────────────────────┤
│                アダプター & トランスポート                │
│   adapter-onebot11 · transport-ws · transport-http     │
├─────────────────────────────────────────────────────┤
│                   組み込みモジュール                     │
│   mod-command · mod-admin · mod-scheduler · mod-bridge  │
└─────────────────────────────────────────────────────┘
```

## クイックスタート

### 必要環境

- Rust 1.89+（2024 Edition）
- OneBot 11 実装（例：[Lagrange.OneBot](https://github.com/LagrangeDev/Lagrange.Core)、[NapCat](https://github.com/NapNeko/NapCatQQ) など）

### ビルド & 実行

```bash
git clone https://github.com/lvyunqi/QimenBot.git
cd QimenBot

# 設定を編集（endpoint、owners などを変更）
vim config/base.toml

# 実行
cargo run
```

### 設定例

```toml
[runtime]
env = "dev"

[official_host]
builtin_modules = ["command", "admin", "scheduler"]
plugin_modules  = ["example-plugin"]

[[bots]]
id        = "qq-main"
protocol  = "onebot11"
transport = "ws-forward"
endpoint  = "ws://127.0.0.1:3001"
enabled   = true
owners    = ["123456"]

# ポーク自動応答
auto_reply_poke_enabled = true
auto_reply_poke_message = "つつかないで！"
```

環境変数の展開をサポート：`access_token = "${QQ_TOKEN}"`

## プラグイン開発

QimenBot はプロシージャルマクロでボイラープレートを最小化します：

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin", version = "0.1.0")]
#[commands]
impl MyPlugin {
    /// コマンド名はメソッド名から自動推定（my_cmd → "my-cmd"）
    #[command("Say hello")]
    async fn hello(&self) -> &str {
        "Hello from QimenBot!"
    }

    /// パラメータインジェクション対応：args, ctx, または両方
    #[command("Echo message", aliases = ["e"])]
    async fn echo(&self, args: Vec<String>) -> Message {
        Message::text(args.join(" "))
    }

    /// システムイベントルーティング
    #[notice(GroupPoke, PrivatePoke)]
    async fn on_poke(&self) -> Message {
        Message::text("つつかないで！")
    }
}
```

### 戻り値の自動ラッピング

メソッドは以下の型を返すことができ、フレームワークが自動的にシグナルに変換します：

| 戻り値の型 | 動作 |
|-----------|------|
| `Message` | そのメッセージで応答 |
| `String` / `&str` | テキストメッセージで応答 |
| `CommandPluginSignal` | 完全制御（Reply / Continue / Block / Ignore） |
| `Result<T, E>` | Ok → 通常処理、Err → `"Error: {e}"` で応答 |

### インターセプター

イベントがプラグインに到達する前後に処理を実行：

```rust
pub struct MyInterceptor;

#[async_trait]
impl MessageEventInterceptor for MyInterceptor {
    async fn pre_handle(&self, _bot_id: &str, event: &NormalizedEvent) -> bool {
        // false を返すとイベントをブロック、true で通過
        true
    }

    async fn after_completion(&self, _bot_id: &str, _event: &NormalizedEvent) {
        // すべてのプラグインの処理後に実行（逆順）
    }
}

#[module(id = "my-plugin", interceptors = [MyInterceptor])]
#[commands]
impl MyPlugin { /* ... */ }
```

### イベント処理パイプライン

```
イベント受信
  → システムイベント配信（notice / request / meta）
  → メッセージ重複排除
  → グループイベントフィルタリング
  → トークンバケット レート制限
  → インターセプターチェーン pre_handle
  → 権限解決
  → コマンドマッチング & プラグイン配信
  → インターセプターチェーン after_completion
```

## 組み込みコマンド

| コマンド | 説明 |
|---------|------|
| `ping` / `/ping` | pong を返す |
| `echo <text>` / `/echo <text>` | テキストをエコー |
| `status` / `/status` | ランタイムステータス |
| `help` / `/help` | 自動生成されたヘルプ |
| `plugins` / `/plugins` | ロード済みプラグイン一覧 |

トリガー方法：ダイレクトメッセージ、`/プレフィックス`、`@bot メンション`、リプライベース。

## プロジェクト構成

```
QimenBot/
├── apps/
│   ├── qimenbotd/           # Bot デーモン
│   └── qimenctl/            # CLI 管理ツール
├── crates/
│   ├── qimen-plugin-api/    # プラグイン API
│   ├── qimen-plugin-derive/ # プロシージャルマクロ
│   ├── qimen-runtime/       # イベントディスパッチ、インターセプター
│   ├── qimen-message/       # メッセージモデル
│   ├── qimen-adapter-onebot11/ # OneBot 11 アダプター
│   ├── qimen-transport-ws/  # WebSocket トランスポート
│   ├── qimen-transport-http/# HTTP トランスポート
│   ├── qimen-mod-command/   # コマンド検出 & マッチング
│   ├── qimen-mod-admin/     # 権限管理
│   ├── qimen-mod-scheduler/ # Cron タスクスケジューリング
│   └── ...                  # その他のコア crate
├── plugins/
│   └── qimen-plugin-example/# サンプルプラグイン
└── config/
    ├── base.toml            # メイン設定
    ├── dev.toml             # 開発環境オーバーライド
    └── prod.toml            # 本番環境オーバーライド
```

## プロトコルサポート

| プロトコル | ステータス | トランスポート |
|-----------|----------|--------------|
| OneBot 11 | ✅ 本番対応 | WS 正方向、WS 逆方向、HTTP API、HTTP POST |
| OneBot 12 | 🔲 計画中 | — |
| Satori | 🔲 計画中 | — |

## 謝辞

QimenBot の設計は以下の優れたプロジェクトを参考にしています：

- [Shiro](https://github.com/MisakaTAT/Shiro) — Java ベースの OneBot フレームワーク。インターセプターとプラグインモデルのインスピレーション
- [Kovi](https://github.com/ThriceCola/Kovi) — Rust OneBot フレームワーク。クリーンな API デザインのリファレンス

## ライセンス

[MIT](LICENSE)
