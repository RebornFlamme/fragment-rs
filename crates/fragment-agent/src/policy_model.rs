// Port de app/src/ui/views/agent/policy/policyModel.ts (+ policy/presets.ts)
//
// ═══════════════════════════════════════════════════════════════════════════
//  LA POLITIQUE DE PERMISSIONS — le modèle PUR.
//
//  `decide` : pour un usage d'outil, autoriser / refuser (avec la raison) /
//  DEMANDER (une carte dans la réponse). Toute décision se prend ICI, en Rust
//  testé ; les harness ne font que la transporter (Claude : un hook `PreToolUse`
//  pour les GARDES — dossiers masqués, lecture seule, chemins protégés — et
//  `canUseTool` pour la carte).
//
//  ★ Toute règle s'évalue sur le chemin canonique (`..`, `.` et `\` résolus) :
//    sans cela, un détour par `notes/../` atteindrait un fichier protégé.
//  ★ Écriture toujours refusée sur les chemins protégés : réglages de l'agent (il
//    pourrait s'y donner des droits), secrets, liste des plugins, journal.
//  ★ Limite assumée : un shell autorisé échappe aux règles de dossier, il peut lire
//    un fichier masqué. `incoherences` le signale dans les réglages.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::contrat::{
    AgentPolicy, DossierExterne, FolderAccess, PermanentRule, PermissionMode, ToolAccess,
    ToolFamily, ToolUse, WebDomains,
};
use crate::motif::Motif;
use crate::utf16::{len_utf16, slice_utf16};

/// Jamais écrits par l'agent, quelle que soit la politique.
pub const PROTECTED: [&str; 4] = [
    ".fragment/plugins/agent/**",
    ".fragment/secrets.json",
    ".fragment/community-plugins.json",
    ".fragment/artefacts/**",
];

/// Les familles d'outils, dans l'ordre du TS (`TOOL_FAMILIES`).
pub const TOOL_FAMILIES: [ToolFamily; 11] = [
    ToolFamily::Read,
    ToolFamily::Search,
    ToolFamily::Write,
    ToolFamily::Edit,
    ToolFamily::Shell,
    ToolFamily::Eval,
    ToolFamily::FragmentCli,
    ToolFamily::WebFetch,
    ToolFamily::WebSearch,
    ToolFamily::Subagents,
    ToolFamily::Skills,
];

pub fn family_label(f: ToolFamily) -> &'static str {
    match f {
        ToolFamily::Read => "Lire des fichiers",
        ToolFamily::Search => "Chercher (fichiers, texte)",
        ToolFamily::Write => "Créer des fichiers",
        ToolFamily::Edit => "Modifier des fichiers",
        ToolFamily::Shell => "Commandes du terminal",
        ToolFamily::Eval => "`fragment eval` (JavaScript dans l’app)",
        ToolFamily::FragmentCli => "Commandes `fragment …`",
        ToolFamily::WebFetch => "Lire des pages web",
        ToolFamily::WebSearch => "Chercher sur le web",
        ToolFamily::Subagents => "Sous-agents",
        ToolFamily::Skills => "Skills",
    }
}

pub fn mode_label(m: PermissionMode) -> &'static str {
    match m {
        PermissionMode::Full => "Accès complet",
        PermissionMode::Ask => "Demander",
        PermissionMode::AcceptEdits => "Modifs auto",
        PermissionMode::Plan => "Plan",
    }
}

/// Le contexte d'une décision.
#[derive(Debug, Clone, Default)]
pub struct DecideContext {
    /// La racine du coffre (chemin absolu, `/` ou `\`).
    pub vault_root: String,
    /// Les dossiers HORS coffre que Fragment donne lui-même à l'agent (captures, pièces jointes, son dossier).
    pub agent_dirs: Vec<DossierExterne>,
    /// Les « toujours » de la conversation (en plus de `policy.permanent`, ceux du coffre).
    pub conversation_rules: Vec<PermanentRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Ask,
    Deny { reason: String },
}

/// La commande d'un shell (Bash / PowerShell).
fn commande(u: &ToolUse) -> String {
    u.input
        .get("command")
        .and_then(Value::as_str)
        .map(|c| c.trim().to_string())
        .unwrap_or_default()
}

static FRAGMENT_EVAL: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"^fragment(\.cmd)?\s+eval\b"));
static FRAGMENT_CLI: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"^fragment(\.cmd)?\s"));
static OPERATEUR_SHELL: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"[|;&]"));

/// La famille NEUTRE d'un outil de Claude Code (un autre harness traduit les siens).
pub fn family_of(u: &ToolUse) -> Option<ToolFamily> {
    match u.tool.as_str() {
        "Read" => Some(ToolFamily::Read),
        "Glob" | "Grep" | "LS" => Some(ToolFamily::Search),
        "Write" => Some(ToolFamily::Write),
        "Edit" | "MultiEdit" | "NotebookEdit" => Some(ToolFamily::Edit),
        "WebFetch" => Some(ToolFamily::WebFetch),
        "WebSearch" => Some(ToolFamily::WebSearch),
        "Task" | "Agent" => Some(ToolFamily::Subagents),
        "Skill" => Some(ToolFamily::Skills),
        "Bash" | "PowerShell" => {
            let c = commande(u);
            if FRAGMENT_EVAL.is_match(&c) {
                return Some(ToolFamily::Eval);
            }
            if FRAGMENT_CLI.is_match(&c) && !OPERATEUR_SHELL.is_match(&c) {
                return Some(ToolFamily::FragmentCli);
            }
            Some(ToolFamily::Shell)
        }
        _ => None,
    }
}

