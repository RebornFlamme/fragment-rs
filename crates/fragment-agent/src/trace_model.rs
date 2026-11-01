// Port de app/src/ui/views/agent/trace/traceModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  LA TRACE d'un tour de l'agent — ce qu'il a fait AVANT de répondre : sa
//  pensée, ses outils, et ce qu'il s'est dit entre deux (la narration, repliée
//  ici au lieu d'encombrer la réponse). Pur.
//
//  Deux chemins pour UNE forme :
//    - en direct : un `TraceBuilder` suit les événements du tour ;
//    - à la relecture : `split_turn` découpe les blocs du transcript.
//  Règle commune : tout ce qui précède la DERNIÈRE pensée / le dernier outil est
//  la trace (un texte suivi d'un outil = une note) ; le texte qui suit = la réponse.
// ═══════════════════════════════════════════════════════════════════════════

use std::sync::LazyLock;

use serde_json::Value;

use crate::contrat::{ChatBlock, Entree, ResultatOutil};
use crate::motif::Motif;
use crate::utf16::{len_utf16, tete_utf16};

/// Les neuf animations de l'orbe : un verbe de l'agent chacune.
// TODO(contrat) : l'orbe est de la présentation (fragment-ui) ; seul le nom compte ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbState {
    Working,
    Searching,
    Solving,
    Listening,
    Connecting,
    Weaving,
    Composing,
    Breathing,
    Shaping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Thinking,
    Note,
    Tool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceStep {
    pub id: String,
    pub kind: StepKind,
    /// Pensée ou narration.
    pub text: Option<String>,
    /// Outil : son nom, son entrée, son résultat (absent = en cours).
    pub name: Option<String>,
    pub input: Option<Entree>,
    pub result: Option<ResultatOutil>,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
}

