//! Tests headless des contrôles du champ et de la carte de permission (tâche `fr-t7-controles`),
//! nommés d'après les cas de `app/e2e/agent-permissions.spec.ts` (la partie CARTE), de
//! `agent-chat.spec.ts` (l'effort), de `agent-bar.spec.ts` (les contrôles de la barre, l'anneau
//! absent, la puce de contexte) et de `controlsModel.test.ts`. Le double du SDK devient une
//! [`SourceControles`] en mémoire et un rappel qui note les décisions.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::RefCell;
use std::rc::Rc;

use fragment_agent::contrat::{
    HarnessModel, PermanentRule, PermissionDecision, PermissionMode, PermissionRequest,
    RememberScope, ToolFamily, TurnUsage,
};
use fragment_agent::controls_model::context_preamble;
use fragment_agent::permission_model::{EtatCarte, PermissionCardData};
use fragment_chat::api::{ChatComposerExtension, ChatMode, ChatPart, ChatSlotContext, SlotAlign};
use fragment_ui::agent_controles::{
    ActionCarte, CarteModele, Controle, Controles, DocumentVu, ExtensionContexte,
    MESSAGE_TOUR_ARRETE, MoteurPropose, NiveauUsage, PART_PERMISSION, ProfilConversation,
    PuceContexte, SourceControles, VarianteBouton, mode_depuis_id, mode_id, refus_a_larret,
    repondre, slots_controles,
};
use serde_json::json;

// ── le double du fournisseur ─────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Etat {
    moteur: String,
    fige: bool,
    profil: ProfilConversation,
    usage: Option<TurnUsage>,
}

struct Faux {
    e: RefCell<Etat>,
}

fn modele(id: &str, label: &str, efforts: &[&str], defaut: bool) -> HarnessModel {
    HarnessModel {
        id: id.into(),
        label: label.into(),
        description: None,
        efforts: (!efforts.is_empty()).then(|| efforts.iter().map(|e| (*e).to_owned()).collect()),
        small: None,
        default: defaut.then_some(true),
    }
}

impl Faux {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            e: RefCell::new(Etat {
                moteur: "claude".into(),
                ..Etat::default()
            }),
        })
    }
}

impl SourceControles for Faux {
    fn moteurs(&self) -> Vec<MoteurPropose> {
        vec![
            MoteurPropose {
                id: "claude".into(),
                label: "Claude Code".into(),
                peut_demander: true,
            },
            MoteurPropose {
                id: "tiers".into(),
                label: "Tiers".into(),
                peut_demander: false,
            },
        ]
    }
    fn moteur_de(&self, _: &str) -> String {
        self.e.borrow().moteur.clone()
    }
    fn peut_changer_moteur(&self, _: &str) -> bool {
        !self.e.borrow().fige
    }
    fn definir_moteur(&self, _: &str, moteur: &str) {
        self.e.borrow_mut().moteur = moteur.into();
    }
    fn modeles(&self, moteur: &str) -> Vec<HarnessModel> {
        match moteur {
            "claude" => vec![
                modele("claude-opus", "Opus 5.5", &["low", "medium", "high"], true),
                modele("claude-sonnet-5-5", "Sonnet 5.5", &["low", "high"], false),
            ],
            "tiers" => vec![modele("t1", "Modèle tiers", &[], true)],
            _ => Vec::new(),
        }
    }
    fn profil(&self, _: &str) -> ProfilConversation {
        self.e.borrow().profil.clone()
    }
    fn definir_modele(&self, _: &str, m: &str) {
        self.e.borrow_mut().profil.model = Some(m.into());
    }
    fn definir_effort(&self, _: &str, e: Option<&str>) {
        self.e.borrow_mut().profil.effort = e.map(str::to_owned);
    }
    fn definir_mode(&self, _: &str, m: PermissionMode) {
        self.e.borrow_mut().profil.mode = Some(m);
    }
    fn mode_de_base(&self) -> PermissionMode {
        PermissionMode::Ask
    }
    fn usage(&self, _: &str) -> Option<TurnUsage> {
        self.e.borrow().usage.clone()
    }
}

