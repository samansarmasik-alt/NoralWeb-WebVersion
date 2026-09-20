//! NoralWeb web sunucusu — masaüstü çekirdeğin HTTP cephesi.
//!
//! KURAL: çekirdek (fetch.rs, research.rs, neural.rs, nim.rs) masaüstüyle
//! BİREBİR aynıdır. Masaüstü gelişince `sync-core.ps1` ile kopyalanır.
//! Web'e özel her şey BU dosyadadır: HTTP, env-anahtar boot, harvestsız ajan.
//! Harvest (gizli WebView) sunucuda YOK — ureq yolları + sayfa-2 yedekleri çalışır.

mod fetch;
mod neural;
mod nim;
mod research;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
struct AppState {
    model: Arc<Mutex<neural::TrainState>>,
}

// ---------- boot: env anahtarları → dosya (çekirdek dosyadan okur, değişmedi) ----------
fn key_path(file: &str) -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            return d.join(file);
        }
    }
    nim::appdata_dir().join(file)
}

fn boot_keys() {
    // Ücretli/anahtarlı hatlar: önce env (Render), yoksa dosyaya dokunma.
    let pairs = [
        ("APINEX_KEY", "apinex-key.txt"),
        ("LANGSEARCH_KEY", "langsearch-key.txt"),
        ("BRAVE_KEY", "brave-key.txt"),
        ("EXA_KEY", "exa-key.txt"),
        ("TAVILY_KEY", "tavily-key.txt"),
        ("SERPER_KEY", "serper-key.txt"),
    ];
    for (env, file) in pairs {
        if let Ok(v) = std::env::var(env) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                let p = key_path(file);
                if let Some(d) = p.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                let _ = std::fs::write(p, v);
            }
        }
    }
    if let Ok(v) = std::env::var("NIM_KEY") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            let d = nim::appdata_dir();
            let _ = std::fs::create_dir_all(&d);
            let _ = std::fs::write(d.join("nim-key.txt"), v);
        }
    }
}

fn key_on(file: &str) -> bool {
    // exe-yanı + appdata iki konuma da bak (boot exe-yanına yazar).
    for p in [key_path(file), nim::appdata_dir().join(file)] {
        if std::fs::read_to_string(p).map(|k| !k.trim().is_empty()).unwrap_or(false) {
            return true;
        }
    }
    false
}

// ---------- istek/yanıt ----------
#[derive(Deserialize)]
struct ResearchReq {
    query: String,
    depth: Option<u8>,
    apx: Option<u8>,
}

#[derive(Deserialize)]
struct ClickReq {
    feats: [f32; 12],
    skipped: Vec<[f32; 12]>,
}

#[derive(Deserialize)]
struct SaveKeyReq {
    service: String,
    key: String,
}

#[derive(Deserialize)]
struct AgentReq {
    message: String,
    mode: Option<String>,
    apx: Option<u8>,
    history: Option<Vec<nim::Msg>>,
}

#[derive(Serialize)]
struct KeyStatus {
    apinex: bool,
    brave: bool,
    exa: bool,
    tavily: bool,
    serper: bool,
    langsearch: bool,
    nim: bool,
    saved: bool,
}

// ---------- research ----------
async fn api_research(
    State(st): State<AppState>,
    Json(r): Json<ResearchReq>,
) -> Result<Json<research::Report>, StatusCode> {
    let q = r.query.trim().to_string();
    if q.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let deep = r.depth.unwrap_or(1) == 1;
    let apx = r.apx.unwrap_or(1).min(2);
    let snap = {
        let g = st.model.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        (g.net.clone(), g.version, g.clicks)
    };
    // ureq thread'leri bloklar → blocking havuza al.
    let rep = tokio::task::spawn_blocking(move || {
        let t0 = Instant::now();
        let (cands, sources, silent, cost) = fetch::live_search(&q, deep, apx);
        // Sunucuda harvest YOK (GUI ister) — ureq + sayfa-2 yedekleriyle yetinilir.
        let results = research::rank(&research::NeuralRank, &snap.0, &q, cands);
        research::Report {
            query: q,
            candidates: results.len(),
            sources,
            silent,
            elapsed_ms: t0.elapsed().as_millis(),
            engine: "mlp-12-48-28-1 + bm25/cos".to_string(),
            net: snap.0,
            model: neural::ModelInfo {
                version: snap.1,
                clicks: snap.2,
                trained: true,
            },
            depth: if deep { 1u8 } else { 0u8 },
            cost_usd: cost,
            cached: false,
            results,
        }
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rep))
}