impl TraceStep {
    fn vide(id: String, kind: StepKind) -> Self {
        TraceStep { id, kind, text: None, name: None, input: None, result: None, started_at: None, ended_at: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceStatus {
    Running,
    Done,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    pub status: TraceStatus,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub steps: Vec<TraceStep>,
    /// La réponse s'écrit (en direct seulement) : plus aucune étape en cours. La
    /// trace, elle, est FINIE (`ended_at` = début de la réponse) : son chrono s'arrête.
    pub writing: Option<bool>,
}

/// Le type du part de chat qui porte une trace.
pub const AGENT_TRACE_PART: &str = "agent-trace";

/// Un outil qui démarre (`TraceBuilder::tool`).
#[derive(Debug, Clone)]
pub struct OutilEntrant {
    pub id: String,
    pub name: String,
    pub input: Option<Entree>,
}

/// Suit un tour en direct. Mutable ; `snapshot()` rend une copie à émettre.
pub struct TraceBuilder {
    trace: Trace,
    n: usize,
}

impl TraceBuilder {
    pub fn new(now: i64) -> Self {
        TraceBuilder { trace: Trace { status: TraceStatus::Running, started_at: Some(now), ended_at: None, steps: Vec::new(), writing: None }, n: 0 }
    }

    pub fn size(&self) -> usize {
        self.trace.steps.len()
    }

    fn prochain_id(&mut self) -> String {
        let id = format!("s{}", self.n);
        self.n += 1;
        id
    }

    /// Une pensée qui s'écrit (un nouveau bloc de pensée commence par un texte vide).
    /// ★ Elle COMMENCE à la fin de l'étape précédente, sinon au début du tour — pas
    /// à son premier delta : la pensée « summarized » arrive en retard (constaté :
    /// « A réfléchi 7 s » pour un tour de 27 s passé à réfléchir). C'est la règle de
    /// `split_turn` : le direct et la relecture donnent le même chiffre.
    pub fn thinking(&mut self, text: &str, now: i64, nouveau: bool) {
        if let Some(last) = self.trace.steps.last_mut() {
            if !nouveau && last.kind == StepKind::Thinking && last.ended_at.is_none() {
                last.text = Some(format!("{}{}", last.text.as_deref().unwrap_or(""), text));
                return;
            }
        }
        let avait_precedente = !self.trace.steps.is_empty();
        self.fermer(now);
        // ★ `debut` se lit APRÈS `fermer` (comme en TS) : une pensée précédente ouverte vient d'être close à `now`.
        let debut = if avait_precedente {
            self.trace.steps.last().and_then(|s| s.ended_at).unwrap_or(now)
        } else {
            self.trace.started_at.unwrap_or(now)
        };
        let mut s = TraceStep::vide(self.prochain_id(), StepKind::Thinking);
        s.text = Some(text.to_string());
        s.started_at = Some(debut.min(now));
        self.trace.steps.push(s);
    }

    /// La réponse commence à s'écrire : la pensée en cours se termine, et la trace
    /// avec elle — elle mesure ce qui PRÉCÈDE la réponse.
    pub fn ecrire(&mut self, now: i64) {
        self.fermer(now);
        self.trace.writing = Some(true);
        self.trace.ended_at = Some(now);
    }

    /// Ce que l'agent s'est dit avant un outil : une note de la trace.
    pub fn note(&mut self, text: &str, now: i64) {
        let t = text.trim();
        if t.is_empty() {
            return;
        }
        self.fermer(now);
        let mut s = TraceStep::vide(self.prochain_id(), StepKind::Note);
        s.text = Some(t.to_string());
        s.started_at = Some(now);
        s.ended_at = Some(now);
        self.trace.steps.push(s);
    }

    pub fn tool(&mut self, b: OutilEntrant, now: i64) {
        self.fermer(now);
        // Le texte qui s'écrivait n'était pas la réponse (une narration) : on n'écrit plus.
        self.trace.writing = Some(false);
        self.trace.ended_at = None;
        if self.trace.steps.iter().any(|s| s.kind == StepKind::Tool && s.id == b.id) {
            return;
        }
        let mut s = TraceStep::vide(b.id, StepKind::Tool);
        s.name = Some(b.name);
        s.input = b.input;
        s.started_at = Some(now);
        self.trace.steps.push(s);
    }

    pub fn result(&mut self, id: &str, result: ResultatOutil, now: i64) {
        if let Some(s) = self.trace.steps.iter_mut().find(|x| x.kind == StepKind::Tool && x.id == id) {
            s.result = Some(result);
            s.ended_at = Some(now);
        }
    }

    /// La réponse commence : la pensée en cours se termine.
    pub fn fermer(&mut self, now: i64) {
        for s in &mut self.trace.steps {
            if s.kind == StepKind::Thinking && s.ended_at.is_none() {
                s.ended_at = Some(now);
            }
        }
    }

    pub fn finish(&mut self, now: i64, error: bool) {
        self.fermer(now);
        // Un outil resté sans résultat (tour interrompu) s'arrête là.
        for s in &mut self.trace.steps {
            if s.ended_at.is_none() {
                s.ended_at = Some(now);
            }
        }
        self.trace.status = if error { TraceStatus::Error } else { TraceStatus::Done };
        // La réponse écrite ne compte pas : la trace s'est arrêtée quand elle a commencé.
        if self.trace.writing != Some(true) {
            self.trace.ended_at = Some(now);
        }
        self.trace.writing = Some(false);
    }

    pub fn snapshot(&self) -> Trace {
        self.trace.clone()
    }
}

/// La relecture d'un tour : sa trace (`None` : aucune pensée ni outil) et les textes de sa réponse.
#[derive(Debug, Clone, PartialEq)]
pub struct TourDecoupe {
    pub trace: Option<Trace>,
    pub answer: Vec<String>,
}

/// La relecture d'un tour (les blocs d'une bulle assistant, dans l'ordre).
/// `started_at` : l'heure de la question (le début du tour).
pub fn split_turn(blocks: &[ChatBlock], started_at: Option<i64>) -> TourDecoupe {
    let dernier = blocks.iter().rposition(|b| matches!(b, ChatBlock::Thinking { .. } | ChatBlock::Tool(_)));
    let debut_reponse = dernier.map_or(0, |d| d + 1);
    let answer: Vec<String> = blocks[debut_reponse..]
        .iter()
        .filter_map(|b| match b {
            ChatBlock::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    let Some(dernier) = dernier else { return TourDecoupe { trace: None, answer } };
    let mut steps: Vec<TraceStep> = Vec::new();
    let mut avant = started_at;
    for (i, b) in blocks[..=dernier].iter().enumerate() {
        match b {
            ChatBlock::Thinking { text, at } => {
                let mut s = TraceStep::vide(format!("s{i}"), StepKind::Thinking);
                s.text = Some(text.clone());
                s.started_at = avant;
                s.ended_at = *at;
                steps.push(s);
                avant = at.or(avant);
            }
            ChatBlock::Text { text, at } => {
                if !text.trim().is_empty() {
                    let mut s = TraceStep::vide(format!("s{i}"), StepKind::Note);
                    s.text = Some(text.trim().to_string());
                    s.started_at = *at;
                    s.ended_at = *at;
                    steps.push(s);
                }
            }
            ChatBlock::Tool(t) => {
                let mut s = TraceStep::vide(t.id.clone(), StepKind::Tool);
                s.name = Some(t.name.clone());
                s.input = t.input.clone();
                s.result = t.result.clone();
                s.started_at = t.at;
                s.ended_at = t.result_at.or(t.at);
                steps.push(s);
                avant = t.result_at.or(t.at).or(avant);
            }
            ChatBlock::Attachment { .. } => {}
        }
    }
    // La fin de la trace = celle de sa dernière étape (la réponse qui suit ne compte pas).
    let fins = blocks[..=dernier].iter().filter_map(|b| match b {
        ChatBlock::Tool(t) => t.result_at.or(t.at),
        ChatBlock::Text { at, .. } | ChatBlock::Thinking { at, .. } => *at,
        ChatBlock::Attachment { .. } => None,
    });
    let erreur = steps.iter().any(|s| s.kind == StepKind::Tool && s.result.is_none());
    let started = started_at.or(steps.first().and_then(|s| s.started_at));
    TourDecoupe {
        trace: Some(Trace { status: if erreur { TraceStatus::Error } else { TraceStatus::Done }, started_at: started, ended_at: fins.max(), steps, writing: None }),
        answer,
    }
}

// ── Ce qu'on DIT d'une étape ─────────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepView {
    pub icon: String,
    /// Le verbe, au présent pendant (« Lit »), au passé après (« A lu »).
    pub verb: String,
    /// La cible (un fichier, un motif, une commande…), montrée en puce.
    pub target: Option<String>,
    pub target_icon: Option<String>,
    /// La cible est du code (commande, motif) : police à chasse fixe.
    pub mono: Option<bool>,
}

/// Le dernier segment d'un chemin (`/` ou `\`) ; vide si `p` n'est pas une chaîne.
fn nom(p: Option<&Value>) -> String {
    match p.and_then(Value::as_str) {
        Some(s) => s.rsplit(['\\', '/']).next().unwrap_or(s).to_string(),
        None => String::new(),
    }
}

/// `String(x ?? '')` du JS.
fn texte(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(autre) => autre.to_string(),
    }
}

/// L'icône d'un fichier par son extension.
pub fn file_icon(path: &str) -> &'static str {
    let bas = path.to_lowercase();
    let ext = bas.rsplit('.').next().unwrap_or("");
    match ext {
        "md" => "file-text",
        "pdf" => "file-type",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" => "image",
        "html" | "htm" => "globe",
        "js" | "ts" | "mjs" | "cjs" | "json" | "css" | "py" => "file-code",
        _ => "file",
    }
}

/// La valeur d'un paramètre `clé=valeur` d'une ligne de commande (guillemets compris).
pub fn param(command: &str, cle: &str) -> Option<String> {
    let motif = Motif::new(&format!(r#"(?:^|\s)({}=("([^"]*)"|'([^']*)'|(\S+)))"#, regex::escape(cle)));
    let m = motif.captures(command)?;
    // groupes : 3 = double quote, 4 = simple, 5 = nu (comme m[2] ?? m[3] ?? m[4] du TS).
    m.get(3).or_else(|| m.get(4)).or_else(|| m.get(5)).map(|g| g.as_str().to_string())
}

type Verbe = (&'static str, &'static str, &'static str);

/// Les sous-commandes de `fragment` que l'agent emploie le plus, en mots.
const FRAGMENT: [(&str, Verbe); 17] = [
    ("agent:screen", ("monitor", "Regarde l’écran", "A regardé l’écran")),
    ("agent:leaf", ("panel-top", "Lit la vue", "A lu la vue")),
    ("agent:files", ("folder", "Liste les fichiers", "A listé les fichiers")),
    ("agent:focus", ("crosshair", "Active une vue", "A activé une vue")),
    ("annotation:draw", ("pen-line", "Dessine sur", "A dessiné sur")),
    ("annotation:strokes", ("pen-line", "Lit les dessins de", "A lu les dessins de")),
    ("annotation:erase", ("eraser", "Efface des traits", "A effacé des traits")),
    ("artefacts:place", ("shapes", "Pose un objet dans", "A posé un objet dans")),
    ("artefacts:list", ("shapes", "Liste les objets de", "A listé les objets de")),
    ("artefacts:move", ("move", "Déplace un objet", "A déplacé un objet")),
    ("artefacts:remove", ("shapes", "Retire un objet", "A retiré un objet")),
    ("command", ("square-terminal", "Lance la commande", "A lancé la commande")),
    ("commands", ("list", "Liste les commandes", "A listé les commandes")),
    ("eval", ("code", "Exécute du code dans Fragment", "A exécuté du code dans Fragment")),
    ("help", ("book-open", "Lit l’aide de Fragment", "A lu l’aide de Fragment")),
    ("plugin-reload", ("refresh-cw", "Recharge le plugin", "A rechargé le plugin")),
    ("plugins", ("puzzle", "Liste les plugins", "A listé les plugins")),
];

fn verbe_fragment(sous: &str) -> Option<Verbe> {
    FRAGMENT.iter().find(|(k, _)| *k == sous).map(|(_, v)| *v)
}

static CMD_FRAGMENT: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"(?i)^(?:&\s*)?fragment(?:\.cmd)?\s+(\S+)(.*)$"));

pub fn describe_step(step: &TraceStep, running: bool) -> StepView {
    let v = |icon: &str, present: &str, passe: &str, target: Option<String>| StepView {
        icon: icon.into(),
        verb: (if running { present } else { passe }).into(),
        target: target.filter(|t| !t.is_empty()),
        ..StepView::default()
    };
    match step.kind {
        StepKind::Thinking => return v("brain", "Réfléchit", "A réfléchi", None),
        StepKind::Note => return StepView { icon: "message-square".into(), verb: step.text.clone().unwrap_or_default(), ..StepView::default() },
        StepKind::Tool => {}
    }
    let vide = Entree::new();
    let i = step.input.as_ref().unwrap_or(&vide);
    let fichier = |present: &str, passe: &str, icon: &str| {
        let mut r = v(icon, present, passe, Some(nom(i.get("file_path"))));
        r.target_icon = Some(file_icon(&texte(i.get("file_path"))).into());
        r
    };
    match step.name.as_deref() {
        Some("Read") => {
            let mut r = fichier("Lit", "A lu", "file-text");
            r.icon = "file-text".into();
            r
        }
        Some("Write") => fichier("Écrit", "A écrit", "file-plus"),
        Some("Edit") => fichier("Modifie", "A modifié", "pencil"),
        Some("Glob") => {
            let mut r = v("search", "Cherche les fichiers", "A cherché les fichiers", Some(texte(i.get("pattern"))));
            r.mono = Some(true);
            r
        }
        Some("Grep") => v("search", "Cherche", "A cherché", Some(format!("« {} »", texte(i.get("pattern"))))),
        Some("Skill") => {
            let brut = match i.get("skill").filter(|x| !x.is_null()) {
                Some(s) => texte(Some(s)),
                None => texte(i.get("name")),
            };
            let court = brut.rsplit(':').next().unwrap_or(&brut).to_string();
            v("sparkles", "Utilise le skill", "A utilisé le skill", Some(court))
        }
        Some("Bash") | Some("PowerShell") => {
            let cmd = texte(i.get("command")).trim().to_string();
            if let Some(f) = CMD_FRAGMENT.captures(&cmd) {
                let sous = &f[1];
                let connu = verbe_fragment(sous);
                let (icon, present, passe) = connu.unwrap_or(("terminal", "Lance", "A lancé"));
                let reste = &f[2];
                let doc = param(reste, "doc");
                let cible = doc.clone().or_else(|| param(reste, "id")).or_else(|| if connu.is_some() { None } else { Some(format!("fragment {sous}")) });
                let affiche = cible.as_ref().map(|c| {
                    let n = nom(Some(&Value::String(c.clone())));
                    if n.is_empty() { c.clone() } else { n }
                });
                let mut r = v(icon, present, passe, affiche);
                if cible.is_some() && connu.is_none() {
                    r.mono = Some(true);
                } else if let Some(d) = doc {
                    r.target_icon = Some(file_icon(&d).into());
                }
                return r;
            }
            let court = if len_utf16(&cmd) > 60 { format!("{}…", tete_utf16(&cmd, 59)) } else { cmd };
            let mut r = v("terminal", "Exécute", "A exécuté", Some(court));
            r.mono = Some(true);
            r
        }
        autre => v("wrench", "Utilise", "A utilisé", autre.map(str::to_string)),
    }
}

// ── Durées et résumé ──────────────────────────────────────────────────────────

/// « 8 s », « 1 min 5 s ».
pub fn duree(ms: i64) -> String {
    let s = ((ms as f64) / 1000.0).round().max(0.0) as i64;
    if s < 60 {
        return format!("{s} s");
    }
    let m = s / 60;
    if s % 60 != 0 { format!("{m} min {} s", s % 60) } else { format!("{m} min") }
}

fn pluriel(n: usize, un: &str, plusieurs: &str) -> String {
    format!("{n} {}", if n > 1 { plusieurs } else { un })
}

/// Le résumé d'une trace finie : « A réfléchi 3 s · lu 2 fichiers · 1 recherche · 3 commandes ».
pub fn summarize(trace: &Trace) -> String {
    let outils: Vec<&TraceStep> = trace.steps.iter().filter(|s| s.kind == StepKind::Tool).collect();
    let compte = |noms: &[&str]| outils.iter().filter(|s| noms.contains(&s.name.as_deref().unwrap_or(""))).count();
    let pensee: i64 = trace
        .steps
        .iter()
        .filter(|s| s.kind == StepKind::Thinking)
        .map(|s| match (s.started_at, s.ended_at) {
            (Some(a), Some(b)) => b - a,
            _ => 0,
        })
        .sum();
    let mut morceaux: Vec<String> = Vec::new();
    if trace.steps.iter().any(|s| s.kind == StepKind::Thinking) {
        morceaux.push(if pensee >= 1000 { format!("a réfléchi {}", duree(pensee)) } else { "a réfléchi".into() });
    }
    let lus = compte(&["Read"]);
    if lus > 0 {
        morceaux.push(format!("lu {}", pluriel(lus, "fichier", "fichiers")));
    }
    let recherches = compte(&["Glob", "Grep"]);
    if recherches > 0 {
        morceaux.push(pluriel(recherches, "recherche", "recherches"));
    }
    let ecrits = compte(&["Write", "Edit"]);
    if ecrits > 0 {
        morceaux.push(pluriel(ecrits, "fichier modifié", "fichiers modifiés"));
    }
    let commandes = compte(&["Bash", "PowerShell"]);
    if commandes > 0 {
        morceaux.push(pluriel(commandes, "commande", "commandes"));
    }
    let autres = outils.len() - lus - recherches - ecrits - commandes;
    if autres > 0 {
        morceaux.push(pluriel(autres, "autre action", "autres actions"));
    }
    if morceaux.is_empty() {
        return if trace.status == TraceStatus::Error { "Interrompu".into() } else { "Terminé".into() };
    }
    let texte = morceaux.join(" · ");
    if trace.status == TraceStatus::Error {
        return format!("Interrompu · {texte}");
    }
    let mut c = texte.chars();
    match c.next() {
        Some(p) => p.to_uppercase().chain(c).collect(),
        None => texte,
    }
}

/// L'animation de l'orbe pour un tour en cours : ce que fait l'agent, en un geste —
/// écrit sa réponse, cherche, modifie, agit ; sinon il RÉFLÉCHIT.
pub fn orb_state_of(trace: &Trace) -> OrbState {
    if trace.writing == Some(true) {
        return OrbState::Composing;
    }
    let Some(step) = current_step(trace) else { return OrbState::Solving };
    if step.kind != StepKind::Tool || step.ended_at.is_some() {
        return OrbState::Solving;
    }
    match step.name.as_deref() {
        Some("Read") | Some("Grep") | Some("Glob") => OrbState::Searching,
        Some("Write") | Some("Edit") => OrbState::Composing,
        _ => OrbState::Working,
    }
}

/// Une trace faite d'UNE pensée : elle n'a pas de liste — son corps EST la pensée,
/// et sa durée est celle du résumé (le chrono ferait doublon).
pub fn seule_pensee(trace: &Trace) -> bool {
    trace.steps.len() == 1 && trace.steps[0].kind == StepKind::Thinking
}

/// L'étape EN COURS (la dernière sans fin), sinon la dernière.
pub fn current_step(trace: &Trace) -> Option<&TraceStep> {
    trace.steps.iter().rev().find(|s| s.ended_at.is_none() && s.kind != StepKind::Note).or_else(|| trace.steps.last())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrat::BlocOutil;
    use serde_json::json;

    fn entree(v: Value) -> Option<Entree> {
        v.as_object().cloned()
    }

    fn out(id: &str, name: &str, input: Option<Value>) -> OutilEntrant {
        OutilEntrant { id: id.into(), name: name.into(), input: input.and_then(entree) }
    }

    fn res(texte: &str) -> ResultatOutil {
        ResultatOutil { is_error: false, text: texte.into() }
    }

    // ── TraceBuilder — un tour en direct ──

    #[test]
    fn pensee_qui_secrit_outil_resultat_note_fin_etapes_durees_statut() {
        let mut t = TraceBuilder::new(1000);
        t.thinking("", 1000, true);
        t.thinking("Je cherche ", 1100, false);
        t.thinking("la note.", 1200, false);
        t.tool(out("a", "Grep", Some(json!({ "pattern": "photon" }))), 3000);
        t.result("a", res("cours.md"), 3200);
        t.note("Je lis la note.", 3300);
        t.tool(out("b", "Read", Some(json!({ "file_path": "C:/v/cours.md" }))), 3400);
        t.tool(out("b", "Read", None), 3401); // doublon ignoré
        assert_eq!(current_step(&t.snapshot()).map(|s| s.id.clone()), Some("b".into()));
        t.result("b", res("# Cours"), 3900);
        t.finish(5000, false);
        let s = t.snapshot();
        assert_eq!(s.status, TraceStatus::Done);
        assert_eq!(s.steps.iter().map(|x| x.kind).collect::<Vec<_>>(), vec![StepKind::Thinking, StepKind::Tool, StepKind::Note, StepKind::Tool]);
        assert_eq!((s.steps[0].text.as_deref(), s.steps[0].started_at, s.steps[0].ended_at), (Some("Je cherche la note."), Some(1000), Some(3000)));
        assert_eq!((s.steps[1].started_at, s.steps[1].ended_at), (Some(3000), Some(3200)));
        assert_eq!(s.steps[1].result.as_ref().map(|r| r.text.as_str()), Some("cours.md"));
        assert_eq!(s.ended_at, Some(5000));
    }

    #[test]
    fn une_pensee_commence_a_la_fin_de_letape_precedente_sinon_au_debut_du_tour_pas_a_son_1er_delta_en_retard() {
        let mut t = TraceBuilder::new(1000);
        t.thinking("", 8000, true);
        t.tool(out("r", "Read", None), 9000);
        t.result("r", res("x"), 9500);
        t.thinking("", 12000, true);
        t.ecrire(13000);
        let s = t.snapshot();
        assert_eq!((s.steps[0].started_at, s.steps[0].ended_at), (Some(1000), Some(9000)));
        assert_eq!((s.steps[2].started_at, s.steps[2].ended_at), (Some(9500), Some(13000)));
        assert_eq!(summarize(&Trace { status: TraceStatus::Done, ..s.clone() }), "A réfléchi 12 s · lu 1 fichier");
        // La réponse s'écrit : la trace est finie là (son chrono s'arrête), pas à la fin du tour.
        assert_eq!((s.writing, s.ended_at), (Some(true), Some(13000)));
        t.finish(20000, false);
        let f = t.snapshot();
        assert_eq!((f.status, f.writing, f.ended_at), (TraceStatus::Done, Some(false), Some(13000)));
    }

    #[test]
    fn une_seule_pensee_pas_de_liste_une_narration_suivie_dun_outil_rouvre_le_chrono() {
        let mut t = TraceBuilder::new(0);
        t.thinking("", 3000, true);
        t.ecrire(6000);
        assert!(seule_pensee(&t.snapshot()));
        assert_eq!((t.snapshot().steps[0].started_at, t.snapshot().steps[0].ended_at), (Some(0), Some(6000)));
        t.tool(out("x", "Read", None), 7000);
        assert_eq!((t.snapshot().writing, t.snapshot().ended_at), (Some(false), None));
        assert!(!seule_pensee(&t.snapshot()));
    }

    #[test]
    fn un_nouveau_bloc_de_pensee_apres_un_outil_est_une_nouvelle_etape_la_reponse_ferme_la_pensee() {
        let mut t = TraceBuilder::new(0);
        t.thinking("a", 0, true);
        t.tool(out("x", "Read", None), 100);
        t.thinking("", 200, true);
        t.thinking("b", 300, false);
        t.fermer(400);
        let s = t.snapshot();
        let pensees: Vec<(Option<String>, Option<i64>)> = s.steps.iter().filter(|x| x.kind == StepKind::Thinking).map(|x| (x.text.clone(), x.ended_at)).collect();
        assert_eq!(pensees, vec![(Some("a".into()), Some(100)), (Some("b".into()), Some(400))]);
    }

    #[test]
    fn un_outil_sans_resultat_tour_interrompu_sarrete_a_la_fin_erreur_statut_error() {
        let mut t = TraceBuilder::new(0);
        t.tool(out("x", "Bash", Some(json!({ "command": "sleep 9" }))), 10);
        t.finish(50, true);
        let s = t.snapshot();
        assert_eq!(s.status, TraceStatus::Error);
        assert_eq!(s.steps[0].ended_at, Some(50));
    }

    #[test]
    fn une_narration_vide_ne_fait_pas_detape_snapshot_est_une_copie() {
        let mut t = TraceBuilder::new(0);
        t.note("   ", 1);
        assert_eq!(t.size(), 0);
        t.note("ok", 2);
        let mut a = t.snapshot();
        a.steps[0].text = Some("modifié".into());
        assert_eq!(t.snapshot().steps[0].text.as_deref(), Some("ok"));
    }

    // ── splitTurn — la relecture d'un tour ──

    #[test]
    fn tout_jusquau_dernier_outil_pensee_la_trace_texte_intercale_note_le_reste_la_reponse() {
        let TourDecoupe { trace, answer } = split_turn(
            &[
                ChatBlock::Thinking { text: "Hmm".into(), at: Some(1500) },
                ChatBlock::Text { text: "Je regarde.".into(), at: Some(1500) },
                ChatBlock::Tool(BlocOutil {
                    id: "t1".into(),
                    name: "Read".into(),
                    summary: "a.md".into(),
                    input: entree(json!({ "file_path": "a.md" })),
                    result: Some(res("x")),
                    at: Some(2000),
                    result_at: Some(2500),
                }),
                ChatBlock::Thinking { text: "Donc…".into(), at: Some(4000) },
                ChatBlock::Text { text: "Réponse.".into(), at: Some(4000) },
                ChatBlock::Text { text: "Suite.".into(), at: Some(4100) },
            ],
            Some(1000),
        );
        assert_eq!(answer, vec!["Réponse.".to_string(), "Suite.".to_string()]);
        let trace = trace.unwrap();
        assert_eq!(trace.steps.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![StepKind::Thinking, StepKind::Note, StepKind::Tool, StepKind::Thinking]);
        assert_eq!((trace.steps[0].started_at, trace.steps[0].ended_at), (Some(1000), Some(1500)));
        assert_eq!((trace.steps[3].started_at, trace.steps[3].ended_at), (Some(2500), Some(4000)));
        // Fin = la dernière étape (la réponse qui suit ne compte pas).
        assert_eq!((trace.status, trace.started_at, trace.ended_at), (TraceStatus::Done, Some(1000), Some(4000)));
    }

    #[test]
    fn un_tour_sans_outil_ni_pensee_pas_de_trace() {
        assert_eq!(split_turn(&[ChatBlock::texte("Salut")], None), TourDecoupe { trace: None, answer: vec!["Salut".into()] });
    }

    #[test]
    fn un_outil_reste_sans_resultat_trace_en_erreur() {
        let b = ChatBlock::Tool(BlocOutil { id: "x".into(), name: "Bash".into(), summary: String::new(), input: None, result: None, at: None, result_at: None });
        assert_eq!(split_turn(&[b], None).trace.map(|t| t.status), Some(TraceStatus::Error));
    }

    // ── describeStep ──

    fn outil(name: &str, input: Value) -> TraceStep {
        TraceStep { name: Some(name.into()), input: entree(input), ..TraceStep::vide("x".into(), StepKind::Tool) }
    }

    #[test]
    fn verbes_au_present_pendant_au_passe_apres_cible_en_puce() {
        let r = describe_step(&outil("Read", json!({ "file_path": "C:\\v\\Cours\\photon.md" })), true);
        assert_eq!((r.verb.as_str(), r.target.as_deref(), r.target_icon.as_deref()), ("Lit", Some("photon.md"), Some("file-text")));
        let r = describe_step(&outil("Read", json!({ "file_path": "a.pdf" })), false);
        assert_eq!((r.verb.as_str(), r.target_icon.as_deref()), ("A lu", Some("file-type")));
        let r = describe_step(&outil("Grep", json!({ "pattern": "photon" })), false);
        assert_eq!((r.verb.as_str(), r.target.as_deref()), ("A cherché", Some("« photon »")));
        let r = describe_step(&outil("Glob", json!({ "pattern": "**/*.md" })), true);
        assert_eq!((r.verb.as_str(), r.mono), ("Cherche les fichiers", Some(true)));
        assert_eq!(describe_step(&outil("Edit", json!({ "file_path": "n.md" })), false).verb, "A modifié");
        assert_eq!(describe_step(&outil("Skill", json!({ "skill": "fragment-explain:intuition" })), false).target.as_deref(), Some("intuition"));
        let pensee = TraceStep { text: Some(String::new()), ..TraceStep::vide("t".into(), StepKind::Thinking) };
        assert_eq!(describe_step(&pensee, true).verb, "Réfléchit");
    }

    #[test]
    fn les_commandes_fragment_en_mots_une_autre_commande_telle_quelle() {
        let sh = |c: &str, running: bool| describe_step(&outil("PowerShell", json!({ "command": c })), running);
        let r = sh("fragment agent:screen vault=Coffre", false);
        assert_eq!((r.icon.as_str(), r.verb.as_str()), ("monitor", "A regardé l’écran"));
        let r = sh("fragment annotation:draw doc=\"Cours/photon.md\" passage=\"x\" vault=C", true);
        assert_eq!((r.verb.as_str(), r.target.as_deref()), ("Dessine sur", Some("photon.md")));
        let r = sh("fragment command id=editor:toggle-bold vault=C", false);
        assert_eq!((r.verb.as_str(), r.target.as_deref()), ("A lancé la commande", Some("editor:toggle-bold")));
        assert_eq!(sh("fragment.cmd eval code=\"1+1\"", false).verb, "A exécuté du code dans Fragment");
        let r = sh("fragment truc:inconnu", false);
        assert_eq!((r.verb.as_str(), r.target.as_deref(), r.mono), ("A lancé", Some("fragment truc:inconnu"), Some(true)));
        let r = sh("Get-ChildItem | Select-String x", false);
        assert_eq!((r.icon.as_str(), r.verb.as_str(), r.mono), ("terminal", "A exécuté", Some(true)));
    }

    #[test]
    fn param_guillemets_doubles_simples_nus() {
        assert_eq!(param("a doc=\"x y.md\" id='z' n=3", "doc").as_deref(), Some("x y.md"));
        assert_eq!(param("a doc=\"x y.md\" id='z' n=3", "id").as_deref(), Some("z"));
        assert_eq!(param("a doc=\"x\" n=3", "n").as_deref(), Some("3"));
        assert_eq!(param("a", "doc"), None);
    }

    // ── résumé et durées ──

    #[test]
    fn duree_formate() {
        assert_eq!(duree(400), "0 s");
        assert_eq!(duree(8200), "8 s");
        assert_eq!(duree(65_000), "1 min 5 s");
        assert_eq!(duree(120_000), "2 min");
    }

    #[test]
    fn summarize_compte_par_famille_pensee_avec_sa_duree() {
        let mut t = TraceBuilder::new(0);
        t.thinking("", 0, true);
        for (id, nom, a, b) in [("a", "Read", 3000, 3100), ("b", "Read", 3200, 3300), ("c", "Grep", 3400, 3500), ("d", "PowerShell", 3600, 3700), ("e", "Edit", 3800, 3900)] {
            t.tool(out(id, nom, None), a);
            t.result(id, res(""), b);
        }
        t.finish(4000, false);
        assert_eq!(summarize(&t.snapshot()), "A réfléchi 3 s · lu 2 fichiers · 1 recherche · 1 fichier modifié · 1 commande");
        let mut e = TraceBuilder::new(0);
        e.tool(out("x", "Bash", None), 1);
        e.finish(2, true);
        assert_eq!(summarize(&e.snapshot()), "Interrompu · 1 commande");
    }

    // ── orbStateOf ──

    fn en_cours(name: &str) -> Trace {
        let mut t = TraceBuilder::new(0);
        t.tool(out(name, name, None), 1);
        t.snapshot()
    }

    #[test]
    fn reflechit_avant_tout_pendant_une_pensee_entre_deux_etapes_ecrit_cherche_modifie_agit() {
        let mut t = TraceBuilder::new(0);
        assert_eq!(orb_state_of(&t.snapshot()), OrbState::Solving);
        t.thinking("", 1, true);
        assert_eq!(orb_state_of(&t.snapshot()), OrbState::Solving);
        for n in ["Read", "Grep", "Glob"] {
            assert_eq!(orb_state_of(&en_cours(n)), OrbState::Searching);
        }
        for n in ["Write", "Edit"] {
            assert_eq!(orb_state_of(&en_cours(n)), OrbState::Composing);
        }
        for n in ["Bash", "PowerShell", "Skill"] {
            assert_eq!(orb_state_of(&en_cours(n)), OrbState::Working);
        }
        t.tool(out("r", "Read", None), 2);
        t.result("r", res("x"), 3);
        assert_eq!(orb_state_of(&t.snapshot()), OrbState::Solving);
        t.ecrire(4);
        assert_eq!(orb_state_of(&t.snapshot()), OrbState::Composing);
        // Un outil suit : ce n'était pas la réponse.
        t.tool(out("b", "Bash", None), 5);
        assert_eq!(orb_state_of(&t.snapshot()), OrbState::Working);
        t.finish(6, false);
        assert_eq!(t.snapshot().writing, Some(false));
    }
}
