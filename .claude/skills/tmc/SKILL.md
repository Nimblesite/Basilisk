---
name: tmc
description: Too Many Cooks (TMC) multi-agent coordination system. Use when collaborating with other agents — registering, sending/receiving messages, locking files, updating plans, and checking system status.
user-invocable: false
---

# Too Many Cooks (TMC) — Multi-Agent Coordination

TMC coordinates multiple Claude agents working on the same codebase. It provides registration, messaging, file locking, and plan tracking.

## Tools

### Register

Register before using any other TMC tool.

**First time** — pick a unique name:
```
mcp__too-many-cooks__register({"name": "my-agent"})
```
Returns `agent_key`. Store it — required for reconnecting.

**Reconnect** — use your stored key:
```
mcp__too-many-cooks__register({"key": "abc123..."})
```

Never send both `name` and `key` together.

---

### Messages

Send, receive, and mark messages as read.

**Check for messages:**
```
mcp__too-many-cooks__message({"action": "get"})
```
Add `"unread_only": false` to see all messages.

**Send to one agent:**
```
mcp__too-many-cooks__message({"action": "send", "to_agent": "agent-name", "content": "your message"})
```

**Broadcast to all agents:**
```
mcp__too-many-cooks__message({"action": "send", "to_agent": "*", "content": "your message"})
```

**Mark as read:**
```
mcp__too-many-cooks__message({"action": "mark_read", "message_id": "MSG_ID"})
```

Content limit: 200 characters max.

---

### Plans

Track what you're working on so other agents can see.

**Update your plan:**
```
mcp__too-many-cooks__plan({"action": "update", "goal": "what you're trying to achieve", "current_task": "what you're doing right now"})
```

**Get an agent's plan:**
```
mcp__too-many-cooks__plan({"action": "get"})
```

**List all plans:**
```
mcp__too-many-cooks__plan({"action": "list"})
```

Goal: max 100 chars. Current task: max 100 chars.

---

### File Locks

Prevent conflicts by locking files before editing.

**Acquire a lock:**
```
mcp__too-many-cooks__lock({"action": "acquire", "file_path": "/path/to/file", "reason": "why"})
```

**Release a lock:**
```
mcp__too-many-cooks__lock({"action": "release", "file_path": "/path/to/file"})
```

**Check if a file is locked:**
```
mcp__too-many-cooks__lock({"action": "query", "file_path": "/path/to/file"})
```

**List all locks:**
```
mcp__too-many-cooks__lock({"action": "list"})
```

**Force-release a stale lock:**
```
mcp__too-many-cooks__lock({"action": "force_release", "file_path": "/path/to/file"})
```

**Renew a lock:**
```
mcp__too-many-cooks__lock({"action": "renew", "file_path": "/path/to/file"})
```

---

### Status

Get a full system overview — all agents, locks, plans, and recent messages.

```
mcp__too-many-cooks__status()
```

No parameters required.

---

## Best Practices

- **Check messages frequently** — every few tool calls, call `message` with `action: "get"`
- **Lock before editing** — always acquire a lock before modifying a file
- **Never edit locked files** — if another agent has the lock, work on something else
- **Release locks promptly** — don't hold locks longer than needed
- **Update your plan** — keep it current so others know what you're doing
- **Keep messages short** — 200 char limit, be concise
