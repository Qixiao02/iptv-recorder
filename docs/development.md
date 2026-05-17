# 开发指南

## 开发环境搭建

### 环境要求

- **Rust**: 1.75+ (推荐使用最新稳定版)
- **Git**: 版本控制
- **SQLite**: 3.0+ (通常系统自带)
- **N_m3u8DL-RE**: HLS 流下载工具（测试时需要）

### 安装 Rust

```bash
# 使用 rustup 安装
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 或使用系统包管理器
# Windows (winget)
winget install Rustlang.Rust.MSVC

# macOS (Homebrew)
brew install rust

# Linux (apt)
sudo apt install rustc cargo
```

### 克隆项目

```bash
git clone https://github.com/yourusername/iptv-recorder.git
cd iptv-recorder
```

### 安装依赖

```bash
# 下载并编译依赖
cargo fetch

# 验证编译
cargo check
```

### 开发工具

推荐安装的 VS Code 扩展：

| 扩展 | 用途 |
|------|------|
| rust-analyzer | Rust 语言服务器 |
| CodeLLDB | 调试支持 |
| Even Better TOML | TOML 文件支持 |
| Error Lens | 内联错误显示 |

## 项目结构

```
iptv-recorder/
├── src/
│   ├── main.rs              # 程序入口
│   ├── config.rs            # 配置管理
│   │
│   ├── api/                 # API 层
│   │   ├── mod.rs
│   │   ├── router.rs        # 路由定义
│   │   ├── handlers.rs      # HTTP 处理器
│   │   └── websocket.rs     # WebSocket 处理
│   │
│   ├── core/                # 核心基础设施
│   │   ├── mod.rs
│   │   ├── database.rs      # 数据库
│   │   ├── event.rs         # 事件总线
│   │   └── process.rs       # 进程管理
│   │
│   ├── services/            # 业务服务层
│   │   ├── mod.rs
│   │   ├── channel.rs       # 频道服务
│   │   ├── schedule.rs      # 计划服务
│   │   └── recording.rs     # 录制服务
│   │
│   └── models/              # 数据模型
│       └── mod.rs
│
├── tests/                   # 集成测试
│   ├── api_tests.rs
│   └── service_tests.rs
│
├── config/                  # 配置示例
│   └── default.toml
│
├── docs/                    # 项目文档
│
└── Cargo.toml               # 依赖配置
```

## 常用命令

### 构建

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release

# 检查编译（不生成二进制）
cargo check
```

### 运行

```bash
# 直接运行
cargo run

# 带参数运行
cargo run -- --config config/custom.toml

# 设置环境变量运行
IPTV__SERVER__PORT=8080 cargo run
```

### 开发脚本

统一使用项目脚本管理前后端：

```bash
# 启动前后端
./scripts/dev.sh start

# 仅启动后端或前端
./scripts/dev.sh start backend
./scripts/dev.sh start frontend

# 停止
./scripts/dev.sh stop
./scripts/dev.sh stop backend

# 重启
./scripts/dev.sh restart
./scripts/dev.sh restart frontend

# 查看状态
./scripts/dev.sh status

# 查看日志
./scripts/dev.sh logs backend
./scripts/dev.sh logs frontend
./scripts/dev.sh logs all --follow
```

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_channel_create

# 显示测试输出
cargo test -- --nocapture

# 运行文档测试
cargo test --doc
```

### 代码检查

```bash
# Clippy lint 检查
cargo clippy

# 格式化代码
cargo fmt

# 检查未使用的依赖
cargo +nightly udeps
```

### 文档

```bash
# 生成并打开文档
cargo doc --open

# 检查文档链接
cargo doc --document-private-items
```

## 代码规范

### 命名规范

| 类型 | 规范 | 示例 |
|------|------|------|
| 模块 | snake_case | `mod channel_service` |
| 结构体 | PascalCase | `struct ChannelService` |
| 枚举 | PascalCase | `enum TaskStatus` |
| 函数 | snake_case | `fn create_channel()` |
| 常量 | SCREAMING_SNAKE_CASE | `const MAX_RETRY: i32 = 3` |
| 宏 | snake_case! | `println!()` |

### 文档注释

```rust
/// 创建新的频道
///
/// # 参数
///
/// * `name` - 频道名称
/// * `url` - 频道流地址
///
/// # 返回
///
/// 返回创建的频道对象
///
/// # 示例
///
/// ```no_run
/// use iptv_recorder::services::ChannelService;
/// let channel = service.create("CCTV-1", "http://...").await?;
/// ```
pub async fn create(&self, name: &str, url: &str) -> Result<Channel> {
    // ...
}
```

### 错误处理

使用 `anyhow::Result` 作为应用层错误类型：

```rust
use anyhow::{Result, Context};

