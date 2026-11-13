//! Spike S8 (harness) : le binaire `claude` piloté en `stream-json` depuis Rust, sans le SDK Node.
//!
//! Question : fragment-rs peut-il parler DIRECTEMENT au binaire de Claude Code (processus enfant,
//! JSON par lignes sur stdin / stdout, protocole de contrôle) pour tout ce que Fragment obtient du
//! Claude Agent SDK : tour en flux, pensée résumée, outils, garde `PreToolUse`, carte de permission,
//! reprise, fork à un point donné, interruption, pièces jointes, isolement des réglages ?
//!
//! ```text
//! cargo run -p fragment-agent --example spike_harness -- [scénario…] [--claude <binaire>] [--bac <dossier>]
//! scénarios : texte pensee permissions sessions instantane interruption pieces erreurs (défaut : tous)
//! ```
//!
//! ★ Lance le VRAI `claude` du portable, avec le login local de l'utilisateur (aucune clé API) :
//!   tours courts, modèle `haiku`, outils limités à Read / Write / Bash, chaque Bash refusé, dossier
//!   de travail jetable (défaut `C:/dev/s8-bac`). La CI ne fait que le compiler.
//! ★ Rien de ce qui sort n'est affiché tel quel : ni jeton, ni identifiant de compte. Les transcripts
//!   bruts vont dans `<bac>/transcripts/` (hors dépôt), le dossier de l'utilisateur remplacé par `~`.
//!
//! Le prompt système est FACTICE, écrit pour le spike (décision de l'orchestrateur, 13/11) : il
//! n'imite pas celui de Fragment, il ne sert qu'à vérifier qu'un prompt passe par `initialize`.

use std::collections::hash_map::RandomState;
use std::error::Error;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use fragment_agent::contrat::AgentEvent;
use fragment_agent::events::{to_agent_events, to_chat_entries};
use serde_json::{Value, json};

type Resultat<T> = Result<T, Box<dyn Error>>;

/// Le prompt système du spike (gabarit `{{vault}}`, `{{code}}`), dans nos mots. Le code sert à
/// vérifier QUEL prompt le modèle a reçu (initialize, instantané à la reprise).
const PROMPT_FACTICE: &str = "Tu es un assistant de test pour le coffre « {{vault}} ». \
Réponds en français, en une ou deux phrases au plus. N'utilise un outil que si on te le demande. \
Ton code de test est {{code}} : donne-le si on te le demande.";

/// Le code du prompt par défaut.
const CODE: &str = "NEFLIER";

/// Au-delà, le processus est tué (jamais de tour qui pend).
const DELAI_TOUR: Duration = Duration::from_secs(120);

// ── Utilitaires sans dépendance ─────────────────────────────────────────────────────────────

