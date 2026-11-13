// Port de app/e2e/agent-chat.spec.ts, agent-harness.spec.ts, agent-permissions.spec.ts (la part
// fournisseur), agent-trace.spec.ts (direct, relue, tour simple) et chat-attachments.spec.ts
// (envoi, relecture). Le moteur est le FauxHarness : aucun appel à `claude`.
//
// Un `#[test]` par cas TS, même nom en snake_case ; les cas qui sont de la VUE (puces du champ,
// vue rouverte, réglages déclaratifs, geste flou, orbe) sont notés dans `parite.d`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fragment_agent::attachment_blocks::base64;
use fragment_agent::contrat::{
    AgentEvent, FolderAccess, GuardDecision, PermanentRule, PermissionDecision, PermissionMode,
    Remember, RememberScope, ToolFamily, ToolUse,
};
use fragment_agent::fournisseur::parts::trace_depuis_json;
use fragment_agent::fournisseur::pieces::FormatImage;
use fragment_agent::fournisseur::{
    AgentChatProvider, AgentHote, CodecImageFaux, EnvoiAgent, FauxHarness, FauxHote, Pas,
    titre_depuis,
};
use fragment_agent::permission_model::{EtatCarte, PermissionCardData, describe_permission};
use fragment_agent::policy_model::{DecideContext, Decision, decide, open_vault_policy};
use fragment_agent::trace_model::{StepKind, TraceStatus, describe_step, summarize};
use fragment_chat::api::{ChatAttachment, ChatEvent, ChatPart, ChatProvider, ChatQuote};
use serde_json::{Value, json};

static N: AtomicUsize = AtomicUsize::new(0);

/// Le banc : un hôte, un moteur « claude » inscrit, le fournisseur.
struct Banc {
    hote: Rc<FauxHote>,
    /// Le moteur « claude » (celui des conversations neuves).
    claude: Rc<FauxHarness>,
    /// Le moteur dont on lit les appels (claude, sauf `banc_echo`).
    faux: Rc<FauxHarness>,
    p: Rc<AgentChatProvider>,
    dossier: std::path::PathBuf,
}

impl Drop for Banc {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dossier);
    }
}