// ---------- click (tıklama öğrenmesi) ----------
async fn api_click(State(st): State<AppState>, Json(r): Json<ClickReq>) -> StatusCode {
    let mut g = match st.model.lock() {
        Ok(g) => g,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };
    neural::learn_click(&mut g.net, r.feats, &r.skipped);
    g.clicks += 1;
    neural::save(&g);
    StatusCode::NO_CONTENT
}

// ---------- testmode ----------
async fn api_testmode(State(st): State<AppState>) -> Json<serde_json::Value> {
    let s = st.model.lock().map(|g| g.clone()).unwrap_or_else(|_| neural::load_or_train());
    let probes = neural::self_test(&s.net);
    let pass = probes.iter().filter(|p| p.pass).count();
    Json(serde_json::json!({
        "arch": s.net.arch,
        "params": s.net.params,
        "net": s.net,
        "probes": probes,
        "training": {
            "base_pairs": neural::BASE_PAIRS.len(),
            "base_epochs": neural::BASE_EPOCHS,
            "version": s.version,
            "clicks": s.clicks,
            "passed": pass,
        },
    }))
}

// ---------- keys ----------
async fn api_keystatus() -> Json<KeyStatus> {
    Json(KeyStatus {
        apinex: key_on("apinex-key.txt"),
        brave: key_on("brave-key.txt"),
        exa: key_on("exa-key.txt"),
        tavily: key_on("tavily-key.txt"),
        serper: key_on("serper-key.txt"),
        langsearch: key_on("langsearch-key.txt"),
        nim: nim::nim_key().is_some(),
        saved: false,
    })
}

async fn api_savekey(Json(r): Json<SaveKeyReq>) -> Json<KeyStatus> {
    let file = match r.service.as_str() {
        "nim" => "nim-key.txt",
        _ => &format!("{}-key.txt", r.service),
    };
    let p = if r.service == "nim" {
        nim::appdata_dir().join(file)
    } else {
        key_path(file)
    };
    if !r.key.trim().is_empty() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, r.key.trim());
    }
    Json(KeyStatus {
        apinex: key_on("apinex-key.txt"),
        brave: key_on("brave-key.txt"),
        exa: key_on("exa-key.txt"),
        tavily: key_on("tavily-key.txt"),
        serper: key_on("serper-key.txt"),
        langsearch: key_on("langsearch-key.txt"),
        nim: nim::nim_key().is_some(),
        saved: true,
    })
}

// ---------- agent (harvestsız, open_tab'sız) ----------
async fn api_agent(
    State(st): State<AppState>,
    Json(r): Json<AgentReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let message = r.message.trim().to_string();
    if message.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mode = if r.mode.as_deref() == Some("osint") {
        "osint".to_string()
    } else {
        "arastirma".to_string()
    };
    let apx = r.apx.unwrap_or(1).min(2);
    let history = r.history.unwrap_or_default();
    let key = match nim::nim_key() {
        Some(k) => k,
        None => {
            return Ok(Json(serde_json::json!({
                "mode": mode, "model": nim::DEFAULT_MODEL,
                "answer": "NIM anahtarı yok. Ortam değişkeni NIM_KEY ver ya da panelden kaydet, sonra tekrar dene.",
                "steps": [], "need_key": true,
            })));
        }
    };
    let snap = {
        let g = st.model.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        g.net.clone()
    };
    // NIM 90sn'ye kadar bekleyebilir → blocking havuz.
    let out = tokio::task::spawn_blocking(move || web_agent(key, mode, message, history, apx, &snap))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(out))
}