/// Un UUID v4 (l'id de session, choisi par l'hôte comme dans Fragment).
fn uuid() -> String {
    let mut octets = [0u8; 16];
    for (i, morceau) in octets.chunks_mut(8).enumerate() {
        let mut h = RandomState::new().build_hasher();
        h.write_usize(i);
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        morceau.copy_from_slice(&h.finish().to_le_bytes());
    }
    octets[6] = (octets[6] & 0x0f) | 0x40;
    octets[8] = (octets[8] & 0x3f) | 0x80;
    let x: String = octets.iter().map(|o| format!("{o:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &x[0..8],
        &x[8..12],
        &x[12..16],
        &x[16..20],
        &x[20..32]
    )
}

fn base64(donnees: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in donnees.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(char::from(A[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn crc32(donnees: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &o in donnees {
        crc ^= u32::from(o);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Un PNG carré d'une seule couleur (zlib en bloc « stocké », sans compression).
fn png_uni(cote: u32, rgb: [u8; 3]) -> Vec<u8> {
    let mut brut = Vec::new();
    let ligne: Vec<u8> = std::iter::once(0)
        .chain((0..cote).flat_map(|_| rgb))
        .collect();
    for _ in 0..cote {
        brut.extend_from_slice(&ligne);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &o in &brut {
        a = (a + u32::from(o)) % 65521;
        b = (b + a) % 65521;
    }
    let len = brut.len() as u16;
    let mut z = vec![0x78, 0x01, 0x01];
    z.extend_from_slice(&len.to_le_bytes());
    z.extend_from_slice(&(!len).to_le_bytes());
    z.extend_from_slice(&brut);
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&cote.to_be_bytes());
    ihdr.extend_from_slice(&cote.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    for (genre, corps) in [(&b"IHDR"[..], ihdr), (b"IDAT", z), (b"IEND", Vec::new())] {
        png.extend_from_slice(&(corps.len() as u32).to_be_bytes());
        let mut c = genre.to_vec();
        c.extend_from_slice(&corps);
        png.extend_from_slice(&c);
        png.extend_from_slice(&crc32(&c).to_be_bytes());
    }
    png
}

/// Un PDF d'une page qui porte un seul mot (table xref calculée).
fn pdf_mot(mot: &str) -> Vec<u8> {
    let flux = format!("BT /F1 36 Tf 72 700 Td ({mot}) Tj ET");
    let objets = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .to_string(),
        format!("<< /Length {} >>\nstream\n{flux}\nendstream", flux.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut s = String::from("%PDF-1.4\n");
    let mut decalages = Vec::new();
    for (i, o) in objets.iter().enumerate() {
        decalages.push(s.len());
        s.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
    }
    let xref = s.len();
    s.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objets.len() + 1
    ));
    for d in decalages {
        s.push_str(&format!("{d:010} 00000 n \n"));
    }
    s.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objets.len() + 1
    ));
    s.into_bytes()
}

fn ms(d: Duration) -> u128 {
    d.as_millis()
}

/// Le dossier de l'utilisateur, remplacé par `~` dans tout ce qui est écrit ou affiché ; toute
/// adresse de courriel remplacée par `<courriel>`.
fn masquer(texte: &str) -> String {
    let courriel = regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+\.[A-Za-z0-9.-]+").ok();
    let mut t = match courriel {
        Some(re) => re.replace_all(texte, "<courriel>").into_owned(),
        None => texte.to_string(),
    };
    if let Ok(home) = std::env::var("USERPROFILE") {
        for forme in [
            home.clone(),
            home.replace('\\', "/"),
            home.replace('\\', "\\\\"),
        ] {
            if !forme.is_empty() {
                t = t.replace(&forme, "~");
            }
        }
    }
    t
}

fn court(texte: &str, n: usize) -> String {
    let t: String = texte.chars().take(n).collect();
    masquer(&t.replace('\n', " "))
}

// ── Le processus `claude` ───────────────────────────────────────────────────────────────────

enum Ligne {
    Json(Value),
    Brut(String),
    Fin,
}

/// Une demande du binaire à l'hôte (protocole de contrôle).
struct Demande {
    sous_type: String,
    corps: Value,
}

/// Ce que l'hôte répond à une demande (le champ `response` du control_response).
type Politique<'a> = dyn FnMut(&Demande) -> Value + 'a;

struct Claude {
    enfant: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Ligne>,
    t0: Instant,
    journal: fs::File,
    stderr: Arc<Mutex<String>>,
    compteur: u32,
}

/// Ce qu'on a vu pendant un tour.
#[derive(Default)]
struct Tour {
    evenements: Vec<AgentEvent>,
    resultat: Option<Value>,
    init: Option<Value>,
    premier_flux: Option<Duration>,
    premier_texte: Option<Duration>,
    premiere_pensee: Option<Duration>,
    fin: Option<Duration>,
    texte: String,
    pensee: String,
    cartes: Vec<Value>,
    gardes: Vec<Value>,
    outils: Vec<String>,
    resultats_outils: Vec<(bool, String)>,
    dernier_uuid_assistant: Option<String>,
    interrompu_a: Option<Duration>,
    reponse_interruption: Option<Duration>,
    lignes_inconnues: Vec<String>,
}

struct Options<'a> {
    binaire: &'a str,
    cwd: &'a Path,
    transcript: PathBuf,
    extra: Vec<String>,
}

impl Claude {
    fn lancer(o: &Options) -> Resultat<Self> {
        let mut cmd = Command::new(o.binaire);
        cmd.args([
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
        ]);
        cmd.args(&o.extra);
        cmd.current_dir(o.cwd);
        // ★ L'environnement de l'hôte ne doit RIEN transmettre de Claude Code : le spike tourne
        //   lui-même dans une session (CLAUDECODE, CLAUDE_CODE_SIMPLE = mode --bare, une URL de
        //   proxy…). L'app non plus n'a rien à y mettre : le binaire lit le login local.
        for (k, _) in std::env::vars() {
            if k.starts_with("CLAUDE") || k.starts_with("ANTHROPIC") {
                cmd.env_remove(&k);
            }
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let t0 = Instant::now();
        let mut enfant = cmd.spawn()?;
        let stdout = enfant.stdout.take().ok_or("pas de stdout")?;
        let stderr_flux = enfant.stderr.take().ok_or("pas de stderr")?;
        let stdin = enfant.stdin.take();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for ligne in BufReader::new(stdout).lines() {
                let Ok(ligne) = ligne else { break };
                let msg = match serde_json::from_str::<Value>(&ligne) {
                    Ok(v) => Ligne::Json(v),
                    Err(_) => Ligne::Brut(ligne),
                };
                if tx.send(msg).is_err() {
                    break;
                }
            }
            let _ = tx.send(Ligne::Fin);
        });
        let stderr = Arc::new(Mutex::new(String::new()));
        let copie = Arc::clone(&stderr);
        thread::spawn(move || {
            let mut s = String::new();
            let _ = BufReader::new(stderr_flux).read_to_string(&mut s);
            if let Ok(mut c) = copie.lock() {
                *c = s;
            }
        });
        if let Some(parent) = o.transcript.parent() {
            fs::create_dir_all(parent)?;
        }
        let journal = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&o.transcript)?;
        Ok(Self {
            enfant,
            stdin,
            rx,
            t0,
            journal,
            stderr,
            compteur: 0,
        })
    }

    fn ecrire(&mut self, v: &Value) -> Resultat<()> {
        let ligne = v.to_string();
        writeln!(self.journal, "{}", masquer(&format!("> {ligne}")))?;
        let stdin = self.stdin.as_mut().ok_or("stdin fermé")?;
        stdin.write_all(ligne.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        Ok(())
    }

    fn controle(&mut self, requete: Value) -> Resultat<String> {
        self.compteur += 1;
        let id = format!("fr-{}", self.compteur);
        self.ecrire(&json!({"type": "control_request", "request_id": id, "request": requete}))?;
        Ok(id)
    }

    /// `initialize` : les hooks que l'hôte veut recevoir, et le prompt système.
    fn initialiser(&mut self, garde: bool, systeme: Option<&str>) -> Resultat<(Duration, Value)> {
        let mut req = json!({"subtype": "initialize"});
        if garde {
            req["hooks"] = json!({"PreToolUse": [{"matcher": null, "hookCallbackIds": ["garde"]}]});
        }
        if let Some(s) = systeme {
            req["systemPrompt"] = json!([s]);
        }
        let id = self.controle(req)?;
        let debut = Instant::now();
        loop {
            match self.lire(debut)? {
                Some(v) if v["type"] == "control_response" && v["response"]["request_id"] == id => {
                    return Ok((self.t0.elapsed(), v["response"].clone()));
                }
                Some(_) => {}
                None => return Err("le processus est parti avant de répondre à initialize".into()),
            }
        }
    }

    /// Une ligne JSON (journalisée), ou `None` à la fin du flux.
    fn lire(&mut self, debut: Instant) -> Resultat<Option<Value>> {
        loop {
            if debut.elapsed() > DELAI_TOUR {
                let _ = self.enfant.kill();
                return Err("délai dépassé : processus tué".into());
            }
            match self.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Ligne::Json(v)) => {
                    // ★ La réponse à `initialize` porte le COMPTE (courriel, organisation,
                    //   abonnement) : jamais écrit, même hors dépôt.
                    let mut copie = v.clone();
                    if let Some(r) = copie.pointer_mut("/response/response")
                        && let Some(o) = r.as_object_mut()
                        && o.contains_key("account")
                    {
                        o.insert("account".into(), json!("<masqué>"));
                    }
                    writeln!(self.journal, "{}", masquer(&format!("< {copie}")))?;
                    return Ok(Some(v));
                }
                Ok(Ligne::Brut(b)) => {
                    writeln!(self.journal, "{}", masquer(&format!("< (brut) {b}")))?;
                }
                Ok(Ligne::Fin) | Err(RecvTimeoutError::Disconnected) => return Ok(None),
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }

    fn utilisateur(&mut self, contenu: Value) -> Resultat<()> {
        self.ecrire(&json!({
            "type": "user",
            "message": {"role": "user", "content": contenu},
            "parent_tool_use_id": null,
            "session_id": "",
        }))
    }

    /// Un tour : envoie le message, sert les demandes de contrôle, s'arrête au `result`.
    /// `interrompre` : envoie `interrupt` dès le premier texte.
    fn tour(
        &mut self,
        contenu: Value,
        politique: &mut Politique,
        interrompre: bool,
    ) -> Resultat<Tour> {
        let debut = Instant::now();
        self.utilisateur(contenu)?;
        let mut t = Tour::default();
        let mut id_interruption = None;
        loop {
            let Some(v) = self.lire(debut)? else {
                t.lignes_inconnues.push("flux fermé avant le result".into());
                break;
            };
            let genre = v["type"].as_str().unwrap_or_default().to_string();
            match genre.as_str() {
                "control_request" => {
                    let d = Demande {
                        sous_type: v["request"]["subtype"].as_str().unwrap_or_default().into(),
                        corps: v["request"].clone(),
                    };
                    if d.sous_type == "can_use_tool" {
                        t.cartes.push(d.corps.clone());
                    } else if d.sous_type == "hook_callback" {
                        t.gardes.push(d.corps.clone());
                    }
                    let reponse = politique(&d);
                    self.ecrire(&json!({
                        "type": "control_response",
                        "response": {"subtype": "success", "request_id": v["request_id"], "response": reponse},
                    }))?;
                }
                "control_response" => {
                    if id_interruption.is_some()
                        && v["response"]["request_id"].as_str() == id_interruption.as_deref()
                    {
                        t.reponse_interruption = Some(debut.elapsed());
                    }
                }
                "system" if v["subtype"] == "init" => t.init = Some(v.clone()),
                _ => {}
            }
            if genre == "stream_event" && t.premier_flux.is_none() {
                t.premier_flux = Some(debut.elapsed());
            }
            if genre == "assistant" {
                for b in v["message"]["content"].as_array().into_iter().flatten() {
                    if b["type"] == "tool_use" {
                        t.outils.push(format!(
                            "{} {}",
                            b["name"].as_str().unwrap_or("?"),
                            court(&b["input"].to_string(), 80)
                        ));
                    }
                }
                if let Some(u) = v["uuid"].as_str() {
                    t.dernier_uuid_assistant = Some(u.to_string());
                }
            }
            for e in to_agent_events(&v) {
                match &e {
                    AgentEvent::TextDelta { text } => {
                        if t.premier_texte.is_none() {
                            t.premier_texte = Some(debut.elapsed());
                        }
                        t.texte.push_str(text);
                        if interrompre && id_interruption.is_none() {
                            id_interruption = Some(self.controle(json!({"subtype": "interrupt"}))?);
                            t.interrompu_a = Some(debut.elapsed());
                        }
                    }
                    AgentEvent::ThinkingDelta { text, .. } => {
                        if t.premiere_pensee.is_none() {
                            t.premiere_pensee = Some(debut.elapsed());
                        }
                        t.pensee.push_str(text);
                    }
                    AgentEvent::ToolResult { is_error, text, .. } => {
                        t.resultats_outils.push((*is_error, court(text, 100)));
                    }
                    _ => {}
                }
                t.evenements.push(e);
            }
            if genre == "result" {
                t.fin = Some(debut.elapsed());
                t.resultat = Some(v);
                break;
            }
        }
        Ok(t)
    }

    /// Ferme l'entrée et attend la sortie : (code, durée de la fermeture, fin de stderr).
    fn fermer(mut self) -> (Option<i32>, Duration, String) {
        let debut = Instant::now();
        self.stdin.take();
        let mut code = None;
        while debut.elapsed() < Duration::from_secs(20) {
            if let Ok(Some(s)) = self.enfant.try_wait() {
                code = s.code();
                break;
            }
            // Vider le flux pour que l'enfant ne bloque pas sur un tuyau plein.
            let _ = self.rx.recv_timeout(Duration::from_millis(50));
        }
        if code.is_none() {
            let _ = self.enfant.kill();
        }
        thread::sleep(Duration::from_millis(100));
        let err = self.stderr.lock().map(|s| s.clone()).unwrap_or_default();
        (code, debut.elapsed(), court(err.trim(), 300))
    }
}

// ── Les arguments d'un tour (ce que fera fr-t7-harness-claude) ───────────────────────────────

struct Session<'a> {
    nouvelle: Option<&'a str>,
    reprise: Option<&'a str>,
    fork_a: Option<&'a str>,
}

fn arguments(session: &Session, outils: &str, permis: &str, pensee: bool) -> Vec<String> {
    let mut a: Vec<String> = [
        "--model",
        "haiku",
        "--include-partial-messages",
        // ★ Isolement (comme `settingSources: []` + `strictMcpConfig` du SDK) : ni CLAUDE.md, ni
        //   réglages, ni serveurs MCP de l'utilisateur, ni connecteurs du compte.
        "--setting-sources=",
        "--strict-mcp-config",
        // ★ La carte passe par le flux : `can_use_tool` arrive en control_request.
        "--permission-prompt-tool",
        "stdio",
        "--permission-mode",
        "default",
        "--tools",
        outils,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    if !permis.is_empty() {
        a.extend(["--allowedTools".into(), permis.into()]);
    }
    if pensee {
        a.extend([
            "--thinking=adaptive".into(),
            "--thinking-display".into(),
            "summarized".into(),
        ]);
    }
    if let Some(id) = session.reprise {
        a.extend(["--resume".into(), id.into()]);
    }
    if let Some(at) = session.fork_a {
        a.extend([
            "--fork-session".into(),
            "--resume-session-at".into(),
            at.into(),
        ]);
    }
    if let Some(id) = session.nouvelle {
        a.extend(["--session-id".into(), id.into()]);
    }
    a
}

fn systeme(bac: &Path, code: &str) -> String {
    PROMPT_FACTICE
        .replace("{{vault}}", &bac.display().to_string())
        .replace("{{code}}", code)
}

fn tout_accepter(_: &Demande) -> Value {
    json!({})
}

fn rapport_tour(nom: &str, t: &Tour) {
    let r = t.resultat.as_ref();
    let opt = |d: Option<Duration>| d.map_or("—".into(), |d| format!("{} ms", ms(d)));
    println!("  [{nom}]");
    println!(
        "    flux {} · 1er texte {} · 1re pensée {} · result {}",
        opt(t.premier_flux),
        opt(t.premier_texte),
        opt(t.premiere_pensee),
        opt(t.fin)
    );
    if let Some(r) = r {
        println!(
            "    result : subtype={} is_error={} num_turns={} duration_ms={} duration_api_ms={} cost={}",
            r["subtype"],
            r["is_error"],
            r["num_turns"],
            r["duration_ms"],
            r["duration_api_ms"],
            r["total_cost_usd"]
        );
        if r["is_error"] == true || r["subtype"] != "success" {
            println!(
                "    erreurs : {} / {}",
                court(&r["errors"].to_string(), 200),
                court(&r["result"].to_string(), 200)
            );
        }
    }
    let mut genres = std::collections::BTreeMap::<&str, usize>::new();
    for e in &t.evenements {
        let g = match e {
            AgentEvent::TextDelta { .. } => "text-delta",
            AgentEvent::ThinkingDelta { .. } => "thinking-delta",
            AgentEvent::Assistant { .. } => "assistant",
            AgentEvent::ToolResult { .. } => "tool-result",
            AgentEvent::Usage { .. } => "usage",
            AgentEvent::Done { .. } => "done",
            AgentEvent::Error { .. } => "error",
        };
        *genres.entry(g).or_default() += 1;
    }
    println!("    AgentEvent : {genres:?}");
    println!("    texte : « {} »", court(&t.texte, 160));
    if !t.pensee.is_empty() {
        println!(
            "    pensée ({} car.) : « {} »",
            t.pensee.chars().count(),
            court(&t.pensee, 120)
        );
    }
    for o in &t.outils {
        println!("    outil : {o}");
    }
    for (e, x) in &t.resultats_outils {
        println!("    résultat outil (erreur={e}) : {x}");
    }
    for g in &t.gardes {
        println!(
            "    garde : {} {}",
            g["input"]["tool_name"],
            court(&g["input"]["tool_input"].to_string(), 60)
        );
    }
    for c in &t.cartes {
        println!(
            "    carte : {} {} · suggestions={} · raison={}",
            c["tool_name"],
            court(&c["input"].to_string(), 60),
            court(&c["permission_suggestions"].to_string(), 160),
            court(&c["decision_reason"].to_string(), 80)
        );
    }
    for l in &t.lignes_inconnues {
        println!("    ! {l}");
    }
}

fn rapport_init(init: Option<&Value>) {
    let Some(i) = init else {
        println!("    (pas de message system/init)");
        return;
    };
    println!(
        "    init : version={} modèle={} permissionMode={} outils={} mcp_servers={} plugins={} skills={} slash_commands={} agents={}",
        i["claude_code_version"],
        i["model"],
        i["permissionMode"],
        i["tools"],
        i["mcp_servers"],
        i["plugins"],
        court(&i["skills"].to_string(), 200),
        i["slash_commands"].as_array().map_or(0, Vec::len),
        court(&i["agents"].to_string(), 120),
    );
}

fn fermeture(c: Claude) {
    let (code, d, err) = c.fermer();
    println!(
        "    sortie : code={code:?} en {} ms après fermeture de stdin{}",
        ms(d),
        if err.is_empty() {
            String::new()
        } else {
            format!(" · stderr « {err} »")
        }
    );
}

// ── Scénarios ───────────────────────────────────────────────────────────────────────────────

struct Contexte {
    binaire: String,
    bac: PathBuf,
    transcripts: PathBuf,
}

impl Contexte {
    fn options(&self, nom: &str, extra: Vec<String>) -> Options<'_> {
        Options {
            binaire: &self.binaire,
            cwd: &self.bac,
            transcript: self.transcripts.join(format!("{nom}.jsonl")),
            extra,
        }
    }
}

fn texte(cx: &Contexte) -> Resultat<()> {
    println!("\n== texte : un tour de texte, isolement, latences");
    let id = uuid();
    let s = Session {
        nouvelle: Some(&id),
        reprise: None,
        fork_a: None,
    };
    let mut c = Claude::lancer(&cx.options("texte", arguments(&s, "Read", "", false)))?;
    let (t_init, rep) = c.initialiser(false, Some(&systeme(&cx.bac, CODE)))?;
    println!(
        "    initialize : {} ms après le lancement · réponse {}",
        ms(t_init),
        court(&rep["subtype"].to_string(), 40)
    );
    let t = c.tour(
        json!("Dis bonjour en un seul mot."),
        &mut tout_accepter,
        false,
    )?;
    rapport_init(t.init.as_ref());
    rapport_tour("tour 1", &t);
    // Un 2e tour dans le MÊME processus : le coût de lancement ne se paie qu'une fois.
    let t2 = c.tour(json!("Et au revoir, en un mot."), &mut tout_accepter, false)?;
    rapport_tour("tour 2 (même processus)", &t2);
    let t3 = c.tour(
        json!("Quel est ton code de test ? Réponds seulement le code."),
        &mut tout_accepter,
        false,
    )?;
    rapport_tour("tour 3 (prompt reçu par initialize ?)", &t3);
    println!(
        "    prompt système appliqué ? {}",
        t3.texte.to_uppercase().contains(CODE)
    );
    fermeture(c);
    Ok(())
}

fn pensee(cx: &Contexte) -> Resultat<()> {
    println!("\n== pensee : pensée adaptative résumée");
    let mut extra = arguments(
        &Session {
            nouvelle: None,
            reprise: None,
            fork_a: None,
        },
        "Read",
        "",
        true,
    );
    extra.extend(["--effort".into(), "high".into()]);
    let mut c = Claude::lancer(&cx.options("pensee", extra))?;
    c.initialiser(false, Some(&systeme(&cx.bac, CODE)))?;
    let t = c.tour(
        json!("Réfléchis avant de répondre : combien de lundis compte une année non bissextile qui commence un lundi ? Donne seulement le nombre."),
        &mut tout_accepter,
        false,
    )?;
    rapport_tour("pensée", &t);
    fermeture(c);
    Ok(())
}

fn permissions(cx: &Contexte) -> Resultat<()> {
    println!("\n== permissions : garde PreToolUse et carte can_use_tool par le flux");
    fs::write(cx.bac.join("note.md"), "Le mot du jour est GIROFLE.\n")?;
    fs::write(cx.bac.join("secret.md"), "Ne doit pas être lu.\n")?;
    for f in ["a.txt", "b.txt", "c.txt"] {
        let _ = fs::remove_file(cx.bac.join(f));
    }
    let s = Session {
        nouvelle: None,
        reprise: None,
        fork_a: None,
    };
    let mut c = Claude::lancer(&cx.options(
        "permissions",
        arguments(&s, "Read,Write,Bash", "Read", false),
    ))?;
    c.initialiser(true, Some(&systeme(&cx.bac, CODE)))?;

    // La GARDE de Fragment (policyModel) : refuse secret.md, force la carte pour Bash (même `ls`,
    // que Claude Code tiendrait pour lecture seule), laisse le reste aux règles du binaire.
    // La CARTE : refuse a.txt, accepte b.txt avec « toujours » (suggestions du binaire), refuse Bash.
    let mut politique = |d: &Demande| -> Value {
        match d.sous_type.as_str() {
            "hook_callback" => {
                let entree = &d.corps["input"];
                let outil = entree["tool_name"].as_str().unwrap_or_default();
                let tout = entree["tool_input"].to_string();
                if tout.contains("secret.md") {
                    json!({"hookSpecificOutput": {"hookEventName": "PreToolUse",
                        "permissionDecision": "deny",
                        "permissionDecisionReason": "La garde de test refuse ce fichier."}})
                } else if outil == "Bash" {
                    json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "ask"}})
                } else {
                    json!({})
                }
            }
            "can_use_tool" => {
                let outil = d.corps["tool_name"].as_str().unwrap_or_default();
                let tout = d.corps["input"].to_string();
                if outil == "Write" && tout.contains("b.txt") {
                    // ★ « Toujours » par une RÈGLE de session (`addRules`), pas par la
                    //   suggestion du binaire (2.1.316 propose `setMode: acceptEdits`, qui
                    //   ouvrirait toutes les écritures).
                    json!({"behavior": "allow", "updatedInput": d.corps["input"],
                        "updatedPermissions": [{"type": "addRules", "rules": [{"toolName": "Write"}],
                            "behavior": "allow", "destination": "session"}]})
                } else {
                    json!({"behavior": "deny", "message": "Refusé par l'utilisateur (test)."})
                }
            }
            _ => json!({}),
        }
    };
    let demandes = [
        (
            "lecture permise",
            "Lis le fichier note.md et donne le mot du jour.",
        ),
        (
            "lecture gardée",
            "Lis le fichier secret.md. S'il est refusé, dis simplement « refusé ».",
        ),
        (
            "écriture refusée",
            "Écris le mot UN dans le fichier a.txt avec l'outil Write. Si c'est refusé, n'insiste pas.",
        ),
        (
            "écriture permise, toujours",
            "Écris le mot DEUX dans le fichier b.txt avec l'outil Write.",
        ),
        (
            "écriture après « toujours »",
            "Écris le mot TROIS dans le fichier c.txt avec l'outil Write.",
        ),
        (
            "lecture seule forcée à la carte",
            "Lance la commande `ls` avec l'outil Bash. Si c'est refusé, n'insiste pas.",
        ),
    ];
    for (nom, texte) in demandes {
        let t = c.tour(json!(texte), &mut politique, false)?;
        if nom == "lecture permise" {
            rapport_init(t.init.as_ref());
        }
        rapport_tour(nom, &t);
    }
    for f in ["a.txt", "b.txt", "c.txt"] {
        println!(
            "    fichier {f} : {}",
            fs::read_to_string(cx.bac.join(f)).map_or("absent".into(), |s| court(s.trim(), 20))
        );
    }
    fermeture(c);
    Ok(())
}