/// Le chemin visé par un outil de fichier (absolu ou relatif au coffre), sinon `None`.
pub fn path_of(u: &ToolUse) -> Option<String> {
    ["file_path", "notebook_path", "path"].iter().find_map(|k| {
        u.input
            .get(*k)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    })
}

fn norme(p: &str) -> String {
    p.replace('\\', "/")
}

fn est_absolu(n: &str) -> bool {
    let b = n.as_bytes();
    n.starts_with('/')
        || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/')
}

fn a_lecteur(n: &str) -> bool {
    let b = n.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// Normalise un chemin (`.`, `..`, séparateurs) ; garde un préfixe de lecteur / la racine.
pub fn normalize(p: &str) -> String {
    let n = norme(p);
    let abs = est_absolu(&n);
    let prefixe = if a_lecteur(&n) {
        n[..2].to_uppercase()
    } else {
        String::new()
    };
    let mut out: Vec<&str> = Vec::new();
    for seg in n[prefixe.len()..].split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            out.pop();
        } else {
            out.push(seg);
        }
    }
    let r = format!("{prefixe}{}{}", if abs { "/" } else { "" }, out.join("/"));
    if r.is_empty() { ".".to_string() } else { r }
}

/// Un chemin rapporté au coffre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheminCoffre {
    pub inside: bool,
    /// Sans `/` initial ; `""` = la racine. Hors coffre : le chemin absolu.
    pub rel: String,
    pub abs: String,
}

/// Un chemin rapporté au coffre : `{inside, rel}`.
pub fn relative_to_vault(vault_root: &str, p: &str) -> CheminCoffre {
    let racine_norm = normalize(vault_root);
    let root = racine_norm
        .strip_suffix('/')
        .unwrap_or(&racine_norm)
        .to_string();
    let n = norme(p);
    let abs = if est_absolu(&n) {
        normalize(&n)
    } else {
        normalize(&format!("{root}/{n}"))
    };
    let majuscule =
        root.len() >= 2 && root.as_bytes()[0].is_ascii_uppercase() && root.as_bytes()[1] == b':';
    let cmp = |s: &str| {
        if majuscule {
            s.to_lowercase()
        } else {
            s.to_string()
        }
    };
    if cmp(&abs) == cmp(&root) {
        return CheminCoffre {
            inside: true,
            rel: String::new(),
            abs,
        };
    }
    if cmp(&abs).starts_with(&format!("{}/", cmp(&root))) {
        let rel = slice_utf16(&abs, len_utf16(&root) + 1, len_utf16(&abs)).to_string();
        return CheminCoffre {
            inside: true,
            rel,
            abs,
        };
    }
    CheminCoffre {
        inside: false,
        rel: abs.clone(),
        abs,
    }
}

/// Un motif glob simple : `**` = tout (sous-dossiers compris), `*` = tout sauf `/`.
pub fn glob_match(pattern: &str, s: &str) -> bool {
    let re: String = pattern
        .split("**")
        .map(|p| {
            p.split('*')
                .map(regex::escape)
                .collect::<Vec<_>>()
                .join("[^/]*")
        })
        .collect::<Vec<_>>()
        .join(".*");
    Motif::new(&format!("(?i)^{re}$")).is_match(s)
}

pub fn is_protected(rel: &str) -> bool {
    PROTECTED
        .iter()
        .any(|g| glob_match(g, rel) || (g.ends_with("/**") && rel == &g[..g.len() - 3]))
}

/// L'accès à un chemin du coffre : le plus proche ancêtre déclaré ; la racine absente vaut `write`.
pub fn folder_access(folders: &BTreeMap<String, FolderAccess>, rel: &str) -> FolderAccess {
    let segs: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
    for i in (0..=segs.len()).rev() {
        if let Some(a) = folders.get(&segs[..i].join("/")) {
            return *a;
        }
    }
    FolderAccess::Write
}

/// Les dossiers MASQUÉS sous `rel` (une recherche qui les traverserait).
pub fn hidden_below(folders: &BTreeMap<String, FolderAccess>, rel: &str) -> Vec<String> {
    let base = if rel.is_empty() {
        String::new()
    } else {
        format!("{rel}/")
    };
    folders
        .iter()
        .filter(|(k, a)| {
            **a == FolderAccess::Hidden && !k.is_empty() && (rel.is_empty() || k.starts_with(&base))
        })
        .map(|(k, _)| k.clone())
        .collect()
}

/// Le nom d'hôte d'une URL (`new URL(url).hostname`), vide si illisible.
fn host_of(url: &str) -> String {
    let Some(i) = url.find("://") else {
        return String::new();
    };
    let schema = &url[..i];
    if schema.is_empty()
        || !schema
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        || !schema
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    {
        return String::new();
    }
    let reste = &url[i + 3..];
    let fin = reste.find(['/', '?', '#', '\\']).unwrap_or(reste.len());
    let autorite = &reste[..fin];
    let hote = autorite.rsplit('@').next().unwrap_or(autorite);
    let hote = if let Some(fermant) = hote
        .strip_prefix('[')
        .and_then(|h| h.find(']').map(|j| &h[..j]))
    {
        fermant
    } else {
        hote.split(':').next().unwrap_or(hote)
    };
    hote.to_lowercase()
}

