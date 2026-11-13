// Port de app/e2e/agent-harness.spec.ts (le moteur figé, l'état persisté, le moteur par défaut,
// le moteur absent), agent-permissions.spec.ts (la politique, la garde, « toujours », le plan),
// agent-chat.spec.ts (`openAnchored`) et des comportements de SessionIndex.ts, AgentPlugin.ts
// (réglages, dossier de l'agent). Le moteur est le FauxHarness : aucun appel à `claude`, aucun réseau.
//
// Un `#[test]` par cas TS, même nom en snake_case ; les cas qui sont de la VUE ou du harness réel
// sont notés dans `parite.d/fr-t7-service.toml`. ★ Le prompt de l'agent est un prompt FACTICE,
// écrit pour ces tests (décision de l'orchestrateur : rien de Fragment n'est copié).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fs;
use std::rc::Rc;

use fragment_agent::contrat::{
    AgentChatOptions, AgentEvent, AgentHarness, AgentQuery, AgentService, ErreurHarness,
    FolderAccess, GuardDecision, PermanentRule, PermissionDecision, PermissionMode,
    PermissionRequest, Remember, RememberScope, ToolFamily, ToolUse,
};
use fragment_agent::fournisseur::{AgentChatProvider, EnvoiAgent};
use fragment_agent::fournisseur::{AgentHote, Auteur, DemandeTour, FauxHarness, Pas, SortieTour};
use fragment_agent::policy_model::{assistant_policy, open_vault_policy};
use fragment_agent::service::dossiers::{Dossiers, nom_de_dossier};
use fragment_agent::service::reglages::{Reglages, StockageMemoire, nom_utilisateur};
use fragment_agent::service::sessions::{Journal, ModifSession};
use fragment_agent::service::vues::{
    ChatOuvert, EtatOuverture, HoteVues, OuvertureAncree, OuvertureChat,
};
use fragment_agent::service::{ConfigService, ServiceAgent};
use fragment_artefacts::artefact_manager::NoVaultFiles;
use fragment_artefacts::clock::Now;
use fragment_artefacts::{ArtefactManager, ArtefactManagerOptions, MemoryFs};
use fragment_chat::api::{ChatAttachment, ChatEvent, ChatProvider};
use fragment_chat::chat_key::chat_key;
use fragment_chat::chat_surface_model::MESSAGE_STRIDE;
use serde_json::{Value, json};

// ── Le banc ──────────────────────────────────────────────────────────────────

fn horloge() -> Now {
    let h = Rc::new(Cell::new(1_792_497_600_000_i64));
    Rc::new(move || {
        let t = h.get();
        h.set(t + 1);
        t
    })
}

fn journal_sur(fs: &Rc<MemoryFs>, appareil: &str, now: &Now) -> Journal {
    Rc::new(RefCell::new(ArtefactManager::new(
        fs.clone(),
        ".fragment",
        Rc::new(NoVaultFiles),
        ArtefactManagerOptions {
            now: Some(now.clone()),
            device_id: Some(appareil.into()),
            session: Some("b71".into()),
            ..Default::default()
        },
    )))
}

struct Banc {
    svc: Rc<ServiceAgent>,
    claude: Rc<FauxHarness>,
    stockage: Rc<StockageMemoire>,
    _dir: tempfile::TempDir,
}

fn banc_avec(config: impl FnOnce(&mut ConfigService)) -> Banc {
    let dir = tempfile::tempdir().expect("dossier temporaire");
    let now = horloge();
    let fs = Rc::new(MemoryFs::new(now.clone()));
    let stockage = Rc::new(StockageMemoire::default());
    let mut c = ConfigService {
        racine_coffre: Some("C:/coffre".into()),
        nom_coffre: "Mon coffre".into(),
        donnees: dir.path().join("userData"),
        dossier_personnel: Some(dir.path().join("home")),
        installation_fragment: None,
        // ★ Un prompt FACTICE, dans nos mots : il ne sert qu'au mécanisme (gabarit, relecture).
        consignes: "Consignes factices du coffre {{vault}}.".into(),
        compte: Some("julie".into()),
    };
    config(&mut c);
    let svc = Rc::new(ServiceAgent::new(
        c,
        journal_sur(&fs, "k3f9", &now),
        stockage.clone(),
        now,
    ));
    svc.demarrer();
    let claude = FauxHarness::new("claude");
    svc.inscrire_harness("claude", claude.clone() as Rc<dyn AgentHarness>, -100);
    Banc {
        svc,
        claude,
        stockage,
        _dir: dir,
    }
}

fn banc() -> Banc {
    banc_avec(|_| {})
}

/// Une sortie de tour de test : garde les événements et les demandes, répond dans l'ordre.
#[derive(Default)]
struct Sortie {
    evenements: Vec<AgentEvent>,
    decisions: VecDeque<PermissionDecision>,
    demandes: Vec<PermissionRequest>,
}

