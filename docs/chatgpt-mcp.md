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

## 3. 在 ChatGPT 中创建 App

1. 在 ChatGPT 设置中开启 Developer Mode。
2. 创建自定义 App，连接方式选择 Tunnel。
3. 选择刚创建的 tunnel，或粘贴 `tunnel_id`。
4. 扫描工具，确认能看到 6 个只读工具。
5. 在 Project Chat 的工具菜单选择该 App，或在问题中使用 `@Solo Cost`。

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