/// Une règle « toujours » couvre-t-elle cet usage ?
pub fn rule_matches(r: &PermanentRule, family: ToolFamily, u: &ToolUse, rel: Option<&str>) -> bool {
    if r.family != family {
        return false;
    }
    let Some(pattern) = r.pattern.as_deref().filter(|p| !p.is_empty()) else {
        return true;
    };
    match family {
        ToolFamily::Shell | ToolFamily::Eval | ToolFamily::FragmentCli => {
            glob_match(pattern, &commande(u))
        }
        ToolFamily::WebFetch => u
            .input
            .get("url")
            .and_then(Value::as_str)
            .is_some_and(|url| host_of(url) == pattern),
        _ => rel.is_some_and(|r| glob_match(pattern, r)),
    }
}

/// La règle à retenir pour un « toujours » donné sur cet usage (la carte la propose).
pub fn rule_for(u: &ToolUse, vault_root: &str) -> Option<PermanentRule> {
    let f = family_of(u)?;
    let sans = |family| PermanentRule {
        family,
        pattern: None,
    };
    if matches!(
        f,
        ToolFamily::Shell | ToolFamily::Eval | ToolFamily::FragmentCli
    ) {
        let mots: Vec<String> = commande(u)
            .split_whitespace()
            .take(if f == ToolFamily::Shell { 1 } else { 2 })
            .map(str::to_string)
            .collect();
        let mot = mots.join(" ");
        return Some(if mot.is_empty() {
            sans(f)
        } else {
            PermanentRule {
                family: f,
                pattern: Some(format!("{mot}*")),
            }
        });
    }
    if f == ToolFamily::WebFetch
        && let Some(url) = u.input.get("url").and_then(Value::as_str)
    {
        return Some(PermanentRule {
            family: f,
            pattern: Some(host_of(url)),
        });
    }
    if let (Some(p), true) = (
        path_of(u),
        matches!(f, ToolFamily::Write | ToolFamily::Edit),
    ) {
        let r = relative_to_vault(vault_root, &p);
        let dossier = r
            .rel
            .rfind('/')
            .map(|i| r.rel[..i].to_string())
            .unwrap_or_default();
        return Some(PermanentRule {
            family: f,
            pattern: Some(if dossier.is_empty() {
                "**".to_string()
            } else {
                format!("{dossier}/**")
            }),
        });
    }
    Some(sans(f))
}

fn est_ecriture(f: ToolFamily) -> bool {
    matches!(f, ToolFamily::Write | ToolFamily::Edit)
}

fn acces(p: &AgentPolicy, f: ToolFamily) -> Option<ToolAccess> {
    p.tools.get(&f).copied()
}

