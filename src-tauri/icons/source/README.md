# 应用图标源文件

`icon.svg` 是图标的唯一源文件，`src-tauri/icons/` 下所有 PNG/ICNS/ICO 以及
`public/favicon.svg` 都由它生成。改图标只改这个文件，然后重新生成。

## 重新生成

macOS 没有自带 SVG 光栅化工具，用 headless Chrome 渲染：

```bash
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  --headless --disable-gpu --hide-scrollbars \
  --force-device-scale-factor=1 --default-background-color=00000000 \
  --window-size=1024,1024 \
  --screenshot=src-tauri/icons/source/icon-1024.png \
  "file://$PWD/src-tauri/icons/source/icon.svg"

pnpm tauri icon src-tauri/icons/source/icon-1024.png
```

`tauri icon` 会顺带生成 `icons/android/` 和 `icons/ios/`。本项目目前只发布桌面端，
这两个目录直接删掉即可；将来接入移动端时再保留。

`public/favicon.svg` 是同一份图形，只把 `viewBox` 裁到底板范围
（`72 72 880 880`）——浏览器标签页不需要 macOS dock 那圈留白。

## 设计说明

- 底板圆角方形按 Apple 图标网格内缩（1024 画布里 880 见方），macOS dock 里大小才对齐
- 配色取自 `src/lib/brand.ts` 的「账本」品牌色，不引入新色：
  - 底板 = `INK`，玻璃框 = `PAPER`
  - 沙漏上半 = `VERMILION`，尚未支出的时间
  - 沙漏下半 = `INDIGO`，沉淀下来的钱
  - 这正是应用内这两个色现在的用法（朱砂=支出/逾期，靛蓝=净额/进行中）
- 币的两档靛蓝比 `INDIGO` 原值提亮过（`#44557B` / `#6E82AD`）：原值压在 `INK` 底板上
  32px 时糊成一团，提亮后才分得开
- 沙漏内的沙与币都 clip 到玻璃内轮廓，收窄由玻璃壁本身完成
- 不放 `¥` 字符：32px 下不可读，币片叠层比符号更耐缩放