fn controles(f: &Rc<Faux>) -> Controles {
    Controles::new(f.clone(), "c1")
}

fn titres(c: &Controles, k: Controle) -> Vec<String> {
    c.entrees(k).into_iter().map(|e| e.titre).collect()
}

// ── agent-chat : « effort (contribution au composer, liste du modèle) » ───────────────────────

#[test]
fn effort_contribution_au_composer_liste_du_modele_approfondi_donne_high_garde_dans_la_session() {
    let f = Faux::new();
    let c = controles(&f);
    // La liste propose « Par défaut » puis les niveaux du modèle courant (le défaut du moteur).
    assert_eq!(
        titres(&c, Controle::Effort),
        ["Par défaut", "Rapide", "Normal", "Approfondi"]
    );
    assert_eq!(c.etat(Controle::Effort).libelle, "Par défaut");
    assert!(c.choisir(Controle::Effort, "high"));
    assert_eq!(f.profil("c1").effort.as_deref(), Some("high"));
    assert_eq!(c.etat(Controle::Effort).libelle, "Approfondi");
    // « Par défaut » efface le choix.
    assert!(c.choisir(Controle::Effort, ""));
    assert_eq!(f.profil("c1").effort, None);
}

#[test]
fn effort_cache_pour_un_modele_sans_niveaux_et_suit_le_modele_choisi() {
    let f = Faux::new();
    let c = controles(&f);
    assert!(!c.etat(Controle::Effort).masque);
    // Sonnet ne propose que deux niveaux : la liste sœur suit sans rafraîchissement manuel.
    c.choisir(Controle::Modele, "claude-sonnet-5-5");
    assert_eq!(
        titres(&c, Controle::Effort),
        ["Par défaut", "Rapide", "Approfondi"]
    );
    // Un moteur dont le modèle n'a pas d'effort : l'emplacement se cache.
    c.choisir(Controle::Moteur, "tiers");
    assert!(c.etat(Controle::Effort).masque);
}

// ── agent-bar : « les contrôles de la barre valent pour la suivante » ─────────────────────────

#[test]
fn les_controles_de_la_barre_modele_et_effort_se_lisent_a_l_envoi_suivant() {
    let f = Faux::new();
    let c = controles(&f);
    c.choisir(Controle::Modele, "claude-sonnet-5-5");
    c.choisir(Controle::Effort, "high");
    let p = f.profil("c1");
    assert_eq!(p.model.as_deref(), Some("claude-sonnet-5-5"));
    assert_eq!(p.effort.as_deref(), Some("high"));
    assert_eq!(c.etat(Controle::Modele).libelle, "Sonnet 5.5");
    assert_eq!(c.etat(Controle::Modele).info, "Modèle : Sonnet 5.5");
    // La ligne cochée est celle du modèle choisi.
    let coches: Vec<_> = c
        .entrees(Controle::Modele)
        .into_iter()
        .filter(|e| e.coche)
        .map(|e| e.id)
        .collect();
    assert_eq!(coches, ["claude-sonnet-5-5"]);
}

#[test]
fn un_modele_inconnu_est_refuse() {
    let f = Faux::new();
    assert!(!controles(&f).choisir(Controle::Modele, "n-existe-pas"));
    assert_eq!(f.profil("c1").model, None);
}

