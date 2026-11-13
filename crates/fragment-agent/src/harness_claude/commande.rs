// Port de app/src/ui/views/agent/AgentHost.ts (`send` : options du SDK, `envDuTour`) et de
// sdkProcess.mjs (`send` : reprise, fork ou session neuve), traduits en ligne de commande du
// binaire `claude` (ADR-016 §2, §3)
//
// Tout ici est PUR : le choix de la session, les arguments, l'environnement. Le SDK écrivait ces
// options pour nous ; S9 a relevé lesquelles il passe et les rejoue.

use std::collections::BTreeMap;

use crate::policy_model::{ClaudeCompiled, ClaudePermissionMode};

/// Les efforts que le binaire accepte (un autre effort n'est pas transmis).
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// Comment le tour se rattache à sa conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoixSession {
    /// Une conversation neuve, sous l'id choisi par Fragment (`--session-id`).
    Neuve { id: String },
    /// La suite d'une conversation dont le transcript existe (`--resume`).
    Reprise { id: String },
    /// Une conversation NEUVE née de `depuis`, coupée au repère `at` (l'uuid du transcript).
    Fork {
        id: String,
        depuis: String,
        at: String,
    },
}

impl ChoixSession {
    /// L'id de la conversation du tour.
    pub fn id(&self) -> &str {
        match self {
            ChoixSession::Neuve { id }
            | ChoixSession::Reprise { id }
            | ChoixSession::Fork { id, .. } => id,
        }
    }
}

/// Reprendre si le transcript existe, sinon brancher (fork demandé), sinon créer.
/// ★ Une session absente ne se reprend pas : `--resume` sort en code 1 avant `initialize`.
pub fn choisir_session(
    id: &str,
    fork: Option<(&str, &str)>,
    existe: impl Fn(&str) -> bool,
) -> ChoixSession {
    if existe(id) {
        return ChoixSession::Reprise { id: id.into() };
    }
    match fork {
        Some((depuis, at)) => ChoixSession::Fork {
            id: id.into(),
            depuis: depuis.into(),
            at: at.into(),
        },
        None => ChoixSession::Neuve { id: id.into() },
    }
}

/// Ce que la ligne de commande d'un tour demande en plus de la politique compilée.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OptionsTour {
    pub modele: Option<String>,
    pub effort: Option<String>,
    /// Le dossier de l'agent (son plugin local de skills), s'il existe.
    pub dossier_agent: Option<String>,
    /// Les skills présents dans ce dossier, noms qualifiés (`<plugin>:<skill>`).
    pub skills: Vec<String>,
    /// Les dossiers hors du coffre que Read doit pouvoir lire (captures, pièces, dossier de l'agent…).
    pub dossiers: Vec<String>,
}

fn nom_mode(m: ClaudePermissionMode) -> &'static str {
    match m {
        ClaudePermissionMode::DontAsk => "dontAsk",
        ClaudePermissionMode::Default => "default",
        ClaudePermissionMode::AcceptEdits => "acceptEdits",
        ClaudePermissionMode::Plan => "plan",
    }
}

/// Les arguments d'un tour (sans le programme).
pub fn arguments(
    session: &ChoixSession,
    politique: &ClaudeCompiled,
    o: &OptionsTour,
) -> Vec<String> {
    let mut a: Vec<String> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        // ★ L'isolement (`settingSources: []` + `strictMcpConfig` du SDK) : ni réglages, ni
        //   CLAUDE.md, ni serveurs MCP, ni connecteurs du compte de l'utilisateur.
        "--setting-sources=",
        "--strict-mcp-config",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut pousser = |cle: &str, valeur: &str| {
        a.push(cle.to_string());
        a.push(valeur.to_string());
    };
    if let Some(m) = o.modele.as_deref().filter(|m| !m.is_empty()) {
        pousser("--model", m);
    }
    if let Some(e) = o.effort.as_deref().filter(|e| EFFORTS.contains(e)) {
        pousser("--effort", e);
    }
    pousser("--permission-mode", nom_mode(politique.permission_mode));
    // La CARTE passe par le flux (`can_use_tool`) — sauf en `dontAsk`, qui ne demande jamais
    // (le SDK ne posait alors pas `canUseTool`).
    if politique.permission_mode != ClaudePermissionMode::DontAsk {
        pousser("--permission-prompt-tool", "stdio");
    }
    let mut permis = politique.allowed_tools.clone();
    permis.extend(o.skills.iter().map(|s| format!("Skill({s})")));
    if !permis.is_empty() {
        pousser("--allowedTools", &permis.join(","));
    }
    if !politique.disallowed_tools.is_empty() {
        pousser("--disallowedTools", &politique.disallowed_tools.join(","));
    }
    let mut vus = Vec::new();
    for d in &o.dossiers {
        if !d.is_empty() && !vus.contains(d) {
            vus.push(d.clone());
            pousser("--add-dir", d);
        }
    }
    if let Some(d) = o.dossier_agent.as_deref().filter(|d| !d.is_empty()) {
        pousser("--plugin-dir", d);
    }
    // La PENSÉE résumée (options cachées du binaire, passées par le SDK).
    a.push("--thinking=adaptive".into());
    a.extend(["--thinking-display".into(), "summarized".into()]);
    match session {
        ChoixSession::Neuve { id } => a.extend(["--session-id".into(), id.clone()]),
        ChoixSession::Reprise { id } => {
            a.extend(["--resume".into(), id.clone()]);
            // ★ Sans lui, une reprise garde le prompt système de son PREMIER tour (S9).
            a.extend(["--system-prompt-snapshot".into(), "off".into()]);
        }
        ChoixSession::Fork { id, depuis, at } => {
            a.extend([
                "--resume".into(),
                depuis.clone(),
                "--fork-session".into(),
                "--session-id".into(),
                id.clone(),
                "--resume-session-at".into(),
                at.clone(),
                "--system-prompt-snapshot".into(),
                "off".into(),
            ]);
        }
    }
    a
}

