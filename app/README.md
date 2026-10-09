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

- 打开书籍后，点击标题栏「悬浮阅读」切换为紧凑窗口。正文继续使用真实 TXT 分页和 PDF 阅读器；拖动顶部移动窗口，拖动四边或四角调整大小，悬浮尺寸和位置会记住，返回书架时恢复原窗口大小。
- 顶部移入鼠标显示外观、置顶、隐藏和返回按钮。外观面板提供纯文字／淡底／纸面、自定义文字颜色、字号、三种字体和行距。透明度只影响背景，正文保持清晰；设置保存到原有 SQLite，旧设置自动使用系统字体。窗口背后显示实际桌面，没有内置 Excel 背景。
- `Ctrl+Alt+R` 全局隐藏或显示主窗口；原有句点键和托盘入口继续可用。悬浮模式按 `Ctrl+B` 或返回按钮回到书架，可继续导入书籍、打开完整设置与备份。翻译、朗读和发音练习沿用原来的独立释义窗口。

- 通过系统文件对话框导入 TXT（最多 32 MiB）和 PDF（最多 64 MiB）。文件复制进应用受管目录；重复内容打开已有书。可改书名和作者、归档、彻底删除及导出受管原文件。
- TXT 支持 UTF-8、带 BOM 的 UTF-16 和 GB18030；导入前可预览并手动选编码；按阅读区域的完整文本行动态分页，并以段落和码点位置续读。
- PDF.js Worker 按页提取文字与渲染原页。文字页可切换重排和原页；无文字扫描页保留原页并显示提示，不执行 OCR。
- 阅读位置、字号、行距、字体颜色、背景、侧栏状态、翻页快捷键、PDF 缩放和每书 PDF 模式保存在本地 SQLite。
- 本地备份包含数据库一致性快照和受管原文件，恢复前会预检版本、大小与 SHA-256。备份未加密，应保存到安全位置。
- 程序启动后默认隐藏主窗口，可在设置中选择长按 `.` 临时显示或双击 `.` 切换显示状态；也可单击托盘图标打开。`Ctrl+B` 展开或收起左侧菜单，阅读时默认使用左右方向键翻页，并可在设置中自定义。窄窗口会自动收起菜单；点击菜单外区域也会收起菜单。关闭窗口后程序继续在托盘运行，选择“退出程序”才会完全退出。

## 里程碑 2 翻译框架

现已提供 `analyze_selection` Tauri 命令、前端调用类型，以及按书籍、所选文本、原文句子与相邻句、模型／提示词／词典版本区分的本地 SQLite 缓存。删除书籍会级联删除对应缓存。选区长度限制为单词 80、句子 500、段落 2000 字符；失败结果不会缓存，也不会自动重试。翻译提供者接口在 Rust 端，前端不传密钥。

已接入 MiMo `mimo-v2.6-pro` 翻译、`mimo-v2.5-tts` 单词朗读和 `mimo-v2.5-asr` 英文录音转写。选中阅读区文本后可点击“释义与翻译”；英文单词释义后可朗读或录音练习。录音最多 8 秒，仅在内存中转换为 WAV 后发送给 MiMo，不保存本地文件。发音反馈先比较 ASR 转写与目标词，再尝试由 MiMo-V2.6-Pro 听音给出定性建议；后者失败时保留转写反馈。**这不是经过校准的音素、重音或口音评分**。

释义卡片使用独立无边框悬浮窗，可以显示在主窗口外；选中英文单词后自动请求释义，选中句段后可手动请求。重新选词会更新该窗口，按 `Esc`、点击阅读窗口其他位置或关闭/隐藏主窗口会关闭悬浮窗。

单词卡依次展示常用标准义项（含词性）、结合原句的语境释义、3 个双语例句。应用随机抽样义项：有至少 3 个时选取 3 个不同义项，不足 3 个时允许重复义项，但生成不同句子。点击例句区「换一组」只刷新例句并更新本地缓存，不重新请求标准释义；失败时保留旧例句。义项和例句由 AI 生成，尚未接入外部词典。新版提示词使用独立缓存版本，旧的简短单词结果不会被误用。

三款模型通过 `https://token-plan-cn.xiaomimimo.com/v1` 调用。在「设置与备份 → AI 功能」填写 MiMo Token Plan API Key 并保存，立即生效，可测试连接、更换或移除密钥。密钥通过 Windows DPAPI 加密保存在本机，不进入书籍备份，不回传至设置页；更换电脑或 Windows 用户时需重新填写。首次录音需要允许 WebView2 和 Windows 的麦克风权限。词典和更精细的发音评估尚未接入。

## 代码位置

- `src/app`：应用壳与导航；`src/features`：书架、阅读和设置页面。
- `src/shared/contracts`：前后端序列化契约；`src/shared/api`：Tauri 命令调用。
- `src-tauri/src/commands`：受限 IPC；`src-tauri/src/services`：导入、阅读、备份业务；`src-tauri/src/storage`：SQLite 与受管文件。
- `src-tauri/migrations`：数据库迁移；`tests/fixtures`：TXT/PDF 验收样本。

原始书籍只从已导入的 `bookId` 读取，前端不会获得任意磁盘路径访问权限。应用数据保存在 Tauri 的用户应用数据目录；删除安装包或卸载应用前请自行制作备份。