#[test]
fn le_modele_de_la_liste_porte_sa_description() {
    struct Decrit(Rc<Faux>);
    impl SourceControles for Decrit {
        fn moteurs(&self) -> Vec<MoteurPropose> {
            self.0.moteurs()
        }
        fn moteur_de(&self, c: &str) -> String {
            self.0.moteur_de(c)
        }
        fn peut_changer_moteur(&self, c: &str) -> bool {
            self.0.peut_changer_moteur(c)
        }
        fn definir_moteur(&self, c: &str, m: &str) {
            self.0.definir_moteur(c, m);
        }
        fn modeles(&self, _: &str) -> Vec<HarnessModel> {
            let mut m = modele("a", "Alpha", &[], true);
            m.description = Some("le plus rapide".into());
            vec![m]
        }
        fn profil(&self, c: &str) -> ProfilConversation {
            self.0.profil(c)
        }
        fn definir_modele(&self, c: &str, m: &str) {
            self.0.definir_modele(c, m);
        }
        fn definir_effort(&self, c: &str, e: Option<&str>) {
            self.0.definir_effort(c, e);
        }
        fn definir_mode(&self, c: &str, m: PermissionMode) {
            self.0.definir_mode(c, m);
        }
        fn mode_de_base(&self) -> PermissionMode {
            self.0.mode_de_base()
        }
        fn usage(&self, c: &str) -> Option<TurnUsage> {
            self.0.usage(c)
        }
    }
    let c = Controles::new(Rc::new(Decrit(Faux::new())), "c1");
    assert_eq!(titres(&c, Controle::Modele), ["Alpha — le plus rapide"]);
}

// ── le moteur : figé après le premier message ─────────────────────────────────────────────────

#[test]
fn le_moteur_est_fige_apres_le_premier_message_grise_et_sans_liste() {
    let f = Faux::new();
    let c = controles(&f);
    let libre = c.etat(Controle::Moteur);
    assert_eq!(libre.libelle, "Claude Code");
    assert_eq!(libre.info, "Moteur de la conversation");
    assert!(!libre.desactive);
    f.e.borrow_mut().fige = true;
    let fige = c.etat(Controle::Moteur);
    assert!(fige.desactive);
    assert_eq!(
        fige.info,
        "Moteur : Claude Code (figé après le premier message)"
    );
    assert!(!c.choisir(Controle::Moteur, "tiers"));
    assert_eq!(f.moteur_de("c1"), "claude");
}

#[test]
fn le_moteur_change_les_listes_soeurs() {
    let f = Faux::new();
    let c = controles(&f);
    assert!(c.choisir(Controle::Moteur, "tiers"));
    assert_eq!(c.etat(Controle::Modele).libelle, "Modèle tiers");
    let coches: Vec<_> = c
        .entrees(Controle::Moteur)
        .into_iter()
        .filter(|e| e.coche)
        .map(|e| e.titre)
        .collect();
    assert_eq!(coches, ["Tiers"]);
}

#[test]
fn un_moteur_indisponible_grise_la_liste_des_modeles() {
    let f = Faux::new();
    f.e.borrow_mut().moteur = "absent".into();
    let m = controles(&f).etat(Controle::Modele);
    assert_eq!(
        (m.libelle.as_str(), m.info.as_str()),
        ("—", "Moteur indisponible")
    );
    assert!(m.desactive);
}

// ── le mode de permission ─────────────────────────────────────────────────────────────────────

#[test]
fn mode_plan_choisi_dans_le_champ_il_s_affiche_dans_le_bouton() {
    let f = Faux::new();
    let c = controles(&f);
    // Sans choix : le mode de la politique de base.
    assert_eq!(c.etat(Controle::Mode).libelle, "Demander");
    assert_eq!(
        titres(&c, Controle::Mode),
        ["Accès complet", "Demander", "Modifs auto", "Plan"]
    );
    assert!(c.choisir(Controle::Mode, "plan"));
    assert_eq!(c.etat(Controle::Mode).libelle, "Plan");
    assert_eq!(c.mode(), PermissionMode::Plan);
    // « Lancer » passe la conversation en modifs auto : le bouton le dit.
    c.choisir(Controle::Mode, "acceptEdits");
    assert_eq!(c.etat(Controle::Mode).libelle, "Modifs auto");
    assert_eq!(c.etat(Controle::Mode).info, "Permissions : Modifs auto");
}