impl Sortie {
    fn repondant(d: Vec<PermissionDecision>) -> Self {
        Self {
            decisions: d.into(),
            ..Default::default()
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
}

impl SortieTour for Sortie {
    fn evenement(&mut self, e: AgentEvent) {
        self.evenements.push(e);
    }

    fn permission(&mut self, r: PermissionRequest) -> PermissionDecision {
        self.demandes.push(r);
        self.decisions
            .pop_front()
            .unwrap_or(PermissionDecision::Deny {
                message: Some("personne".into()),
            })
    }
}

fn demande_de(conv: &str, prompt: &str) -> DemandeTour {
    DemandeTour {
        prompt: prompt.into(),
        session_id: conv.into(),
        harness: "claude".into(),
        ..Default::default()
    }
}

fn tour(b: &Banc, conv: &str, prompt: &str) -> Sortie {
    let mut s = Sortie::default();
    let r = b.svc.lancer(demande_de(conv, prompt), &mut s);
    assert!(r.is_ok(), "{r:?}");
    s
}

fn tour_avec(b: &Banc, d: DemandeTour, s: &mut Sortie) -> Result<(), ErreurHarness> {
    b.svc.lancer(d, s)
}

fn meta_de(b: &Banc, conv: &str) -> serde_json::Map<String, Value> {
    b.svc
        .sessions()
        .get(conv)
        .map(|s| s.meta)
        .unwrap_or_default()
}

fn requete(tool: &str, famille: Option<ToolFamily>, input: Value) -> PermissionRequest {
    PermissionRequest {
        tool: tool.into(),
        family: famille,
        input: input.as_object().cloned().unwrap_or_default(),
        title: None,
        reason: None,
        plan: None,
    }
}

fn politique_assistant(b: &Banc) {
    let r = b.svc.reglages().poser(
        "policy",
        serde_json::to_value(assistant_policy()).unwrap_or_default(),
    );
    assert!(r.is_ok());
}

fn utilisation(tool: &str, input: Value) -> ToolUse {
    ToolUse {
        tool: tool.into(),
        input: input.as_object().cloned().unwrap_or_default(),
    }
}

// ═══════════════════════ SessionIndex : l'historique au journal ═══════════════════════

#[test]
fn sessions_relues_au_journal_la_plus_recente_d_abord() {
    let b = banc();
    tour(&b, "a", "première");
    tour(&b, "b", "seconde");
    // Un tour de plus sur « a » la rend la plus récente.
    tour(&b, "a", "encore");
    let ids: Vec<String> = b.svc.sessions().liste().into_iter().map(|s| s.id).collect();
    assert_eq!(ids, ["a", "b"]);
    assert_eq!(
        b.svc.sessions().get("b").map(|s| s.title),
        Some("seconde".to_string())
    );
}

#[test]
fn une_session_ecrite_par_un_autre_appareil_apparait_au_chargement_du_journal() {
    let now = horloge();
    let fs = Rc::new(MemoryFs::new(now.clone()));
    let ja = journal_sur(&fs, "k3f9", &now);
    let a = service_sur(&ja, &now);
    a.sessions().toucher(
        "ordi",
        ModifSession {
            title: Some("écrit depuis l'ordinateur".into()),
            ..Default::default()
        },
        Auteur::Utilisateur,
    );
    // Le journal de l'appareil A est vidé sur le disque partagé.
    ja.borrow_mut().flushed();
    let jb = journal_sur(&fs, "m2x7", &now);
    let b = service_sur(&jb, &now);
    assert!(
        b.sessions().liste().is_empty(),
        "avant `ready`, l'index est vide"
    );
    jb.borrow_mut().load();
    let titres: Vec<String> = b.sessions().liste().into_iter().map(|s| s.title).collect();
    assert_eq!(titres, ["écrit depuis l'ordinateur"]);
}

fn service_sur(j: &Journal, now: &Now) -> Rc<ServiceAgent> {
    let svc = Rc::new(ServiceAgent::new(
        ConfigService::default(),
        Rc::clone(j),
        Rc::new(StockageMemoire::default()),
        now.clone(),
    ));
    svc.demarrer();
    svc
}

#[test]
fn l_index_suit_les_ecritures_et_une_suppression_pose_une_tombe() {
    let b = banc();
    let v0 = b.svc.sessions().version();
    tour(&b, "c", "bonjour");
    assert!(
        b.svc.sessions().version() > v0,
        "l'abonnement a invalidé l'index"
    );
    assert_eq!(b.svc.sessions().liste().len(), 1);
    b.svc.supprimer("c");
    assert!(b.svc.sessions().get("c").is_none());
    assert!(b.svc.sessions().liste().is_empty());
}

#[test]
fn la_session_neuve_a_son_titre_tire_de_la_demande_ses_dates_et_son_modele() {
    let b = banc();
    tour(&b, "t", "  une   très\n longue demande  ");
    let info = b.svc.sessions().info("t");
    assert_eq!(
        info.as_ref().map(|i| i.title.as_str()),
        Some("une très longue demande")
    );
    // Claude lit lui-même le réglage `model` : la session garde celui des réglages.
    assert_eq!(
        info.as_ref().and_then(|i| i.model.clone()),
        Some("claude-opus-5-5".to_string())
    );
    assert!(info.is_some_and(|i| i.created_at > 0 && i.updated_at >= i.created_at));
}

#[test]
fn toucher_d_une_session_inconnue_est_sans_effet_pour_le_fournisseur() {
    let b = banc();
    b.svc
        .toucher("fantome", serde_json::Map::new(), Auteur::Plugin);
    assert!(b.svc.sessions().get("fantome").is_none());
}

// ═══════════════════════ agent-harness.spec.ts ═══════════════════════

#[test]
fn le_moteur_tiers_est_inscrit_a_cote_de_claude_natif_priorite_100() {
    let b = banc();
    b.svc
        .inscrire_harness("echo", FauxHarness::new("echo") as Rc<dyn AgentHarness>, 0);
    let mut ids: Vec<String> = b
        .svc
        .harnesses()
        .iter()
        .map(|h| h.id().to_string())
        .collect();
    ids.sort();
    assert_eq!(ids, ["claude", "echo"]);
}

#[test]
fn un_tour_confie_au_moteur_modele_effort_consignes_politique() {
    let b = banc();
    let echo = FauxHarness::new("echo");
    b.svc
        .inscrire_harness("echo", echo.clone() as Rc<dyn AgentHarness>, 0);
    let mut s = Sortie::default();
    let r = tour_avec(
        &b,
        DemandeTour {
            harness: "echo".into(),
            effort: Some("high".into()),
            system: Some("Profil de test.".into()),
            ..demande_de("e", "bonjour moteur")
        },
        &mut s,
    );
    assert!(r.is_ok());
    assert_eq!(s.texte(), "Écho (faux-1, high) : bonjour moteur");
    let t = &echo.appels()[0];
    // Le modèle par défaut d'un moteur tiers : celui qu'il déclare.
    assert_eq!(t.model.as_deref(), Some("faux-1"));
    assert_eq!(t.effort.as_deref(), Some("high"));
    assert_eq!(t.profile.as_deref(), Some("Profil de test."));
    // Les consignes de l'hôte, `{{vault}}` rempli.
    assert_eq!(t.instructions, "Consignes factices du coffre Mon coffre.");
    assert_eq!(t.cwd, "C:/coffre");
    assert_eq!(t.policy.mode, PermissionMode::Full);
    // Les dossiers hors coffre : captures, pièces jointes, dossier de l'agent.
    assert_eq!(t.extra_dirs.len(), 3);
    assert!(t.extra_dirs[0].contains("screens") && t.extra_dirs[0].contains("Mon_coffre"));
    assert!(t.extra_dirs[1].contains("attachments"));
    assert!(t.extra_dirs[2].contains("skills-plugin"));
    // Le moteur est inscrit dans la session de l'historique.
    assert_eq!(meta_de(&b, "e").get("harness"), Some(&json!("echo")));
}

#[test]
fn la_suite_recoit_l_etat_persiste_le_moteur_est_fige_apres_le_1er_tour() {
    let b = banc();
    let echo = FauxHarness::new("echo");
    b.svc
        .inscrire_harness("echo", echo.clone() as Rc<dyn AgentHarness>, 0);
    let d = |p: &str| DemandeTour {
        harness: "echo".into(),
        ..demande_de("f", p)
    };
    let mut s = Sortie::default();
    assert!(tour_avec(&b, d("un"), &mut s).is_ok());
    // Le moteur est figé dès le 1er tour, et son état est dans la session.
    let m = meta_de(&b, "f");
    assert_eq!(m.get("started"), Some(&json!(true)));
    assert_eq!(
        m.get("harnessState"),
        Some(&json!({"thread": "T-f", "tours": 1}))
    );
    // Les réglages changent de moteur par défaut : la conversation garde le sien.
    assert!(b.svc.reglages().poser("harness", json!("claude")).is_ok());
    let mut s = Sortie::default();
    assert!(
        tour_avec(
            &b,
            DemandeTour {
                harness: "claude".into(),
                ..demande_de("f", "deux")
            },
            &mut s
        )
        .is_ok()
    );
    assert_eq!(echo.appels().len(), 2);
    assert!(b.claude.appels().is_empty());
    assert_eq!(
        echo.appels()[1].conversation.state,
        Some(json!({"thread": "T-f", "tours": 1}))
    );
}

#[test]
fn le_moteur_par_defaut_des_reglages_s_applique_a_une_conversation_neuve() {
    let b = banc();
    let echo = FauxHarness::new("echo");
    b.svc
        .inscrire_harness("echo", echo.clone() as Rc<dyn AgentHarness>, 0);
    assert!(b.svc.reglages().poser("harness", json!("echo")).is_ok());
    assert_eq!(b.svc.harness_par_defaut().as_deref(), Some("echo"));
    // Le fournisseur ne désigne aucun moteur : celui des réglages.
    let mut s = Sortie::default();
    let d = DemandeTour {
        harness: String::new(),
        ..demande_de("n", "salut")
    };
    assert!(tour_avec(&b, d, &mut s).is_ok());
    assert_eq!(echo.appels().len(), 1);
    // Le modèle par défaut du moteur : le réglage par moteur l'emporte.
    assert!(
        b.svc
            .reglages()
            .poser("harnessModels", json!({"echo": "echo-2"}))
            .is_ok()
    );
    let mut s = Sortie::default();
    let d = DemandeTour {
        harness: String::new(),
        ..demande_de("n2", "salut")
    };
    assert!(tour_avec(&b, d, &mut s).is_ok());
    assert_eq!(echo.appels()[1].model.as_deref(), Some("echo-2"));
}

#[test]
fn plugin_du_moteur_desactive_la_conversation_dit_que_son_moteur_manque_rien_ne_casse() {
    let b = banc();
    let echo = FauxHarness::new("echo");
    let jeton = b
        .svc
        .inscrire_harness("echo", echo as Rc<dyn AgentHarness>, 0);
    let mut s = Sortie::default();
    let d = DemandeTour {
        harness: "echo".into(),
        ..demande_de("g", "un")
    };
    assert!(tour_avec(&b, d, &mut s).is_ok());
    b.svc.retirer_harness("echo", jeton);
    let mut s = Sortie::default();
    let r = tour_avec(&b, demande_de("g", "deux"), &mut s);
    let Err(ErreurHarness::Indisponible(m)) = r else {
        panic!("attendu : moteur indisponible");
    };
    assert!(m.contains("echo") && m.contains("plugin"), "{m}");
    // Une conversation neuve sur un moteur absent n'entre pas dans l'historique.
    let r = tour_avec(
        &b,
        DemandeTour {
            harness: "absent".into(),
            ..demande_de("h", "x")
        },
        &mut s,
    );
    assert!(r.is_err());
    assert!(b.svc.sessions().get("h").is_none());
}

#[test]
fn hors_bureau_l_agent_n_a_pas_de_dossier_de_travail() {
    let b = banc_avec(|c| c.racine_coffre = None);
    let mut s = Sortie::default();
    let r = tour_avec(&b, demande_de("z", "x"), &mut s);
    assert!(matches!(r, Err(ErreurHarness::Indisponible(m)) if m.contains("bureau")));
    assert!(b.svc.sessions().liste().is_empty());
}

#[test]
fn une_branche_recoit_l_etat_du_parent() {
    let b = banc();
    tour(&b, "parent", "racine");
    let mut s = Sortie::default();
    let r = tour_avec(
        &b,
        DemandeTour {
            fork: Some(("parent".into(), "uuid-3".into())),
            ..demande_de("enfant", "branche")
        },
        &mut s,
    );
    assert!(r.is_ok());
    let f = b.claude.appels()[1].fork.clone();
    assert_eq!(f.as_ref().map(|f| f.at.as_str()), Some("uuid-3"));
    assert_eq!(
        f.and_then(|f| f.from.state),
        Some(json!({"thread": "T-parent", "tours": 1}))
    );
}

// ═══════════════════════ le dossier de l'agent ═══════════════════════

#[test]
fn le_dossier_de_l_agent_est_relu_a_chaque_tour_le_gabarit_remplace_les_consignes() {
    let b = banc();
    let propre = b.svc.dossiers().propre();
    // Rien n'est livré : le dossier de l'agent n'existe pas tant que l'utilisateur n'en a pas un.
    tour(&b, "d", "sans dossier");
    assert!(!propre.exists());
    assert_eq!(
        b.claude.appels()[0].instructions,
        "Consignes factices du coffre Mon coffre."
    );
    // L'utilisateur (ou l'agent lui-même) écrit son prompt : il est relu au tour suivant.
    assert!(fs::create_dir_all(&propre).is_ok());
    assert!(
        fs::write(
            propre.join("prompt.md"),
            "Prompt factice : coffre {{vault}}, dossier {{dir}}.\n"
        )
        .is_ok()
    );
    tour(&b, "d", "avec dossier");
    let attendu = format!(
        "Prompt factice : coffre Mon coffre, dossier {}.",
        propre.display()
    );
    assert_eq!(b.claude.appels()[1].instructions, attendu);
    // Il le perfectionne : le tour suivant le voit.
    assert!(fs::write(propre.join("prompt.md"), "Version 2.").is_ok());
    tour(&b, "d", "encore");
    assert_eq!(b.claude.appels()[2].instructions, "Version 2.");
}

#[test]
fn le_dossier_de_l_installation_de_fragment_sert_a_defaut_du_sien() {
    let install = tempfile::tempdir().expect("dossier temporaire");
    assert!(
        fs::write(
            install.path().join("prompt.md"),
            "Prompt d'installation factice."
        )
        .is_ok()
    );
    let chemin = install.path().to_path_buf();
    let b = banc_avec(move |c| c.installation_fragment = Some(chemin));
    tour(&b, "i", "x");
    assert_eq!(
        b.claude.appels()[0].instructions,
        "Prompt d'installation factice."
    );
    // Dès que l'utilisateur a le sien, c'est le sien qui compte.
    let propre = b.svc.dossiers().propre();
    assert!(fs::create_dir_all(&propre).is_ok());
    assert!(fs::write(propre.join("prompt.md"), "Le mien.").is_ok());
    tour(&b, "i", "y");
    assert_eq!(b.claude.appels()[1].instructions, "Le mien.");
}

#[test]
fn les_dossiers_de_l_agent_sont_sous_user_data_hors_du_coffre() {
    let d = Dossiers::new("C:/donnees", "Mon coffre/é", None);
    assert!(
        d.propre().ends_with("agent/skills-plugin") || d.propre().ends_with("agent\\skills-plugin")
    );
    assert!(d.ecrans().to_string_lossy().contains("Mon_coffre_"));
    assert!(d.pieces().to_string_lossy().contains("attachments"));
    assert_eq!(nom_de_dossier("Mes notes (2026) !"), "Mes_notes_2026_");
    assert_eq!(nom_de_dossier("a.b-c_d"), "a.b-c_d");
}

// ═══════════════════════ agent-permissions.spec.ts ═══════════════════════

#[test]
fn sans_reglage_coffre_ouvert_mode_full_et_un_moteur_qui_ne_demande_pas_reste_en_acces_complet() {
    let b = banc();
    tour(&b, "p", "x");
    assert_eq!(b.claude.appels()[0].policy, open_vault_policy());
    // Une politique « Assistant » : mode ask… sauf pour un moteur qui ne sait pas DEMANDER.
    politique_assistant(&b);
    tour(&b, "p", "y");
    assert_eq!(b.claude.appels()[1].policy.mode, PermissionMode::Ask);
    let mut c = b.claude.capabilities_pour_test();
    c.permission_prompts = false;
    b.claude.avec_capacites(c);
    tour(&b, "p", "z");
    assert_eq!(b.claude.appels()[2].policy.mode, PermissionMode::Full);
}

#[test]
fn le_mode_de_la_conversation_l_emporte_sur_celui_des_reglages() {
    let b = banc();
    politique_assistant(&b);
    let mut s = Sortie::default();
    let mut meta = serde_json::Map::new();
    meta.insert("profile".into(), json!({"mode": "plan"}));
    let r = tour_avec(
        &b,
        DemandeTour {
            meta: Some(meta),
            ..demande_de("m", "x")
        },
        &mut s,
    );
    assert!(r.is_ok());
    assert_eq!(b.claude.appels()[0].policy.mode, PermissionMode::Plan);
}

#[test]
fn assistant_une_demande_va_a_la_carte_et_la_reponse_revient_au_moteur() {
    let b = banc();
    politique_assistant(&b);
    b.claude.scenario(|_| {
        vec![
            Pas::Carte(requete(
                "Bash",
                Some(ToolFamily::Shell),
                json!({"command": "ls"}),
            )),
            Pas::Bilan,
        ]
    });
    let mut s = Sortie::repondant(vec![PermissionDecision::Allow { remember: None }]);
    assert!(tour_avec(&b, demande_de("a", "go"), &mut s).is_ok());
    assert_eq!(s.demandes.len(), 1);
    assert_eq!(s.texte(), "Bash: oui");
}

#[test]
fn les_interdits_de_la_politique_ne_demandent_rien() {
    let b = banc();
    politique_assistant(&b);
    // Les sous-agents sont refusés par « Assistant » : jamais de carte.
    b.claude.scenario(|_| {
        vec![
            Pas::Carte(requete("Task", Some(ToolFamily::Subagents), json!({}))),
            Pas::Bilan,
        ]
    });
    let mut s = Sortie::default();
    assert!(tour_avec(&b, demande_de("a", "go"), &mut s).is_ok());
    assert!(s.demandes.is_empty());
    assert!(s.texte().starts_with("Task: non"), "{}", s.texte());
}

fn carte_shell_git() -> Vec<Pas> {
    vec![
        Pas::Carte(requete(
            "Bash",
            Some(ToolFamily::Shell),
            json!({"command": "git log"}),
        )),
        Pas::Bilan,
    ]
}

fn toujours(portee: RememberScope) -> PermissionDecision {
    PermissionDecision::Allow {
        remember: Some(Remember {
            scope: portee,
            rule: PermanentRule {
                family: ToolFamily::Shell,
                pattern: Some("git*".into()),
            },
        }),
    }
}

#[test]
fn toujours_dans_cette_conversation_la_suite_n_est_plus_demandee_meme_famille_meme_motif() {
    let b = banc();
    politique_assistant(&b);
    b.claude.scenario(|_| carte_shell_git());
    let mut s = Sortie::repondant(vec![toujours(RememberScope::Conversation)]);
    assert!(tour_avec(&b, demande_de("c", "log ?"), &mut s).is_ok());
    assert_eq!(s.demandes.len(), 1);
    // La règle est dans la SESSION.
    let permanentes = meta_de(&b, "c").get("permanent").cloned();
    assert_eq!(
        permanentes,
        Some(json!([{"family": "shell", "pattern": "git*"}]))
    );
    // La suite de la même conversation : plus de carte.
    let mut s = Sortie::default();
    assert!(tour_avec(&b, demande_de("c", "encore"), &mut s).is_ok());
    assert!(s.demandes.is_empty());
    assert_eq!(s.texte(), "Bash: oui");
    // Retenue une seule fois : la même règle ne s'ajoute pas deux fois.
    let r = PermanentRule {
        family: ToolFamily::Shell,
        pattern: Some("git*".into()),
    };
    assert!(b.svc.retenir("c", RememberScope::Conversation, &r).is_ok());
    assert_eq!(
        meta_de(&b, "c")
            .get("permanent")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    // Une AUTRE conversation redemande.
    let mut s = Sortie::repondant(vec![PermissionDecision::Allow { remember: None }]);
    assert!(tour_avec(&b, demande_de("autre", "log ?"), &mut s).is_ok());
    assert_eq!(s.demandes.len(), 1);
}

#[test]
fn refuser_avec_un_pourquoi_l_agent_recoit_le_message() {
    let b = banc();
    politique_assistant(&b);
    b.claude.scenario(|_| carte_shell_git());
    let mut s = Sortie::repondant(vec![PermissionDecision::Deny {
        message: Some("pas dans ce dossier".into()),
    }]);
    assert!(tour_avec(&b, demande_de("r", "x"), &mut s).is_ok());
    assert_eq!(s.texte(), "Bash: non (pas dans ce dossier)");
}

#[test]
fn toujours_dans_ce_coffre_retenu_dans_les_reglages_et_valable_dans_une_autre_conversation() {
    let b = banc();
    politique_assistant(&b);
    b.claude.scenario(|_| carte_shell_git());
    let mut s = Sortie::repondant(vec![toujours(RememberScope::Vault)]);
    assert!(tour_avec(&b, demande_de("v1", "x"), &mut s).is_ok());
    assert_eq!(b.svc.politique_de_base().permanent.len(), 1);
    // Écrit dans le data.json de l'agent, et la politique complète s'y relit.
    let ecrit = b.stockage_lire();
    assert_eq!(
        ecrit["policy"]["permanent"],
        json!([{"family": "shell", "pattern": "git*"}])
    );
    let mut s = Sortie::default();
    assert!(tour_avec(&b, demande_de("v2", "y"), &mut s).is_ok());
    assert!(s.demandes.is_empty());
    assert_eq!(s.texte(), "Bash: oui");
}

#[test]
fn la_garde_dossier_masque_refuse_dossiers_de_l_agent_ouverts_chemins_de_fragment_proteges() {
    let b = banc();
    let mut p = open_vault_policy();
    p.folders.insert("Privé".into(), FolderAccess::Hidden);
    assert!(
        b.svc
            .reglages()
            .poser("policy", serde_json::to_value(&p).unwrap_or_default())
            .is_ok()
    );
    let dossier_agent = b.svc.dossiers().actif().join("skills").join("a.md");
    let appels = vec![
        utilisation("Read", json!({"file_path": "Privé/journal.md"})),
        utilisation("Grep", json!({"pattern": "x"})),
        utilisation("Read", json!({"file_path": "Cours/a.md"})),
        utilisation(
            "Write",
            json!({"file_path": "notes/../.fragment/plugins/agent/data.json"}),
        ),
        // Le dossier de l'agent, hors du coffre, est à lui : l'agent y édite ses skills.
        utilisation(
            "Write",
            json!({"file_path": dossier_agent.to_string_lossy()}),
        ),
    ];
    b.claude.scenario({
        let a = appels.clone();
        move |_| a.iter().cloned().map(Pas::Garde).collect()
    });
    tour(&b, "g", "garde");
    let g: Vec<Option<GuardDecision>> = b.claude.gardes().into_iter().map(|(_, d)| d).collect();
    let refus = |d: &Option<GuardDecision>| match d {
        Some(GuardDecision::Deny { message }) => message.clone(),
        _ => String::new(),
    };
    assert!(refus(&g[0]).contains("masqué"), "{:?}", g[0]);
    assert!(refus(&g[1]).contains("Privé"), "{:?}", g[1]);
    assert_eq!(g[2], Some(GuardDecision::Allow));
    assert!(refus(&g[3]).contains("protégé"), "{:?}", g[3]);
    assert_eq!(g[4], Some(GuardDecision::Allow), "{:?}", g[4]);
}

#[test]
fn mode_plan_la_carte_du_plan_lancer_passe_la_conversation_en_modifs_auto() {
    let b = banc();
    politique_assistant(&b);
    let mut meta = serde_json::Map::new();
    meta.insert("profile".into(), json!({"mode": "plan"}));
    b.claude.scenario(|_| {
        let mut r = requete("ExitPlanMode", None, json!({"plan": "# Plan"}));
        r.plan = Some("# Plan".into());
        vec![Pas::Carte(r), Pas::Bilan]
    });
    let mut s = Sortie::repondant(vec![PermissionDecision::Allow { remember: None }]);
    let r = tour_avec(
        &b,
        DemandeTour {
            meta: Some(meta),
            ..demande_de("pl", "prépare")
        },
        &mut s,
    );
    assert!(r.is_ok());
    // Un plan passe TOUJOURS par la carte, la politique ne le décide pas.
    assert_eq!(s.demandes.len(), 1);
    assert_eq!(b.claude.appels()[0].policy.mode, PermissionMode::Plan);
    // « Lancer » : la suite de la conversation passe en modifs auto.
    assert_eq!(
        meta_de(&b, "pl").get("profile"),
        Some(&json!({"mode": "acceptEdits"}))
    );
    b.claude.scenario(|_| vec![Pas::texte("ok")]);
    tour(&b, "pl", "vas-y");
    assert_eq!(
        b.claude.appels()[1].policy.mode,
        PermissionMode::AcceptEdits
    );
}

#[test]
fn arreter_un_tour_qui_attend_une_reponse_sans_personne_la_carte_est_refusee() {
    let b = banc();
    politique_assistant(&b);
    b.claude.scenario(|_| carte_shell_git());
    // Un tour SANS le chat (`run`) : personne ne répond aux cartes, elles sont refusées.
    let evts: Rc<RefCell<Vec<AgentEvent>>> = Rc::default();
    let e2 = Rc::clone(&evts);
    let run = b.svc.run(AgentQuery {
        prompt: "go".into(),
        session_id: Some("q".into()),
        harness: None,
        model: None,
        system: None,
        effort: None,
        tools: None,
        title: Some("Sans chat".into()),
        meta: None,
        fork: None,
        content: vec![],
        mode: None,
        on_event: Box::new(move |e| e2.borrow_mut().push(e)),
    });
    let Ok(run) = run else {
        panic!("le tour doit partir")
    };
    assert_eq!(run.session_id(), "q");
    assert!(run.est_termine());
    let texte: String = evts
        .borrow()
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextDelta { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        texte.starts_with("Bash: non (Personne pour répondre"),
        "{texte}"
    );
    // La session entre dans l'historique, rouvrable dans le chat.
    let info = b.svc.get_session("q");
    assert_eq!(info.map(|i| i.title), Some("Sans chat".to_string()));
}

#[test]
fn interrompre_arrive_au_moteur_par_le_crochet_du_montage() {
    let b = banc();
    let faux = Rc::clone(&b.claude);
    b.svc
        .brancher_interruption("claude", move |c| faux.interrompre(c));
    b.claude
        .scenario(|_| vec![Pas::texte("avant"), Pas::Pause, Pas::texte("après")]);
    let svc = Rc::clone(&b.svc);
    b.claude.au_pas(move |_| {
        svc.interrompre("stop");
        true
    });
    let s = tour(&b, "stop", "long");
    assert!(s.texte().contains("avant"));
    assert!(!s.texte().contains("après"));
}

// ═══════════════════════ réglages (data.json de l'agent) ═══════════════════════

#[test]
fn les_reglages_gardent_les_cles_des_autres_et_un_null_retire_la_cle() {
    let b = banc();
    let r = b.svc.reglages();
    assert_eq!(r.modele(), "claude-opus-5-5");
    assert!(r.poser("quick", json!({"count": 3})).is_ok());
    assert!(r.poser("orb", json!("pulse")).is_ok());
    assert!(r.poser("model", json!("claude-sonnet-5-5")).is_ok());
    assert!(r.poser("harness", json!("echo")).is_ok());
    assert_eq!(r.orbe().as_deref(), Some("pulse"));
    assert!(r.poser("harness", Value::Null).is_ok());
    assert_eq!(r.harness(), None);
    let ecrit = b.stockage_lire();
    assert_eq!(ecrit["quick"], json!({"count": 3}));
    assert_eq!(ecrit["model"], json!("claude-sonnet-5-5"));
    assert!(ecrit.get("harness").is_none());
}

#[test]
fn la_fenetre_de_reglages_prevenue_relit_le_data_json_et_la_sauvegarde_diffuse() {
    let stockage = Rc::new(StockageMemoire::default());
    let a = Reglages::new(stockage.clone());
    let b = Reglages::new(stockage);
    let diffusions = Rc::new(Cell::new(0));
    let d = Rc::clone(&diffusions);
    a.sur_diffusion(move || d.set(d.get() + 1));
    assert!(a.poser("userName", json!("  Camille ")).is_ok());
    assert_eq!(diffusions.get(), 1);
    // L'autre instance ne voit rien tant qu'on ne la prévient pas…
    assert_eq!(b.nom_saisi(), None);
    // …puis elle relit (`onExternalSettingsChange`).
    b.au_changement_externe();
    assert_eq!(b.nom_saisi().as_deref(), Some("Camille"));
}

#[test]
fn le_nom_de_l_utilisateur_est_le_reglage_sinon_le_compte_capitalise_sinon_moi() {
    assert_eq!(nom_utilisateur(Some(" Léa "), Some("julie")), "Léa");
    assert_eq!(nom_utilisateur(Some("  "), Some("julie")), "Julie");
    assert_eq!(nom_utilisateur(None, None), "Moi");
    let b = banc();
    assert_eq!(b.svc.nom_utilisateur(), "Julie");
    assert!(b.svc.reglages().poser("userName", json!("Camille")).is_ok());
    assert_eq!(b.svc.nom_utilisateur(), "Camille");
}

#[test]
fn une_politique_ancienne_incomplete_est_completee_par_coffre_ouvert() {
    let b = banc();
    assert!(
        b.svc
            .reglages()
            .poser(
                "policy",
                json!({"mode": "ask", "shellRules": ["git status*"]})
            )
            .is_ok()
    );
    let p = b.svc.politique_de_base();
    assert_eq!(p.mode, PermissionMode::Ask);
    assert_eq!(p.shell_rules, ["git status*"]);
    assert_eq!(p.tools.len(), open_vault_policy().tools.len());
}

#[test]
fn les_reglages_sont_relus_a_chaque_tour() {
    let b = banc();
    tour(&b, "r", "un");
    // La fenêtre de réglages écrit un autre moteur d'effort par défaut… ici, une autre politique,
    // directement dans le stockage (l'autre fenêtre n'a pas prévenu : on relit au tour).
    let nouvelle = json!({"policy": serde_json::to_value(assistant_policy()).unwrap_or_default()});
    b.stockage_ecrire(&nouvelle);
    tour(&b, "r", "deux");
    assert_eq!(b.claude.appels()[1].policy.mode, PermissionMode::Ask);
}

// ═══════════════════════ openChat / openAnchored (côté données) ═══════════════════════

#[derive(Default)]
struct VuesFausses {
    chats: RefCell<Vec<ChatOuvert>>,
    montres: RefCell<Vec<OuvertureChat>>,
    ancres: RefCell<Vec<(OuvertureAncree, String, Option<EtatOuverture>)>>,
    refuse_ancre: Cell<bool>,
}

impl HoteVues for VuesFausses {
    fn chats_dock(&self) -> Vec<ChatOuvert> {
        self.chats.borrow().clone()
    }

    fn montrer_chat(&self, o: OuvertureChat) {
        self.montres.borrow_mut().push(o);
    }

    fn ouvrir_ancre(
        &self,
        o: &OuvertureAncree,
        session: &str,
        etat: Option<EtatOuverture>,
    ) -> bool {
        if self.refuse_ancre.get() {
            return false;
        }
        self.ancres
            .borrow_mut()
            .push((o.clone(), session.to_string(), etat));
        true
    }
}

fn avec_vues(b: &Banc) -> Rc<VuesFausses> {
    let v = Rc::new(VuesFausses::default());
    b.svc.brancher_vues(v.clone());
    v
}

fn chat(conv: &str, occupe: bool) -> ChatOuvert {
    ChatOuvert {
        conversation: conv.into(),
        occupe,
    }
}

#[test]
fn open_chat_montre_la_conversation_demandee_sinon_celle_du_dock_sinon_la_plus_recente() {
    let b = banc();
    let v = avec_vues(&b);
    tour(&b, "vieille", "a");
    tour(&b, "recente", "b");
    // Aucun chat dans le dock : la plus récente.
    b.svc.ouvrir_chat(None, &AgentChatOptions::default());
    assert_eq!(v.montres.borrow()[0].conversation, "recente");
    assert_eq!(v.montres.borrow()[0].reutiliser, None);
    // Un chat affiche déjà « vieille » : on la garde.
    *v.chats.borrow_mut() = vec![chat("vieille", false)];
    b.svc
        .ouvrir_chat(Some("salut"), &AgentChatOptions::default());
    let m = v.montres.borrow()[1].clone();
    assert_eq!(
        (m.conversation.as_str(), m.reutiliser),
        ("vieille", Some(0))
    );
    assert_eq!(m.prompt.as_deref(), Some("salut"));
    // Une conversation précise l'emporte.
    b.svc.ouvrir_chat(
        None,
        &AgentChatOptions {
            session_id: Some("recente".into()),
            ..Default::default()
        },
    );
    assert_eq!(v.montres.borrow()[2].conversation, "recente");
}

#[test]
fn open_chat_new_session_prend_un_chat_libre_jamais_celui_dont_un_tour_court() {
    let b = banc();
    let v = avec_vues(&b);
    *v.chats.borrow_mut() = vec![chat("occupee", true), chat("libre", false)];
    b.svc.ask(
        "neuve",
        AgentChatOptions {
            new_session: true,
            effort: Some("high".into()),
            ..Default::default()
        },
    );
    let m = v.montres.borrow()[0].clone();
    assert_eq!(m.reutiliser, Some(1));
    assert_ne!(m.conversation, "libre");
    assert!(!m.conversation.is_empty());
    assert_eq!(
        (m.prompt.as_deref(), m.effort.as_deref()),
        (Some("neuve"), Some("high"))
    );
    // Tous occupés : une feuille neuve.
    *v.chats.borrow_mut() = vec![chat("occupee", true)];
    b.svc.ouvrir_chat(
        None,
        &AgentChatOptions {
            new_session: true,
            ..Default::default()
        },
    );
    assert_eq!(v.montres.borrow()[1].reutiliser, None);
}

fn ouverture(file: &str) -> OuvertureAncree {
    OuvertureAncree {
        file: file.into(),
        from: 10,
        to: 20,
        title: "Question".into(),
        ..Default::default()
    }
}

#[test]
fn open_anchored_la_conversation_ancree_au_passage_on_montre_la_question_courte_on_envoie_le_long_prompt_la_suite_garde_le_profil()
 {
    let b = banc();
    let v = avec_vues(&b);
    let id = b.svc.ouvrir_ancre(&OuvertureAncree {
        quote: Some("le passage".into()),
        prompt: Some("Un très long prompt avec tout le document.".into()),
        display: Some("Pourquoi ?".into()),
        system: Some("Réponds en une phrase.".into()),
        tools: Some(vec![]),
        effort: Some("low".into()),
        meta: Some(
            json!({"from": "test"})
                .as_object()
                .cloned()
                .unwrap_or_default(),
        ),
        ..ouverture("notes/a.md")
    });
    let Some(id) = id else {
        panic!("la session doit exister")
    };
    // La vue s'ouvre avec le premier tour : on MONTRE la question courte, on ENVOIE le long prompt.
    let (o, sid, etat) = v.ancres.borrow()[0].clone();
    assert_eq!(sid, id);
    assert_eq!(o.file, "notes/a.md");
    let envoi = etat.and_then(|e| e.envoi);
    assert_eq!(envoi.as_ref().map(|e| e.texte.as_str()), Some("Pourquoi ?"));
    assert_eq!(
        envoi.as_ref().map(|e| e.prompt.as_str()),
        Some("Un très long prompt avec tout le document.")
    );
    // La session : titre, profil, origine, ce qu'on affiche.
    let s = b.svc.sessions().get(&id);
    assert_eq!(s.as_ref().map(|s| s.title.as_str()), Some("Question"));
    let m = meta_de(&b, &id);
    assert_eq!(m.get("from"), Some(&json!("test")));
    assert_eq!(m.get("file"), Some(&json!("notes/a.md")));
    assert_eq!(
        m.get("profile"),
        Some(&json!({"system": "Réponds en une phrase.", "tools": [], "effort": "low"}))
    );
    assert_eq!(
        m.get("origin"),
        Some(&json!({"doc": "notes/a.md", "from": 10, "to": 20, "quote": "le passage"}))
    );
    assert_eq!(m.get("display"), Some(&json!("Pourquoi ?")));
    assert!(m.get("harness").is_none() && m.get("fork").is_none());
}

#[test]
fn open_anchored_sans_hote_de_widgets_n_ecrit_aucune_session() {
    let b = banc();
    // Pas de vues branchées : rien.
    assert!(b.svc.ouvrir_ancre(&ouverture("notes/a.md")).is_none());
    let v = avec_vues(&b);
    v.refuse_ancre.set(true);
    assert!(b.svc.ouvrir_ancre(&ouverture("notes/a.md")).is_none());
    assert!(b.svc.sessions().liste().is_empty());
}

#[test]
fn open_anchored_sans_prompt_le_champ_prend_le_focus_avec_le_brouillon_et_les_pieces() {
    let b = banc();
    let v = avec_vues(&b);
    let pj = ChatAttachment {
        name: "zone.png".into(),
        mime: "image/png".into(),
        data: vec![1, 2, 3],
    };
    let id = b.svc.ouvrir_ancre(&OuvertureAncree {
        preamble: Some("Le document :".into()),
        draft: Some("Que penser de".into()),
        attachments: vec![pj.clone()],
        ..ouverture("notes/a.md")
    });
    let Some(id) = id else {
        panic!("la session doit exister")
    };
    let etat = v.ancres.borrow()[0].2.clone();
    assert_eq!(
        etat,
        Some(EtatOuverture {
            envoi: None,
            focus: true,
            brouillon: Some("Que penser de".into()),
            pieces_du_champ: vec![pj],
        })
    );
    assert_eq!(
        meta_de(&b, &id).get("preamble"),
        Some(&json!("Le document :"))
    );
    // Rien à poser dans le champ : la vue s'ouvre sans état.
    let id2 = b.svc.ouvrir_ancre(&ouverture("notes/b.md"));
    assert!(id2.is_some());
    assert_eq!(v.ancres.borrow()[1].2, None);
}

#[test]
fn open_anchored_un_passage_d_une_de_nos_conversations_est_une_branche_du_meme_moteur() {
    let b = banc();
    let v = avec_vues(&b);
    tour(&b, "parent", "racine");
    let cle = chat_key("agent", "parent");
    let id = b.svc.ouvrir_ancre(&OuvertureAncree {
        from: 3 * MESSAGE_STRIDE + 5,
        quote: Some("un   passage\n cité".into()),
        preamble: Some("document".into()),
        ..ouverture(&cle)
    });
    let Some(id) = id else {
        panic!("la session doit exister")
    };
    let m = meta_de(&b, &id);
    // Le repère est le RANG du message, la branche naîtra du parent au premier envoi.
    assert_eq!(
        m.get("fork"),
        Some(&json!({"from": "parent", "message": 3}))
    );
    // Le préambule ne porte plus le document : le passage visé.
    assert_eq!(
        m.get("preamble"),
        Some(&json!(
            "À propos de « un passage cité » (un passage de la conversation ci-dessus) :"
        ))
    );
    assert!(v.ancres.borrow()[0].2.is_some());
}

#[test]
fn open_anchored_un_moteur_qui_ne_sait_pas_brancher_donne_une_conversation_racine() {
    let b = banc();
    let _v = avec_vues(&b);
    let echo = FauxHarness::new("echo");
    let mut c = echo.capabilities_pour_test();
    c.fork = false;
    echo.avec_capacites(c);
    b.svc
        .inscrire_harness("echo", echo as Rc<dyn AgentHarness>, 0);
    let mut s = Sortie::default();
    let r = tour_avec(
        &b,
        DemandeTour {
            harness: "echo".into(),
            ..demande_de("parent", "x")
        },
        &mut s,
    );
    assert!(r.is_ok());
    let id = b
        .svc
        .ouvrir_ancre(&ouverture(&chat_key("agent", "parent")))
        .unwrap_or_default();
    let m = meta_de(&b, &id);
    assert!(m.get("fork").is_none());
    // Sans branche, le moteur est celui des réglages (ici Claude : absent de la meta).
    assert!(m.get("harness").is_none());
}

#[test]
fn open_anchored_reprendre_une_session_garde_son_moteur() {
    let b = banc();
    let _v = avec_vues(&b);
    let echo = FauxHarness::new("echo");
    b.svc
        .inscrire_harness("echo", echo as Rc<dyn AgentHarness>, 0);
    let mut s = Sortie::default();
    let r = tour_avec(
        &b,
        DemandeTour {
            harness: "echo".into(),
            ..demande_de("ancienne", "x")
        },
        &mut s,
    );
    assert!(r.is_ok());
    let id = b.svc.ouvrir_ancre(&OuvertureAncree {
        session_id: Some("ancienne".into()),
        ..ouverture("notes/a.md")
    });
    assert_eq!(id.as_deref(), Some("ancienne"));
    // La session existante garde son moteur : aucune méta n'est réécrite à sa place.
    assert_eq!(meta_de(&b, "ancienne").get("harness"), Some(&json!("echo")));
}

#[test]
fn le_fournisseur_du_chat_tourne_sur_le_vrai_service_envoi_historique_et_suppression() {
    let b = banc();
    let p = AgentChatProvider::new(Rc::clone(&b.svc) as Rc<dyn AgentHote>);
    let conv = p.create().unwrap_or_default();
    let evts: Rc<RefCell<Vec<ChatEvent>>> = Rc::default();
    let e2 = Rc::clone(&evts);
    p.envoyer(&conv, "bonjour", EnvoiAgent::default(), &mut move |e| {
        e2.borrow_mut().push(e)
    });
    assert!(matches!(
        evts.borrow().last(),
        Some(ChatEvent::Done { error: None })
    ));
    // Le tour a créé la session au journal, moteur figé.
    assert_eq!(
        b.svc.sessions().get(&conv).map(|s| s.title),
        Some("bonjour".to_string())
    );
    assert_eq!(meta_de(&b, &conv).get("started"), Some(&json!(true)));
    assert!(p.delete(&conv).is_ok());
    assert!(b.svc.sessions().get(&conv).is_none());
}

// ── Petites aides propres aux tests ──────────────────────────────────────────

trait AidesBanc {
    fn stockage_lire(&self) -> Value;
    fn stockage_ecrire(&self, v: &Value);
}

impl AidesBanc for Banc {
    fn stockage_lire(&self) -> Value {
        use fragment_agent::service::reglages::StockageReglages;
        self.stockage.lire().unwrap_or(Value::Null)
    }

    fn stockage_ecrire(&self, v: &Value) {
        use fragment_agent::service::reglages::StockageReglages;
        assert!(self.stockage.ecrire(v).is_ok());
        self.svc.reglages().au_changement_externe();
    }
}

trait AidesFaux {
    fn capabilities_pour_test(&self) -> fragment_agent::contrat::HarnessCapabilities;
}

impl AidesFaux for Rc<FauxHarness> {
    fn capabilities_pour_test(&self) -> fragment_agent::contrat::HarnessCapabilities {
        AgentHarness::capabilities(self.as_ref())
    }
}