/// Une variable que le processus n'hérite jamais : tout `CLAUDE*` et `ANTHROPIC*`.
/// ★ Une session Claude Code exporte `CLAUDECODE`, `CLAUDE_CODE_SIMPLE` (le mode `--bare`, qui
///   ignore le login) ou une URL de proxy : le binaire de l'agent ne doit rien en voir.
pub fn variable_exclue(cle: &str) -> bool {
    let c = cle.to_ascii_uppercase();
    c.starts_with("CLAUDE") || c.starts_with("ANTHROPIC")
}

/// L'environnement du tour : celui de l'app, moins les variables exclues, le shim `fragment` en
/// tête du `PATH`. ★ Sous Windows la clé s'écrit souvent `Path` : on garde celle qui existe (en
/// ajouter une seconde laisserait l'OS choisir).
pub fn environnement(
    base: impl IntoIterator<Item = (String, String)>,
    shim: Option<&str>,
    windows: bool,
) -> BTreeMap<String, String> {
    let mut env: BTreeMap<String, String> = base
        .into_iter()
        .filter(|(k, _)| !variable_exclue(k))
        .collect();
    if let Some(dossier) = shim.filter(|d| !d.is_empty()) {
        let cle = env
            .keys()
            .find(|k| k.eq_ignore_ascii_case("PATH"))
            .cloned()
            .unwrap_or_else(|| "PATH".to_string());
        let separateur = if windows { ";" } else { ":" };
        let valeur = match env.get(&cle).filter(|v| !v.is_empty()) {
            Some(v) => format!("{dossier}{separateur}{v}"),
            None => dossier.to_string(),
        };
        env.insert(cle, valeur);
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_model::{assistant_policy, compile_claude, open_vault_policy};

    fn valeur<'a>(a: &'a [String], cle: &str) -> Option<&'a str> {
        a.iter()
            .position(|x| x == cle)
            .and_then(|i| a.get(i + 1))
            .map(String::as_str)
    }

    #[test]
    fn reprendre_si_le_transcript_existe_sinon_brancher_sinon_creer() {
        assert_eq!(
            choisir_session("a", Some(("p", "u")), |_| true),
            ChoixSession::Reprise { id: "a".into() }
        );
        assert_eq!(
            choisir_session("a", Some(("p", "u")), |_| false),
            ChoixSession::Fork {
                id: "a".into(),
                depuis: "p".into(),
                at: "u".into()
            }
        );
        assert_eq!(
            choisir_session("a", None, |_| false),
            ChoixSession::Neuve { id: "a".into() }
        );
    }

    #[test]
    fn sans_reglage_coffre_ouvert_compile_la_liste_blanche_historique_dontask_bash_write_dans_le_coffre()
     {
        let c = compile_claude(&open_vault_policy(), None);
        let a = arguments(
            &ChoixSession::Neuve { id: "s".into() },
            &c,
            &OptionsTour::default(),
        );
        assert_eq!(valeur(&a, "--permission-mode"), Some("dontAsk"));
        // `dontAsk` ne demande jamais : pas de carte par le flux.
        assert!(!a.contains(&"--permission-prompt-tool".to_string()));
        let permis: Vec<&str> = valeur(&a, "--allowedTools")
            .unwrap_or("")
            .split(',')
            .collect();
        for t in ["Read", "Bash", "PowerShell", "Write(./**)"] {
            assert!(permis.contains(&t), "{t} manque : {permis:?}");
        }
        let refuses = valeur(&a, "--disallowedTools").unwrap_or("");
        assert!(refuses.contains("Write(./.fragment/plugins/agent/**)"));
    }

    #[test]
    fn assistant_demande_par_le_flux_en_mode_default() {
        let c = compile_claude(&assistant_policy(), None);
        let a = arguments(
            &ChoixSession::Neuve { id: "s".into() },
            &c,
            &OptionsTour::default(),
        );
        assert_eq!(valeur(&a, "--permission-mode"), Some("default"));
        assert_eq!(valeur(&a, "--permission-prompt-tool"), Some("stdio"));
    }

    #[test]
    fn isolement_pensee_et_flux_toujours_passes() {
        let c = compile_claude(&open_vault_policy(), None);
        let a = arguments(
            &ChoixSession::Neuve { id: "s".into() },
            &c,
            &OptionsTour::default(),
        );
        for x in [
            "-p",
            "--verbose",
            "--include-partial-messages",
            "--setting-sources=",
            "--strict-mcp-config",
            "--thinking=adaptive",
        ] {
            assert!(a.contains(&x.to_string()), "{x} manque");
        }
        assert_eq!(valeur(&a, "--input-format"), Some("stream-json"));
        assert_eq!(valeur(&a, "--output-format"), Some("stream-json"));
        assert_eq!(valeur(&a, "--thinking-display"), Some("summarized"));
        assert_eq!(valeur(&a, "--session-id"), Some("s"));
        // Une session neuve n'a pas d'instantané à couper.
        assert!(!a.contains(&"--system-prompt-snapshot".to_string()));
    }

    #[test]
    fn modele_effort_dossiers_et_skills() {
        let c = compile_claude(&open_vault_policy(), Some("C:/agent"));
        let o = OptionsTour {
            modele: Some("claude-haiku-5-5".into()),
            effort: Some("high".into()),
            dossier_agent: Some("C:/agent".into()),
            skills: vec!["fragment-explain:cours".into()],
            dossiers: vec![
                "C:/captures".into(),
                "C:/pieces".into(),
                "C:/captures".into(),
            ],
        };
        let a = arguments(&ChoixSession::Neuve { id: "s".into() }, &c, &o);
        assert_eq!(valeur(&a, "--model"), Some("claude-haiku-5-5"));
        assert_eq!(valeur(&a, "--effort"), Some("high"));
        assert_eq!(valeur(&a, "--plugin-dir"), Some("C:/agent"));
        assert_eq!(a.iter().filter(|x| *x == "--add-dir").count(), 2);
        let permis = valeur(&a, "--allowedTools").unwrap_or("");
        assert!(permis.contains("Skill(fragment-explain:cours)"));
        assert!(permis.contains("Write(C:/agent/**)"));
        // Un effort inconnu n'est pas transmis.
        let o = OptionsTour {
            effort: Some("turbo".into()),
            ..OptionsTour::default()
        };
        let a = arguments(&ChoixSession::Neuve { id: "s".into() }, &c, &o);
        assert!(!a.contains(&"--effort".to_string()));
    }

    #[test]
    fn reprise_sans_instantane_du_prompt() {
        let c = compile_claude(&open_vault_policy(), None);
        let a = arguments(
            &ChoixSession::Reprise { id: "s".into() },
            &c,
            &OptionsTour::default(),
        );
        assert_eq!(valeur(&a, "--resume"), Some("s"));
        assert_eq!(valeur(&a, "--system-prompt-snapshot"), Some("off"));
        assert!(!a.contains(&"--session-id".to_string()));
    }

    #[test]
    fn fork_resume_fork_session_session_id_resume_session_at() {
        let c = compile_claude(&open_vault_policy(), None);
        let f = ChoixSession::Fork {
            id: "b".into(),
            depuis: "a".into(),
            at: "u".into(),
        };
        let a = arguments(&f, &c, &OptionsTour::default());
        assert_eq!(valeur(&a, "--resume"), Some("a"));
        assert!(a.contains(&"--fork-session".to_string()));
        assert_eq!(valeur(&a, "--session-id"), Some("b"));
        assert_eq!(valeur(&a, "--resume-session-at"), Some("u"));
        assert_eq!(valeur(&a, "--system-prompt-snapshot"), Some("off"));
    }

    #[test]
    fn environnement_sans_claude_ni_anthropic_et_shim_en_tete_du_path() {
        let base = [
            ("Path", "C:/Windows"),
            ("CLAUDECODE", "1"),
            ("CLAUDE_CODE_SIMPLE", "1"),
            ("anthropic_base_url", "http://proxy"),
            ("ANTHROPIC_API_KEY", "x"),
            ("HOME", "C:/u"),
        ]
        .map(|(k, v)| (k.to_string(), v.to_string()));
        let env = environnement(base, Some("C:/shim"), true);
        assert_eq!(
            env.keys().map(String::as_str).collect::<Vec<_>>(),
            ["HOME", "Path"]
        );
        assert_eq!(
            env.get("Path").map(String::as_str),
            Some("C:/shim;C:/Windows")
        );
        // Sans PATH du tout : la clé est créée.
        let env = environnement(Vec::new(), Some("/shim"), false);
        assert_eq!(env.get("PATH").map(String::as_str), Some("/shim"));
        // Sans shim : le PATH ne change pas.
        let env = environnement([("PATH".to_string(), "/bin".to_string())], None, false);
        assert_eq!(env.get("PATH").map(String::as_str), Some("/bin"));
    }
}