#[test]
fn un_moteur_qui_ne_sait_pas_demander_reste_en_acces_complet() {
    let f = Faux::new();
    let c = controles(&f);
    c.choisir(Controle::Moteur, "tiers");
    let m = c.etat(Controle::Mode);
    assert_eq!(m.libelle, "Accès complet");
    assert_eq!(
        m.info,
        "Permissions : accès complet (ce moteur ne sait pas demander)"
    );
    // Les autres modes sont grisés, et refusés s'ils arrivent quand même.
    let grises: Vec<_> = c
        .entrees(Controle::Mode)
        .into_iter()
        .map(|e| (e.id, e.desactive))
        .collect();
    assert_eq!(
        grises,
        [
            ("full".to_string(), false),
            ("ask".to_string(), true),
            ("acceptEdits".to_string(), true),
            ("plan".to_string(), true),
        ]
    );
    assert!(!c.choisir(Controle::Mode, "plan"));
    assert!(c.choisir(Controle::Mode, "full"));
}

#[test]
fn les_noms_de_mode_font_l_aller_retour() {
    for id in ["full", "ask", "acceptEdits", "plan"] {
        assert_eq!(mode_id(mode_depuis_id(id).unwrap()), id);
    }
    assert_eq!(mode_depuis_id("autre"), None);
}

// ── l'anneau d'usage ──────────────────────────────────────────────────────────────────────────

#[test]
fn anneau_d_usage_cache_sans_usage_puis_fraction_palier_et_cout() {
    let f = Faux::new();
    let c = controles(&f);
    assert_eq!(c.anneau(), None);
    let usage = |ctx: u64| TurnUsage {
        context_tokens: Some(ctx),
        context_window: Some(200_000),
        cost_usd: Some(0.0421),
        ..TurnUsage::default()
    };
    f.e.borrow_mut().usage = Some(usage(50_000));
    let a = c.anneau().unwrap();
    assert_eq!(a.fraction, 0.25);
    assert_eq!(a.niveau, NiveauUsage::Bas);
    assert_eq!(
        a.label,
        "50 k / 200 k tokens de contexte · dernier tour 0,042 $"
    );
    f.e.borrow_mut().usage = Some(usage(150_000));
    assert_eq!(c.anneau().unwrap().niveau, NiveauUsage::Moyen);
    f.e.borrow_mut().usage = Some(usage(180_000));
    assert_eq!(c.anneau().unwrap().niveau, NiveauUsage::Haut);
    // Le palier se décide au-dessus de 60 % et 85 % (strict).
    f.e.borrow_mut().usage = Some(usage(120_000));
    assert_eq!(c.anneau().unwrap().niveau, NiveauUsage::Bas);
    f.e.borrow_mut().usage = Some(usage(170_000));
    assert_eq!(c.anneau().unwrap().niveau, NiveauUsage::Moyen);
}

// ── les emplacements : où chaque contrôle s'applique ──────────────────────────────────────────

fn contexte(fournisseur: &str, mode: ChatMode) -> ChatSlotContext {
    ChatSlotContext {
        provider: fournisseur.into(),
        conversation: "c1".into(),
        mode,
        message: None,
        streaming: false,
    }
}

fn presents(ctx: &ChatSlotContext) -> Vec<String> {
    slots_controles()
        .into_iter()
        .filter(|s| s.applies(ctx))
        .map(|s| s.id)
        .collect()
}

#[test]
fn ouverte_un_champ_seul_ni_fil_ni_carte_ni_anneau_d_usage_dans_la_barre() {
    let barre = presents(&contexte("agent", ChatMode::Bar));
    assert!(!barre.contains(&"agent-usage".to_string()));
    assert_eq!(
        barre,
        ["agent-harness", "agent-model", "agent-effort", "agent-mode"]
    );
}

#[test]
fn le_panneau_porte_les_cinq_controles_dans_l_ordre() {
    assert_eq!(
        presents(&contexte("agent", ChatMode::Panel)),
        [
            "agent-harness",
            "agent-model",
            "agent-effort",
            "agent-mode",
            "agent-usage"
        ]
    );
}

#[test]
fn une_conversation_ancree_n_a_pas_de_choix_du_moteur() {
    let ids = presents(&contexte("agent", ChatMode::Anchored));
    assert!(!ids.contains(&"agent-harness".to_string()));
    assert!(ids.contains(&"agent-model".to_string()));
}

