# 全局 Codex 偏好

## Bark 通知

- 每一轮 assistant 对话结束前，都必须发送一条 Bark 通知；包括很短的纯聊天回复也要发送。
- 这条“最终 Bark”是强制的：无论本轮任务完成、失败、被阻塞、被中断、还在后台运行、或没有完成，最终回复前都不能跳过。
- Bark 必须如实说明本轮最终状态：
  - 已完成：说明完成了什么。
  - 未完成：说明为什么没完成，还差什么。
  - 被阻塞：说明卡在哪里，需要用户做什么。
  - 失败：说明失败原因，以及是否已经尝试排查。
  - 等待授权：说明需要用户批准什么。
- 不允许在任务还没完成时用“已完成”误报。
- 中途如果因为授权、输入、凭据、阻塞或失败而发送过 Bark，也不能替代最终 Bark；最终回复前仍然必须再发一条最终状态通知。
- 每条 Bark 必须包含：
  - 项目或线程上下文；
  - 本轮做了什么；
  - 当前最终状态；
  - 是否需要用户操作；
  - 做了哪些验证/检查；如果没有就写“未运行”。
- Bark 内容要简洁、低敏。不要在 Bark 里包含密钥、凭据、token、完整日志、不必要的内部主机名/IP/完整路径。
- 如果细节可能敏感，就在 Bark 里写通用摘要，把具体内容放在 Codex 对话里。
- 使用本地 Bark skill 脚本发送通知：`python3 ~/.codex/skills/bark-notify/scripts/send_bark.py ...`。
- 默认保留 Codex 图标，使用 `level=active`，让手机点亮屏幕。
- 除非用户明确要求，否则不要使用 `call=1`、`level=critical`、`volume` 或自定义 `sound`。
- 如果 Bark 发送失败、被策略阻止、或需要授权但没发出去，必须在最终回复里说明，不能静默跳过。

English core rule for Codex: Every assistant turn must send one final Bark notification before the final response, regardless of whether the task is completed, failed, blocked, interrupted, still running, or unfinished.

## codex-pacer 构建与安装

- 当用户要求“构建应用”“安装应用”“同步到 macOS app / Applications”或完成一轮可运行 app 改动后需要交付本机应用时，默认执行完整 macOS `.app` 构建并同步安装。
- 推荐构建命令：`npm run tauri -- build --bundles app`。
- 构建产物路径：`src-tauri/target/release/bundle/macos/Codex Pacer.app`。
- 安装/同步目标：`/Applications/Codex Pacer.app`。
- 安装方式使用固定目标目录同步，而不是只报告构建产物位置：优先用 `rsync -a --delete` 将构建产物 `.app/` 同步到 `/Applications/Codex Pacer.app/`。
- 写入 `/Applications` 需要系统权限；执行同步命令时按 Codex 权限流程请求授权。
- 同步后至少验证 `/Applications/Codex Pacer.app` 存在，并检查 `Contents/Info.plist` 中的版本/名称等关键信息。