pub async fn do_something() -> Result<()> {
    let content = tokio::fs::read_to_string("config.toml")
        .await
        .context("Failed to read config file")?;
    Ok(())
}
```

### 日志规范

```rust
use tracing::{info, warn, error, debug};

// 不同级别日志
debug!("Detailed debugging info: {}", value);
info!("Application started successfully");
warn!("Configuration file not found, using defaults");
error!("Failed to connect to database: {:?}", err);
```

## 测试

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_name_validation() {
        assert!(is_valid_name("CCTV-1"));
        assert!(!is_valid_name(""));
    }

    #[tokio::test]
    async fn test_channel_create() {
        let service = ChannelService::new(mock_db());
        let channel = service.create(req).await.unwrap();
        assert_eq!(channel.name, "Test Channel");
    }
}
```

### 集成测试

```rust
// tests/api_tests.rs
use axum::http::StatusCode;
use reqwest::Client;

#[tokio::test]
async fn test_create_channel_api() {
    let app = create_test_app().await;
    let client = Client::new();

    let response = client
        .post("http://localhost:3000/api/channels")
        .json(&serde_json::json!({
            "name": "Test Channel",
            "url": "http://example.com/stream.m3u8"
        }))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);
}
```

## 调试

### VS Code 配置

创建 `.vscode/launch.json`：

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug iptv-recorder",
      "cargo": {
        "args": ["build", "--bin=iptv-recorder"],
        "filter": {
          "name": "iptv-recorder",
          "kind": "bin"
        }
      },
      "args": [],
      "cwd": "${workspaceFolder}",
      "env": {
        "RUST_LOG": "debug"
      }
    }
  ]
}
```

### 日志调试

```bash
# 设置日志级别
RUST_LOG=debug cargo run

# 特定模块日志
RUST_LOG=iptv_recorder::services=trace cargo run

# JSON 格式日志（便于解析）
RUST_LOG=info RUST_FORMAT=json cargo run
```

## 提交规范

### Commit Message 格式

```
<type>(<scope>): <subject>

<body>

<footer>
```

**类型**:
- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式
- `refactor`: 重构
- `test`: 测试相关
- `chore`: 构建/工具

**示例**:

```
feat(services): add channel health check

- Implement periodic health checking for channels
- Add HEAD request validation
- Update channel status based on check results

Closes #123
```

## 发布流程

### 版本号

遵循语义化版本 `MAJOR.MINOR.PATCH`：

```toml
[package]
name = "iptv-recorder"
version = "0.1.0"  # 主版本.次版本.补丁版本
```

### 发布步骤

```bash
# 1. 更新版本号
cargo edit package version

# 2. 更新 CHANGELOG.md
# 3. 创建 git tag
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0

# 4. 构建 release
cargo build --release

# 5. 发布到 crates.io（可选）
cargo publish
```

## 性能分析

### 火焰图

```bash
# 安装 flamegraph
cargo install flamegraph

# 生成火焰图
cargo flamegraph --bin iptv-recorder

# 查看结果
open flamegraph.svg
```

### 基准测试

```bash
# 安装 cargo-criterion
cargo install cargo-criterion

# 运行基准测试
cargo criterion
```

## 常见问题

### Q: 编译缓慢

```bash
# 使用 sccache 缓存编译
cargo install sccache
export RUSTC_WRAPPER=sccache
```

### Q: 测试数据库污染

```bash
# 使用临时数据库进行测试
export IPTV__DATABASE__PATH="/tmp/test-XXXXXX.db"
cargo test
```

### Q: 依赖冲突

```bash
# 更新依赖
cargo update

# 查看依赖树
cargo tree

# 检查可用的更新
cargo outdated
```

## 贡献指南

1. Fork 项目
2. 创建功能分支 (`git checkout -b feat/amazing-feature`)
3. 提交更改 (`git commit -m 'feat: add amazing feature'`)
4. 推送分支 (`git push origin feat/amazing-feature`)
5. 创建 Pull Request

### PR 要求

- 通过所有测试 (`cargo test`)
- 通过 clippy 检查 (`cargo clippy`)
- 更新相关文档
- 添加测试用例

## 资源链接

- [Rust 官方文档](https://doc.rust-lang.org/)
- [Axum 文档](https://docs.rs/axum/)
- [Tokio 教程](https://tokio.rs/tokio/tutorial)
- [SQLx 文档](https://docs.rs/sqlx/)
- [N_m3u8DL-RE](https://github.com/nilaoda/N_m3u8DL-RE)