#[test]
fn un_autre_fournisseur_ne_recoit_aucun_controle() {
    assert!(presents(&contexte("autre", ChatMode::Panel)).is_empty());
}

#[test]
fn l_anneau_se_place_a_la_fin_de_la_barre_les_listes_au_debut() {
    let slots = slots_controles();
    let fin: Vec<_> = slots
        .iter()
        .filter(|s| s.align == Some(SlotAlign::End))
        .map(|s| s.id.as_str())
        .collect();
    assert_eq!(fin, ["agent-usage"]);
    for k in Controle::TOUS {
        assert!(slots.iter().any(|s| s.id == k.id_slot()));
    }
}

// ── la puce de contexte (agent-bar : « la puce de contexte ») ─────────────────────────────────

fn doc(cle: &str, titre: &str) -> DocumentVu {
    DocumentVu {
        doc: cle.into(),
        titre: titre.into(),
        icone: "file-text".into(),
    }
}

/// Un hôte dont le document regardé se change à la main.
fn hote(d: Option<DocumentVu>) -> (PuceContexte, Rc<RefCell<Option<DocumentVu>>>) {
    let courant = Rc::new(RefCell::new(d));
    let lu = courant.clone();
    (
        PuceContexte::new(Rc::new(move || lu.borrow().clone())),
        courant,
    )
}

#[test]
fn la_puce_montre_le_document_regarde() {
    let (p, _) = hote(Some(doc("article.md", "article")));
    let v = p.vue().unwrap();
    assert_eq!(
        (v.titre.as_str(), v.icone.as_str()),
        ("article", "file-text")
    );
    assert_eq!(v.nom, "Contexte : article");
    assert_eq!(v.passage, None);
}

#[test]
fn entree_envoie_le_contexte_du_fichier_avant_la_question() {
    let (p, _) = hote(Some(doc("article.md", "article")));
    let c = p.prendre().unwrap();
    assert_eq!(
        context_preamble(&c, "Résume la note"),
        "[Context: the user is looking at the file \"article.md\" of the vault.]\n\nRésume la note"
    );
}

#[test]
fn la_selection_est_retenue_puis_consommee_a_l_envoi() {
    let (p, _) = hote(Some(doc("article.md", "article")));
    p.selection(Some("un passage choisi"));
    assert_eq!(
        p.vue().unwrap().passage.as_deref(),
        Some("« un passage choisi »")
    );
    let c = p.prendre().unwrap();
    assert_eq!(c.selection.as_deref(), Some("un passage choisi"));
    // Consommé : la question suivante part sans ce passage.
    assert_eq!(p.prendre().unwrap().selection, None);
    assert_eq!(p.vue().unwrap().passage, None);
}

#[test]
fn le_passage_est_coupe_a_quarante_caracteres_dans_la_puce() {
    let (p, _) = hote(Some(doc("a.md", "a")));
    p.selection(Some(&format!("  {}  ", "mot ".repeat(30))));
    let passage = p.vue().unwrap().passage.unwrap();
    // 40 caractères espaces compactés, puis « … ».
    assert_eq!(passage, format!("« {}… »", "mot ".repeat(10)));
}

#[test]
fn un_passage_trop_long_n_est_pas_joint() {
    let (p, _) = hote(Some(doc("a.md", "a")));
    p.selection(Some(&"x".repeat(4000)));
    assert_eq!(p.prendre().unwrap().selection, None);
    p.selection(Some(&"x".repeat(3999)));
    assert!(p.prendre().unwrap().selection.is_some());
}

#[test]
fn la_croix_retire_le_contexte_la_question_part_seule() {
    let (p, _) = hote(Some(doc("article.md", "article")));
    p.retirer();
    assert!(!p.visible());
    assert_eq!(p.vue(), None);
    assert_eq!(p.prendre(), None);
    // Et il ne revient pas avec l'onglet suivant.
    p.document_change();
    assert!(!p.visible());
}

