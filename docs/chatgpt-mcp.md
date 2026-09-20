# 在 ChatGPT Project Chat 中查询沃工本

沃工本启动后会提供只读 MCP 地址：

```text
http://127.0.0.1:47831/mcp
```

该地址只监听本机回环网络。数据库必须保持解锁；锁定后所有业务查询返回 `APP_LOCKED`。沃工本不接收或保存 OpenAI API Key。

## 1. 检查本地服务

打开沃工本并解锁，然后前往“设置 → ChatGPT 连接”。确认：

- 服务状态为“运行中”。
- 数据库状态为“已解锁，可查询”。

也可以在终端检查：

```bash
curl http://127.0.0.1:47831/health
```

## 2. 创建 Secure MCP Tunnel

在 OpenAI Platform 的 Tunnel 设置中创建 tunnel，取得 `tunnel_id` 和供 tunnel-client 使用的运行时 API Key。按照 Platform 页面提供的下载方式安装最新 tunnel-client，然后初始化 HTTP MCP 配置：

```bash
export CONTROL_PLANE_API_KEY="<你的运行时 API Key>"

tunnel-client init \
  --profile solo-cost \
  --tunnel-id "<你的 tunnel_id>" \
  --mcp-server-url "http://127.0.0.1:47831/mcp"

tunnel-client doctor --profile solo-cost --explain
tunnel-client run --profile solo-cost
```

保持 tunnel-client 和沃工本同时运行。API Key 只保存在你选择的运行环境中，不要写入项目文件或数据库。

本项目提供了启动脚本。它会在运行时静默读取 API Key，不会把密钥写入仓库：

```bash
./scripts/run-solo-cost-tunnel.sh
```

脚本会先执行 `doctor`，检查通过后自动启动 Tunnel；终端窗口需要保持运行。

## 3. 在 ChatGPT 中创建 App

1. 在 ChatGPT 设置中开启 Developer Mode。
2. 创建自定义 App，连接方式选择 Tunnel。
3. 选择刚创建的 tunnel，或粘贴 `tunnel_id`。
4. 扫描工具，确认能看到 6 个只读工具。
5. 在 Project Chat 的工具菜单选择该 App，或在问题中使用 `@Solo Cost`。

### 3.1 连接失败时的判断

如果 ChatGPT 回复“需要先让 Solo Cost 在当前会话的可用工具中暴露出来”，通常表示本地
服务本身没有问题，但当前 Project Chat 尚未启用这个自定义 App。`127.0.0.1` 是本机地址，
ChatGPT 云端不能直接访问；必须同时保持 `tunnel-client run` 和沃工本运行。

按以下顺序检查：

1. 沃工本“设置 → ChatGPT 连接”显示服务运行中、数据库已解锁。
2. `tunnel-client doctor --profile solo-cost --explain` 显示客户端配置和连接检查通过。
3. ChatGPT 的自定义 App 能看到 Solo Cost，并扫描出 6 个只读工具。
4. 在 Project Chat 工具菜单中启用该 App；必要时在问题中写 `@Solo Cost`。

连接成功后，ChatGPT 的回复中应出现工具调用，而不是只给出 MCP 配置建议。

不要把 `CONTROL_PLANE_API_KEY` 写入项目文件、Shell 历史提交、数据库或截图；密钥只应
保存在本机运行环境中。

示例：

```text
@Solo Cost 总结当前公司今年的实收、已收到手、其中人工收入和剩余利润。

@Solo Cost 按已收到手排列今年的项目，并解释前三名的销售分成和一般成本。
```

到手已经包含人工收入，关系始终为：

```text
到手 = 人工收入 + 剩余利润
```

## 使用边界

- 默认只查询沃工本当前选中的公司。
- 只有明确要求“全部公司”时才跨公司查询。
- MCP 只能读取数据，不能新建、修改或删除记录。
- 锁定或退出沃工本后，ChatGPT 无法继续查询数据。
- 如果更换电脑，需要迁移沃工本加密备份，并在新电脑重新配置 tunnel-client。
