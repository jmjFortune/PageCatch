# PageCatch 后端接口规范：失效死链任务 (Dead Task)

- **状态**: 规范已确立 (前端已完成适配)
- **文档路径**: `docs/api/dead-task-backend-spec.md`
- **对应前端路由**: `web/index.html` (Dead Filter Tab), `web/app.js` (Action: `mark-dead`)

---

## 1. 业务背景与设计目标

在视频下载场景中，部分任务因源站 404、防盗链签名永久过期、视频已被下架等原因导致持续下载失败。为避免这些死链长期滞留在 `Failed`（失败）列表中干扰重试与管理，PageCatch 前端新增 **`Dead` (失效/死链)** 归档分类。

用户在 `Failed` 列表中，可手动将无法恢复的任务点击移入 `Dead` 分类。

---

## 2. 后端 API 接口规格

### 2.1 标记任务为失效死链 (Mark Task as Dead)

- **接口地址**: `/api/tasks/{id}/dead`
- **请求方式**: `POST`
- **鉴权要求**: 若服务端配置了 `PC_TOKEN`，需携带请求头 `Authorization: Bearer <token>`
- **请求体**: 空 (无需 Request Body)

#### 响应定义

- **200 OK** (成功转为 Dead 状态):
  ```json
  {
    "id": 104,
    "status": "dead",
    "message": "Marked as dead"
  }
  ```

- **400 Bad Request** / **404 Not Found** (任务不存在或状态非 fail):
  ```json
  {
    "error": "Task not found or task is not in failed status"
  }
  ```

---

## 3. 状态机模型 (State Machine)

| 起始状态 | 触发动作 | 目标状态 | 说明 |
| :--- | :--- | :--- | :--- |
| `fail` | `POST /api/tasks/{id}/dead` | `dead` | 人工从失败任务移至失效死链池 |
| `dead` | `POST /api/tasks/{id}/retry` | `pending` | 人工手动恢复/重新尝试下载 |
| `dead` | `DELETE /api/tasks/{id}` | 已删除 | 从任务库彻底删除记录 |

> **调度器约束**：
> 1. `dead` 状态的任务**严禁**被后台自动重试队列重新调度执行；
> 2. 只有在收到人工重试请求（`POST /api/tasks/{id}/retry`）时，状态才重置为 `pending`。

---

## 4. 后端 Rust 推荐实现参考 (Rust & Axum)

### 4.1 数据库持久层 (`src/store.rs`)

建议在 `Store` 结构体中增加 `mark_dead` 方法：

```rust
impl Store {
    pub fn mark_dead(&self, id: i64) -> anyhow::Result<bool> {
        let rows = self.0.lock().unwrap().execute(
            "UPDATE tasks 
             SET status = 'dead', message = 'Marked as dead', updated = ? 
             WHERE id = ? AND status = 'fail'",
            rusqlite::params![now(), id],
        )?;
        Ok(rows > 0)
    }
}
```

同时，在 `Store::retry` 中允许 `dead` 状态恢复为 `pending`：

```rust
// 修改前:
// WHERE id=? AND status IN ('fail','cancelled')
// 修改后:
WHERE id=? AND status IN ('fail','cancelled','dead')
```

### 4.2 路由控制器 (`src/api.rs`)

在 `src/api.rs` 的路由表中注册：

```rust
Router::new()
    .route("/tasks/{id}/dead", post(mark_dead))
```

处理函数实现：

```rust
async fn mark_dead(
    State(q): State<Arc<Queue>>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, Error> {
    let success = q.store.mark_dead(id).map_err(error)?;
    if !success {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Task not found or not in failed status"})),
        ));
    }
    Ok(Json(json!({"id": id, "status": "dead"})))
}
```

---

## 5. 前端当前已交付能力

1. **分段筛选标签 (Segmented Filter)**：
   - 顶部工具栏在 `Failed` 右侧已加入 `Dead` (失效) 标签卡；
   - 实时统计并展示 `Dead` 状态下的任务数量（数量为 0 时自动收起气泡，与其余标签保持一致）。
2. **人工转移交互 (Manual Move)**：
   - 处于 `fail` 状态的任务，操作栏已呈现【移至失效 (Move to Dead)】按钮；
   - 点击后前端优先调用 `/api/tasks/{id}/dead` 接口，并具备乐观 UI 刷新（状态即刻变为 `dead`，从 `Failed` 移入 `Dead`，并弹出系统提示）；
   - `Dead` 列表中的任务支持【查看日志】、【重试】与【删除记录】。
3. **多语言与外观**：
   - 中英双语已完成对应词条本地化（中文：“失效”、“移至失效”；英文：“Dead”、“Move to Dead”）；
   - 配备 macOS 原生风格灰色描边徽标 (`.badge.dead`)。