fn banc() -> Banc {
    let dossier = std::env::temp_dir().join(format!(
        "fragment-agent-fournisseur-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let hote = FauxHote::new(dossier.join("attachments"));
    let faux = FauxHarness::new("claude");
    hote.enregistrer(&faux);
    let p = Rc::new(AgentChatProvider::new(Rc::clone(&hote) as Rc<dyn AgentHote>));
    Banc {
        hote,
        claude: Rc::clone(&faux),
        faux,
        p,
        dossier,
    }
}

type Journal = Rc<RefCell<Vec<ChatEvent>>>;
/// Ce qu'on voit à chaque pas libéré : (rang, trace, réponse).
type Vus = Vec<(
    usize,
    Option<fragment_agent::trace_model::Trace>,
    Option<String>,
)>;

/// Un envoi dont les événements vont dans un journal partagé (lisible depuis les crochets).
fn envoyer_dans(b: &Banc, conv: &str, text: &str, envoi: EnvoiAgent, journal: &Journal) {
    let j = Rc::clone(journal);
    b.p.envoyer(conv, text, envoi, &mut move |e| j.borrow_mut().push(e));
}

fn envoyer(b: &Banc, conv: &str, text: &str) -> Vec<ChatEvent> {
    envoyer_avec(b, conv, text, EnvoiAgent::default())
}

fn envoyer_avec(b: &Banc, conv: &str, text: &str, envoi: EnvoiAgent) -> Vec<ChatEvent> {
    let j: Journal = Rc::default();
    envoyer_dans(b, conv, text, envoi, &j);
    j.borrow().clone()
}

/// Les parts du message après ces événements (un part ré-émis avec le même id REMPLACE le
/// précédent ; `Remove` le retire) — ce que montre la vue.
fn parts_finaux(evts: &[ChatEvent]) -> Vec<ChatPart> {
    let mut v: Vec<ChatPart> = Vec::new();
    for e in evts {
        match e {
            ChatEvent::Part(p) => match p
                .id
                .as_ref()
                .and_then(|id| v.iter().position(|x| x.id.as_ref() == Some(id)))
            {
                Some(i) => v[i] = p.clone(),
                None => v.push(p.clone()),
            },
            ChatEvent::Remove(id) => v.retain(|x| x.id.as_ref() != Some(id)),
            _ => {}
        }
    }
    v
}

fn texte_de(parts: &[ChatPart], id: &str) -> Option<String> {
    parts
        .iter()
        .find(|p| p.id.as_deref() == Some(id))
        .and_then(|p| p.text.clone())
}

fn erreur_finale(evts: &[ChatEvent]) -> Option<Option<String>> {
    match evts.last() {
        Some(ChatEvent::Done { error }) => Some(error.clone()),
        _ => None,
    }
}

fn trace_de(parts: &[ChatPart]) -> Option<fragment_agent::trace_model::Trace> {
    parts
        .iter()
        .find(|p| p.kind == "agent-trace")
        .and_then(|p| p.data.as_ref())
        .and_then(trace_depuis_json)
}

fn outil_tour(id: &str, commande: &str) -> Vec<Pas> {
    vec![
        Pas::outil(id, "Bash", "fragment files", json!({ "command": commande })),
        Pas::resultat(id, "article.md"),
    ]
}

// ═══════════════════════ agent-chat.spec.ts ═══════════════════════

#[test]
fn ribbon_vue_chat_fournisseur_agent_un_tour_avec_appel_d_outil_la_trace_puis_le_texte() {
    let b = banc();
    b.faux.scenario(|t| {
        let mut v = Vec::new();
        if t.prompt.contains("fichiers") {
            v.extend(outil_tour("t1", "fragment agent:files vault=coffre"));
        }
        v.push(Pas::texte("Voici la réponse."));
        v
    });
    let conv = b.p.create().unwrap_or_default();
    let evts = envoyer(&b, &conv, "liste mes fichiers");

    // La trace part TOUT DE SUITE, vide : l'orbe au lieu des « ••• ».
    let ChatEvent::Part(premier) = &evts[0] else {
        panic!("le premier événement doit être la trace")
    };
    let t0 = trace_depuis_json(premier.data.as_ref().unwrap_or(&Value::Null))
        .unwrap_or_else(|| panic!("trace"));
    assert_eq!(
        (premier.kind.as_str(), t0.steps.len(), t0.status),
        ("agent-trace", 0, TraceStatus::Running)
    );

    // L'appel d'outil est une étape de la TRACE (repliée sur son résumé à la fin).
    let parts = parts_finaux(&evts);
    let t = trace_de(&parts).unwrap_or_else(|| panic!("trace finale"));
    assert_eq!(t.status, TraceStatus::Done);
    assert_eq!(summarize(&t), "1 commande");
    assert_eq!(t.steps.len(), 1);
    assert_eq!(t.steps[0].kind, StepKind::Tool);
    assert!(t.steps[0].result.is_some() && t.steps[0].ended_at.is_some());
    assert_eq!(
        describe_step(&t.steps[0], false).verb,
        "A listé les fichiers"
    );
    assert_eq!(
        texte_de(&parts, "answer").as_deref(),
        Some("Voici la réponse.")
    );
    assert_eq!(erreur_finale(&evts), Some(None));

    // L'avatar de l'assistant, le titre, la requête envoyée au moteur.
    assert_eq!(b.p.participants(&conv)[1].initials.as_deref(), Some("C"));
    assert_eq!(b.p.title(&conv), "liste mes fichiers");
    let appels = b.faux.appels();
    assert_eq!(appels[0].prompt, "liste mes fichiers");
    assert_eq!(appels[0].effort, None);
}

#[test]
fn effort_contribution_au_champ_liste_du_modele_approfondi_effort_high_garde_dans_la_session() {
    let b = banc();
    let conv = b.p.create().unwrap_or_default();
    envoyer(&b, &conv, "liste mes fichiers");
    b.p.set_effort(&conv, Some("high"));
    envoyer(&b, &conv, "et ensuite ?");
    let appels = b.faux.appels();
    assert_eq!(appels[1].effort.as_deref(), Some("high"));
    assert_eq!(appels[1].conversation.id, appels[0].conversation.id); // même session : la suite
    let meta = b.hote.session(&conv).map(|s| s.meta).unwrap_or_default();
    assert_eq!(meta["profile"]["effort"], "high");
}

#[test]
fn historique_une_nouvelle_conversation_puis_rouvrir_la_premiere_la_relit_du_transcript() {
    let b = banc();
    b.faux.scenario(|_| vec![Pas::texte("Voici la réponse.")]);
    let premiere = b.p.create().unwrap_or_default();
    envoyer(&b, &premiere, "liste mes fichiers");
    let nouvelle = b.p.create().unwrap_or_default();
    assert!(b.p.load(&nouvelle).unwrap_or_default().is_empty());
    let liste = b.p.list();
    assert_eq!(liste.len(), 1);
    assert_eq!(
        (liste[0].id.as_str(), liste[0].title.as_str()),
        (premiere.as_str(), "liste mes fichiers")
    );
    let m = b.p.load(&premiere).unwrap_or_default();
    assert_eq!(m.len(), 2);
    assert_eq!(
        (m[0].author.as_str(), m[0].parts[0].text.as_deref()),
        ("user", Some("liste mes fichiers"))
    );
    assert_eq!(m[1].author, "claude");
    assert_eq!(m[1].parts[0].text.as_deref(), Some("Voici la réponse."));
}

#[test]
fn open_anchored_la_conversation_ancree_au_passage_on_montre_la_question_courte_on_envoie_le_long_prompt_la_suite_garde_le_profil()
 {
    let b = banc();
    b.faux.scenario(|_| vec![Pas::texte("Voici la réponse.")]);
    // La session telle que l'ouvre `openAnchored` (fr-t7-service) : son profil et sa question courte.
    b.hote.poser_session(
        "ancree",
        "Explain « NatSec »",
        json!({
            "profile": { "system": "SYS-LECTEUR", "tools": [], "effort": "low" },
            "display": "Explain « NatSec »",
            "origin": { "doc": "article.md", "from": 10, "to": 16, "quote": "NatSec" },
        }),
    );
    let long = "<document>…tout le texte…</document>\nExplain \"NatSec\"";
    envoyer_avec(
        &b,
        "ancree",
        "Explain « NatSec »",
        EnvoiAgent {
            prompt: Some(long.into()),
            ..EnvoiAgent::default()
        },
    );
    let mut req = b.faux.appels().pop().unwrap_or_else(|| panic!("appel"));
    assert_eq!(req.profile.as_deref(), Some("SYS-LECTEUR"));
    assert_eq!(req.effort.as_deref(), Some("low"));
    assert!(req.prompt.contains("tout le texte"));
    // La suite : le profil est gardé, le prompt = la question seule.
    envoyer(&b, "ancree", "Et Palantir ?");
    req = b.faux.appels().pop().unwrap_or_else(|| panic!("appel"));
    assert_eq!(
        (req.profile.as_deref(), req.prompt.as_str()),
        (Some("SYS-LECTEUR"), "Et Palantir ?")
    );
    // Rechargée depuis le transcript, la première question reste la COURTE.
    let m = b.p.load("ancree").unwrap_or_default();
    assert_eq!(m[0].parts[0].text.as_deref(), Some("Explain « NatSec »"));
    assert_eq!(m.len(), 4);
    // L'origine posée à l'ouverture.
    let o = b.p.origin("ancree").unwrap_or_else(|| panic!("origine"));
    assert_eq!(
        (o.doc.as_str(), o.from, o.to, o.quote.as_deref()),
        ("article.md", Some(10), Some(16), Some("NatSec"))
    );
}

// ═══════════════════════ agent-harness.spec.ts ═══════════════════════

#[test]
fn le_moteur_tiers_est_inscrit_a_cote_de_claude_natif_priorite_100() {
    let b = banc();
    b.hote.enregistrer(&FauxHarness::new("echo"));
    assert_eq!(b.hote.harness_ids(), ["claude", "echo"]);
}

/// Un banc avec un moteur « echo » (modèles echo-1, echo-2) comme dans la fixture du TS.
fn banc_echo() -> Banc {
    let mut b = banc();
    let echo = FauxHarness::new("echo");
    echo.scenario(|t| {
        vec![
            Pas::pensee_debut(),
            Pas::pensee("réflexion"),
            Pas::texte(&format!(
                "Écho ({}, {}) : {}",
                t.model.as_deref().unwrap_or("echo-1"),
                t.effort.as_deref().unwrap_or("sans effort"),
                t.prompt
            )),
            Pas::usage(fragment_agent::contrat::TurnUsage {
                context_tokens: Some(1234),
                context_window: Some(10000),
                cost_usd: Some(0.001),
                ..Default::default()
            }),
        ]
    });
    b.hote.enregistrer(&echo);
    b.faux = echo;
    b
}

#[test]
fn un_tour_confie_au_moteur_modele_effort_consignes_politique_flux_affiche_pensee_outil_texte() {
    let b = banc_echo();
    let conv = b.p.create().unwrap_or_default();
    assert!(b.p.set_harness(&conv, "echo"));
    b.p.set_model(&conv, Some("echo-2"));
    b.p.set_effort(&conv, Some("high"));
    let evts = envoyer(&b, &conv, "bonjour moteur");
    let parts = parts_finaux(&evts);
    assert_eq!(
        texte_de(&parts, "answer").as_deref(),
        Some("Écho (echo-2, high) : bonjour moteur")
    );
    // La TRACE du tour : la pensée du moteur, comme celle de Claude.
    let t = trace_de(&parts).unwrap_or_else(|| panic!("trace"));
    assert_eq!(
        (t.status, t.steps[0].kind),
        (TraceStatus::Done, StepKind::Thinking)
    );

    let tour = b.faux.appels().remove(0);
    assert_eq!(tour.conversation.id, conv);
    assert_eq!(
        (
            tour.prompt.as_str(),
            tour.model.as_deref(),
            tour.effort.as_deref()
        ),
        ("bonjour moteur", Some("echo-2"), Some("high"))
    );
    assert_eq!(tour.conversation.state, None);
    // Les consignes (ici le prompt FACTICE de l'hôte) et la politique « Coffre ouvert ».
    assert!(!tour.instructions.is_empty());
    assert_eq!(tour.policy.mode, PermissionMode::Full);
    assert_eq!(tour.cwd, "C:/coffre");

    // La session : le moteur, son état, le profil choisi, l'usage du tour.
    let meta = b.hote.session(&conv).map(|s| s.meta).unwrap_or_default();
    assert_eq!(meta["harness"], "echo");
    assert_eq!(
        meta["harnessState"],
        json!({ "thread": format!("T-{conv}"), "tours": 1 })
    );
    assert_eq!(
        meta["profile"],
        json!({ "effort": "high", "model": "echo-2" })
    );
    assert_eq!(meta["started"], true);
    assert_eq!(
        meta["usage"],
        json!({ "contextTokens": 1234, "contextWindow": 10000, "costUsd": 0.001 })
    );
    // Le harness Claude n'a rien reçu.
    assert!(b.claude.appels().is_empty());
}

#[test]
fn la_suite_recoit_l_etat_persiste_le_moteur_est_fige_apres_le_1er_tour() {
    let b = banc_echo();
    let conv = b.p.create().unwrap_or_default();
    b.p.set_harness(&conv, "echo");
    b.p.set_model(&conv, Some("echo-2"));
    b.p.set_effort(&conv, Some("high"));
    envoyer(&b, &conv, "bonjour moteur");
    let evts = envoyer(&b, &conv, "encore");
    assert_eq!(
        texte_de(&parts_finaux(&evts), "answer").as_deref(),
        Some("Écho (echo-2, high) : encore")
    );
    let tours = b.faux.appels();
    assert_eq!(tours.len(), 2);
    assert_eq!(
        tours[1].conversation.state,
        Some(json!({ "thread": format!("T-{conv}"), "tours": 1 }))
    );
    assert!(!b.p.set_harness(&conv, "claude"));
    assert_eq!(
        b.hote.session(&conv).map(|s| s.meta["harness"].clone()),
        Some(json!("echo"))
    );
}

#[test]
fn apres_un_rechargement_l_historique_est_relu_chez_le_moteur() {
    let b = banc_echo();
    let conv = b.p.create().unwrap_or_default();
    b.p.set_harness(&conv, "echo");
    b.p.set_model(&conv, Some("echo-2"));
    b.p.set_effort(&conv, Some("high"));
    envoyer(&b, &conv, "bonjour moteur");
    envoyer(&b, &conv, "encore");
    // Un fournisseur NEUF sur le même hôte : tout vient de la session et du moteur.
    let neuf = AgentChatProvider::new(Rc::clone(&b.hote) as Rc<dyn AgentHote>);
    let m = neuf.load(&conv).unwrap_or_default();
    let textes = |auteur: &str| -> Vec<String> {
        m.iter()
            .filter(|x| x.author == auteur)
            .map(|x| {
                x.parts
                    .iter()
                    .filter(|p| p.kind == "text")
                    .filter_map(|p| p.text.clone())
                    .collect::<String>()
            })
            .collect()
    };
    assert_eq!(textes("user"), ["bonjour moteur", "encore"]);
    assert_eq!(
        textes("claude"),
        [
            "Écho (echo-2, high) : bonjour moteur",
            "Écho (echo-2, high) : encore"
        ]
    );
}

#[test]
fn le_moteur_par_defaut_des_reglages_s_applique_a_une_conversation_neuve() {
    let b = banc_echo();
    b.hote.set_harness_par_defaut(Some("echo"));
    let neuve = b.p.create().unwrap_or_default();
    let evts = envoyer(&b, &neuve, "par défaut");
    assert_eq!(
        texte_de(&parts_finaux(&evts), "answer").as_deref(),
        Some("Écho (echo-1, sans effort) : par défaut")
    );
    assert_eq!(
        b.hote.session(&neuve).map(|s| s.meta["harness"].clone()),
        Some(json!("echo"))
    );
}

#[test]
fn plugin_du_moteur_desactive_la_conversation_dit_que_son_moteur_manque_rien_ne_casse() {
    let b = banc_echo();
    let conv = b.p.create().unwrap_or_default();
    b.p.set_harness(&conv, "echo");
    envoyer(&b, &conv, "bonjour");
    b.hote.desactiver("echo");
    let m = b.p.load(&conv).unwrap_or_default();
    assert!(
        m[0].parts[0]
            .text
            .as_deref()
            .unwrap_or("")
            .contains("Le moteur « echo » n’est pas disponible")
    );
    // Envoyer quand même : une erreur dans le tour, pas un plantage ; plus d'orbe pour rien.
    let evts = envoyer(&b, &conv, "toujours là ?");
    let erreur = erreur_finale(&evts).flatten().unwrap_or_default();
    assert!(erreur.contains("n’est pas disponible"), "{erreur}");
    assert!(evts.contains(&ChatEvent::Remove("trace".into())));
    assert!(parts_finaux(&evts).is_empty());
}

// ═══════════════════════ agent-permissions.spec.ts ═══════════════════════

/// Fait répondre les cartes, l'une après l'autre, dès que le tour attend (l'utilisateur qui clique).
fn repondre(b: &Banc, journal: &Journal, decisions: Vec<PermissionDecision>) {
    let p = Rc::clone(&b.p);
    let j = Rc::clone(journal);
    let file = RefCell::new(decisions.into_iter());
    b.hote.set_pompe(move || {
        let Some(d) = file.borrow_mut().next() else {
            return false;
        };
        let id = j
            .borrow()
            .iter()
            .rev()
            .find_map(|e| match e {
                ChatEvent::Part(p) if p.kind == "agent-permission" => p.id.clone(),
                _ => None,
            })
            .unwrap_or_default();
        p.respond_permission(&id, d);
        true
    });
}

fn cartes(evts: &[ChatEvent]) -> Vec<PermissionCardData> {
    evts.iter()
        .filter_map(|e| match e {
            ChatEvent::Part(p) if p.kind == "agent-permission" => {
                serde_json::from_value(p.data.clone()?).ok()
            }
            _ => None,
        })
        .collect()
}

fn demande(
    tool: &str,
    famille: Option<ToolFamily>,
    input: Value,
) -> fragment_agent::contrat::PermissionRequest {
    fragment_agent::contrat::PermissionRequest {
        tool: tool.into(),
        family: famille,
        input: input.as_object().cloned().unwrap_or_default(),
        title: None,
        reason: None,
        plan: None,
    }
}

fn autoriser() -> PermissionDecision {
    PermissionDecision::Allow { remember: None }
}

#[test]
fn assistant_une_carte_dans_la_reponse_autoriser_une_fois_l_outil_passe_la_carte_se_replie() {
    let b = banc();
    b.faux.scenario(|_| {
        vec![
            Pas::Carte(demande(
                "Bash",
                Some(ToolFamily::Shell),
                json!({ "command": "git status" }),
            )),
            Pas::Bilan,
        ]
    });
    let j: Journal = Rc::default();
    repondre(&b, &j, vec![autoriser()]);
    envoyer_dans(&b, "c", "statut ?", EnvoiAgent::default(), &j);
    let evts = j.borrow().clone();
    let cs = cartes(&evts);
    assert_eq!(cs.len(), 2); // la carte en attente, puis la carte repliée
    assert_eq!(cs[0].state, EtatCarte::Pending);
    let d = describe_permission(&cs[0].request);
    assert_eq!(
        (d.verb.as_str(), d.detail.as_str()),
        ("Veut exécuter une commande", "git status")
    );
    assert_eq!(
        (cs[1].state, cs[1].id.as_str()),
        (EtatCarte::Allowed, cs[0].id.as_str())
    );
    // Une seule carte dans le message : la repliée a remplacé l'ouverte.
    let parts = parts_finaux(&evts);
    assert_eq!(
        parts
            .iter()
            .filter(|p| p.kind == "agent-permission")
            .count(),
        1
    );
    assert_eq!(texte_de(&parts, "answer").as_deref(), Some("Bash: oui"));
}

#[test]
fn toujours_dans_cette_conversation_la_suite_n_est_plus_demandee_meme_famille_meme_motif() {
    let b = banc();
    b.faux.scenario(|_| {
        vec![
            Pas::Carte(demande(
                "Bash",
                Some(ToolFamily::Shell),
                json!({ "command": "git log" }),
            )),
            Pas::Bilan,
        ]
    });
    let j: Journal = Rc::default();
    let regle = PermanentRule {
        family: ToolFamily::Shell,
        pattern: Some("git*".into()),
    };
    repondre(
        &b,
        &j,
        vec![PermissionDecision::Allow {
            remember: Some(Remember {
                scope: RememberScope::Conversation,
                rule: regle,
            }),
        }],
    );
    envoyer_dans(&b, "c", "log ?", EnvoiAgent::default(), &j);
    let cs = cartes(&j.borrow());
    assert_eq!(
        cs[1].note.as_deref(),
        Some("toujours dans cette conversation")
    );
    // Retenir la règle est l'affaire de l'hôte (fr-t7-service) : le fournisseur renvoie la décision au moteur.
    assert!(matches!(
        &b.faux.cartes()[0].1,
        PermissionDecision::Allow { remember: Some(r) } if r.scope == RememberScope::Conversation
    ));
}

#[test]
fn refuser_avec_un_pourquoi_l_agent_recoit_le_message() {
    let b = banc();
    b.faux.scenario(|_| {
        vec![
            Pas::Carte(demande(
                "Write",
                Some(ToolFamily::Write),
                json!({ "file_path": "Cours/b.md", "content": "x" }),
            )),
            Pas::Bilan,
        ]
    });
    let j: Journal = Rc::default();
    repondre(
        &b,
        &j,
        vec![PermissionDecision::Deny {
            message: Some("pas maintenant".into()),
        }],
    );
    envoyer_dans(&b, "c", "écris", EnvoiAgent::default(), &j);
    let evts = j.borrow().clone();
    let cs = cartes(&evts);
    assert_eq!(
        describe_permission(&cs[0].request).verb,
        "Veut créer « b.md »"
    );
    assert_eq!(
        (cs[1].state, cs[1].note.as_deref()),
        (EtatCarte::Denied, Some("pas maintenant"))
    );
    assert_eq!(
        texte_de(&parts_finaux(&evts), "answer").as_deref(),
        Some("Write: non (pas maintenant)")
    );
}

#[test]
fn toujours_dans_ce_coffre_retenu_dans_les_reglages_et_valable_dans_une_autre_conversation() {
    let b = banc();
    b.faux.scenario(|_| {
        vec![
            Pas::Carte(demande(
                "WebFetch",
                Some(ToolFamily::WebFetch),
                json!({ "url": "https://fr.wikipedia.org/wiki/X" }),
            )),
            Pas::Bilan,
        ]
    });
    let j: Journal = Rc::default();
    // La carte PROPOSE la règle (l'hôte la calcule) ; « toujours dans ce coffre » la renvoie.
    let proposee = PermanentRule {
        family: ToolFamily::WebFetch,
        pattern: Some("fr.wikipedia.org".into()),
    };
    repondre(
        &b,
        &j,
        vec![PermissionDecision::Allow {
            remember: Some(Remember {
                scope: RememberScope::Vault,
                rule: proposee.clone(),
            }),
        }],
    );
    envoyer_dans(&b, "c", "web", EnvoiAgent::default(), &j);
    let cs = cartes(&j.borrow());
    assert_eq!(cs[0].rule, Some(proposee));
    assert_eq!(cs[1].note.as_deref(), Some("toujours dans ce coffre"));
    assert_eq!(
        b.faux.cartes()[0].1,
        PermissionDecision::Allow {
            remember: Some(Remember {
                scope: RememberScope::Vault,
                rule: PermanentRule {
                    family: ToolFamily::WebFetch,
                    pattern: Some("fr.wikipedia.org".into())
                },
            }),
        }
    );
}

#[test]
fn la_garde_dossier_masque_refuse_chemins_de_fragment_proteges_meme_en_acces_complet() {
    let b = banc();
    let mut politique = open_vault_policy();
    politique.folders = BTreeMap::from([("Privé".to_string(), FolderAccess::Hidden)]);
    b.hote.set_garde(move |u: &ToolUse| {
        let ctx = DecideContext {
            vault_root: "C:/coffre".into(),
            ..DecideContext::default()
        };
        match decide(&politique, u, &ctx) {
            Decision::Allow => GuardDecision::Allow,
            Decision::Ask => GuardDecision::Ask,
            Decision::Deny { reason } => GuardDecision::Deny { message: reason },
        }
    });
    let usage = |tool: &str, input: Value| ToolUse {
        tool: tool.into(),
        input: input.as_object().cloned().unwrap_or_default(),
    };
    let appels = [
        usage("Read", json!({ "file_path": "Privé/journal.md" })),
        usage("Grep", json!({ "pattern": "x" })),
        usage("Read", json!({ "file_path": "Cours/a.md" })),
        usage(
            "Write",
            json!({ "file_path": "notes/../.fragment/plugins/agent/data.json" }),
        ),
    ];
    b.faux.scenario({
        let a = appels.clone();
        move |_| a.iter().cloned().map(Pas::Garde).collect()
    });
    let conv = b.p.create().unwrap_or_default();
    envoyer(&b, &conv, "garde");
    let g: Vec<Option<GuardDecision>> = b.faux.gardes().into_iter().map(|(_, d)| d).collect();
    let refus = |d: &Option<GuardDecision>| match d {
        Some(GuardDecision::Deny { message }) => message.clone(),
        _ => String::new(),
    };
    assert!(refus(&g[0]).contains("masqué"), "{:?}", g[0]);
    assert!(refus(&g[1]).contains("Privé"), "{:?}", g[1]);
    assert_eq!(g[2], Some(GuardDecision::Allow));
    assert!(refus(&g[3]).contains("protégé"), "{:?}", g[3]);
}

#[test]
fn mode_plan_choisi_dans_le_champ_la_carte_du_plan_lancer_passe_la_conversation_en_modifs_auto() {
    let b = banc();
    let conv = b.p.create().unwrap_or_default();
    b.p.set_mode(&conv, Some(PermissionMode::Plan));
    b.faux.scenario(|_| {
        let mut r = demande(
            "ExitPlanMode",
            None,
            json!({ "plan": "# Mon plan\n\n1. Lire\n2. Écrire" }),
        );
        r.plan = Some("# Mon plan\n\n1. Lire\n2. Écrire".into());
        vec![Pas::Carte(r), Pas::Bilan]
    });
    let j: Journal = Rc::default();
    repondre(&b, &j, vec![autoriser()]);
    envoyer_dans(&b, &conv, "prépare un plan", EnvoiAgent::default(), &j);
    assert_eq!(b.faux.appels()[0].policy.mode, PermissionMode::Plan);
    let cs = cartes(&j.borrow());
    assert!(
        cs[0]
            .request
            .plan
            .as_deref()
            .unwrap_or("")
            .contains("Mon plan")
    );
    assert_eq!(describe_permission(&cs[0].request).verb, "Propose un plan");
    assert_eq!(
        texte_de(&parts_finaux(&j.borrow()), "answer").as_deref(),
        Some("ExitPlanMode: oui")
    );
    // « Lancer » : le champ passe la conversation en modifs auto, pour la suite.
    b.p.set_mode(&conv, Some(PermissionMode::AcceptEdits));
    assert_eq!(b.p.profile(&conv).mode, Some(PermissionMode::AcceptEdits));
    b.faux.scenario(|_| vec![Pas::texte("ok")]);
    envoyer(&b, &conv, "vas-y");
    assert_eq!(b.faux.appels()[1].policy.mode, PermissionMode::AcceptEdits);
}

#[test]
fn arreter_un_tour_qui_attend_une_reponse_la_carte_est_refusee_et_repliee() {
    let b = banc();
    b.faux.scenario(|_| {
        vec![
            Pas::Carte(demande(
                "Bash",
                Some(ToolFamily::Shell),
                json!({ "command": "rm -rf build" }),
            )),
            Pas::texte("jamais émis"),
        ]
    });
    let evts: Journal = Rc::default();
    // L'utilisateur clique « Arrêter » pendant que la carte est ouverte.
    let (p, j) = (Rc::clone(&b.p), Rc::clone(&evts));
    let mut arrete = false;
    b.p.envoyer("c", "nettoie", EnvoiAgent::default(), &mut move |e| {
        let carte_ouverte = matches!(&e, ChatEvent::Part(x) if x.kind == "agent-permission");
        j.borrow_mut().push(e);
        if carte_ouverte && !arrete {
            arrete = true;
            p.stop("c");
        }
    });
    let evts = evts.borrow().clone();
    let cs = cartes(&evts);
    assert_eq!(cs.len(), 2);
    assert_eq!(
        (cs[1].state, cs[1].note.as_deref()),
        (EtatCarte::Denied, Some("Tour arrêté."))
    );
    assert_eq!(describe_permission(&cs[1].request).detail, "rm -rf build");
    assert!(texte_de(&parts_finaux(&evts), "answer").is_none());
    assert_eq!(erreur_finale(&evts), Some(None));
}

#[test]
fn sans_personne_pour_repondre_la_carte_est_refusee_au_lieu_de_bloquer() {
    let b = banc();
    b.faux.scenario(|_| {
        vec![
            Pas::Carte(demande(
                "Bash",
                Some(ToolFamily::Shell),
                json!({ "command": "ls" }),
            )),
            Pas::Bilan,
        ]
    });
    let evts = envoyer(&b, "c", "x");
    assert_eq!(
        texte_de(&parts_finaux(&evts), "answer").as_deref(),
        Some("Bash: non (Personne pour répondre.)")
    );
}

// ═══════════════════════ agent-trace.spec.ts ═══════════════════════

#[test]
fn en_direct_pensee_outils_narration_repliee_reponse_seule_resume() {
    let b = banc();
    let j: Journal = Rc::default();
    // Les états observés à chaque pas libéré : (rang, trace, réponse, nombre d'événements).
    let vus: Rc<RefCell<Vus>> = Rc::default();
    {
        let (j, vus) = (Rc::clone(&j), Rc::clone(&vus));
        b.faux.au_pas(move |n| {
            let parts = parts_finaux(&j.borrow());
            vus.borrow_mut()
                .push((n, trace_de(&parts), texte_de(&parts, "answer")));
            true
        });
    }
    b.faux.scenario(|_| {
        vec![
            Pas::Pause, // 1 : la trace vide, l'orbe
            Pas::pensee_debut(),
            Pas::pensee("Je dois trouver où l’article parle de Palantir. "),
            Pas::Pause, // 2 : la pensée
            Pas::outil("g1", "Grep", "Palantir", json!({ "pattern": "Palantir" })),
            Pas::Pause, // 3 : l'outil tourne
            Pas::resultat("g1", "article.md:4: The first was Palantir."),
            Pas::Pause, // 4 : l'outil est posé
            Pas::texte("Je lis la note pour vérifier."),
            Pas::Pause, // 5 : la narration s'écrit comme une réponse
            Pas::outil(
                "r1",
                "Read",
                "article.md",
                json!({ "file_path": "C:/coffre/article.md" }),
            ),
            Pas::Pause, // 6 : la narration est passée dans la trace
            Pas::resultat("r1", "FDEs have the hottest job in AI."),
            Pas::Pause, // 7
            Pas::texte("Palantir est "),
            Pas::texte("la **première** "),
            Pas::texte("expérience citée."),
            Pas::Pause, // 8 : la réponse
        ]
    });
    envoyer_dans(&b, "c", "où est Palantir ?", EnvoiAgent::default(), &j);
    let vus = vus.borrow();
    let etat = |n: usize| {
        vus.iter()
            .find(|v| v.0 == n)
            .cloned()
            .unwrap_or_else(|| panic!("pas {n}"))
    };
    let genres = |t: &Option<fragment_agent::trace_model::Trace>| -> Vec<StepKind> {
        t.as_ref()
            .map(|t| t.steps.iter().map(|s| s.kind).collect())
            .unwrap_or_default()
    };
    assert_eq!(genres(&etat(1).1), []);
    assert_eq!(etat(1).1.map(|t| t.status), Some(TraceStatus::Running));
    assert_eq!(genres(&etat(2).1), [StepKind::Thinking]);
    let (t3, t4) = (etat(3).1, etat(4).1);
    assert!(t3.as_ref().is_some_and(|t| t.steps[1].result.is_none()));
    assert!(t4.as_ref().is_some_and(|t| t.steps[1].result.is_some()));
    // La narration s'écrit d'abord comme une RÉPONSE (la trace s'arrête : « écrit »)…
    assert_eq!(etat(5).2.as_deref(), Some("Je lis la note pour vérifier."));
    assert_eq!(etat(5).1.and_then(|t| t.writing), Some(true));
    // …puis un outil la fait passer dans la TRACE : plus de réponse, une note de plus.
    assert_eq!(etat(6).2, None);
    assert_eq!(
        genres(&etat(6).1),
        [
            StepKind::Thinking,
            StepKind::Tool,
            StepKind::Note,
            StepKind::Tool
        ]
    );
    // Fin : la réponse SEULE sous la trace, repliée sur son résumé.
    let parts = parts_finaux(&j.borrow());
    let t = trace_de(&parts).unwrap_or_else(|| panic!("trace"));
    assert_eq!(t.status, TraceStatus::Done);
    assert_eq!(t.steps.len(), 4);
    assert!(summarize(&t).contains("a réfléchi") || !summarize(&t).is_empty());
    assert_eq!(
        texte_de(&parts, "answer").as_deref(),
        Some("Palantir est la **première** expérience citée.")
    );
    assert_eq!(parts.len(), 2);
}

#[test]
fn relue_rouvrir_la_conversation_meme_trace_repliee_resume_durees_puis_la_reponse() {
    let b = banc();
    b.faux.scenario(|_| {
        vec![
            Pas::pensee_debut(),
            Pas::pensee("Je dois trouver Palantir."),
            Pas::outil("g1", "Grep", "Palantir", json!({ "pattern": "Palantir" })),
            Pas::resultat("g1", "article.md:4"),
            Pas::texte("Je lis la note pour vérifier."),
            Pas::outil(
                "r1",
                "Read",
                "article.md",
                json!({ "file_path": "C:/coffre/article.md" }),
            ),
            Pas::resultat("r1", "FDEs…"),
            Pas::texte("Palantir est la **première** expérience citée."),
        ]
    });
    let conv = b.p.create().unwrap_or_default();
    let direct = parts_finaux(&envoyer(&b, &conv, "où est Palantir ?"));
    let m = b.p.load(&conv).unwrap_or_default();
    assert_eq!(m.len(), 2);
    let relu = &m[1].parts;
    assert_eq!(relu.len(), 2);
    // Même ordre qu'en direct : la trace, puis la réponse seule.
    let (td, tr) = (
        trace_de(&direct).unwrap_or_else(|| panic!("direct")),
        trace_de(relu).unwrap_or_else(|| panic!("relue")),
    );
    let forme = |t: &fragment_agent::trace_model::Trace| -> Vec<(StepKind, Option<String>)> {
        t.steps
            .iter()
            .map(|s| (s.kind, s.name.clone().or(s.text.clone())))
            .collect()
    };
    assert_eq!(forme(&td), forme(&tr));
    assert_eq!(tr.status, TraceStatus::Done);
    assert!(
        tr.steps
            .iter()
            .filter(|s| s.kind == StepKind::Tool)
            .all(|s| s.result.is_some())
    );
    assert_eq!(
        relu[1].text.as_deref(),
        Some("Palantir est la **première** expérience citée.")
    );
    assert_eq!(texte_de(&direct, "answer"), relu[1].text);
}

#[test]
fn un_tour_sans_pensee_ni_outil_l_orbe_ecrit_pendant_la_reponse_puis_plus_de_trace() {
    let b = banc();
    b.faux
        .scenario(|_| vec![Pas::texte("Oui, "), Pas::texte("tout simplement.")]);
    let conv = b.p.create().unwrap_or_default();
    let evts = envoyer(&b, &conv, "[simple]");
    // Pendant la réponse : une trace vide « en train d'écrire » ; à la fin, plus de trace.
    let ecrit = evts.iter().any(|e| {
        matches!(e, ChatEvent::Part(p) if p.kind == "agent-trace"
        && p.data.as_ref().and_then(trace_depuis_json).is_some_and(|t| t.writing == Some(true)))
    });
    assert!(ecrit);
    let n = evts.len();
    assert_eq!(evts[n - 2], ChatEvent::Remove("trace".into()));
    let parts = parts_finaux(&evts);
    assert_eq!(parts.len(), 1);
    assert_eq!(
        texte_de(&parts, "answer").as_deref(),
        Some("Oui, tout simplement.")
    );
    // Relue : la réponse seule, sans trace.
    let m = b.p.load(&conv).unwrap_or_default();
    assert_eq!(m[1].parts.len(), 1);
    assert_eq!(m[1].parts[0].text.as_deref(), Some("Oui, tout simplement."));
}

// ═══════════════════════ chat-attachments.spec.ts ═══════════════════════

const PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

fn piece(nom: &str, mime: &str, octets: &[u8]) -> ChatAttachment {
    ChatAttachment {
        name: nom.into(),
        mime: mime.into(),
        data: octets.to_vec(),
    }
}

fn png() -> Vec<u8> {
    fragment_chat_decode(PNG_B64)
}

fn fragment_chat_decode(s: &str) -> Vec<u8> {
    fragment_agent::fournisseur::pieces::decoder_base64(s).unwrap_or_default()
}

#[test]
fn envoyer_image_pdf_texte_zip_blocs_natifs_au_sdk_fichier_autre_par_son_chemin_pieces_dans_la_bulle()
 {
    let b = banc();
    b.faux.scenario(|_| vec![Pas::texte("Vu.")]);
    let conv = b.p.create().unwrap_or_default();
    let evts = envoyer_avec(
        &b,
        &conv,
        "que contiennent-ils ?",
        EnvoiAgent {
            attachments: vec![
                piece("photo.png", "image/png", &png()),
                piece("cours.pdf", "application/pdf", b"%PDF-1.4"),
                piece("notes.md", "text/markdown", b"# Mes notes"),
                piece("archive.zip", "application/zip", b"PK"),
            ],
            ..EnvoiAgent::default()
        },
    );
    assert_eq!(erreur_finale(&evts), Some(None));
    let req = b.faux.appels().remove(0);
    assert_eq!(req.prompt, "que contiennent-ils ?");
    let types: Vec<&str> = req
        .content
        .iter()
        .map(|c| c["type"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(types, ["image", "document", "document", "text"]);
    assert_eq!(
        req.content[0]["source"],
        json!({ "type": "base64", "media_type": "image/png", "data": PNG_B64 })
    );
    assert_eq!(req.content[1]["title"], "cours.pdf");
    assert_eq!(req.content[1]["source"]["media_type"], "application/pdf");
    assert_eq!(req.content[2]["title"], "notes.md");
    assert_eq!(req.content[2]["source"]["type"], "text");
    assert_eq!(req.content[2]["source"]["data"], "# Mes notes");
    // Le fichier « autre » est rangé dans le dossier des pièces de l'agent, lisible par son chemin.
    let marqueur = req.content[3]["text"].as_str().unwrap_or("");
    let chemin = marqueur
        .split("path=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("");
    assert!(
        chemin.ends_with("archive.zip") && chemin.contains("attachments"),
        "{chemin}"
    );
    assert!(std::path::Path::new(chemin).starts_with(b.hote.dossier_pieces()));
    assert_eq!(std::fs::read(chemin).unwrap_or_default(), b"PK");

    // La bulle relue : le texte, puis les pièces (l'image avec sa vignette).
    let m = b.p.load(&conv).unwrap_or_default();
    let bulle = &m[0].parts;
    assert_eq!(bulle[0].text.as_deref(), Some("que contiennent-ils ?"));
    let jointes: Vec<&ChatPart> = bulle.iter().filter(|p| p.kind == "attachment").collect();
    assert_eq!(jointes.len(), 4);
    let noms: Vec<&str> = jointes[1..]
        .iter()
        .map(|p| {
            p.data
                .as_ref()
                .map(|d| d["name"].as_str().unwrap_or(""))
                .unwrap_or("")
        })
        .collect();
    assert_eq!(noms, ["cours.pdf", "notes.md", "archive.zip"]);
}

#[test]
fn une_image_seule_sans_texte_part_le_titre_de_la_conversation_est_son_nom() {
    let b = banc();
    let conv = b.p.create().unwrap_or_default();
    envoyer_avec(
        &b,
        &conv,
        "",
        EnvoiAgent {
            attachments: vec![piece("capture.png", "image/png", &png())],
            ..EnvoiAgent::default()
        },
    );
    let req = b.faux.appels().remove(0);
    assert_eq!(req.prompt, "");
    assert_eq!(req.content.len(), 1);
    assert_eq!(b.p.title(&conv), "capture.png");
}

#[test]
fn relecture_rouvrir_la_conversation_rend_ses_pieces_jointes_vignette_comprise() {
    let b = banc();
    let conv = b.p.create().unwrap_or_default();
    envoyer_avec(
        &b,
        &conv,
        "que contiennent-ils ?",
        EnvoiAgent {
            attachments: vec![piece("photo.png", "image/png", &png())],
            ..EnvoiAgent::default()
        },
    );
    // Un fournisseur neuf : tout est relu du transcript.
    let neuf = AgentChatProvider::new(Rc::clone(&b.hote) as Rc<dyn AgentHote>);
    let m = neuf.load(&conv).unwrap_or_default();
    let image = m[0]
        .parts
        .iter()
        .find(|p| p.kind == "attachment")
        .and_then(|p| p.data.clone())
        .unwrap_or_default();
    assert_eq!(image["mime"], "image/png");
    assert!(
        image["url"]
            .as_str()
            .unwrap_or("")
            .starts_with("data:image/png;base64,")
    );
}

#[test]
fn rebuild_une_image_de_plus_de_1568_px_est_reduite_avant_l_envoi() {
    let b = banc();
    let codec = CodecImageFaux::new(Some((3136, 1568)), &[3]);
    b.hote.set_codec(codec.clone());
    let conv = b.p.create().unwrap_or_default();
    envoyer_avec(
        &b,
        &conv,
        "regarde",
        EnvoiAgent {
            attachments: vec![piece("grande.png", "image/png", &[1; 10])],
            ..EnvoiAgent::default()
        },
    );
    assert_eq!(codec.appels(), [(1568, 784, FormatImage::Png)]);
    let req = b.faux.appels().remove(0);
    assert_eq!(req.content[0]["source"]["data"], base64(&[7, 7, 7]));
}

// ═══════════════════════ ce que le TS ne teste pas isolément ═══════════════════════

#[test]
fn titre_depuis_une_ligne_bornee_a_60() {
    assert_eq!(titre_depuis("  a\n  b\t c "), "a b c");
    assert_eq!(titre_depuis("   "), "Nouvelle session");
    let long = "x".repeat(80);
    let t = titre_depuis(&long);
    assert_eq!((t.chars().count(), t.ends_with('…')), (60, true));
    assert_eq!(titre_depuis(&"y".repeat(60)), "y".repeat(60));
}

#[test]
fn participants_titre_profil_et_icone() {
    let b = banc();
    b.hote.set_nom("Julie");
    let p = b.p.participants("c");
    assert_eq!((p[0].id.as_str(), p[0].name.as_str()), ("user", "Julie"));
    assert_eq!((p[1].id.as_str(), p[1].name.as_str()), ("claude", "Claude"));
    assert_eq!(
        (b.p.id(), b.p.label(), b.p.icon()),
        ("agent", "Agent", Some("sparkles"))
    );
    assert_eq!(b.p.title("inconnue"), "");
}

#[test]
fn choix_en_attente_avant_le_1er_envoi_copy_pending_et_set_pending_meta() {
    let b = banc();
    let modele = b.p.create().unwrap_or_default();
    b.p.set_effort(&modele, Some("low"));
    b.p.set_model(&modele, Some("faux-1"));
    b.p.set_mode(&modele, Some(PermissionMode::Ask));
    b.p.set_pending_meta(
        &modele,
        json!({ "bar": true })
            .as_object()
            .cloned()
            .unwrap_or_default(),
    );
    // La barre en lance une neuve à chaque Entrée : les choix suivent, la meta non.
    let neuve = b.p.create().unwrap_or_default();
    b.p.copy_pending(&modele, &neuve);
    let p = b.p.profile(&neuve);
    assert_eq!(
        (p.effort.as_deref(), p.model.as_deref(), p.mode),
        (Some("low"), Some("faux-1"), Some(PermissionMode::Ask))
    );
    assert!(b.p.meta_of(&neuve).is_empty());
    assert_eq!(b.p.meta_of(&modele)["bar"], true);
    // Au 1er envoi, la session naît avec le profil et la meta en attente.
    b.p.set_pending_meta(
        &neuve,
        json!({ "bar": true, "origin": { "doc": "a.md" } })
            .as_object()
            .cloned()
            .unwrap_or_default(),
    );
    envoyer(&b, &neuve, "salut");
    let meta = b.hote.session(&neuve).map(|s| s.meta).unwrap_or_default();
    assert_eq!(meta["bar"], true);
    assert_eq!(
        meta["profile"],
        json!({ "effort": "low", "model": "faux-1" })
    );
    assert_eq!(b.faux.appels()[0].policy.mode, PermissionMode::Ask);
    // Les choix en attente sont consommés ; le profil vient désormais de la session.
    assert_eq!(b.p.profile(&neuve).effort.as_deref(), Some("low"));
    b.p.set_effort(&neuve, Some("high"));
    assert_eq!(
        b.hote
            .session(&neuve)
            .map(|s| s.meta["profile"]["effort"].clone()),
        Some(json!("high"))
    );
    // Une conversation qui existe déjà ne reçoit plus de meta en attente.
    b.p.set_pending_meta(
        &neuve,
        json!({ "x": 1 }).as_object().cloned().unwrap_or_default(),
    );
    assert!(b.p.meta_of(&neuve).get("x").is_none());
}

#[test]
fn le_moteur_se_change_jusqu_au_premier_tour_et_oublie_le_modele() {
    let b = banc_echo();
    let conv = b.p.create().unwrap_or_default();
    assert_eq!(b.p.harness_of(&conv), "claude");
    b.p.set_model(&conv, Some("faux-1"));
    assert!(b.p.can_change_harness(&conv));
    assert!(b.p.set_harness(&conv, "echo"));
    assert_eq!(
        (b.p.harness_of(&conv), b.p.profile(&conv).model),
        ("echo".to_string(), None)
    );
}

#[test]
fn une_branche_part_du_transcript_du_parent_et_load_saute_les_bulles_heritees() {
    let b = banc();
    b.faux
        .scenario(|t| vec![Pas::texte(&format!("re: {}", t.prompt))]);
    let parent = b.p.create().unwrap_or_default();
    envoyer(&b, &parent, "premier");
    envoyer(&b, &parent, "second");
    let fin_du_premier_tour = b.faux.transcript(&parent)[1].last.clone();
    // La branche vise le message de rang 0 (la première question) : elle garde ce tour entier.
    b.hote.poser_session(
        "branche",
        "Branche",
        json!({ "fork": { "from": parent, "message": 0 } }),
    );
    envoyer(&b, "branche", "et si ?");
    let tour = b.faux.appels().pop().unwrap_or_else(|| panic!("appel"));
    let f = tour.fork.unwrap_or_else(|| panic!("fork"));
    assert_eq!(
        (f.from.id.as_str(), Some(f.at)),
        (parent.as_str(), fin_du_premier_tour)
    );
    // Le point est figé dans la session.
    let meta = b
        .hote
        .session("branche")
        .map(|s| s.meta)
        .unwrap_or_default();
    assert_eq!(meta["fork"]["inherited"], 2);
    // Le transcript de la branche : l'hérité (2 bulles), puis ses propres tours — `load` ne remontre que les siens.
    assert_eq!(b.faux.transcript("branche").len(), 4);
    let m = b.p.load("branche").unwrap_or_default();
    assert_eq!(
        m.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
        ["m2", "m3"]
    );
    assert_eq!(m[0].parts[0].text.as_deref(), Some("et si ?"));
    // Un second envoi garde le même point.
    envoyer(&b, "branche", "encore");
    assert_eq!(
        b.faux
            .appels()
            .pop()
            .and_then(|t| t.fork)
            .map(|f| f.at)
            .unwrap_or_default(),
        tour_at(&b)
    );
}

fn tour_at(b: &Banc) -> String {
    b.hote
        .session("branche")
        .and_then(|s| s.meta["fork"]["at"].as_str().map(str::to_string))
        .unwrap_or_default()
}

#[test]
fn le_preambule_part_avec_la_premiere_question_une_fois_et_on_montre_la_question() {
    let b = banc();
    b.hote.poser_session(
        "ask",
        "Ask",
        json!({ "preamble": "<doc>texte</doc> Explique :" }),
    );
    envoyer(&b, "ask", "Palantir ?");
    assert_eq!(
        b.faux.appels()[0].prompt,
        "<doc>texte</doc> Explique : Palantir ?"
    );
    let meta = b.hote.session("ask").map(|s| s.meta).unwrap_or_default();
    assert!(meta.get("preamble").is_none());
    assert_eq!(meta["display"], "Palantir ?");
    envoyer(&b, "ask", "et Anduril ?");
    assert_eq!(b.faux.appels()[1].prompt, "et Anduril ?");
    let m = b.p.load("ask").unwrap_or_default();
    assert_eq!(m[0].parts[0].text.as_deref(), Some("Palantir ?"));
    assert_eq!(m[2].parts[0].text.as_deref(), Some("et Anduril ?"));
}

#[test]
fn la_citation_et_le_contexte_precedent_la_question_la_zone_entouree_part_en_image() {
    let b = banc();
    let conv = b.p.create().unwrap_or_default();
    envoyer_avec(
        &b,
        &conv,
        "pourquoi ?",
        EnvoiAgent {
            quote: Some(ChatQuote {
                from: 0,
                to: 5,
                text: Some("a\nb".into()),
                image: Some(piece("zone.png", "image/png", &png())),
            }),
            context: Some(fragment_agent::controls_model::PromptContext {
                doc: "x.md".into(),
                title: "x".into(),
                selection: None,
            }),
            ..EnvoiAgent::default()
        },
    );
    let req = b.faux.appels().remove(0);
    assert_eq!(
        req.prompt,
        "[Context: the user is looking at the file \"x.md\" of the vault.]\n\nIn reply to this part of the conversation:\n> a\n> b\n\npourquoi ?"
    );
    assert_eq!(req.content.len(), 1); // l'image de la zone citée
    // Sans texte : la mention de l'image.
    envoyer_avec(
        &b,
        &conv,
        "et là ?",
        EnvoiAgent {
            quote: Some(ChatQuote {
                from: 0,
                to: 1,
                text: None,
                image: Some(piece("z.png", "image/png", &png())),
            }),
            ..EnvoiAgent::default()
        },
    );
    assert!(
        b.faux.appels()[1]
            .prompt
            .starts_with("In reply to this part of the conversation:\n> (the attached image")
    );
}

#[test]
fn les_options_json_du_chat_sont_decodees() {
    let opts = json!({
        "prompt": "long", "effort": "high", "model": "m",
        "context": { "doc": "a.md", "title": "A", "selection": "s" },
        "quote": { "from": 1, "to": 4, "text": "t" },
        "attachments": [{ "name": "p.png", "mime": "image/png", "data": PNG_B64 }, { "name": "?" }],
    });
    let e = EnvoiAgent::depuis_opts(opts.as_object().unwrap_or(&Default::default()));
    assert_eq!(
        (e.prompt.as_deref(), e.effort.as_deref(), e.model.as_deref()),
        (Some("long"), Some("high"), Some("m"))
    );
    assert_eq!(
        e.context.map(|c| (c.doc, c.selection)),
        Some(("a.md".into(), Some("s".into())))
    );
    assert_eq!(
        e.quote.map(|q| (q.from, q.to, q.text)),
        Some((1, 4, Some("t".into())))
    );
    assert_eq!(e.attachments.len(), 1); // l'entrée sans données est ignorée
    assert_eq!(e.attachments[0].data, png());
    // Par le trait : un envoi sans options.
    let b = banc();
    let mut n = 0;
    let r = b.p.send("c", "salut", &mut |_| n += 1, None);
    assert!(r.is_ok() && n > 0);
}

#[test]
fn l_usage_du_tour_est_garde_dans_la_session_et_l_echec_du_tour_se_dit() {
    let b = banc();
    b.faux.scenario(|_| {
        vec![
            Pas::usage(fragment_agent::contrat::TurnUsage {
                input_tokens: Some(5),
                ..Default::default()
            }),
            Pas::usage(fragment_agent::contrat::TurnUsage {
                output_tokens: Some(7),
                ..Default::default()
            }),
            Pas::texte("ok"),
        ]
    });
    envoyer(&b, "u", "x");
    let meta = b.hote.session("u").map(|s| s.meta).unwrap_or_default();
    assert_eq!(
        meta["usage"],
        json!({ "inputTokens": 5, "outputTokens": 7 })
    ); // les relevés se fusionnent
    b.faux.scenario(|_| {
        vec![
            Pas::texte("début"),
            Pas::Evenement(AgentEvent::Done {
                session_id: "e".into(),
                is_error: true,
                errors: vec!["quota".into()],
                usage: None,
            }),
        ]
    });
    let evts = envoyer(&b, "e", "x");
    assert_eq!(erreur_finale(&evts), Some(Some("quota".into())));
    b.faux.scenario(|_| vec![Pas::erreur("réseau coupé")]);
    assert_eq!(
        erreur_finale(&envoyer(&b, "f", "x")),
        Some(Some("réseau coupé".into()))
    );
}

#[test]
fn origine_ancienne_forme_supprimer_et_liste_la_plus_recente_d_abord() {
    let b = banc();
    b.hote.poser_session(
        "vieille",
        "Vieille",
        json!({ "file": "note.md", "quote": "passage" }),
    );
    b.hote.poser_session("sans", "Sans", json!({}));
    let o = b.p.origin("vieille").unwrap_or_else(|| panic!("origine"));
    assert_eq!(
        (o.doc.as_str(), o.from, o.quote.as_deref()),
        ("note.md", None, Some("passage"))
    );
    assert!(b.p.origin("sans").is_none() && b.p.origin("absente").is_none());
    envoyer(&b, "sans", "touche");
    assert_eq!(
        b.p.list().iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["sans", "vieille"]
    );
    assert!(b.p.delete("sans").is_ok());
    assert_eq!(b.p.list().len(), 1);
}
