# Agnes Chat 踩坑日志

> Tauri 2 + React + Agnes AI API 开发过程中遇到的坑与解决记录。
> 格式：**现象 → 原因 → 解决**

---

## 1. Tauri CLI 没有 `check` 子命令

**现象**：`cargo-tauri check` 报 `error: unrecognized subcommand 'check'`。

**原因**：`tauri-cli`（`cargo-tauri`）只有 `dev` / `build` / `bundle` 等子命令，没有 `check`。

**解决**：在 `src-tauri/` 目录直接跑 `cargo check` 检查 Rust 代码。

---

## 2. Tauri 2 命令参数 `State<AppState>` 缺少生命周期标注

**现象**：编译报 `error[E0726]: implicit elided lifetime not allowed here`。

**原因**：Tauri 2.11+ 的 `tauri::State` 需要显式生命周期 `State<'_, AppState>`。

**解决**：所有命令签名改为 `state: State<'_, AppState>`。

---

## 3. `ApiMessage` 只派生 `Deserialize`，序列化请求时编译失败

**现象**：`ChatRequest { messages: Vec<&ApiMessage> }` 报 `the trait bound ApiMessage: serde::Serialize is not satisfied`。

**原因**：请求体需要 `Serialize`，但模型只实现了 `Deserialize`。

**解决**：`#[derive(Debug, Clone, Deserialize, Serialize)]`。

---

## 4. 流式读取需要 `StreamExt`，且弃用 `tokio` 直接依赖

**现象**：`stream.next().await` 报 `no method named next`，且 `while let Some(x) = stream.next()` 出现奇怪的 `[u8]` Sized 错误。

**原因**：`reqwest::Response::bytes_stream()` 返回的 Stream 需要 `futures_util::StreamExt` trait 在作用域内。

**解决**：`Cargo.toml` 添加 `futures-util = "0.3"`，`use futures_util::StreamExt;`。同时移除未使用的 `tokio` 直接依赖。

---

## 5. Tauri 打包 icon 要求 RGBA PNG，且需用 CLI 生成全平台图标

**现象**：`cargo check` 保宏 `tauri::generate_context!` 报 `icon xxx.png is not RGBA`；`cargo-tauri build` 报 `Failed to create app icon: No matching IconType`。

**原因**：Tauri 2 的 icon 必须是 RGBA PNG；macOS 打包还需要 `icon.icns` / `icon.ico`。

**解决**：
1. 生成带 alpha 通道的 PNG（注意 PNG scanline 首字节必须是 filter type `0x00`）。
2. `cargo-tauri icon icon_512x512.png -o src-tauri/icons` 由单一源图生成 `.icns`、`.ico`、多尺寸 PNG。
3. `tauri.conf.json` 的 `bundle.icon` 引用生成的文件。

---

## 6. 用 Python 手写 PNG 时 filter 字节错误

**现象**：sips 提示 "not a valid file"；`cargo-tauri icon` 报 `Unknown filter method 255`。

**原因**：每行 scanline 首字节应为 filter type（0 = None），误把 alpha 值写在了错误位置。

**解决**：修正生成脚本，每行以 `b'\x00'` 开头再拼接 RGBA 像素。

---

## 7. bash 工具默认不执行 `workdir`，导致 npm/vite 报错定位困难

**现象**：`npm run build` 报 `Missing script: "build"`；`npx tsc --noEmit` 打印的是帮助信息；`vite build` 报 `Could not resolve entry module "index.html"`。

**原因**：命令实际在错误目录执行，`tsc` 找不到 `tsconfig.json` 便打印帮助，vite 找不到入口。

**解决**：所有命令用绝对路径或 `sh -c 'cd <dir> && cmd'` 执行。

---

## 8. 遗留 dev 进程占用构建产物，导致 `cargo build` 失败

**现象**：`cargo-tauri build` 报 `failed to write asset ... Operation not permitted (os error 1)`。

**原因**：后台残留的 `cargo-tauri dev` / vite / agnes-chat 进程占用 dist 与 build 产物文件。

**解决**：`kill` 残留进程（`pgrep -fl "cargo-tauri|vite|agnes-chat"`）后再构建。

---

## 9. sccache 导致 cc-rs 编译失败：Operation not permitted

**现象**：所有 C 代码编译报 `cc-rs: unable to open output file 'xxx.o': 'Operation not permitted'`，且必须经 `env -i` 才能正常编译。

**原因**：本机 PATH 中的 Homebrew `sccache` 与 macOS 权限组合异常，白盒检查可通过但编译失败。

**解决**：通过 `env -i HOME=$HOME PATH="/Users/yonh/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin" cargo build --release` 绕过 sccache。修改 `~/.cargo/config.toml` 的 wrapper 配置亦可根治。

