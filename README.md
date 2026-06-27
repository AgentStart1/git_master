# Git Master

一个基于 [GPUI](https://www.gpui.rs/) 构建的桌面应用，用于一次性查看某个父目录下所有 Git 仓库的状态。打开一个工作区目录，即可在左侧列表中浏览每个仓库的脏/净状态、当前分支以及与上游的 ahead/behind 数量，并在右侧面板查看仓库详情和提交历史。

## 功能

- **批量扫描**：选择一个父目录，自动发现其下所有的 Git 仓库并按名称排序。
- **状态总览**：每个仓库显示
  - 当前分支
  - 工作区是否干净（`✓` 绿色 / `●` 红色）
  - 相对上游的 `↑ahead ↓behind` 数量
- **详情面板（Info 标签）**：仓库路径、当前分支、远端 URL，以及文件状态统计（新增 / 修改 / 删除 / 重命名 / 冲突）。
- **提交历史（Git Log 标签）**：最近 200 条提交，包含短哈希、提交信息、作者和时间。
- **非阻塞 UI**：所有 Git I/O 都在后台线程执行，扫描与读取详情时界面不会卡顿；过期的扫描/详情结果会被自动丢弃。

## 技术栈

- [`gpui`](https://crates.io/crates/gpui) — GPU 加速的 Rust UI 框架
- [`git2`](https://crates.io/crates/git2) — libgit2 的 Rust 绑定
- [`chrono`](https://crates.io/crates/chrono) — 提交时间格式化

## 构建与运行

需要 Rust 工具链（edition 2024，建议使用较新的稳定版）。

```bash
# 调试运行
cargo run

# 构建发布版本
cargo build --release
./target/release/git_master
```

启动后点击右上角的 **Open Directory**，选择一个包含多个 Git 仓库的父目录即可。

## 项目结构

```
src/
├── main.rs              # 入口，创建窗口
├── app_state.rs         # 应用状态与顶层 Render
├── git_ops.rs           # 基于 git2 的扫描/详情/提交日志读取
├── models.rs            # RepoInfo / RepoDetail / LogEntry 等数据结构
└── ui/
    ├── mod.rs
    ├── top_bar.rs       # 顶部目录选择栏
    ├── repo_list.rs     # 左侧仓库列表
    ├── detail_panel.rs  # 右侧详情 / 提交历史面板
    └── theme.rs         # 配色常量
```

## 说明

- 应用仅做只读展示，不会修改任何仓库。
- 扫描只检查父目录的直接子目录，不会递归进入子目录寻找仓库。