/// Le transcript que le binaire range sous `~/.claude/projects/<cwd encodé>/<id>.jsonl`.
fn transcript_session(bac: &Path, id: &str) -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE").ok()?;
    let racine = Path::new(&home).join(".claude").join("projects");
    let encode: String = bac
        .display()
        .to_string()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let p = racine.join(&encode).join(format!("{id}.jsonl"));
    if p.exists() {
        return Some(p);
    }
    // Repli : chercher le fichier par son nom dans tous les projets.
    fs::read_dir(&racine).ok()?.flatten().find_map(|d| {
        let q = d.path().join(format!("{id}.jsonl"));
        q.exists().then_some(q)
    })
}

fn relire_session(bac: &Path, id: &str) {
    let Some(p) = transcript_session(bac, id) else {
        println!("    transcript {id} : introuvable");
        return;
    };
    let lignes: Vec<Value> = fs::read_to_string(&p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let messages: Vec<Value> = lignes
        .iter()
        .filter(|v| v["type"] == "user" || v["type"] == "assistant")
        .cloned()
        .collect();
    let entrees = to_chat_entries(&messages);
    println!(
        "    transcript {} : {} lignes, {} messages, to_chat_entries → {} bulles",
        masquer(&p.display().to_string()),
        lignes.len(),
        messages.len(),
        entrees.len()
    );
    for e in &entrees {
        println!(
            "      {:?} last={} « {} »",
            e.role,
            e.last.as_deref().unwrap_or("—"),
            court(&format!("{:?}", e.blocks), 80)
        );
    }
}

fn sessions(cx: &Contexte) -> Resultat<()> {
    println!("\n== sessions : id choisi par l'hôte, reprise, fork à un point donné");
    let id = uuid();
    let neuve = Session {
        nouvelle: Some(&id),
        reprise: None,
        fork_a: None,
    };
    let mut c = Claude::lancer(&cx.options("session-1", arguments(&neuve, "Read", "", false)))?;
    c.initialiser(false, Some(&systeme(&cx.bac, CODE)))?;
    let t1 = c.tour(
        json!("Retiens le mot ALBATROS. Réponds seulement OK."),
        &mut tout_accepter,
        false,
    )?;
    rapport_tour("tour 1 (session neuve)", &t1);
    let session_vue = t1.resultat.as_ref().map(|r| r["session_id"].clone());
    println!(
        "    session_id rendu = choisi ? {}",
        session_vue == Some(json!(id))
    );
    let point = t1
        .dernier_uuid_assistant
        .clone()
        .ok_or("pas d'uuid assistant")?;
    fermeture(c);

    let reprise = Session {
        nouvelle: None,
        reprise: Some(&id),
        fork_a: None,
    };
    let debut = Instant::now();
    let mut c =
        Claude::lancer(&cx.options("session-2-reprise", arguments(&reprise, "Read", "", false)))?;
    c.initialiser(false, Some(&systeme(&cx.bac, CODE)))?;
    let t2 = c.tour(
        json!(
            "Retiens aussi le mot BALEINE, puis cite tous les mots que je t'ai demandé de retenir."
        ),
        &mut tout_accepter,
        false,
    )?;
    println!(
        "    reprise : lancement → result {} ms",
        ms(debut.elapsed())
    );
    rapport_tour("tour 2 (--resume)", &t2);
    println!(
        "    même session ? {}",
        t2.resultat.as_ref().map(|r| r["session_id"].clone()) == Some(json!(id))
    );
    fermeture(c);

    let branche = uuid();
    let fork = Session {
        nouvelle: Some(&branche),
        reprise: Some(&id),
        fork_a: Some(&point),
    };
    let mut c = Claude::lancer(&cx.options("session-3-fork", arguments(&fork, "Read", "", false)))?;
    c.initialiser(false, Some(&systeme(&cx.bac, CODE)))?;
    let t3 = c.tour(
        json!("Cite tous les mots que je t'ai demandé de retenir."),
        &mut tout_accepter,
        false,
    )?;
    rapport_tour("tour 3 (fork au tour 1)", &t3);
    println!(
        "    branche : session_id = id choisi ? {} · BALEINE absent ? {} · ALBATROS présent ? {}",
        t3.resultat.as_ref().map(|r| r["session_id"].clone()) == Some(json!(branche)),
        !t3.texte.to_uppercase().contains("BALEINE"),
        t3.texte.to_uppercase().contains("ALBATROS")
    );
    fermeture(c);
    relire_session(&cx.bac, &id);
    relire_session(&cx.bac, &branche);
    Ok(())
}

/// Une reprise avec un AUTRE prompt : lequel le modèle voit-il ? (`--system-prompt-snapshot`)
fn instantane(cx: &Contexte) -> Resultat<()> {
    println!(
        "
== instantane : le prompt système d'une session reprise"
    );
    for (nom, extra) in [
        ("defaut", Vec::<String>::new()),
        ("off", vec!["--system-prompt-snapshot".into(), "off".into()]),
    ] {
        let id = uuid();
        let neuve = Session {
            nouvelle: Some(&id),
            reprise: None,
            fork_a: None,
        };
        let mut c = Claude::lancer(&cx.options(
            &format!("instantane-{nom}-1"),
            arguments(&neuve, "Read", "", false),
        ))?;
        c.initialiser(false, Some(&systeme(&cx.bac, CODE)))?;
        c.tour(json!("Réponds seulement OK."), &mut tout_accepter, false)?;
        fermeture(c);
        let reprise = Session {
            nouvelle: None,
            reprise: Some(&id),
            fork_a: None,
        };
        let mut a = arguments(&reprise, "Read", "", false);
        a.extend(extra);
        let mut c = Claude::lancer(&cx.options(&format!("instantane-{nom}-2"), a))?;
        c.initialiser(false, Some(&systeme(&cx.bac, "SORBIER")))?;
        let t = c.tour(
            json!("Quel est ton code de test ? Réponds seulement le code."),
            &mut tout_accepter,
            false,
        )?;
        println!(
            "  [{nom}] reprise avec le code SORBIER : le modèle répond « {} »",
            court(&t.texte, 40)
        );
        fermeture(c);
    }
    Ok(())
}

fn interruption(cx: &Contexte) -> Resultat<()> {
    println!("\n== interruption : control_request interrupt au premier texte");
    let s = Session {
        nouvelle: None,
        reprise: None,
        fork_a: None,
    };
    let mut c = Claude::lancer(&cx.options("interruption", arguments(&s, "Read", "", false)))?;
    c.initialiser(false, Some(&systeme(&cx.bac, CODE)))?;
    let t = c.tour(
        json!("Écris les nombres de un à cent en toutes lettres, un par ligne."),
        &mut tout_accepter,
        true,
    )?;
    rapport_tour("interrompu", &t);
    if let (Some(a), Some(f)) = (t.interrompu_a, t.fin) {
        println!(
            "    interrupt envoyé à {} ms · réponse de contrôle {} · result {} ms après l'interrupt",
            ms(a),
            t.reponse_interruption
                .map_or("—".into(), |d| format!("{} ms", ms(d))),
            ms(f.saturating_sub(a))
        );
    }
    let t2 = c.tour(json!("Réponds seulement OK."), &mut tout_accepter, false)?;
    rapport_tour("tour suivant, même processus", &t2);
    fermeture(c);
    Ok(())
}

fn pieces(cx: &Contexte) -> Resultat<()> {
    println!("\n== pieces : image et PDF en blocs de contenu");
    let s = Session {
        nouvelle: None,
        reprise: None,
        fork_a: None,
    };
    let mut c = Claude::lancer(&cx.options("pieces", arguments(&s, "Read", "", false)))?;
    c.initialiser(false, Some(&systeme(&cx.bac, CODE)))?;
    let contenu = json!([
        {"type": "image", "source": {"type": "base64", "media_type": "image/png",
            "data": base64(&png_uni(32, [220, 30, 30]))}},
        {"type": "document", "source": {"type": "base64", "media_type": "application/pdf",
            "data": base64(&pdf_mot("PERROQUET"))}},
        {"type": "text", "text": "De quelle couleur est l'image, et quel mot porte le PDF ? Réponds en une ligne."},
    ]);
    let t = c.tour(contenu, &mut tout_accepter, false)?;
    rapport_tour("image + PDF", &t);
    fermeture(c);
    Ok(())
}

/// Un lancement qui doit échouer : code de sortie, `result` éventuel, stderr.
fn echec(cx: &Contexte, nom: &str, extra: Vec<String>) -> Resultat<()> {
    let debut = Instant::now();
    let mut c = Claude::lancer(&cx.options(nom, extra))?;
    match c.initialiser(false, None) {
        Ok(_) => {
            let t = c.tour(json!("Réponds OK."), &mut tout_accepter, false)?;
            rapport_tour(nom, &t);
        }
        Err(e) => println!("  [{nom}] initialize : {e}"),
    }
    print!("    ({} ms) ", ms(debut.elapsed()));
    fermeture(c);
    Ok(())
}

fn erreurs(cx: &Contexte) -> Resultat<()> {
    println!("\n== erreurs : codes de sortie et result en erreur");
    let rien = Session {
        nouvelle: None,
        reprise: None,
        fork_a: None,
    };
    let mut modele = arguments(&rien, "Read", "", false);
    if let Some(i) = modele.iter().position(|a| a == "haiku") {
        modele[i] = "claude-modele-inexistant".into();
    }
    echec(cx, "modele-inconnu", modele)?;
    let absente = uuid();
    echec(
        cx,
        "reprise-inconnue",
        arguments(
            &Session {
                nouvelle: None,
                reprise: Some(&absente),
                fork_a: None,
            },
            "Read",
            "",
            false,
        ),
    )?;
    echec(
        cx,
        "option-inconnue",
        vec!["--option-qui-n-existe-pas".into()],
    )?;
    echec(
        cx,
        "session-id-invalide",
        arguments(
            &Session {
                nouvelle: Some("pas-un-uuid"),
                reprise: None,
                fork_a: None,
            },
            "Read",
            "",
            false,
        ),
    )?;
    Ok(())
}

fn main() -> Resultat<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut prendre = |cle: &str| -> Option<String> {
        let i = args.iter().position(|a| a == cle)?;
        let v = args.get(i + 1).cloned();
        args.drain(i..(i + 2).min(args.len()));
        v
    };
    let binaire = prendre("--claude").unwrap_or_else(|| "claude".into());
    let bac = PathBuf::from(prendre("--bac").unwrap_or_else(|| "C:/dev/s8-bac".into()));
    fs::create_dir_all(&bac)?;
    let version = Command::new(&binaire).arg("--version").output()?;
    println!(
        "binaire : {} · version : {}",
        masquer(&binaire),
        String::from_utf8_lossy(&version.stdout).trim()
    );
    let cx = Contexte {
        transcripts: bac.join("transcripts"),
        binaire,
        bac,
    };
    let tous = [
        "texte",
        "pensee",
        "permissions",
        "sessions",
        "instantane",
        "interruption",
        "pieces",
        "erreurs",
    ];
    let choisis: Vec<&str> = if args.is_empty() {
        tous.to_vec()
    } else {
        args.iter().map(String::as_str).collect()
    };
    for s in choisis {
        let r = match s {
            "texte" => texte(&cx),
            "pensee" => pensee(&cx),
            "permissions" => permissions(&cx),
            "sessions" => sessions(&cx),
            "instantane" => instantane(&cx),
            "interruption" => interruption(&cx),
            "pieces" => pieces(&cx),
            "erreurs" => erreurs(&cx),
            autre => Err(format!("scénario inconnu : {autre}").into()),
        };
        if let Err(e) = r {
            println!("  !! {s} : {}", masquer(&e.to_string()));
        }
    }
    Ok(())
}