#[test]
fn un_autre_document_efface_le_passage_retenu() {
    let (p, courant) = hote(Some(doc("a.md", "a")));
    p.selection(Some("texte"));
    *courant.borrow_mut() = Some(doc("b.md", "b"));
    p.document_change();
    let c = p.prendre().unwrap();
    assert_eq!((c.doc.as_str(), c.selection), ("b.md", None));
    // Le même document : le passage reste.
    p.selection(Some("autre"));
    p.document_change();
    assert_eq!(p.prendre().unwrap().selection.as_deref(), Some("autre"));
}

#[test]
fn sans_document_regarde_rien_a_joindre() {
    let (p, courant) = hote(None);
    p.selection(Some("x"));
    assert_eq!(p.vue(), None);
    assert_eq!(p.prendre(), None);
    *courant.borrow_mut() = Some(doc("a.md", "a"));
    p.document_change();
    assert!(p.visible());
}

#[test]
fn l_extension_du_champ_ne_s_applique_qu_a_la_barre_de_l_agent() {
    let (p, _) = hote(Some(doc("a.md", "a")));
    let e = ExtensionContexte::new(p);
    assert_eq!(e.id(), "agent-context");
    assert!(e.applies_to(&contexte("agent", ChatMode::Bar)));
    assert!(!e.applies_to(&contexte("agent", ChatMode::Panel)));
    assert!(!e.applies_to(&contexte("autre", ChatMode::Bar)));
}

#[test]
fn l_envoi_porte_le_contexte_dans_les_options() {
    let (p, _) = hote(Some(doc("article.md", "article")));
    p.selection(Some("le passage"));
    let mut h = ExtensionContexte::new(p).create(&contexte("agent", ChatMode::Bar));
    let pris = h.take().unwrap();
    assert_eq!(
        serde_json::Value::Object(pris.opts.unwrap()),
        json!({ "context": { "doc": "article.md", "title": "article", "selection": "le passage" } })
    );
    assert!(pris.parts.is_empty());
}

// ── agent-permissions : la carte ──────────────────────────────────────────────────────────────

fn entree(v: serde_json::Value) -> fragment_agent::contrat::Entree {
    v.as_object().cloned().unwrap_or_default()
}

fn donnees(
    outil: &str,
    famille: Option<ToolFamily>,
    input: serde_json::Value,
    regle: Option<(ToolFamily, Option<&str>)>,
) -> PermissionCardData {
    PermissionCardData {
        id: "p1".into(),
        request: PermissionRequest {
            tool: outil.into(),
            family: famille,
            input: entree(input),
            title: None,
            reason: None,
            plan: None,
        },
        state: EtatCarte::Pending,
        rule: regle.map(|(family, p)| PermanentRule {
            family,
            pattern: p.map(str::to_owned),
        }),
        note: None,
    }
}

fn bash() -> PermissionCardData {
    donnees(
        "Bash",
        Some(ToolFamily::Shell),
        json!({ "command": "git status" }),
        Some((ToolFamily::Shell, Some("git status*"))),
    )
}

fn plan() -> PermissionCardData {
    let mut d = donnees("ExitPlanMode", None, json!({}), None);
    d.request.plan = Some("# Mon plan\n\n1. Lire\n2. Écrire".into());
    d
}

fn libelles(c: &CarteModele) -> Vec<&'static str> {
    c.boutons().into_iter().map(|b| b.libelle).collect()
}

#[test]
fn assistant_une_carte_dans_la_reponse_dit_ce_que_veut_l_agent() {
    let c = CarteModele::new(bash());
    assert!(c.en_attente() && !c.est_plan());
    assert_eq!(c.description.verb, "Veut exécuter une commande");
    assert_eq!(c.detail(), Some("git status"));
    assert_eq!(c.ligne_repondue(), None);
    assert_eq!(
        libelles(&c),
        [
            "Autoriser une fois",
            "Toujours dans cette conversation",
            "Toujours dans ce coffre",
            "Refuser"
        ]
    );
    let variantes: Vec<_> = c.boutons().into_iter().map(|b| b.variante).collect();
    assert_eq!(
        variantes,
        [
            VarianteBouton::Principal,
            VarianteBouton::Normal,
            VarianteBouton::Normal,
            VarianteBouton::Refus
        ]
    );
}

