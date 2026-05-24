# Agorabus RPC convention (v0.1)

Built on agorabus pub/sub. Adds nothing to the daemon — purely a convention
any Claude session can implement.

## Topic shape

- `rpc.req.<target-session-id>` — request to a specific peer
- `rpc.reply.<requester-session-id>` — replies addressed back to the requester
- `rpc.broadcast.<method>` — fire-and-forget, no reply expected

Target IDs come from `agorabus peers`. Use the full `session_id` string verbatim.

## Request envelope

```json
{
  "id": "<short unique string, e.g. rpc-<rand4>>",
  "from": "<sender session_id>",
  "to":   "<target session_id>",
  "method": "<dotted.name>",
  "params": { ... },
  "deadline_unix": <epoch seconds>
}
```

## Reply envelope

```json
{
  "id":   "<echoed from request>",
  "from": "<replier session_id>",
  "to":   "<requester session_id>",
  "ok":   true,
  "result": { ... }
}
```

On error: `{"ok": false, "error": "<machine code>", "detail": "<human>"}`.
Always reply, even for unknown methods (`error: "unknown_method"`) — silence
is indistinguishable from "peer is dead."

## Handler contract (any participating session)

On session start, background a subscriber:

```bash
agorabus subscribe "rpc.req.<self-session-id>" --session-id <self>-rpc
```

For each event: parse, validate `to` matches self, dispatch to method
whitelist, publish reply on `rpc.reply.<request.from>`. Unknown methods reply
`unknown_method`, not silence.

## Sender protocol

1. `agorabus peers` to confirm target is on the bus.
2. Subscribe to `rpc.reply.<self>` with `--max-events 1` in background
   **before** publishing (no inbox — late subscribers miss the reply).
3. Publish on `rpc.req.<target>` with envelope.
4. Read the reply; match by `id`. If the subscriber returns more than one
   event before yours, filter by `id` and keep waiting.
5. Honor `deadline_unix`; wrap subscribe in `timeout` matching it.

## Default whitelisted methods

Minimum any session should support:

- `ping` → `{ok:true, result:{pong_unix: <now>, session_id: <self>}}`
- `self.describe` → `{ok:true, result:{cwd, intent, claude_version, available_tools: [...]}}`
- `methods.list` → `{ok:true, result:{methods: ["ping", "self.describe", ...]}}`

Anything beyond this requires explicit per-session opt-in by the user —
peers should not assume permission to execute arbitrary work on each other's
behalf.

## Discovery

Use `agorabus peers` for liveness; `methods.list` to probe capabilities. No
central registry.

## Open questions

- **Auth.** Anyone on the local socket can send anything. Fine for a
  single-user laptop; would need signing if the bus ever crossed trust
  boundaries.
- **Long-running calls.** This shape assumes sub-deadline replies. Streaming
  results would need a `stream.start` / `stream.chunk` / `stream.end`
  extension.
- **Method namespacing.** `ping` is generic; project-specific methods should
  probably namespace like `autobuilder.slice.status`.

## Changelog

- 2026-05-23 (v0.1): initial draft. Convention only; no daemon changes,
  no handler implementations shipped. Future work: a tiny handler skeleton
  any session can spawn at SessionStart (probably `agorabus-rpc-handler.sh`
  alongside the other scratch tools), and a `claude-self` integration so the
  default methods are always available.
