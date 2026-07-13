# WebSocket Events — Task Tracker

## 1. Overview

WebSocket endpoint: `wss://{host}:19876/ws/v1`. Используется для real-time обновлений kanban-доски, issue detail, уведомлений и активности.

## 2. Connection

### Handshake

```http
GET /ws/v1 HTTP/1.1
Host: tasktracker.example.com:19876
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: ...
Sec-WebSocket-Version: 13
Authorization: Bearer <access_token>
Cookie: refresh_token=<httpOnly>
```

### Subscription

После открытия клиент отправляет:

```json
{
  "type": "subscribe",
  "topics": ["project:abc", "issue:xyz", "user:self"]
}
```

## 3. Server → Client Events

| Event | Topic | Payload |
|-------|-------|---------|
| `issue_updated` | `project:{id}` | `{ issueId, key, changes, actor }` |
| `issue_status_changed` | `project:{id}` | `{ issueId, oldStatusId, newStatusId }` |
| `issue_assigned` | `project:{id}` | `{ issueId, assigneeId }` |
| `comment_added` | `issue:{id}` | `{ commentId, authorId, body, createdAt }` |
| `attachment_added` | `issue:{id}` | `{ attachmentId, filename }` |
| `board_column_changed` | `board:{id}` | `{ issueId, columnId, rank }` |
| `sprint_started` | `project:{id}` | `{ sprintId, name }` |
| `sprint_closed` | `project:{id}` | `{ sprintId, completedIssues, incompleteIssues }` |
| `user_notification` | `user:{id}` | `{ notificationId, type, title, link }` |
| `user_online` | `project:{id}` | `{ userId, online }` |

## 4. Client → Server Events

| Event | Purpose | Payload |
|-------|---------|---------|
| `subscribe` | Подписка на топики | `{ topics: string[] }` |
| `unsubscribe` | Отписка | `{ topics: string[] }` |
| `heartbeat` | Keep-alive | `{ type: "heartbeat" }` |
| `cursor_position` | Presence (опционально) | `{ boardId, x, y }` |

## 5. Error Frames

```json
{
  "type": "error",
  "code": "UNAUTHORIZED",
  "message": "Invalid or expired token"
}
```

## 6. Reconnection

- Экспоненциальный backoff: 1s, 2s, 4s, 8s, max 30s.
- После reconnect заново subscribe и запросить diff с `lastEventId`.
- Client-side buffer до восстановления соединения.

## 7. Scaling

- API instances stateless.
- Redis pub/sub для broadcast между instances.
- Topic routing через Redis channel per project/issue/user.

## 8. Security

- Access token в handshake query или header.
- Refresh cookie проверяется для продления сессии.
- Rate limit: max 50 messages/sec per connection.
- Input validation всех inbound frames.

## References

- `docs/API.md`
- `docs/ARCHITECTURE.md`
- `docs/CACHING.md`
- `docs/SECURITY.md`
