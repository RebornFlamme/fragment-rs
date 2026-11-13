// Port de app/e2e/agent-harness.spec.ts et agent-permissions.spec.ts (la part du MOTEUR : ce que
// le double du SDK recevait et rendait), rejoués sur les transcripts RÉELS de S9 réécrits en
// fixtures (`tests/fixtures/harness/`). Aucun `claude` n'est lancé : le faux processus rejoue.
//
// Un `#[test]` par cas TS quand il y en a un (même nom en snake_case), sinon un cas ajouté
// (reprise, fork, interruption, erreurs du binaire, courriel), noté dans `parite.d`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fragment_agent::contrat::{
    AgentEvent, AgentHarness, AgentPolicy, ChatBlock, Entree, ForkSource, GuardDecision,
    HarnessConversation, HarnessIO, HarnessTurn, PermanentRule, PermissionDecision,
    PermissionRequest, Remember, RememberScope, Role, ToolFamily, ToolUse,
};
use fragment_agent::harness_claude::faux::FauxLanceur;
use fragment_agent::harness_claude::protocole::REFUS_ARRET;
use fragment_agent::harness_claude::transcript::encoder_cwd;
use fragment_agent::harness_claude::{ConfigClaude, HarnessClaude, Interrupteur, PRIORITE};
use fragment_agent::harness_registry::{CLAUDE_HARNESS_ID, HarnessRegistry};
use fragment_agent::policy_model::{assistant_policy, open_vault_policy};
use serde_json::{Value, json};

const COFFRE: &str = "C:\\coffre";
const A: &str = "5e550000-0000-4000-8000-000000000006";
const B: &str = "5e550000-0000-4000-8000-000000000007";

static N: AtomicUsize = AtomicUsize::new(0);

fn fixture(nom: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/harness")
        .join(nom);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Le banc : un harness sur le faux lanceur, un dossier de config jetable, un journal gardé.
struct Banc {
    h: Rc<HarnessClaude>,
    lanceur: FauxLanceur,
    journal: Rc<RefCell<Vec<String>>>,
    dossier: PathBuf,
}

impl Drop for Banc {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dossier);
    }
}

