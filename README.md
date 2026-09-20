# NoralWeb — Web Version (no download, runs in the browser)

The hosted half of [NoralWeb](https://github.com/samansarmasik-alt/NoralWeb):
the same neural research engine, served over HTTP. Open the link, type anything
that isn't a URL, get a ranked report from 40+ live sources.

[![Deploy to Render](https://render.com/images/deploy-to-render-button.svg)](https://render.com/deploy?repo=https://github.com/samansarmasik-alt/NoralWeb-WebVersion)

![Rust](https://img.shields.io/badge/Rust-axum-blue)
![Docker](https://img.shields.io/badge/deploy-Docker%20%2B%20Render-46E3B7)
![License](https://img.shields.io/badge/license-MIT-green)

---

## Same project, two halves

| | Desktop (`NoralWeb`) | Web (this repo) |
|---|---|---|
| core (`fetch`/`research`/`neural`/`nim`) | identical | identical |
| UI | native WebView2 window | same UI in your browser |
| harvester (hidden real browser) | yes | **no** (needs a GUI) |
| agent tabs | opens tabs | links cited as evidence |
| model file | persists next to exe | ephemeral (resets on redeploy) |
| keys | files next to exe | **env vars** (or files) |

**Sync rule:** desktop improves → run `sync-core.ps1` → core files are
byte-copied here → commit + push. Web-only code lives in `src/main.rs`,
`ui/index.html`, `Dockerfile` — the sync never touches them.

```powershell
.\sync-core.ps1                      # default desktop path
.\sync-core.ps1 -Desktop "D:\x\noral web"
cargo check
```

## Deploy (Render, free tier OK)

1. Push this repo to GitHub (already done).
2. [render.com](https://render.com) → New → Blueprint → select the repo
   (`render.yaml` wires everything).
3. Optional env vars (dashboard → Environment): `NIM_KEY` (agent),
   `APINEX_KEY`, `LANGSEARCH_KEY`, `BRAVE_KEY`, `EXA_KEY`, `TAVILY_KEY`,
   `SERPER_KEY`. Without keys the full 40+ free-source pipeline still runs.
4. Open the `*.onrender.com` URL. Done — no download for your users.

> Free tier sleeps when idle: the first query after sleep takes ~60s
> (cold start + model training). Later queries are seconds.

## Local run

```bash
cargo run --release
# → http://localhost:10000
```

Keys locally: env vars (`NIM_KEY=… cargo run --release`) or plain
`*-key.txt` files next to the binary (same format as desktop).

## API

| method | path | body | returns |
|---|---|---|---|
| GET | `/` | — | UI |
| POST | `/api/research` | `{query, depth(0/1), apx(0/1/2)}` | ranked `Report` JSON |
| POST | `/api/click` | `{feats[12], skipped[[12]…]}` | 204 (trains ranker) |
| GET | `/api/testmode` | — | net probes + training info |
| GET | `/api/keystatus` | — | which lanes are on |
| POST | `/api/savekey` | `{service, key}` | updated status |
| POST | `/api/agent` | `{message, mode, apx, history[]}` | `{answer, steps, evidence}` |

## Limits (honest)

- No harvester: bot-walled pages that need a real browser stay silent here
  (desktop's Google-H/Yandex-H don't exist server-side).
- Model + saved keys live on ephemeral disk: redeploys reset click-training.
- Agent calls are blocking: very long OSINT runs can hit hosting timeouts —
  use the desktop app for those.
- Same provider reality as desktop: some sources 429/captcha datacenter IPs;
  the report shows silent-with-reason labels.

## License

MIT — see [LICENSE](LICENSE) (same as desktop).