/// LA DÉCISION. Les GARDES d'abord (elles valent dans tout mode, « accès complet »
/// compris) ; puis les « toujours » ; puis l'accès de la famille, selon le mode.
pub fn decide(p: &AgentPolicy, u: &ToolUse, ctx: &DecideContext) -> Decision {
    let family = family_of(u);
    let chemin = path_of(u);
    let rules: Vec<&PermanentRule> = p
        .permanent
        .iter()
        .chain(ctx.conversation_rules.iter())
        .collect();
    let mut rel: Option<String> = None;
    // ── Les gardes de chemin (fichiers, recherches).
    let famille_fichier =
        family.filter(|f| matches!(f, ToolFamily::Read | ToolFamily::Search) || est_ecriture(*f));
    if let (Some(chemin), Some(family)) = (chemin.as_deref(), famille_fichier) {
        let r = relative_to_vault(&ctx.vault_root, chemin);
        if r.inside {
            rel = Some(r.rel.clone());
            if est_ecriture(family) && is_protected(&r.rel) {
                return Decision::Deny {
                    reason: format!("« {} » est protégé : l’agent ne peut pas l’écrire.", r.rel),
                };
            }
            let a = folder_access(&p.folders, &r.rel);
            if a == FolderAccess::Hidden {
                let nom = if r.rel.is_empty() { "/" } else { &r.rel };
                return Decision::Deny {
                    reason: format!("« {nom} » est un dossier masqué à l’agent."),
                };
            }
            if a == FolderAccess::Read && est_ecriture(family) {
                return Decision::Deny {
                    reason: format!("« {} » est en lecture seule pour l’agent.", r.rel),
                };
            }
            if family == ToolFamily::Search {
                let caches = hidden_below(&p.folders, &r.rel);
                if !caches.is_empty() {
                    return Decision::Deny {
                        reason: format!(
                            "Cette recherche traverserait des dossiers masqués ({}) : cherche dans des sous-dossiers précis.",
                            caches.join(", ")
                        ),
                    };
                }
            }
        } else {
            let a_min = r.abs.to_lowercase();
            let dehors = p.outside.iter().chain(ctx.agent_dirs.iter()).find(|o| {
                let n = normalize(&o.path);
                let base = n.strip_suffix('/').unwrap_or(&n).to_lowercase();
                a_min == base || a_min.starts_with(&format!("{base}/"))
            });
            match dehors {
                None => {
                    return Decision::Deny {
                        reason: format!(
                            "« {} » est hors du coffre et n’est pas un dossier permis.",
                            r.abs
                        ),
                    };
                }
                Some(o) if o.access == FolderAccess::Hidden => {
                    return Decision::Deny {
                        reason: format!(
                            "« {} » est hors du coffre et n’est pas un dossier permis.",
                            r.abs
                        ),
                    };
                }
                Some(o) if o.access == FolderAccess::Read && est_ecriture(family) => {
                    return Decision::Deny {
                        reason: format!("« {} » est en lecture seule pour l’agent.", r.abs),
                    };
                }
                Some(_) => {}
            }
        }
    } else if family == Some(ToolFamily::Search) && chemin.is_none() {
        // Une recherche sans chemin part de la racine du coffre.
        let caches = hidden_below(&p.folders, "");
        if !caches.is_empty() {
            return Decision::Deny {
                reason: format!(
                    "Cette recherche traverserait des dossiers masqués ({}) : précise un dossier.",
                    caches.join(", ")
                ),
            };
        }
    }
    if family == Some(ToolFamily::WebFetch)
        && let (WebDomains::Liste(domaines), Some(url)) =
            (&p.web_domains, u.input.get("url").and_then(Value::as_str))
    {
        let h = host_of(url);
        if !domaines
            .iter()
            .any(|d| h == *d || h.ends_with(&format!(".{d}")))
        {
            return Decision::Deny {
                reason: format!("Le domaine « {h} » n’est pas dans la liste permise."),
            };
        }
    }
    // (Mode plan : c'est le moteur qui reste en lecture seule — Claude Code écrit
    //  pourtant son fichier de plan, hors du coffre : `agent_dirs` le permet.)
    let Some(family) = family else {
        return if p.mode == PermissionMode::Full {
            Decision::Allow
        } else {
            Decision::Ask
        };
    };
    // ── Les « toujours » et les commandes permises.
    if rules
        .iter()
        .any(|r| rule_matches(r, family, u, rel.as_deref()))
    {
        return Decision::Allow;
    }
    if family == ToolFamily::Shell && p.shell_rules.iter().any(|g| glob_match(g, &commande(u))) {
        return Decision::Allow;
    }
    // ── L'accès de la famille, selon le mode.
    let mut a = acces(p, family);
    // `fragment …` hors eval : au pire ce que permet le shell (c'est une commande).
    if family == ToolFamily::FragmentCli
        && a == Some(ToolAccess::Deny)
        && acces(p, ToolFamily::Shell) != Some(ToolAccess::Deny)
    {
        a = acces(p, ToolFamily::Shell);
    }
    if a == Some(ToolAccess::Deny) {
        return Decision::Deny {
            reason: format!(
                "{} : interdit par la configuration de l’agent.",
                family_label(family)
            ),
        };
    }
    if a == Some(ToolAccess::Allow) {
        return Decision::Allow;
    }
    if p.mode == PermissionMode::Full {
        return Decision::Allow;
    }
    if p.mode == PermissionMode::AcceptEdits && est_ecriture(family) {
        return Decision::Allow;
    }
    Decision::Ask
}

/// Ce que la configuration a d'INCOHÉRENT (la bannière d'honnêteté des réglages) —
/// jamais pour faire peur : ce qui ne protège pas ce qu'on croit.
pub fn incoherences(p: &AgentPolicy, platform: &str) -> Vec<String> {
    let mut out = Vec::new();
    let caches = p.folders.values().any(|a| *a != FolderAccess::Write);
    let shell_permis = acces(p, ToolFamily::Shell) != Some(ToolAccess::Deny);
    if caches && shell_permis {
        out.push("Le terminal est permis : une commande peut lire ou écrire n’importe quel fichier, dossiers masqués ou en lecture seule compris. Ces règles ne valent que pour les outils de fichiers.".to_string());
    }
    if acces(p, ToolFamily::Eval) != Some(ToolAccess::Deny) {
        out.push(
            "`fragment eval` donne à l’agent un accès JavaScript complet à l’application."
                .to_string(),
        );
    }
    if platform == "win32" && shell_permis {
        out.push("Sous Windows, Claude Code n’a pas de bac à sable système : le terminal s’exécute avec tes droits.".to_string());
    }
    out
}

// ── La compilation pour le Claude Agent SDK ─────────────────────────────────

/// Le mode de permission du SDK Claude.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClaudePermissionMode {
    DontAsk,
    Default,
    AcceptEdits,
    Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCompiled {
    pub permission_mode: ClaudePermissionMode,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
}

/// Les outils de Claude Code par famille (règles du SDK).
fn outils(f: ToolFamily) -> &'static [&'static str] {
    match f {
        ToolFamily::Read => &["Read"],
        ToolFamily::Search => &["Glob", "Grep"],
        ToolFamily::Write => &["Write(./**)"],
        ToolFamily::Edit => &["Edit(./**)", "NotebookEdit(./**)"],
        ToolFamily::Shell => &["Bash", "PowerShell"],
        ToolFamily::Eval => &["Bash(fragment eval *)", "PowerShell(fragment eval *)"],
        ToolFamily::FragmentCli => &[
            "Bash(fragment *)",
            "Bash(fragment.cmd *)",
            "PowerShell(fragment *)",
        ],
        ToolFamily::WebFetch => &["WebFetch"],
        ToolFamily::WebSearch => &["WebSearch"],
        ToolFamily::Subagents => &["Task"],
        ToolFamily::Skills => &["Skill"],
    }
}