fn banc() -> Banc {
    let dossier = std::env::temp_dir().join(format!(
        "fragment-agent-harness-claude-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::create_dir_all(&dossier);
    let mut config = ConfigClaude::new("Coffre de test");
    config.dossier_config = dossier.join("claude");
    config.racine_coffre = Some(COFFRE.into());
    config.env_base = Some(vec![
        ("Path".into(), "C:/Windows".into()),
        ("CLAUDECODE".into(), "1".into()),
        ("ANTHROPIC_API_KEY".into(), "factice".into()),
    ]);
    config.shim = Some("C:/fragment/bin".into());
    config.dossiers_lisibles = vec!["C:/fragment/captures".into()];
    let lanceur = FauxLanceur::new();
    let journal = Rc::new(RefCell::new(Vec::new()));
    let j = Rc::clone(&journal);
    let h = HarnessClaude::avec_lanceur(config, Box::new(lanceur.clone()))
        .avec_journal(move |m| j.borrow_mut().push(m.to_string()));
    Banc {
        h: Rc::new(h),
        lanceur,
        journal,
        dossier,
    }
}

impl Banc {
    /// Pose le transcript disque d'une session (là où le binaire le range).
    fn poser_transcript(&self, id: &str) {
        let d = self
            .dossier
            .join("claude/projects")
            .join(encoder_cwd(COFFRE));
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(
            d.join(format!("{id}.jsonl")),
            fixture(&format!("disque/{id}.jsonl")),
        );
    }

    fn arguments(&self) -> Vec<String> {
        self.lanceur
            .lancements()
            .last()
            .map(|l| l.arguments.clone())
            .unwrap_or_default()
    }

    fn ecrits(&self) -> Vec<Value> {
        self.lanceur
            .ecrits()
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    /// Les réponses aux demandes du binaire, dans l'ordre.
    fn reponses(&self) -> Vec<Value> {
        self.ecrits()
            .into_iter()
            .filter(|v| v["type"] == "control_response")
            .map(|v| v["response"]["response"].clone())
            .collect()
    }
}

fn valeur<'a>(a: &'a [String], cle: &str) -> Option<&'a str> {
    a.iter()
        .position(|x| x == cle)
        .and_then(|i| a.get(i + 1))
        .map(String::as_str)
}

fn tour(id: &str, prompt: &str, policy: AgentPolicy) -> HarnessTurn {
    HarnessTurn {
        conversation: HarnessConversation {
            id: id.into(),
            state: None,
        },
        prompt: prompt.into(),
        content: vec![],
        model: Some("claude-haiku-5-5".into()),
        effort: Some("medium".into()),
        instructions: "Consignes factices de l'hôte.".into(),
        profile: None,
        cwd: COFFRE.into(),
        extra_dirs: vec![],
        policy,
        fork: None,
    }
}

type Garde = Box<dyn Fn(&ToolUse) -> Option<GuardDecision>>;
type Carte = Box<dyn FnMut(&PermissionRequest) -> PermissionDecision>;

/// L'IO d'un tour : il garde tout, répond par ses règles, et peut lever l'arrêt.
struct Io {
    evenements: Vec<AgentEvent>,
    gardes: RefCell<Vec<ToolUse>>,
    garde: Garde,
    cartes: Vec<PermissionRequest>,
    carte: Carte,
    /// Levé au premier texte (l'utilisateur clique « Arrêter »).
    arreter_au_texte: Option<Interrupteur>,
    /// Levé pendant la carte (« Arrêter » pendant qu'elle attend).
    arreter_a_la_carte: Option<Interrupteur>,
}

impl Io {
    fn new() -> Self {
        Self {
            evenements: vec![],
            gardes: RefCell::new(vec![]),
            garde: Box::new(|_| None),
            cartes: vec![],
            carte: Box::new(|_| PermissionDecision::Allow { remember: None }),
            arreter_au_texte: None,
            arreter_a_la_carte: None,
        }
    }

    fn texte(&self) -> String {
        self.evenements
            .iter()
            .filter_map(|e| match e {
                AgentEvent::TextDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn done(&self) -> Option<(String, bool, Vec<String>)> {
        self.evenements.iter().find_map(|e| match e {
            AgentEvent::Done {
                session_id,
                is_error,
                errors,
                ..
            } => Some((session_id.clone(), *is_error, errors.clone())),
            _ => None,
        })
    }

    fn erreur(&self) -> Option<String> {
        self.evenements.iter().find_map(|e| match e {
            AgentEvent::Error { message } => Some(message.clone()),
            _ => None,
        })
    }

    fn resultats(&self) -> Vec<(bool, String)> {
        self.evenements
            .iter()
            .filter_map(|e| match e {
                AgentEvent::ToolResult { is_error, text, .. } => Some((*is_error, text.clone())),
                _ => None,
            })
            .collect()
    }
}

impl HarnessIO for Io {
    fn event(&mut self, e: AgentEvent) {
        if matches!(e, AgentEvent::TextDelta { .. })
            && let Some(a) = &self.arreter_au_texte
        {
            a.interrompre();
        }
        self.evenements.push(e);
    }

    fn set_state(&mut self, _state: Value) {}

    fn request_permission(&mut self, r: PermissionRequest) -> PermissionDecision {
        if let Some(a) = &self.arreter_a_la_carte {
            a.interrompre();
        }
        let d = (self.carte)(&r);
        self.cartes.push(r);
        d
    }

    fn guard(&self, usage: &ToolUse) -> Option<GuardDecision> {
        self.gardes.borrow_mut().push(usage.clone());
        (self.garde)(usage)
    }
}

// ── agent-harness ────────────────────────────────────────────────────────────

#[test]
fn le_moteur_tiers_est_inscrit_a_cote_de_claude_natif_priorite_100() {
    let b = banc();
    let mut reg = HarnessRegistry::new();
    reg.register(CLAUDE_HARNESS_ID, b.h.clone(), PRIORITE);
    assert_eq!(PRIORITE, -100);
    let h = reg.harness_of("claude").expect("inscrit");
    assert_eq!(h.label(), "Claude");
    let c = h.capabilities();
    assert!(c.resume && c.fork && c.thinking && c.text_deltas && c.permission_prompts);
    assert!(!c.structured_output, "la sortie structurée vient avec T8");
    let defaut: Vec<String> = h
        .models()
        .into_iter()
        .filter(|m| m.default == Some(true))
        .map(|m| m.id)
        .collect();
    assert_eq!(defaut, ["claude-opus-5-5"]);
    // Un modèle réglé à la main, hors liste : proposé en tête, par défaut.
    b.h.configurer(|c| c.modele = Some("claude-maison".into()));
    let m = b.h.models();
    assert_eq!(m[0].id, "claude-maison");
    assert_eq!(m[0].default, Some(true));
    assert_eq!(m.len(), 5);
}

#[test]
fn un_tour_confie_au_moteur_modele_effort_consignes_politique_flux_affiche_pensee_outil_texte() {
    let b = banc();
    b.lanceur.rejouer(&fixture("texte.jsonl"));
    let mut io = Io::new();
    b.h.run(
        tour(A, "Dis bonjour en un seul mot.", open_vault_policy()),
        &mut io,
    )
    .expect("tour");
    assert_eq!(io.texte(), "Bonjour");
    let (sid, err, _) = io.done().expect("done");
    assert!(!err);
    assert_eq!(sid, "5e550000-0000-4000-8000-000000000001");
    let l = &b.lanceur.lancements()[0];
    assert_eq!(l.programme, PathBuf::from("claude"));
    assert_eq!(l.cwd, PathBuf::from(COFFRE));
    assert_eq!(valeur(&l.arguments, "--model"), Some("claude-haiku-5-5"));
    assert_eq!(valeur(&l.arguments, "--effort"), Some("medium"));
    assert_eq!(valeur(&l.arguments, "--permission-mode"), Some("dontAsk"));
    assert_eq!(valeur(&l.arguments, "--session-id"), Some(A));
    assert_eq!(
        valeur(&l.arguments, "--add-dir"),
        Some("C:/fragment/captures")
    );
    // Les consignes de l'hôte passent par `initialize`, jamais en ligne de commande.
    let e = b.ecrits();
    assert_eq!(e[0]["request"]["subtype"], "initialize");
    assert_eq!(
        e[0]["request"]["systemPrompt"],
        json!(["Consignes factices de l'hôte."])
    );
    assert_eq!(
        e[0]["request"]["hooks"]["PreToolUse"][0]["hookCallbackIds"],
        json!(["garde"])
    );
    assert_eq!(e[1]["type"], "user");
    assert_eq!(e[1]["message"]["content"], "Dis bonjour en un seul mot.");
    assert!(!l.arguments.iter().any(|a| a.contains("Consignes")));
    // Le binaire n'hérite de rien de Claude Code ; le shim est en tête du PATH.
    assert!(!l.env.contains_key("CLAUDECODE"));
    assert!(!l.env.contains_key("ANTHROPIC_API_KEY"));
    assert_eq!(
        l.env.get("Path").map(String::as_str),
        Some(if cfg!(windows) {
            "C:/fragment/bin;C:/Windows"
        } else {
            "C:/fragment/bin:C:/Windows"
        })
    );
    // La version du binaire est journalisée.
    assert!(b.journal.borrow().iter().any(|l| l == "claude 2.1.316"));
}

#[test]
fn la_pensee_resumee_arrive_en_deltas_puis_le_texte() {
    let b = banc();
    b.lanceur.rejouer(&fixture("pensee.jsonl"));
    let mut io = Io::new();
    b.h.run(
        tour(A, "Combien font 17 + 36 ?", open_vault_policy()),
        &mut io,
    )
    .expect("tour");
    let debut = io
        .evenements
        .iter()
        .position(|e| {
            matches!(
                e,
                AgentEvent::ThinkingDelta {
                    start: Some(true),
                    ..
                }
            )
        })
        .expect("un bloc de pensée commence");
    let texte = io
        .evenements
        .iter()
        .position(|e| matches!(e, AgentEvent::TextDelta { .. }))
        .expect("du texte");
    assert!(debut < texte);
    let pensee: String = io
        .evenements
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ThinkingDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(!pensee.trim().is_empty());
    assert_eq!(io.texte(), "53");
}

#[test]
fn la_suite_reprend_la_session_sans_instantane_du_prompt() {
    let b = banc();
    b.poser_transcript(A);
    b.lanceur.rejouer(&fixture("session-2-reprise.jsonl"));
    let mut io = Io::new();
    b.h.run(
        tour(
            A,
            "Retiens aussi le mot BALEINE, puis cite tous les mots que je t'ai demandé de retenir.",
            open_vault_policy(),
        ),
        &mut io,
    )
    .expect("tour");
    let a = b.arguments();
    assert_eq!(valeur(&a, "--resume"), Some(A));
    assert_eq!(valeur(&a, "--system-prompt-snapshot"), Some("off"));
    assert!(!a.contains(&"--session-id".to_string()));
    assert_eq!(io.texte(), "ALBATROS, BALEINE.");
}

#[test]
fn une_reprise_voit_le_nouveau_prompt() {
    // Le prompt est relu à chaque tour : à la reprise, le modèle a reçu le NOUVEAU (S9).
    let b = banc();
    b.poser_transcript(A);
    b.lanceur.rejouer(&fixture("instantane-off-2.jsonl"));
    let mut t = tour(
        A,
        "Quel est ton code de test ? Réponds seulement le code.",
        open_vault_policy(),
    );
    t.instructions = "Prompt factice des tests du harness. Code : SORBIER.".into();
    let mut io = Io::new();
    b.h.run(t, &mut io).expect("tour");
    assert_eq!(
        b.ecrits()[0]["request"]["systemPrompt"][0],
        "Prompt factice des tests du harness. Code : SORBIER."
    );
    assert_eq!(
        valeur(&b.arguments(), "--system-prompt-snapshot"),
        Some("off")
    );
    assert_eq!(io.texte(), "SORBIER");
}

#[test]
fn une_branche_nait_au_dernier_point_du_tour() {
    let b = banc();
    b.poser_transcript(A);
    let point = b.h.history(&HarnessConversation {
        id: A.into(),
        state: None,
    })[1]
        .last
        .clone()
        .expect("repère");
    b.lanceur.rejouer(&fixture("session-3-fork.jsonl"));
    let mut t = tour(
        B,
        "Cite tous les mots que je t'ai demandé de retenir.",
        open_vault_policy(),
    );
    t.fork = Some(ForkSource {
        from: HarnessConversation {
            id: A.into(),
            state: None,
        },
        at: point.clone(),
    });
    let mut io = Io::new();
    b.h.run(t, &mut io).expect("tour");
    let a = b.arguments();
    assert_eq!(valeur(&a, "--resume"), Some(A));
    assert!(a.contains(&"--fork-session".to_string()));
    assert_eq!(valeur(&a, "--session-id"), Some(B));
    assert_eq!(valeur(&a, "--resume-session-at"), Some(point.as_str()));
    // B ne connaît que ce qui précède le point.
    assert_eq!(io.texte(), "ALBATROS");
    assert_eq!(io.done().map(|d| d.0).as_deref(), Some(B));
}

#[test]
fn apres_un_rechargement_l_historique_est_relu_chez_le_moteur() {
    let b = banc();
    b.poser_transcript(A);
    b.poser_transcript(B);
    let fil = b.h.history(&HarnessConversation {
        id: A.into(),
        state: None,
    });
    let roles: Vec<Role> = fil.iter().map(|e| e.role).collect();
    assert_eq!(
        roles,
        [Role::User, Role::Assistant, Role::User, Role::Assistant]
    );
    let texte = |i: usize| match &fil[i].blocks[0] {
        ChatBlock::Text { text, .. } => text.clone(),
        autre => panic!("{autre:?}"),
    };
    assert_eq!(texte(0), "Retiens le mot ALBATROS. Réponds seulement OK.");
    assert_eq!(texte(3), "ALBATROS, BALEINE.");
    // La branche garde les repères de sa source jusqu'au point.
    let branche = b.h.history(&HarnessConversation {
        id: B.into(),
        state: None,
    });
    assert_eq!(branche.len(), 4);
    assert_eq!(branche[1].last, fil[1].last);
    assert_ne!(branche[3].last, fil[3].last);
    // Une conversation inconnue : un fil vide, pas une erreur.
    assert!(
        b.h.history(&HarnessConversation {
            id: "5e550000-0000-4000-8000-0000000000ff".into(),
            state: None,
        })
        .is_empty()
    );
}

#[test]
fn le_prompt_et_les_skills_sont_relus_a_chaque_tour_dans_le_dossier_de_l_agent() {
    let b = banc();
    let agent = b.dossier.join("agent");
    let _ = std::fs::create_dir_all(agent.join(".claude-plugin"));
    let _ = std::fs::create_dir_all(agent.join("skills/cours"));
    let _ = std::fs::create_dir_all(agent.join("skills/sans-skill"));
    let _ = std::fs::write(
        agent.join(".claude-plugin/plugin.json"),
        r#"{"name":"essai"}"#,
    );
    let _ = std::fs::write(agent.join("skills/cours/SKILL.md"), "factice");
    let _ = std::fs::write(agent.join("prompt.md"), "Prompt factice pour {{vault}}.");
    b.h.configurer(|c| c.dossier_agent = Some(agent.clone()));
    b.lanceur
        .rejouer(&fixture("texte.jsonl"))
        .rejouer(&fixture("texte.jsonl"));
    let mut t = tour(A, "Dis bonjour en un seul mot.", open_vault_policy());
    t.profile = Some("Profil factice.".into());
    b.h.run(t.clone(), &mut Io::new()).expect("tour");
    let e = b.ecrits();
    assert_eq!(
        e[0]["request"]["systemPrompt"][0],
        "Prompt factice pour Coffre de test.\n\n# Cette conversation\n\nProfil factice."
    );
    let a = b.arguments();
    let dir = agent.display().to_string().replace('\\', "/");
    assert_eq!(valeur(&a, "--plugin-dir"), Some(dir.as_str()));
    let permis = valeur(&a, "--allowedTools").unwrap_or("");
    assert!(permis.contains("Skill(essai:cours)"));
    assert!(!permis.contains("sans-skill"));
    assert!(permis.contains(&format!("Write({dir}/**)")));
    // L'agent a perfectionné son prompt : le tour suivant le voit.
    let _ = std::fs::write(agent.join("prompt.md"), "Prompt factice, version 2.");
    b.h.run(t, &mut Io::new()).expect("tour");
    let init: Vec<Value> = b
        .ecrits()
        .into_iter()
        .filter(|v| v["request"]["subtype"] == "initialize")
        .collect();
    assert_eq!(
        init[1]["request"]["systemPrompt"][0],
        "Prompt factice, version 2.\n\n# Cette conversation\n\nProfil factice."
    );
}

#[test]
fn les_pieces_jointes_partent_en_blocs_avant_le_texte() {
    let b = banc();
    b.lanceur.rejouer(&fixture("pieces.jsonl"));
    let image: Entree = json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AAAA"}})
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut t = tour(A, "De quelle couleur est l'image ?", open_vault_policy());
    t.content = vec![image];
    let mut io = Io::new();
    b.h.run(t, &mut io).expect("tour");
    let user = b
        .ecrits()
        .into_iter()
        .find(|v| v["type"] == "user")
        .expect("message");
    let blocs = user["message"]["content"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(blocs.len(), 2);
    assert_eq!(blocs[0]["type"], "image");
    assert_eq!(
        blocs[1],
        json!({"type": "text", "text": "De quelle couleur est l'image ?"})
    );
    assert!(io.texte().contains("PERROQUET"));
}

// ── agent-permissions ────────────────────────────────────────────────────────

#[test]
fn sans_reglage_coffre_ouvert_compile_la_liste_blanche_historique_dontask_bash_write_dans_le_coffre()
 {
    let b = banc();
    b.lanceur.rejouer(&fixture("texte.jsonl"));
    b.h.run(tour(A, "bonjour", open_vault_policy()), &mut Io::new())
        .expect("tour");
    let a = b.arguments();
    assert_eq!(valeur(&a, "--permission-mode"), Some("dontAsk"));
    assert!(!a.contains(&"--permission-prompt-tool".to_string()));
    let permis: Vec<&str> = valeur(&a, "--allowedTools")
        .unwrap_or("")
        .split(',')
        .collect();
    for t in ["Read", "Bash", "PowerShell", "Write(./**)"] {
        assert!(permis.contains(&t), "{t}");
    }
    assert!(
        valeur(&a, "--disallowedTools")
            .unwrap_or("")
            .contains("Write(./.fragment/plugins/agent/**)")
    );
}

#[test]
fn la_garde_est_appelee_avant_chaque_outil_lecture_comprise() {
    let b = banc();
    b.lanceur.rejouer(&fixture("garde-autorise.jsonl"));
    let mut io = Io::new();
    io.garde = Box::new(|_| Some(GuardDecision::Allow));
    b.h.run(
        tour(
            A,
            "Lis le fichier note.md et donne le mot du jour.",
            assistant_policy(),
        ),
        &mut io,
    )
    .expect("tour");
    let gardes = io.gardes.borrow().clone();
    assert_eq!(gardes.len(), 1);
    assert_eq!(gardes[0].tool, "Read");
    assert_eq!(gardes[0].input["file_path"], "C:\\coffre\\note.md");
    // Autorisé : `{}`, les règles du binaire décident.
    assert_eq!(b.reponses(), [json!({})]);
    assert_eq!(io.resultats().len(), 1);
    assert!(!io.resultats()[0].0);
    assert!(io.texte().contains("GIROFLE"));
}

#[test]
fn la_garde_dossier_masque_refuse_chemins_de_fragment_proteges_meme_en_acces_complet() {
    let b = banc();
    b.lanceur.rejouer(&fixture("garde-refuse.jsonl"));
    let mut io = Io::new();
    io.garde = Box::new(|_| {
        Some(GuardDecision::Deny {
            message: "La garde de test refuse ce fichier.".into(),
        })
    });
    b.h.run(
        tour(A, "Lis le fichier secret.md.", open_vault_policy()),
        &mut io,
    )
    .expect("tour");
    assert_eq!(
        b.reponses(),
        [json!({"hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "La garde de test refuse ce fichier.",
        }})]
    );
    // L'outil ne s'exécute pas, aucune carte : le modèle lit le refus.
    assert!(io.cartes.is_empty());
    let r = io.resultats();
    assert!(r[0].0);
    assert!(r[0].1.contains("La garde de test refuse ce fichier."));
}

#[test]
fn la_garde_demander_force_la_carte_meme_pour_une_commande_en_lecture_seule() {
    let b = banc();
    b.lanceur.rejouer(&fixture("garde-demande.jsonl"));
    let mut io = Io::new();
    io.garde = Box::new(|_| Some(GuardDecision::Ask));
    io.carte = Box::new(|_| PermissionDecision::Deny {
        message: Some("Refusé par l'utilisateur (test).".into()),
    });
    b.h.run(
        tour(A, "Lance la commande `ls`.", assistant_policy()),
        &mut io,
    )
    .expect("tour");
    let r = b.reponses();
    assert_eq!(r[0]["hookSpecificOutput"]["permissionDecision"], "ask");
    assert_eq!(io.cartes.len(), 1);
    assert_eq!(io.cartes[0].tool, "Bash");
    assert_eq!(io.cartes[0].family, Some(ToolFamily::Shell));
    assert_eq!(io.cartes[0].input["command"], "ls");
    // Comme le TS (`reason ?? description`) : une raison VIDE du binaire masque la description.
    assert_eq!(io.cartes[0].reason, None);
    assert_eq!(
        r[1],
        json!({"behavior": "deny", "message": "Refusé par l'utilisateur (test)."})
    );
    assert!(io.resultats()[0].0);
}

#[test]
fn assistant_une_carte_dans_la_reponse_autoriser_une_fois_l_outil_passe() {
    let b = banc();
    b.lanceur.rejouer(&fixture("carte-autorise.jsonl"));
    let mut io = Io::new();
    // « Toujours » : la règle reste chez Fragment, le binaire ne reçoit que l'autorisation.
    io.carte = Box::new(|_| PermissionDecision::Allow {
        remember: Some(Remember {
            scope: RememberScope::Conversation,
            rule: PermanentRule {
                family: ToolFamily::Write,
                pattern: None,
            },
        }),
    });
    b.h.run(
        tour(
            A,
            "Écris le mot DEUX dans le fichier b.txt.",
            assistant_policy(),
        ),
        &mut io,
    )
    .expect("tour");
    assert_eq!(
        valeur(&b.arguments(), "--permission-prompt-tool"),
        Some("stdio")
    );
    assert_eq!(valeur(&b.arguments(), "--permission-mode"), Some("default"));
    assert_eq!(io.cartes.len(), 1);
    assert_eq!(io.cartes[0].tool, "Write");
    assert_eq!(io.cartes[0].family, Some(ToolFamily::Write));
    let carte = b.reponses().pop().expect("réponse de la carte");
    assert_eq!(
        carte,
        json!({"behavior": "allow", "updatedInput": {"content": "DEUX", "file_path": "C:\\coffre\\b.txt"}})
    );
    assert!(carte.get("updatedPermissions").is_none());
    assert!(io.texte().contains("DEUX"));
}

#[test]
fn refuser_avec_un_pourquoi_l_agent_recoit_le_message() {
    let b = banc();
    b.lanceur.rejouer(&fixture("carte-refus.jsonl"));
    let mut io = Io::new();
    io.carte = Box::new(|_| PermissionDecision::Deny {
        message: Some("pas maintenant".into()),
    });
    b.h.run(
        tour(
            A,
            "Écris le mot UN dans le fichier a.txt.",
            assistant_policy(),
        ),
        &mut io,
    )
    .expect("tour");
    let carte = b.reponses().pop().expect("réponse de la carte");
    assert_eq!(
        carte,
        json!({"behavior": "deny", "message": "pas maintenant"})
    );
}

#[test]
fn arreter_un_tour_qui_attend_une_reponse_la_carte_est_refusee_et_repliee() {
    let b = banc();
    b.lanceur.rejouer(&fixture("carte-refus.jsonl"));
    let mut io = Io::new();
    io.arreter_a_la_carte = Some(b.h.interrupteur(A));
    io.carte = Box::new(|_| PermissionDecision::Allow { remember: None });
    b.h.run(
        tour(
            A,
            "Écris le mot UN dans le fichier a.txt.",
            assistant_policy(),
        ),
        &mut io,
    )
    .expect("tour");
    let carte = b.reponses().pop().expect("réponse de la carte");
    assert_eq!(carte, json!({"behavior": "deny", "message": REFUS_ARRET}));
    // L'interruption est partie juste après, une seule fois.
    let interruptions = b
        .ecrits()
        .into_iter()
        .filter(|v| v["request"]["subtype"] == "interrupt")
        .count();
    assert_eq!(interruptions, 1);
    // Un tour arrêté finit sans erreur.
    let (_, err, errors) = io.done().expect("done");
    assert!(!err && errors.is_empty());
}

// ── Cas ajoutés : interruption, erreurs du binaire, compte ────────────────────

#[test]
fn interrompre_au_premier_texte_rend_un_done_d_arret_sans_erreur() {
    let b = banc();
    b.lanceur.rejouer(&fixture("interruption.jsonl"));
    let mut io = Io::new();
    io.arreter_au_texte = Some(b.h.interrupteur(A));
    let run =
        b.h.run(
            tour(A, "Écris les nombres de un à cent.", open_vault_policy()),
            &mut io,
        )
        .expect("tour");
    assert!(run.est_termine());
    let e = b.ecrits();
    assert_eq!(
        e.iter()
            .filter(|v| v["request"]["subtype"] == "interrupt")
            .count(),
        1
    );
    let (_, err, errors) = io.done().expect("done");
    assert!(!err, "un arrêt voulu n'est pas une erreur");
    assert!(errors.is_empty(), "[ede_diagnostic] n'est pas montré");
    assert!(io.erreur().is_none());
}

#[test]
fn interrompre_avant_le_message_ne_l_envoie_pas() {
    let b = banc();
    b.lanceur.rejouer(&fixture("texte.jsonl"));
    b.h.interrupteur(A).interrompre();
    let mut io = Io::new();
    b.h.run(tour(A, "Dis bonjour.", open_vault_policy()), &mut io)
        .expect("tour");
    assert!(b.ecrits().iter().all(|v| v["type"] != "user"));
    assert_eq!(io.done(), Some((A.to_string(), false, vec![])));
    // Le drapeau ne survit pas au tour : le suivant part normalement.
    b.lanceur.rejouer(&fixture("texte.jsonl"));
    let mut io = Io::new();
    b.h.run(tour(A, "Dis bonjour.", open_vault_policy()), &mut io)
        .expect("tour");
    assert_eq!(io.texte(), "Bonjour");
    // Sans tour en cours, interrompre ne fait rien.
    b.h.interrompre("inconnue");
}

#[test]
fn option_inconnue_le_message_est_la_fin_de_stderr() {
    let b = banc();
    b.lanceur.rejouer(&fixture("option-inconnue.jsonl"));
    let mut io = Io::new();
    b.h.run(tour(A, "Réponds OK.", open_vault_policy()), &mut io)
        .expect("tour");
    let m = io.erreur().expect("erreur");
    assert!(m.contains("code 1"), "{m}");
    assert!(m.contains("unknown option"), "{m}");
    assert!(io.done().is_none());
}

#[test]
fn session_id_invalide_sort_avant_initialize() {
    let b = banc();
    b.lanceur.rejouer(&fixture("session-id-invalide.jsonl"));
    let mut io = Io::new();
    b.h.run(
        tour("pas-un-uuid", "Réponds OK.", open_vault_policy()),
        &mut io,
    )
    .expect("tour");
    assert!(io.erreur().expect("erreur").contains("Invalid session ID"));
}

#[test]
fn reprise_d_une_session_absente_est_une_erreur_du_tour() {
    let b = banc();
    b.lanceur.rejouer(&fixture("reprise-inconnue.jsonl"));
    let mut io = Io::new();
    b.h.run(tour(A, "Réponds OK.", open_vault_policy()), &mut io)
        .expect("tour");
    assert!(
        io.erreur()
            .expect("erreur")
            .contains("No conversation found")
    );
}

#[test]
fn modele_inconnu_le_texte_du_result_est_l_erreur_du_done() {
    let b = banc();
    b.lanceur.rejouer(&fixture("modele-inconnu.jsonl"));
    let mut io = Io::new();
    b.h.run(tour(A, "Réponds OK.", open_vault_policy()), &mut io)
        .expect("tour");
    let (_, err, errors) = io.done().expect("done");
    assert!(err);
    assert!(errors[0].contains("claude-modele-inexistant"), "{errors:?}");
}

#[test]
fn un_binaire_introuvable_rend_le_harness_indisponible() {
    let b = banc();
    b.lanceur.echouer("introuvable");
    let r =
        b.h.run(tour(A, "Réponds OK.", open_vault_policy()), &mut Io::new());
    assert!(r.is_err());
    b.h.configurer(|c| c.binaire = PathBuf::from("C:/rien/claude.exe"));
    assert!(!b.h.status().ready);
}

#[test]
fn la_reponse_de_demarrage_et_son_courriel_ne_sont_jamais_journalises_ni_ecrits() {
    let b = banc();
    // ★ Le courriel est construit ici, jamais écrit dans une fixture.
    let courriel = format!("{}@{}", "titulaire.du.compte", "exemple.test");
    let mut lignes: Vec<String> = fixture("texte.jsonl")
        .replace("<courriel>", &courriel)
        .lines()
        .map(str::to_string)
        .collect();
    // Une ligne non JSON qui le porte aussi (journalisée, masquée).
    lignes.insert(2, format!("< compte connecté : {courriel}"));
    b.lanceur.rejouer(&lignes.join("\n"));
    let avant = fichiers(&b.dossier);
    let mut io = Io::new();
    b.h.run(tour(A, "Dis bonjour.", open_vault_policy()), &mut io)
        .expect("tour");
    assert_eq!(io.texte(), "Bonjour");
    let journal = b.journal.borrow().join("\n");
    assert!(journal.contains("ligne non JSON ignorée : compte connecté : <courriel>"));
    assert!(!journal.contains(&courriel));
    assert!(!format!("{:?}", io.evenements).contains(&courriel));
    assert!(!b.lanceur.ecrits().join("\n").contains(&courriel));
    // Rien n'est écrit sur le disque par le harness.
    assert_eq!(fichiers(&b.dossier), avant);
}

fn fichiers(d: &Path) -> BTreeMap<PathBuf, u64> {
    let mut out = BTreeMap::new();
    let mut pile = vec![d.to_path_buf()];
    while let Some(p) = pile.pop() {
        for e in std::fs::read_dir(&p).into_iter().flatten().flatten() {
            let q = e.path();
            if q.is_dir() {
                pile.push(q);
            } else {
                out.insert(q, e.metadata().map(|m| m.len()).unwrap_or(0));
            }
        }
    }
    out
}

/// Le VRAI binaire, pour la main : `cargo test -p fragment-agent --test harness_claude -- --ignored`.
/// Lance `claude` du PATH avec le login local (aucune clé), un tour court au petit modèle.
#[test]
#[ignore = "lance le vrai binaire claude (login local) : jamais en CI"]
fn un_vrai_tour_contre_le_binaire_installe() {
    let dossier =
        std::env::temp_dir().join(format!("fragment-harness-reel-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dossier);
    let mut config = ConfigClaude::new("Coffre de test");
    config.racine_coffre = Some(dossier.display().to_string());
    let h = HarnessClaude::new(config);
    let id = "5e550000-0000-4000-8000-00000000abcd";
    let mut t = tour(id, "Réponds seulement OK.", open_vault_policy());
    t.cwd = dossier.display().to_string();
    t.instructions = "Prompt factice d'un test manuel. Réponds en un mot.".into();
    let mut io = Io::new();
    h.run(t, &mut io).expect("tour");
    let _ = std::fs::remove_dir_all(&dossier);
    assert!(io.done().is_some(), "{:?}", io.evenements);
}
