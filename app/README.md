# SSReader 桌面应用

里程碑 1 的 Windows 本地阅读器。`demo/english-reader-ui` 仅为交互原型，演示数据不会进入正式数据库。

## 本地开发与构建

```powershell
cd app
npm install
npm run tauri:dev
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run test:pdf
npm run tauri:build
npm run tauri:build:offline
```

`tauri:dev` 和 `tauri:build` 会从当前 `PATH`、`CARGO_HOME/bin`、用户目录的 `.cargo/bin` 查找 `cargo`。需要 Rust MSVC 工具链及 Tauri Windows 构建依赖。单独运行 `npm run dev` 只能查看前端；浏览器没有 Tauri 本地命令。

常规构建输出 NSIS 和 MSI，安装时如需补装 WebView2，会使用网络下载。`tauri:build:offline` 另外输出包含 WebView2 离线安装程序的 NSIS 包到 `src-tauri/target/release/bundle/offline`。离线构建也会覆盖 `bundle/nsis` 下的同名常规包，发布时应从 `offline` 目录取离线版。

若离线打包在获取微软 WebView2 安装程序时出现 `io: unexpected end of file`，先检查本机代理能否访问微软下载地址。本机代理异常时，在当前 PowerShell 会话临时清除 `HTTP_PROXY` 与 `HTTPS_PROXY` 后重试即可；该操作不改系统代理设置。

## 当前功能

- 通过系统文件对话框导入 TXT（最多 32 MiB）和 PDF（最多 64 MiB）。文件复制进应用受管目录；重复内容打开已有书。可改书名和作者、归档、彻底删除及导出受管原文件。
- TXT 支持 UTF-8、带 BOM 的 UTF-16 和 GB18030；导入前可预览并手动选编码；按阅读区域的完整文本行动态分页，并以段落和码点位置续读。
- PDF.js Worker 按页提取文字与渲染原页。文字页可切换重排和原页；无文字扫描页保留原页并显示提示，不执行 OCR。
- 阅读位置、字号、行距、字体颜色、背景、侧栏状态、翻页快捷键、PDF 缩放和每书 PDF 模式保存在本地 SQLite。
- 本地备份包含数据库一致性快照和受管原文件，恢复前会预检版本、大小与 SHA-256。备份未加密，应保存到安全位置。
- 程序启动后默认隐藏主窗口，可在设置中选择长按 `.` 临时显示或双击 `.` 切换显示状态；也可单击托盘图标打开。`Ctrl+B` 展开或收起左侧菜单，阅读时默认使用左右方向键翻页，并可在设置中自定义。窄窗口会自动收起菜单；点击菜单外区域也会收起菜单。关闭窗口后程序继续在托盘运行，选择“退出程序”才会完全退出。

## 代码位置

- `src/app`：应用壳与导航；`src/features`：书架、阅读和设置页面。
- `src/shared/contracts`：前后端序列化契约；`src/shared/api`：Tauri 命令调用。
- `src-tauri/src/commands`：受限 IPC；`src-tauri/src/services`：导入、阅读、备份业务；`src-tauri/src/storage`：SQLite 与受管文件。
- `src-tauri/migrations`：数据库迁移；`tests/fixtures`：TXT/PDF 验收样本。

原始书籍只从已导入的 `bookId` 读取，前端不会获得任意磁盘路径访问权限。应用数据保存在 Tauri 的用户应用数据目录；删除安装包或卸载应用前请自行制作备份。