fn web_agent(
    key: String,
    mode: String,
    message: String,
    history: Vec<nim::Msg>,
    apx: u8,
    snap_net: &neural::NetSpec,
) -> serde_json::Value {
    let mut sys = nim::sys_prompt(&mode);
    sys.push_str("\nWEB SÜRÜM NOTU: harvest aracı YOK (sunucuda gizli tarayıcı çalışmaz), fetch_page ile yetin. open_tab/open_tabs yerine önemli URL'leri yanıtında KANIT LİNKLERİ olarak listele.");
    let mut messages = vec![nim::Msg {
        role: "system".into(),
        content: sys,
        tool_calls: None,
        tool_call_id: None,
    }];
    for h in history.iter().rev().take(12).collect::<Vec<_>>().into_iter().rev() {
        messages.push(nim::Msg {
            role: h.role.clone(),
            content: h.content.chars().take(2000).collect(),
            tool_calls: None,
            tool_call_id: None,
        });
    }
    messages.push(nim::Msg {
        role: "user".into(),
        content: message.chars().take(2000).collect(),
        tool_calls: None,
        tool_call_id: None,
    });
    // tools_schema(false) = harvestsız şema (open_tab'lar kalır, sunucuda kanıta çevrilir).
    let tools = nim::tools_schema(false);
    let mut evidence: Vec<String> = Vec::new();
    let mut steps: Vec<serde_json::Value> = Vec::new();
    let mut seen_q = std::collections::HashSet::new();
    let mut searches = 0u8;
    let mut fetched = false;
    for _step in 0..6 {
        let reply = match nim::chat(&key, nim::DEFAULT_MODEL, &messages, &tools) {
            Ok(r) => r,
            Err(e) => {
                if evidence.is_empty() {
                    return serde_json::json!({
                        "mode": mode, "model": nim::DEFAULT_MODEL,
                        "answer": format!("Model hatası ({}).", e),
                        "steps": steps, "evidence": evidence,
                    });
                }
                break;
            }
        };
        if reply.tool_calls.is_empty() {
            let answer = if reply.content.trim().is_empty() && !evidence.is_empty() {
                format!("Özet çıkarılamadı. Ham bulgular:\n{}", evidence.join("\n"))
            } else {
                reply.content
            };
            return serde_json::json!({
                "mode": mode, "model": nim::DEFAULT_MODEL,
                "answer": answer, "steps": steps, "evidence": evidence,
            });
        }
        messages.push(nim::Msg {
            role: "assistant".into(),
            content: reply.content.clone(),
            tool_calls: Some(reply.tool_calls.iter().map(|t| t.to_out()).collect()),
            tool_call_id: None,
        });
        for tc in &reply.tool_calls {
            let arg_snip = tc.args.to_string().chars().take(120).collect::<String>();
            steps.push(serde_json::json!({"tool": tc.name, "args": arg_snip}));
            let out = match tc.name.as_str() {
                "web_search" => {
                    let q = tc.args.get("query").and_then(|x| x.as_str()).unwrap_or("");
                    let qn = q.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
                    if !seen_q.insert(qn) {
                        serde_json::json!({"error": "bu sorguyu zaten aradın. TEKRAR ARAMA YAPMA; fetch_page ile adayları oku."}).to_string()
                    } else {
                        searches += 1;
                        let (cands, _, _, _) = fetch::live_search(q, false, apx);
                        let ranked = research::rank(&research::NeuralRank, snap_net, q, cands);
                        for c in ranked.iter().take(5) {
                            if evidence.len() < 10 {
                                evidence.push(format!(
                                    "• {} ({})",
                                    c.title.chars().take(90).collect::<String>(),
                                    c.url
                                ));
                            }
                        }
                        let top: Vec<serde_json::Value> = ranked
                            .into_iter()
                            .take(12)
                            .map(|c| {
                                serde_json::json!({
                                    "title": c.title,
                                    "url": c.url,
                                    "snippet": c.snippet.chars().take(220).collect::<String>(),
                                    "source": c.source,
                                    "neural": c.detail.neural,
                                })
                            })
                            .collect();
                        serde_json::json!({ "results": top, "count": top.len() }).to_string()
                    }
                }
                "fetch_page" => {
                    fetched = true;
                    let u = tc.args.get("url").and_then(|x| x.as_str()).unwrap_or("");
                    match fetch::fetch_page(u).or_else(|| {
                        fetch::apinex_contents(u).map(|(t, md)| fetch::PageData {
                            title: t,
                            text: md,
                            links: Vec::new(),
                            meta_desc: String::new(),
                            og_image: String::new(),
                        })
                    }) {
                        Some(p) => {
                            if evidence.len() < 10 {
                                evidence.push(format!(
                                    "• SAYFA {}: {}",
                                    p.title.chars().take(80).collect::<String>(),
                                    p.text.chars().take(300).collect::<String>()
                                ));
                            }
                            serde_json::json!({
                                "title": p.title.chars().take(200).collect::<String>(),
                                "text": p.text.chars().take(2500).collect::<String>(),
                                "links": p.links.into_iter().take(14).map(|(a, u)| {
                                    serde_json::json!({"anchor": a.chars().take(120).collect::<String>(), "url": u})
                                }).collect::<Vec<_>>(),
                            })
                            .to_string()
                        }
                        None => serde_json::json!({"error": "sayfa çekilemedi"}).to_string(),
                    }
                }
                "open_tab" | "open_tabs" => {
                    // Sunucuda sekme yok — URL'leri kanıt havuzuna al, model yanıtta listelesin.
                    let urls: Vec<String> = tc
                        .args
                        .get("urls")
                        .and_then(|x| x.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .take(5)
                                .collect()
                        })
                        .or_else(|| {
                            tc.args
                                .get("url")
                                .and_then(|x| x.as_str())
                                .map(|s| vec![s.to_string()])
                        })
                        .unwrap_or_default();
                    for u in &urls {
                        if evidence.len() < 10 {
                            evidence.push(format!("• SEKME(adaya eklendi): {}", u));
                        }
                    }
                    serde_json::json!({
                        "opened": urls,
                        "note": "web sürümde sekmeler yeni pencerede açılır; bu URL'leri yanıtındaki KANIT LİNKLERİ'nde listele."
                    })
                    .to_string()
                }
                _ => serde_json::json!({"error": "web sürümde bu araç kapalı (harvest yok); fetch_page kullan."}).to_string(),
            };
            messages.push(nim::Msg {
                role: "tool".into(),
                content: out,
                tool_calls: None,
                tool_call_id: Some(tc.id.clone()),
            });
            if searches >= 4 && !fetched {
                messages.push(nim::Msg {
                    role: "user".into(),
                    content: "Yeterince adayın var. Yeni web_search YAPMA; en ilgili 2-3 sonucu fetch_page ile oku, sonra toparla.".into(),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }
    }
    serde_json::json!({
        "mode": mode, "model": nim::DEFAULT_MODEL,
        "answer": if evidence.is_empty() { "Adım bütçesi doldu, bulgu yok.".to_string() } else { format!("Adım bütçesi doldu. Ham bulgular:\n{}", evidence.join("\n")) },
        "steps": steps, "evidence": evidence,
    })
}

// ---------- ana ----------
#[tokio::main]
async fn main() {
    boot_keys();
    let state = AppState {
        model: Arc::new(Mutex::new(neural::load_or_train())),
    };
    let app = Router::new()
        .route("/", get(index))
        .route("/api/research", post(api_research))
        .route("/api/click", post(api_click))
        .route("/api/testmode", get(api_testmode))
        .route("/api/keystatus", get(api_keystatus))
        .route("/api/savekey", post(api_savekey))
        .route("/api/agent", post(api_agent))
        .with_state(state);
    // Render $PORT verir; yoksa 10000.
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(10000);
    let addr = format!("0.0.0.0:{}", port);
    println!("noral-web-server dinliyor: {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
}