---

## 10. 生图 API 不支持 `response_format` 参数（UnsupportedParamsError）

**现象**：生图请求返回 400：`UnsupportedParamsError: Setting 'response_format' is not supported by openai, agnes-t2i-general-model`。

**原因**：Agnes 生图后端（agnes-t2i-general-model）不支持 OpenAI 的 `response_format`。

**解决**：
1. 请求体不再发送 `response_format`，默认返回 URL。
2. 需要 base64 时改为由本地下载原图转 base64（客户端永远不缺图）。

---

## 11. 生图成功后前端仍显示"生成中"

**现象**：API 已返回图片，但界面一直转圈。

**原因**：`handleGenerate` 成功分支缺少 `setLoading(false)`——只有 `catch` 分支复位了 loading。

**解决**：成功路径补 `setLoading(false)`。

---

## 12. 生成历史缩略图尺寸不固定

**现象**：生成历史缩略图被原图尺寸撑开，一屏显示不全需要滚动。

**原因**：缩略图容器用 `aspect-square`，随列宽变大；4K 图更明显。

**解决**：容器改为固定高度 `h-44 w-full overflow-hidden`，`img` 用 `object-cover` 裁剪，加 `loading="lazy"`。

---

## 13. 大图 base64 传前端导致 WebView 白屏

**现象**：1:1 高清图生成很久；接口完成后整个界面白屏。

**原因**：2K/4K 原图 base64 可达几十 MB，通过 IPC 传入 WebView 渲染大 data URL，WKWebView 内存溢出崩溃白屏。

**解决**（后端 `download_compressed`）：
1. 下载原图后解码并缩放最长边 ≤ 1024px，重编码 JPEG(85%) 再转 base64（几百 KB）。
2. 原图 URL 保留，供"下载原图"使用。
3. reqwest 加超时（connect 30s / 总 360s）。
4. 阶段日志 `[agnes]` 前缀输出到终端，何时卡住一目了然。
5. 后端发 `image-progress` 事件，前端显示实时阶段文案。

---

## 14. `image-progress` 事件 payload 是 JSON 字符串而非对象

**现象**：前端 `JSON.parse(event.payload)` 解析失败，进度不更新。

**原因**：Rust 侧 `emit("image-progress", "字符串")`，Tauri 序列化后 payload 是带引号的 JSON 字符串。

**解决**：前端直接 `payload.replace(/^"|"$/g, "")` 取文本。

---

## 15. 缩略图按钮嵌套触发点击冒泡

**现象**：点击下载按钮同时也触发了"查看大图"。

**原因**：下载按钮是图片容器的子元素，冒泡触发父级 onClick。

**解决**：下载按钮 `e.stopPropagation()`。

---

## 16. 后端 base64 改为 JPEG 压缩后，前端下载文件名仍是 .png

**现象**：无原图 URL 时下载的预览图实际是 JPEG 内容，扩展名却是 .png，双击打不开。

**原因**：下载逻辑写死 `.png`，但 `download_compressed` 已改为 JPEG(85%)。

**解决**：下载文件名改为 `.jpg`。

---

## 17. 图生图"参考图没传"：extra_body 被 serde 展平到顶层

**现象**：图生图生成的图与参考图无关，像没传参考图一样。

**原因**：`ImageRequest.extra_body` 字段带 `#[serde(flatten)]`，导致 `image` 数组被展平到请求体**顶层**，而 Agnes 要求 `image` 必须嵌在 `extra_body.image` 内。服务端识别不到参考图，静默退化为文生图。

**解决**：去掉 `#[serde(flatten)]`，`image`、`response_format` 正确嵌套进 `extra_body`。顺带修复了之前 `response_format` 顶层报 400 的根因。

---

## 18. `response_format: "url"` 后所有图都走下载压缩，但 image 缺 avif 解码 → 预览全丢

**现象**：修复 extra_body 后（显式请求 URL 输出），生成历史的预览图全部看不到。

**原因**：请求体加了 `extra_body.response_format: "url"` 后 API 不再返回 `b64_json`，全部走 `download_compressed` 下载原图压缩。而 `image` crate 只开了 `jpeg/png/webp` 解码特性，Agnes 存储 URL（storage.googleapis.com）常返回 **AVIF**，`load_from_memory` 解码失败 → 命令直接报错，预览无从显示；且压缩失败无 fallback。

**解决**：
1. Cargo.toml 给 `image` 加 `"avif"` 特性。
2. `download_compressed` 失败不再 `?` 中断整个命令，改为返回空字符串，前端自动回退到原图 URL 显示（原始方案不丢失预览）。