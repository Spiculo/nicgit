# The AI router contract

NicGit never talks to an AI provider directly. Every AI request goes to **the AI router**, the owner's cloud service that picks the model, holds provider keys, and bills AI usage (spec section 13, decision N8). NicBim will use the same router.

## Configuration

| Variable | Meaning |
|---|---|
| `NICGIT_AI_ROUTER_URL` | Base URL, e.g. `https://ai.nicbim.example`. Unset: AI is unavailable on this server. |
| `NICGIT_AI_ROUTER_KEY` | Sent as `Authorization: Bearer <key>`. One key per NicGit installation. |

AI is **off by default for every account**. An account admin turns it on (`PUT /api/accounts/{slug}/ai {"enabled": true}` or Settings > AI). Nothing leaves NicGit for an account that has not turned AI on.

## Request

`POST {router}/v1/chat`

```json
{
  "model": "auto",
  "messages": [
    {"role": "system", "content": "…"},
    {"role": "user", "content": "…"}
  ],
  "metadata": {
    "account": "<account uuid>",
    "project": "<project uuid>",
    "purpose": "proposal_summary",
    "language": "en"
  }
}
```

- `model: "auto"` lets the router choose. A request may later name a tier (`"fast"`, `"best"`); the router may ignore it.
- `metadata.account` is the account that pays. The router may keep its own per-account ledger and refuse with `402`.
- `purpose` values used today: `proposal_summary`. Planned: `version_summary`, `ask_model`.
- Content is the engine's **structured change list** (element kind, class, name, level, field old → new). Model files, raw records and drawings are never sent.

## Response

`200`

```json
{"text": "Scheme B glazes the south wall…", "usage": {"units": 42}}
```

- `text`: plain text, shown labelled as AI-written.
- `usage.units`: what this answer costs, in the router's billing unit. NicGit records it as `ai_units` usage for the account and stops new requests when the account's hard `ai_units` budget is reached. A missing `usage` counts as 1.

Errors: any non-2xx is shown to the user as "the AI router answered <status>". NicGit times out after 120 s.

## Caching

A proposal summary is stored against the proposal's two heads, so the router is asked once per state of the proposal. New versions on either option make a new summary possible.
