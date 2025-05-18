# Cozy Lapce

**Cozy Lapce** 是在 [Lapce](https://github.com/lapce/lapce) 的基础上 fork 的轻量化衍生版本，旨在继续维护和改进这款优秀的编辑器。

---

## 与 Lapce 的差异

- 因为作者不会用 Modal Editing，因此原有Modal Editing 模式没支持
- 只验证了windows/linux平台的功能，mac没有验证
- 原有的样式主题，没有维护，甚至可能有影响到。当前主要用`jb-light`
- 界面增加了很多鼠标可操作的功能，编辑界面增加了折叠功能
- 底层进行了大量的优化：移除多线程、取消大数据的复制、细化配置等等
- rust开发推荐插件：Rust(by dzhou121)，Crates。其他语言开发未验证
- 调试功能 **仅支持在 Windows 系统下使用**: 通过安装插件lldb-win，可以实现debug功能
---

## 使用说明文档

想了解如何充分使用 Cozy Lapce 的各项功能，  
请查看 👉 [**使用说明文档**](./docs/USAGE.md)。

> 包含运行配置、快捷键、调试设置、主题定制等实用技巧。

想了解如何快速开发 Cozy Lapce，  
请查看 👉 [**开发说明文档**](./docs/DEVELOPING.md)。

## 未来计划

- 🐞 调试功能：计划引入调试器支持（如 DAP 协议、LLDB 集成等）

- 🤖 AI 能力探索：尝试引入 AI 辅助功能，如智能补全、对话式编程、代码改写等

## 参与贡献 Cozy Lapce

Cozy Lapce 最初只是为了让作者“每天都愿意打开”的编辑器而生，但我们也非常欢迎社区的力量，让它变得更好！

你可以通过以下方式参与进来：

🧑‍💻 跨平台支持 – 帮忙测试或移植到 Linux / macOS

🎨 UI 和主题设计 – 优化界面，制作更好看的主题

🐞 修复 Bug / 改进功能 – 提交 PR 或反馈 Issue

🧪 调试 / AI 相关功能 – 一起实现调试器集成或 AI 编程支持

💡 有更好的点子？ 欢迎分享你的想法、建议或架构优化方向！

👉 欢迎前往 Issue 区、Discussions，或直接提 PR！

## 🙏 致谢

**本项目基于 [Lapce](https://github.com/lapce/lapce) 开发，感谢原作者及开源社区的卓越贡献。**