fn sans_doublon(v: Vec<String>) -> Vec<String> {
    let mut vus = HashSet::new();
    v.into_iter().filter(|x| vus.insert(x.clone())).collect()
}

/// La politique en options du SDK. `full` : `dontAsk` + la liste blanche des
/// familles permises (« demander » compris) — le comportement historique quand la
/// politique est « Coffre ouvert ». Les autres modes : le mode du SDK, la liste
/// blanche des familles `allow` ; le reste passe par `canUseTool` (la carte).
/// Les GARDES (dossiers, chemins protégés) sont le hook, dans TOUS les modes.
pub fn compile_claude(p: &AgentPolicy, agent_dir: Option<&str>) -> ClaudeCompiled {
    let permis: Vec<ToolFamily> = TOOL_FAMILIES
        .iter()
        .copied()
        .filter(|f| {
            acces(p, *f) == Some(ToolAccess::Allow)
                || (p.mode == PermissionMode::Full && acces(p, *f) == Some(ToolAccess::Ask))
        })
        .collect();
    let mut allowed: Vec<String> = permis
        .iter()
        .flat_map(|f| outils(*f).iter().map(|s| s.to_string()))
        .collect();
    // Le shell entier couvre `fragment …` ; sinon les commandes permises une à une.
    if !permis.contains(&ToolFamily::Shell) {
        allowed.extend(
            p.shell_rules
                .iter()
                .flat_map(|r| [format!("Bash({r})"), format!("PowerShell({r})")]),
        );
    }
    if let Some(dir) = agent_dir.filter(|d| !d.is_empty()) {
        let d = dir.replace('\\', "/");
        let d = d.strip_suffix('/').unwrap_or(&d);
        allowed.push(format!("Write({d}/**)"));
        allowed.push(format!("Edit({d}/**)"));
    }
    let mut disallowed: Vec<String> = PROTECTED
        .iter()
        .flat_map(|g| [format!("Write(./{g})"), format!("Edit(./{g})")])
        .collect();
    disallowed.extend(
        TOOL_FAMILIES
            .iter()
            .filter(|f| {
                acces(p, **f) == Some(ToolAccess::Deny)
                    && **f != ToolFamily::FragmentCli
                    && **f != ToolFamily::Eval
            })
            .flat_map(|f| outils(*f).iter().map(|s| s.to_string())),
    );
    let permission_mode = match p.mode {
        PermissionMode::Full => ClaudePermissionMode::DontAsk,
        PermissionMode::Ask => ClaudePermissionMode::Default,
        PermissionMode::AcceptEdits => ClaudePermissionMode::AcceptEdits,
        PermissionMode::Plan => ClaudePermissionMode::Plan,
    };
    ClaudeCompiled {
        permission_mode,
        allowed_tools: sans_doublon(allowed),
        disallowed_tools: sans_doublon(disallowed),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Les préréglages de la politique de permissions (policy/presets.ts). La phase 4
//  les rend réglables ; d'ici là, chaque tour part avec « Coffre ouvert » — le
//  comportement historique de l'agent (tout le coffre, le shell, `fragment eval`).
// ═══════════════════════════════════════════════════════════════════════════

fn tous(access: ToolAccess) -> BTreeMap<ToolFamily, ToolAccess> {
    TOOL_FAMILIES.iter().map(|f| (*f, access)).collect()
}

/// « Coffre ouvert » = la liste blanche historique (AgentHost.ts) : lire, chercher,
/// écrire dans le coffre, tout le shell, le CLI et `eval`, les skills ; ni le web
/// ni les sous-agents (absents de la liste, donc refusés par `dontAsk`). Mode `full`.
pub fn open_vault_policy() -> AgentPolicy {
    let mut tools = tous(ToolAccess::Allow);
    tools.insert(ToolFamily::WebFetch, ToolAccess::Deny);
    tools.insert(ToolFamily::WebSearch, ToolAccess::Deny);
    tools.insert(ToolFamily::Subagents, ToolAccess::Deny);
    AgentPolicy {
        mode: PermissionMode::Full,
        tools,
        folders: BTreeMap::new(),
        outside: Vec::new(),
        shell_rules: Vec::new(),
        web_domains: WebDomains::Tous,
        permanent: Vec::new(),
    }
}

/// « Assistant » : lire et chercher librement ; écrire, le terminal, `eval` et le web sont DEMANDÉS. Mode `ask`.
pub fn assistant_policy() -> AgentPolicy {
    let mut tools = tous(ToolAccess::Ask);
    tools.insert(ToolFamily::Read, ToolAccess::Allow);
    tools.insert(ToolFamily::Search, ToolAccess::Allow);
    tools.insert(ToolFamily::Skills, ToolAccess::Allow);
    tools.insert(ToolFamily::FragmentCli, ToolAccess::Allow);
    tools.insert(ToolFamily::Subagents, ToolAccess::Deny);
    AgentPolicy {
        mode: PermissionMode::Ask,
        tools,
        ..open_vault_policy()
    }
}

/// « Strict » : ni terminal ni `eval` ni web ; écrire est demandé. Mode `ask`.
pub fn strict_policy() -> AgentPolicy {
    let mut tools = tous(ToolAccess::Deny);
    tools.insert(ToolFamily::Read, ToolAccess::Allow);
    tools.insert(ToolFamily::Search, ToolAccess::Allow);
    tools.insert(ToolFamily::Skills, ToolAccess::Allow);
    tools.insert(ToolFamily::Write, ToolAccess::Ask);
    tools.insert(ToolFamily::Edit, ToolAccess::Ask);
    tools.insert(ToolFamily::FragmentCli, ToolAccess::Ask);
    AgentPolicy {
        mode: PermissionMode::Ask,
        tools,
        ..open_vault_policy()
    }
}

/// Les préréglages (`POLICY_PRESETS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyPreset {
    Open,
    Assistant,
    Strict,
}

impl PolicyPreset {
    pub fn label(self) -> &'static str {
        match self {
            PolicyPreset::Open => "Coffre ouvert",
            PolicyPreset::Assistant => "Assistant",
            PolicyPreset::Strict => "Strict",
        }
    }

    pub fn make(self) -> AgentPolicy {
        match self {
            PolicyPreset::Open => open_vault_policy(),
            PolicyPreset::Assistant => assistant_policy(),
            PolicyPreset::Strict => strict_policy(),
        }
    }
}

