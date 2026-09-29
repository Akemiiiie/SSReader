# SSReader 2026-09-29
Saki for Soyuzuriha book reader

> There is no Frigate like a Book  
> To take us Lands away  
> —— Emily Dickinson, [《There is no Frigate like a Book》](https://www.emilydickinsonmuseum.org/roomitem/no-frigate-like-a-book/)

书像一艘小船，能载我们去往远方。送给 Soyuzuriha 的一份小礼物：希望你能在这里收好喜欢的书，慢慢读懂陌生的句子，也把偶然遇见的词留给未来的自己。

学习不必是一场赶路。每读懂一页、学会一个词，都是一次温柔的小小发现。愿书页带你看见更宽广的世界，也愿你每天都有值得开心的事。

Soyuzuriha，天天开心

## 项目说明

基于 **Tauri 2 + Preact + TypeScript** 的桌面阅读器界面原型。网页预览可直接运行；界面与数据在无网络时仍可使用。默认书籍、释义与例句是演示数据。

## 运行

```powershell
cd demo/english-reader-ui
npm install
npm run dev
```

打开 <http://127.0.0.1:1420/>。可用 `npm run build` 检查前端构建。有 Rust 工具链和 Tauri 桌面依赖时，可运行 `npm run tauri:dev`；本原型的独立置顶词卡只在 Tauri 中成为系统级窗口，网页预览中为页面内置顶。

## 可体验的交互

- 左侧栏可收起；头像打开演示登录弹窗；书架和按书分组的收藏词库可切换。词库搜索支持输入后回车精确定位。
- 底部切换阅读与学习模式。学习卡片可翻面并记录本次“记住 / 再练习”的数量。
- 点击正文单词会更新右侧释义和最近查看；选中单词或短句后右键打开收藏浮窗。浮窗可拖动、缩放、收藏；近似收藏可悬停展开，点击后在释义面板显示原词释义。
- 右侧面板可收起、拖动调宽；最近六词和释义各自滚动；搜索框只检索当前书的收藏。朗读按钮使用系统的 Web Speech 语音能力。
- 书架支持导入本地 TXT 供界面体验。书籍、收藏、最近查看及演示用户保存在本机 `localStorage`。

## 原型边界

登录只模拟本机状态；PDF/EPUB 解析、持久化数据库、真实词典与翻译、OCR、云端分享均未接入。它们分别属于设计文档中后续的阅读器完善、翻译释义、云端分享阶段。导入 TXT 目前仅用于验证阅读界面和本地优先交互，不建议存放大体积书籍或重要资料。
