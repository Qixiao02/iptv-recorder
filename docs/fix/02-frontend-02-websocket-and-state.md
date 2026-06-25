# 前端修复 02：WebSocket 与状态管理

> 优先级：**P0 + P1**
> 预计工时：2-3 天
> 推荐执行人：`frontend-engineer`
> 配套审计章节：`deep-analysis-2026-06-02.md` §3.1 R6, §3.2 I4, §3.2 I6, §3.2 I7

## 范围与背景

本文件覆盖前端的**实时通信层**问题。共 4 个子任务：WS URL 解析 bug、WS 退避、token 改 subprotocol、抽 `useWebSocketBridge` hook。WS 是定时录制能否在前端 Dashboard 看到进度的关键链路，必须扎实。

## 子任务清单

### 子任务 5.1：修复 WS URL 解析 bug（**P0**）

**审计引用**：`frontend/src/api/websocket.ts:29-32`、§3.2 I4 #1

**问题**：
```ts
const wsHost = import.meta.env.VITE_API_BASE_URL
  ? new URL(import.meta.env.VITE_API_BASE_URL).host  // ← 相对路径会抛错
  : window.location.host;
```
`VITE_API_BASE_URL=/api`（典型部署，vite proxy）`new URL('/api')` 会抛 `Invalid URL`——WS 永远连不上。

**修复方案**：
```ts
// frontend/src/api/websocket.ts
const getWsHost = (): string => {
  const apiBase = import.meta.env.VITE_API_BASE_URL;
  if (apiBase) {
    try {
      // 用 window.location.origin 作为 base 处理相对路径
      const url = new URL(apiBase, window.location.origin);
      return url.host;
    } catch (e) {
      console.warn('VITE_API_BASE_URL 解析失败，回退到 window.location.host', e);
      return window.location.host;
    }
  }
  return window.location.host;
};
```

**验收**：
- [ ] `VITE_API_BASE_URL=/api` 时 WS 能连上
- [ ] `VITE_API_BASE_URL=https://api.example.com` 时 WS 用 `api.example.com`
- [ ] 单元测试：3 种 case
- [ ] 手动：生产部署（反代场景）WS 实时数据正常

**风险**：低。

---

### 子任务 5.2：WS token 改 `Sec-WebSocket-Protocol` subprotocol（**P0**）

**审计引用**：`frontend/src/api/websocket.ts:57`、§3.1 R6

**问题**：
```ts
const wsUrl = `${protocol}//${wsHost}/ws?token=${token}`;  // ← token 进 query string
```
反向代理（nginx / Cloudflare / Caddy）的 access log 默认**记录 query string**——JWT 进日志等于泄露。

**修复方案**：
```ts
// frontend/src/api/websocket.ts
const connect = () => {
  const token = getStoredAuthToken();
  const protocols = token ? [`iptv-recorder.v1.auth.${token}`] : [];
  
  const ws = new WebSocket(wsUrl, protocols);
  // ...
};
```

后端配合：
```rust
// backend/src/api/websocket.rs
pub async fn ws_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
    headers: HeaderMap,
) -> impl IntoResponse {
    // 从 Sec-WebSocket-Protocol 提取 token
    let token = headers
        .get("Sec-WebSocket-Protocol")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split('.').last())  // 假设 "iptv-recorder.v1.auth.<token>"
        .map(String::from);
    
    // ... 鉴权逻辑
}
```

**实际**：`Sec-WebSocket-Protocol` 的值在浏览器是字符串数组，需要协商（服务器选一个返回）。RFC 6455 不强制用这个做 auth——但很多项目这么做。

**更标准的做法**：握手首帧（First Frame）发 token：
```ts
ws.onopen = () => {
  ws.send(JSON.stringify({ type: 'auth', token }));
};
```
后端：
```rust
// 读首条消息验 token，失败 close(1008)
```

**选择**：握手首帧法（更标准，浏览器和服务端都不用 hack `Sec-WebSocket-Protocol`）。

**验收**：
- [ ] WS URL 不再含 `?token=...`
- [ ] 抓包/日志检查：access log 没 token
- [ ] 后端读首条消息，验失败 close(1008)
- [ ] 前端收到 close 1008 不再重连

**风险**：中。需要前后端协调发版。**建议先改后端兼容（同时支持 query 和首帧），再改前端去掉 query**。

---

### 子任务 5.3：WS 重连退避（**P0**）

**审计引用**：`frontend/src/api/websocket.ts:90`、§3.2 I4 #2

**问题**：
```ts
// 当前：固定 3s
const RECONNECT_DELAY = 3000;
```
server 短暂 OOM 时所有客户端同时重连，server 一恢复就被打爆（thundering herd）。

**修复方案**：指数退避 + jitter + 上限
```ts
// frontend/src/api/websocket.ts
class ReconnectStrategy {
  private attempt = 0;
  private readonly baseMs = 1000;     // 1s
  private readonly maxMs = 30_000;    // 30s 上限
  private readonly multiplier = 2;
  private readonly jitterRatio = 0.3;  // ±30% jitter