/// Une politique lue d'un `data.json` (incomplète, ancienne).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialPolicy {
    pub mode: Option<PermissionMode>,
    pub tools: Option<BTreeMap<ToolFamily, ToolAccess>>,
    pub folders: Option<BTreeMap<String, FolderAccess>>,
    pub outside: Option<Vec<DossierExterne>>,
    pub shell_rules: Option<Vec<String>>,
    pub web_domains: Option<WebDomains>,
    pub permanent: Option<Vec<PermanentRule>>,
}

/// Une politique incomplète → complète : les manques prennent « Coffre ouvert ».
pub fn complete_policy(p: Option<&PartialPolicy>) -> AgentPolicy {
    let base = open_vault_policy();
    let Some(p) = p else { return base };
    let mut tools = base.tools.clone();
    if let Some(t) = &p.tools {
        tools.extend(t.iter().map(|(k, v)| (*k, *v)));
    }
    AgentPolicy {
        mode: p.mode.unwrap_or(base.mode),
        tools,
        folders: p.folders.clone().unwrap_or_default(),
        outside: p.outside.clone().unwrap_or_default(),
        shell_rules: p.shell_rules.clone().unwrap_or_default(),
        web_domains: p.web_domains.clone().unwrap_or(WebDomains::Tous),
        permanent: p.permanent.clone().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ROOT: &str = "C:\\coffre";

    fn ctx() -> DecideContext {
        DecideContext {
            vault_root: ROOT.into(),
            agent_dirs: vec![DossierExterne {
                path: "C:/agent".into(),
                access: FolderAccess::Write,
            }],
            conversation_rules: vec![],
        }
    }

    fn usage(tool: &str, input: Value) -> ToolUse {
        ToolUse {
            tool: tool.into(),
            input: input.as_object().cloned().unwrap_or_default(),
        }
    }

    fn kind(d: Decision) -> &'static str {
        match d {
            Decision::Allow => "allow",
            Decision::Ask => "ask",
            Decision::Deny { .. } => "deny",
        }
    }

    fn dossiers(l: &[(&str, FolderAccess)]) -> BTreeMap<String, FolderAccess> {
        l.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    // ── politique — familles et chemins ──

    #[test]
    fn familles_des_outils_de_claude_code_le_shell_selon_la_commande() {
        assert_eq!(family_of(&usage("Read", json!({}))), Some(ToolFamily::Read));
        assert_eq!(
            family_of(&usage("Grep", json!({}))),
            Some(ToolFamily::Search)
        );
        assert_eq!(
            family_of(&usage("Bash", json!({ "command": "fragment eval \"1\"" }))),
            Some(ToolFamily::Eval)
        );
        assert_eq!(
            family_of(&usage("PowerShell", json!({ "command": "fragment files" }))),
            Some(ToolFamily::FragmentCli)
        );
        assert_eq!(
            family_of(&usage(
                "Bash",
                json!({ "command": "fragment files | grep x" })
            )),
            Some(ToolFamily::Shell)
        );
        assert_eq!(
            family_of(&usage("Bash", json!({ "command": "git status" }))),
            Some(ToolFamily::Shell)
        );
        assert_eq!(family_of(&usage("mcp__x", json!({}))), None);
    }

    #[test]
    fn canonise_et_les_separateurs_dedans_dehors_du_coffre() {
        assert_eq!(normalize("C:\\a\\b\\..\\c"), "C:/a/c");
        let r = relative_to_vault(ROOT, "notes/../.fragment/secrets.json");
        assert_eq!((r.inside, r.rel.as_str()), (true, ".fragment/secrets.json"));
        let r = relative_to_vault(ROOT, "c:/coffre/x.md");
        assert_eq!((r.inside, r.rel.as_str()), (true, "x.md"));
        assert!(!relative_to_vault(ROOT, "C:/coffre2/x.md").inside);
        assert!(!relative_to_vault(ROOT, "../hors.md").inside);
    }

    #[test]
    fn heritage_des_dossiers_le_plus_proche_ancetre_racine_absente_ecriture() {
        let f = dossiers(&[
            ("Privé", FolderAccess::Hidden),
            ("Cours", FolderAccess::Read),
            ("Cours/Brouillons", FolderAccess::Write),
        ]);
        assert_eq!(folder_access(&f, "Privé/a.md"), FolderAccess::Hidden);
        assert_eq!(folder_access(&f, "Cours/x/y.md"), FolderAccess::Read);
        assert_eq!(
            folder_access(&f, "Cours/Brouillons/z.md"),
            FolderAccess::Write
        );
        assert_eq!(folder_access(&f, "Autre/z.md"), FolderAccess::Write);
    }

    // ── politique — decide ──

    #[test]
    fn chemins_proteges_jamais_ecrits_meme_en_acces_complet_meme_par_un_point_point() {
        let p = open_vault_policy();
        assert_eq!(
            kind(decide(
                &p,
                &usage(
                    "Write",
                    json!({ "file_path": "notes/../.fragment/plugins/agent/data.json" })
                ),
                &ctx()
            )),
            "deny"
        );
        assert_eq!(
            kind(decide(
                &p,
                &usage(
                    "Edit",
                    json!({ "file_path": format!("{ROOT}\\.fragment\\artefacts\\journal\\x.ndjson") })
                ),
                &ctx()
            )),
            "deny"
        );
        assert_eq!(
            kind(decide(
                &p,
                &usage(
                    "Read",
                    json!({ "file_path": ".fragment/plugins/agent/data.json" })
                ),
                &ctx()
            )),
            "allow"
        );
    }

    #[test]
    fn dossier_masque_ni_lu_ni_cherche_une_recherche_a_la_racine_est_refusee_avec_la_liste() {
        let p = AgentPolicy {
            folders: dossiers(&[("Privé", FolderAccess::Hidden)]),
            ..open_vault_policy()
        };
        assert_eq!(
            kind(decide(
                &p,
                &usage("Read", json!({ "file_path": "Privé/journal.md" })),
                &ctx()
            )),
            "deny"
        );
        match decide(&p, &usage("Grep", json!({ "pattern": "x" })), &ctx()) {
            Decision::Deny { reason } => assert!(reason.contains("Privé")),
            autre => panic!("attendu un refus, reçu {autre:?}"),
        }
        assert_eq!(
            kind(decide(
                &p,
                &usage("Grep", json!({ "pattern": "x", "path": "Cours" })),
                &ctx()
            )),
            "allow"
        );
    }

    #[test]
    fn lecture_seule_lire_oui_ecrire_non() {
        let p = AgentPolicy {
            folders: dossiers(&[("Cours", FolderAccess::Read)]),
            ..open_vault_policy()
        };
        assert_eq!(
            kind(decide(
                &p,
                &usage("Read", json!({ "file_path": "Cours/a.md" })),
                &ctx()
            )),
            "allow"
        );
        assert_eq!(
            kind(decide(
                &p,
                &usage("Write", json!({ "file_path": "Cours/a.md" })),
                &ctx()
            )),
            "deny"
        );
    }

    #[test]
    fn hors_du_coffre_seulement_les_dossiers_permis_ceux_de_lagent_compris() {
        let p = open_vault_policy();
        assert_eq!(
            kind(decide(
                &p,
                &usage("Read", json!({ "file_path": "D:/ailleurs/x.txt" })),
                &ctx()
            )),
            "deny"
        );
        assert_eq!(
            kind(decide(
                &p,
                &usage("Write", json!({ "file_path": "C:\\agent\\skills\\a.md" })),
                &ctx()
            )),
            "allow"
        );
        let q = AgentPolicy {
            outside: vec![DossierExterne {
                path: "D:/ailleurs".into(),
                access: FolderAccess::Read,
            }],
            ..p
        };
        assert_eq!(
            kind(decide(
                &q,
                &usage("Read", json!({ "file_path": "D:/ailleurs/x.txt" })),
                &ctx()
            )),
            "allow"
        );
        assert_eq!(
            kind(decide(
                &q,
                &usage("Write", json!({ "file_path": "D:/ailleurs/x.txt" })),
                &ctx()
            )),
            "deny"
        );
    }

    #[test]
    fn les_modes_full_demander_autoriser_ask_carte_accept_edits_ecrire_auto_plan_lecture_seule() {
        let ask = assistant_policy();
        let bash = usage("Bash", json!({ "command": "git status" }));
        let write = usage("Write", json!({ "file_path": "n.md" }));
        let avec = |mode| AgentPolicy {
            mode,
            ..ask.clone()
        };
        assert_eq!(
            kind(decide(&avec(PermissionMode::Full), &bash, &ctx())),
            "allow"
        );
        assert_eq!(kind(decide(&ask, &bash, &ctx())), "ask");
        assert_eq!(
            kind(decide(&avec(PermissionMode::AcceptEdits), &write, &ctx())),
            "allow"
        );
        assert_eq!(
            kind(decide(&avec(PermissionMode::AcceptEdits), &bash, &ctx())),
            "ask"
        );
        assert_eq!(
            kind(decide(&avec(PermissionMode::Plan), &write, &ctx())),
            "ask"
        );
        assert_eq!(
            kind(decide(
                &avec(PermissionMode::Plan),
                &usage("Read", json!({ "file_path": "n.md" })),
                &ctx()
            )),
            "allow"
        );
    }

    #[test]
    fn interdit_reste_interdit_en_acces_complet_toujours_et_commandes_permises() {
        let s = strict_policy();
        let rm = usage("Bash", json!({ "command": "rm -rf x" }));
        assert_eq!(
            kind(decide(
                &AgentPolicy {
                    mode: PermissionMode::Full,
                    ..s.clone()
                },
                &rm,
                &ctx()
            )),
            "deny"
        );
        let git = usage("Bash", json!({ "command": "git status" }));
        let avec_regles = |r: PermanentRule| DecideContext {
            conversation_rules: vec![r],
            ..ctx()
        };
        assert_eq!(
            kind(decide(
                &s,
                &git,
                &avec_regles(PermanentRule {
                    family: ToolFamily::Shell,
                    pattern: Some("git*".into())
                })
            )),
            "allow"
        );
        let ls = usage("Bash", json!({ "command": "ls -la" }));
        assert_eq!(
            kind(decide(
                &AgentPolicy {
                    shell_rules: vec!["ls*".into()],
                    ..s.clone()
                },
                &ls,
                &ctx()
            )),
            "allow"
        );
        let w = usage("Write", json!({ "file_path": "a/b.md" }));
        assert_eq!(
            kind(decide(
                &s,
                &w,
                &avec_regles(PermanentRule {
                    family: ToolFamily::Write,
                    pattern: Some("a/**".into())
                })
            )),
            "allow"
        );
    }

    #[test]
    fn la_regle_proposee_par_la_carte_le_1er_mot_dune_commande_le_dossier_dun_fichier() {
        assert_eq!(
            rule_for(&usage("Bash", json!({ "command": "git status -s" })), ROOT),
            Some(PermanentRule {
                family: ToolFamily::Shell,
                pattern: Some("git*".into())
            })
        );
        assert_eq!(
            rule_for(&usage("Write", json!({ "file_path": "Cours/a.md" })), ROOT),
            Some(PermanentRule {
                family: ToolFamily::Write,
                pattern: Some("Cours/**".into())
            })
        );
        assert_eq!(
            rule_for(
                &usage("WebFetch", json!({ "url": "https://ex.org/a" })),
                ROOT
            ),
            Some(PermanentRule {
                family: ToolFamily::WebFetch,
                pattern: Some("ex.org".into())
            })
        );
    }

    #[test]
    fn domaines_web() {
        let mut p = open_vault_policy();
        p.tools.insert(ToolFamily::WebFetch, ToolAccess::Allow);
        p.web_domains = WebDomains::Liste(vec!["wikipedia.org".into()]);
        assert_eq!(
            kind(decide(
                &p,
                &usage("WebFetch", json!({ "url": "https://fr.wikipedia.org/x" })),
                &ctx()
            )),
            "allow"
        );
        assert_eq!(
            kind(decide(
                &p,
                &usage("WebFetch", json!({ "url": "https://evil.com" })),
                &ctx()
            )),
            "deny"
        );
    }

    // ── politique — compilation et honnêteté ──

    #[test]
    fn coffre_ouvert_compile_la_liste_blanche_historique() {
        let c = compile_claude(&open_vault_policy(), Some("C:\\agent"));
        assert_eq!(c.permission_mode, ClaudePermissionMode::DontAsk);
        for t in [
            "Read",
            "Glob",
            "Grep",
            "Write(./**)",
            "Edit(./**)",
            "Bash",
            "PowerShell",
            "Write(C:/agent/**)",
        ] {
            assert!(c.allowed_tools.iter().any(|x| x == t), "manque {t}");
        }
        assert!(!c.allowed_tools.iter().any(|x| x == "WebFetch"));
        assert!(
            c.disallowed_tools
                .iter()
                .any(|x| x == "Write(./.fragment/artefacts/**)")
        );
    }

    #[test]
    fn strict_mode_default_rien_du_shell_permis_doffice() {
        let c = compile_claude(&strict_policy(), None);
        assert_eq!(c.permission_mode, ClaudePermissionMode::Default);
        assert!(!c.allowed_tools.iter().any(|x| x == "Bash"));
        assert!(c.disallowed_tools.iter().any(|x| x == "Bash"));
    }

    #[test]
    fn incoherences_dossiers_masques_terminal_eval_windows() {
        let p = AgentPolicy {
            folders: dossiers(&[("Privé", FolderAccess::Hidden)]),
            ..open_vault_policy()
        };
        assert_eq!(incoherences(&p, "win32").len(), 3);
        assert_eq!(
            incoherences(&strict_policy(), "linux"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn une_politique_incomplete_se_complete() {
        let partielle = PartialPolicy {
            mode: Some(PermissionMode::Ask),
            tools: Some(BTreeMap::from([(ToolFamily::Shell, ToolAccess::Deny)])),
            ..PartialPolicy::default()
        };
        assert_eq!(
            complete_policy(Some(&partielle))
                .tools
                .get(&ToolFamily::Read),
            Some(&ToolAccess::Allow)
        );
        assert_eq!(complete_policy(None).mode, PermissionMode::Full);
    }

    // ── ajouts du port (absents du TS) ──

    #[test]
    fn glob_et_hote() {
        assert!(glob_match("notes/**", "notes/a/b.md"));
        assert!(glob_match("*.md", "a.MD"));
        assert!(!glob_match("*.md", "a/b.md"));
        assert_eq!(host_of("https://user@Ex.org:8080/a?b"), "ex.org");
        assert_eq!(host_of("pas une url"), "");
    }

    #[test]
    fn les_preselections_ont_leurs_libelles() {
        assert_eq!(PolicyPreset::Open.label(), "Coffre ouvert");
        assert_eq!(PolicyPreset::Strict.make().mode, PermissionMode::Ask);
        assert_eq!(mode_label(PermissionMode::AcceptEdits), "Modifs auto");
    }
}