#[test]
fn autoriser_une_fois_l_outil_passe_la_carte_se_replie() {
    let c = CarteModele::new(bash());
    let d = c.decision(ActionCarte::AutoriserUneFois).unwrap();
    assert_eq!(d, PermissionDecision::Allow { remember: None });
    let suite = CarteModele::new(repondre(&c.data, &d));
    assert_eq!(suite.data.state, EtatCarte::Allowed);
    let l = suite.ligne_repondue().unwrap();
    assert_eq!(l.texte, "Autorisé : git status");
    assert!(suite.boutons().is_empty());
}

#[test]
fn toujours_dans_cette_conversation_la_decision_retient_la_regle() {
    let c = CarteModele::new(bash());
    let d = c.decision(ActionCarte::ToujoursConversation).unwrap();
    let PermissionDecision::Allow { remember: Some(r) } = &d else {
        panic!("une autorisation qui retient la règle");
    };
    assert_eq!(r.scope, RememberScope::Conversation);
    assert_eq!(r.rule.family, ToolFamily::Shell);
    assert_eq!(r.rule.pattern.as_deref(), Some("git status*"));
    let l = CarteModele::new(repondre(&c.data, &d))
        .ligne_repondue()
        .unwrap();
    assert_eq!(
        l.texte,
        "Autorisé : git status — toujours dans cette conversation"
    );
}

#[test]
fn toujours_dans_ce_coffre_la_portee_est_le_coffre() {
    let c = CarteModele::new(donnees(
        "WebFetch",
        Some(ToolFamily::WebFetch),
        json!({ "url": "https://fr.wikipedia.org/wiki/X" }),
        Some((ToolFamily::WebFetch, Some("fr.wikipedia.org"))),
    ));
    let d = c.decision(ActionCarte::ToujoursCoffre).unwrap();
    let PermissionDecision::Allow { remember: Some(r) } = &d else {
        panic!("une autorisation qui retient la règle");
    };
    assert_eq!(r.scope, RememberScope::Vault);
    assert_eq!(r.rule.pattern.as_deref(), Some("fr.wikipedia.org"));
    assert_eq!(c.description.verb, "Veut lire une page web");
    assert_eq!(
        CarteModele::new(repondre(&c.data, &d))
            .ligne_repondue()
            .unwrap()
            .texte,
        "Autorisé : https://fr.wikipedia.org/wiki/X — toujours dans ce coffre"
    );
}

#[test]
fn sans_regle_a_retenir_pas_de_boutons_toujours() {
    let c = CarteModele::new(donnees(
        "Bash",
        Some(ToolFamily::Shell),
        json!({ "command": "ls" }),
        None,
    ));
    assert_eq!(libelles(&c), ["Autoriser une fois", "Refuser"]);
    assert_eq!(c.decision(ActionCarte::ToujoursCoffre), None);
    assert_eq!(c.decision(ActionCarte::ToujoursConversation), None);
}

#[test]
fn refuser_avec_un_pourquoi_l_agent_recoit_le_message() {
    let mut c = CarteModele::new(donnees(
        "Write",
        Some(ToolFamily::Write),
        json!({ "file_path": "Cours/b.md", "content": "x" }),
        None,
    ));
    assert_eq!(c.description.verb, "Veut créer « b.md »");
    assert_eq!(
        c.invite_pourquoi(),
        "Pourquoi (facultatif, l’agent le lira)"
    );
    c.pourquoi = "  pas maintenant ".into();
    let d = c.decision(ActionCarte::Refuser).unwrap();
    assert_eq!(
        d,
        PermissionDecision::Deny {
            message: Some("pas maintenant".into())
        }
    );
    let l = CarteModele::new(repondre(&c.data, &d))
        .ligne_repondue()
        .unwrap();
    assert!(l.texte.starts_with("Refusé"));
    assert_eq!(l.texte, "Refusé : Cours/b.md — pas maintenant");
}

