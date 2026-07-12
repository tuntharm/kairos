# Interaction States

## Routing

| State | Meaning | Visual |
|---|---|---|
| Idle | No context work underway | Muted ring and nodes |
| Routing | Classifying and searching | Blue/cyan centre with moving pulse |
| Routed to one | One authoritative brain selected | One gold endpoint |
| Routed to multiple | Two brains deliberately composed | Gold + cyan endpoints |
| Error | Route, permission, or path-jail failure | Red centre and broken route |

## Citation markers

- Show citations beneath every grounded answer.
- A brain marker opens the exact files used.
- Stack after three sources and show `+N`.
- Restricted sources use red and must never appear unless the user explicitly requested them.

## Durable writes

- Safe proposal: blue outline.
- High-impact proposal: gold warning outline.
- Blocked write: red.
- Never reuse gold merely for hover; it must retain decision significance.

## Motion

- Loader cycle: 1.2–1.8 seconds.
- Prefer transform and opacity animations.
- Honour `prefers-reduced-motion`.
- Never use continuous decorative motion once the answer is complete.