  nextDelay(): number {
    const baseExp = Math.min(
      this.baseMs * Math.pow(this.multiplier, this.attempt),
      this.maxMs
    );
    // jitter: [base * (1 - r), base * (1 + r)]
    const jitter = baseExp * this.jitterRatio * (Math.random() * 2 - 1);
    return Math.floor(baseExp + jitter);
  }

  reset() { this.attempt = 0; }
  onFailure() { this.attempt++; }
  onSuccess() { this.reset(); }
}

// 使用
const reconnectStrategy = new ReconnectStrategy();

const scheduleReconnect = () => {
  const delay = reconnectStrategy.nextDelay();
  setTimeout(connect, delay);
};
```

**验收**：
- [ ] 单元测试：连续失败 6 次的延迟序列 ≈ [1s, 2s, 4s, 8s, 16s, 30s]（带 ±30% jitter）
- [ ] 成功连接后 attempt 重置为 0
- [ ] 手动：服务重启，前端不再"齐刷刷重连"

**风险**：低。

---

### 子任务 5.4：抽 `useWebSocketBridge` hook（**P1**）

**审计引用**：`frontend/src/App.tsx:79-149`、§3.2 I8

**问题**：`App.tsx` 根组件里塞了 70 行 useEffect，直接订阅 5 路 WS 事件 + 改 queryClient cache。"WS → 缓存" 的副作用**焊死**在 `App`，无法单独测。

**修复方案**：
```ts
// frontend/src/hooks/useWebSocketBridge.ts
import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { wsClient } from '@/api/websocket';
import { applyTaskProgressUpdate, applyTaskStatusUpdate } from '@/lib/taskRealtime';

export function useWebSocketBridge() {
  const queryClient = useQueryClient();
  
  useEffect(() => {
    const unsubs = [
      wsClient.on('task.progress', (event) => {
        applyTaskProgressUpdate(queryClient, event);
      }),
      wsClient.on('task.update', (event) => {
        applyTaskStatusUpdate(queryClient, event);
      }),
      wsClient.on('channel.status', (event) => {
        // ...patch channel list cache
      }),
      wsClient.on('system.alert', (event) => {
        // ...useUIStore.addAlert
      }),
      wsClient.onConnectionStateChange((state) => {
        if (state === 'connected') {
          queryClient.invalidateQueries({ queryKey: ['tasks'] });
        }
      }),
    ];
    return () => unsubs.forEach(u => u());
  }, [queryClient]);
}
```

```ts
// frontend/src/App.tsx
function App() {
  useWebSocketBridge();  // 一行替代 70 行 useEffect
  
  return (
    <QueryClientProvider>
      {/* ... */}
    </QueryClientProvider>
  );
}
```

**验收**：
- [ ] `App.tsx` 的 useEffect 数量从 5 个降到 0
- [ ] 行为不变（手动触发录制、Dashboard 实时更新）
- [ ] 单元测试：mock `wsClient`，调用 hook，验证 cache 被 patch

**风险**：低。需要先把 `wsClient.on()` 改成返回 unsubscribe 函数的签名（见下）。

#### 5.4 前置改动：`wsClient` 返回 unsubscribe

```ts
// frontend/src/api/websocket.ts
class WebSocketClient {
  // 旧: on(event, handler) → void
  // 新: on(event, handler) → () => void
  on<K extends keyof WsEventMap>(event: K, handler: WsEventMap[K]): () => void {
    // ...
    return () => this.off(event, handler);
  }
}
```

这是个小重构，前置。

---

## 测试要求

| 子任务 | 测试 |
| --- | --- |
| 5.1 | 3 个单元测试（3 种 URL 配置） |
| 5.2 | 集成测试：抓包确认 URL 不含 token |
| 5.3 | 单元测试：延迟序列断言 |
| 5.4 | 单元测试：mock wsClient，验证 cache patch |

## 提交策略

- 5.1 → 5.3 → 5.4 一个 PR（WS 完整改动）
- 5.2 需要**两个 PR**（先改后端兼容，再改前端去掉 query）

## 风险汇总

| 子任务 | 风险 | 缓解 |
| --- | --- | --- |
| 5.1 | 反代下 host 不对 | 单元测试覆盖 |
| 5.2 | 后端先发版才能去掉 query | 灰度 |
| 5.3 | 退避太久用户体验差 | 30s 上限 |
| 5.4 | 副作用改坏 | 完整集成测试 |

---

*执行入口：5.1 → 5.3 → 5.4 + 5.2（需后端先发版）。*