#[test]
fn refuser_sans_pourquoi_n_envoie_aucun_message() {
    let c = CarteModele::new(bash());
    assert_eq!(
        c.decision(ActionCarte::Refuser),
        Some(PermissionDecision::Deny { message: None })
    );
}

#[test]
fn mode_plan_la_carte_du_plan_lancer_ou_modifier() {
    let mut c = CarteModele::new(plan());
    assert!(c.est_plan());
    assert_eq!(c.description.verb, "Propose un plan");
    assert_eq!(c.detail(), None);
    assert!(c.plan().unwrap().contains("Mon plan"));
    assert_eq!(c.invite_pourquoi(), "Ce qu’il faut changer…");
    assert_eq!(libelles(&c), ["Lancer", "Modifier"]);
    assert_eq!(
        c.decision(ActionCarte::Lancer),
        Some(PermissionDecision::Allow { remember: None })
    );
    // Modifier sans commentaire : l'agent lit la consigne par défaut.
    assert_eq!(
        c.decision(ActionCarte::Modifier),
        Some(PermissionDecision::Deny {
            message: Some("Modifie le plan avant de le lancer.".into())
        })
    );
    c.pourquoi = "ajoute les tests".into();
    assert_eq!(
        c.decision(ActionCarte::Modifier),
        Some(PermissionDecision::Deny {
            message: Some("ajoute les tests".into())
        })
    );
}

#[test]
fn arreter_un_tour_qui_attend_une_reponse_la_carte_est_refusee_et_repliee() {
    let c = CarteModele::new(donnees(
        "Bash",
        Some(ToolFamily::Shell),
        json!({ "command": "rm -rf build" }),
        None,
    ));
    let arretee = CarteModele::new(refus_a_larret(&c.data));
    assert_eq!(arretee.data.state, EtatCarte::Denied);
    assert_eq!(arretee.data.note.as_deref(), Some(MESSAGE_TOUR_ARRETE));
    assert_eq!(
        arretee.ligne_repondue().unwrap().texte,
        "Refusé : rm -rf build — Tour arrêté."
    );
    assert!(arretee.boutons().is_empty());
}

#[test]
fn la_raison_du_moteur_est_montree_et_le_detail_vide_n_a_pas_de_bloc() {
    let mut d = donnees("Skill", None, json!({}), None);
    d.request.reason = Some("parce que".into());
    let c = CarteModele::new(d);
    assert_eq!(c.raison(), Some("parce que"));
    let sans = CarteModele::new(donnees("Bash", Some(ToolFamily::Shell), json!({}), None));
    assert_eq!(sans.raison(), None);
    assert_eq!(sans.detail(), None);
}

#[test]
fn une_nouvelle_version_du_part_sans_changement_garde_le_pourquoi_tape() {
    let mut c = CarteModele::new(bash());
    c.pourquoi = "en cours".into();
    assert!(!c.mettre_a_jour(bash()));
    assert_eq!(c.pourquoi, "en cours");
    // Répondue : la carte est reconstruite, le champ repart de zéro.
    let mut suite = bash();
    suite.state = EtatCarte::Allowed;
    assert!(c.mettre_a_jour(suite));
    assert_eq!(c.pourquoi, "");
    assert!(!c.en_attente());
}

#[test]
fn le_part_agent_permission_se_lit_de_ses_donnees() {
    let part = ChatPart {
        kind: PART_PERMISSION.into(),
        id: Some("p1".into()),
        text: None,
        data: Some(serde_json::to_value(bash()).unwrap()),
    };
    let c = CarteModele::depuis_part(&part).unwrap();
    assert_eq!(c.data.id, "p1");
    assert_eq!(c.signature(), (EtatCarte::Pending, "p1", None));
    // Des données illisibles : pas de carte.
    let mauvais = ChatPart {
        data: Some(json!({ "x": 1 })),
        ..part
    };
    assert_eq!(CarteModele::depuis_part(&mauvais), None);
}
