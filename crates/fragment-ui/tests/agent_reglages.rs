// Port de app/e2e/agent-permissions.spec.ts (test « les RÉGLAGES déclaratifs » et « Toujours dans
// ce coffre ») et de app/e2e/agent-trace.spec.ts (l'orbe choisie dans les réglages), vus depuis
// l'onglet Agent : `AgentSettingTab.getSettingDefinitions` (AgentPlugin.ts).
//
//! Tests headless de l'onglet Agent : on lit les définitions déclaratives que la fenêtre rend, on
//! déclenche leurs callbacks, et on relit le `data.json` du service.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use fragment_agent::contrat::{
    AgentPolicy, FolderAccess, PermanentRule, PermissionMode, ToolAccess, ToolFamily, WebDomains,
};
use fragment_agent::policy_model::open_vault_policy;
use fragment_agent::service::reglages::{Reglages, StockageMemoire, StockageReglages};
use fragment_agent::trace_model::{OrbState, TraceBuilder};
use fragment_core::{
    SecretError, SecretStorage, SettingControl, SettingDefinition, SettingTreeNode,
};
use fragment_ui::agent_reglages::generales::CLE_SECRET_API;
use fragment_ui::agent_reglages::{
    ContexteAgent, ListeDossiers, choix_orbe, definitions_agent, onglet_agent,
};
use fragment_ui::agent_trace::{EtatsTrace, OrbeChoix, Poignee};
use serde_json::{Value, json};

#[derive(Default)]
struct Secrets(RefCell<BTreeMap<String, String>>);

impl SecretStorage for Secrets {
    fn is_available(&self) -> bool {
        true
    }
    fn list(&self) -> Vec<String> {
        self.0.borrow().keys().cloned().collect()
    }
    fn get(&self, key: &str) -> Option<String> {
        self.0.borrow().get(key).cloned()
    }
    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.0.borrow_mut().insert(key.into(), value.into());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.0.borrow_mut().remove(key);
        Ok(())
    }
}

struct Banc {
    stockage: Rc<StockageMemoire>,
    secrets: Rc<Secrets>,
    ctx: ContexteAgent,
}

fn banc() -> Banc {
    let stockage = Rc::new(StockageMemoire::default());
    let secrets = Rc::new(Secrets::default());
    let dossiers: ListeDossiers = Rc::new(|parent| match parent {
        "" => vec!["Privé".into(), "Cours".into(), ".fragment".into()],
        "Cours" => vec!["Cours/L1".into()],
        _ => Vec::new(),
    });
    let reglages = Rc::new(Reglages::new(stockage.clone()));
    let ctx = ContexteAgent::new(reglages, secrets.clone())
        .avec_plateforme("win32")
        .avec_compte(Some("julie".into()))
        .avec_dossiers(dossiers);
    Banc {
        stockage,
        secrets,
        ctx,
    }
}

impl Banc {
    fn defs(&self) -> Vec<SettingDefinition> {
        definitions_agent(&self.ctx)
    }

    /// Une politique écrite d'avance dans le `data.json`.
    fn avec_politique(&self, p: &AgentPolicy) {
        self.ctx.reglages.poser_politique(p).unwrap();
    }

    fn politique(&self) -> AgentPolicy {
        self.ctx.reglages.recharger();
        self.ctx.reglages.politique()
    }

    fn fichier(&self) -> serde_json::Map<String, Value> {
        match self.stockage.lire() {
            Some(Value::Object(o)) => o,
            _ => serde_json::Map::new(),
        }
    }

    fn controle(&self, nom: &str) -> SettingControl {
        trouver(&self.defs(), &|d| match d {
            SettingDefinition::Control(c) if c.name == nom => Some(c.control.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("ligne « {nom} »"))
    }

    fn controle_id(&self, id: &str) -> SettingControl {
        trouver(&self.defs(), &|d| match d {
            SettingDefinition::Control(c) if c.base.id.as_deref() == Some(id) => {
                Some(c.control.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("ligne #{id}"))
    }

    fn liste(&self, nom: &str) -> fragment_core::SettingDefinitionList {
        self.defs()
            .into_iter()
            .find_map(|d| match d {
                SettingDefinition::List(l) if l.name == nom => Some(l),
                _ => None,
            })
            .unwrap_or_else(|| panic!("liste « {nom} »"))
    }
}

/// Cherche dans les définitions, groupes compris.
fn trouver<T>(
    defs: &[SettingDefinition],
    f: &dyn Fn(&SettingDefinition) -> Option<T>,
) -> Option<T> {
    for d in defs {
        if let Some(v) = f(d) {
            return Some(v);
        }
        if let SettingDefinition::Group(g) = d
            && let Some(v) = trouver(&g.items, f)
        {
            return Some(v);
        }
    }
    None
}

fn liste_de(c: SettingControl) -> fragment_core::SettingDropdownControl {
    match c {
        SettingControl::Dropdown(d) => d,
        _ => panic!("pas une liste déroulante"),
    }
}

fn texte_de(c: SettingControl) -> fragment_core::SettingTextControl {
    match c {
        SettingControl::Text(t) => t,
        _ => panic!("pas un champ texte"),
    }
}

fn choisir(c: SettingControl, v: &str) {
    (liste_de(c).on_change.expect("on_change"))(v.into());
}

fn action(b: &Banc, id: &str) -> Rc<dyn Fn()> {
    trouver(&b.defs(), &|d| match d {
        SettingDefinition::Action(a) if a.base.id.as_deref() == Some(id) => {
            Some(a.on_click.clone())
        }
        _ => None,
    })
    .unwrap_or_else(|| panic!("action #{id}"))
}

fn messages_vides(b: &Banc) -> Vec<String> {
    let out = RefCell::new(Vec::new());
    let _ = trouver::<()>(&b.defs(), &|d| {
        if let SettingDefinition::Empty(e) = d {
            out.borrow_mut().push(e.message.clone());
        }
        None
    });
    out.into_inner()
}

fn noeuds(b: &Banc) -> Vec<SettingTreeNode> {
    b.defs()
        .into_iter()
        .find_map(|d| match d {
            SettingDefinition::Tree(t) => Some((t.nodes)()),
            _ => None,
        })
        .expect("arbre des dossiers")
}

fn cliquer_retirer(item: &fragment_core::SettingGroupItem, rang: usize) {
    let controls = match item {
        fragment_core::SettingGroupItem::Item(i) => &i.controls,
        fragment_core::SettingGroupItem::Control(_) => panic!("pas un item"),
    };
    match &controls[rang] {
        SettingControl::ExtraButton(b) => (b.on_click)(),
        _ => panic!("pas un bouton"),
    }
}

// ── sans réglage ─────────────────────────────────────────────────────────────

#[test]
fn sans_reglage_les_defauts_sont_coffre_ouvert_et_le_modele_par_defaut() {
    let b = banc();
    let mode = liste_de(b.controle_id("agent-mode"));
    assert_eq!(mode.value, "full");
    let valeurs: Vec<&str> = mode.options.iter().map(|(v, _)| v.as_str()).collect();
    assert_eq!(valeurs, ["full", "ask", "acceptEdits", "plan"]);
    assert_eq!(liste_de(b.controle_id("agent-tool-shell")).value, "allow");
    assert_eq!(
        liste_de(b.controle_id("agent-tool-web-fetch")).value,
        "deny"
    );
    assert_eq!(texte_de(b.controle("Modèle")).value, "claude-opus-5-5");
}

// ── les réglages déclaratifs (agent-permissions.spec.ts) ────────────────────

#[test]
fn les_reglages_declaratifs_avertissements_prereglages_matrice_arbre_des_dossiers() {
    let b = banc();
    let mut p = open_vault_policy();
    p.folders.insert("Privé".into(), FolderAccess::Hidden);
    b.avec_politique(&p);

    // L'avertissement : le terminal est permis alors que des dossiers sont masqués.
    assert!(
        messages_vides(&b)
            .iter()
            .any(|m| m.contains("Le terminal est permis"))
    );
    assert_eq!(liste_de(b.controle_id("agent-tool-shell")).value, "allow");

    // Un préréglage remplit la matrice et le mode.
    (action(&b, "agent-preset-strict"))();
    assert_eq!(liste_de(b.controle_id("agent-tool-shell")).value, "deny");
    let apres = b.politique();
    assert_eq!(apres.mode, PermissionMode::Ask);
    // Les dossiers de l'utilisateur restent.
    assert_eq!(apres.folders.get("Privé"), Some(&FolderAccess::Hidden));

    // L'arbre : les dossiers du coffre (pas les cachés), avec l'accès de « Privé ».
    let n = noeuds(&b);
    let ids: Vec<&str> = n.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(ids, ["Cours", "Privé"]);
    let prive = n.iter().find(|x| x.id == "Privé").unwrap();
    assert_eq!(liste_de(prive.control.clone().unwrap()).value, "hidden");
    let cours = n.iter().find(|x| x.id == "Cours").unwrap();
    assert_eq!(liste_de(cours.control.clone().unwrap()).value, "inherit");
    // Les enfants se demandent au dépli.
    let enfants = (cours.children.clone().unwrap())();
    assert_eq!(enfants[0].id, "Cours/L1");
    assert_eq!(enfants[0].label, "L1");
}

#[test]
fn les_prereglages_assistant_et_ouvert_posent_leur_mode_et_leur_matrice() {
    let b = banc();
    (action(&b, "agent-preset-assistant"))();
    let p = b.politique();
    assert_eq!(p.mode, PermissionMode::Ask);
    assert_eq!(p.tools[&ToolFamily::Shell], ToolAccess::Ask);
    assert_eq!(p.tools[&ToolFamily::Read], ToolAccess::Allow);
    (action(&b, "agent-preset-open"))();
    let p = b.politique();
    assert_eq!(p.mode, PermissionMode::Full);
    assert_eq!(p.tools[&ToolFamily::Shell], ToolAccess::Allow);
}

#[test]
fn le_mode_et_un_acces_d_outil_s_ecrivent_et_se_relisent() {
    let b = banc();
    choisir(b.controle_id("agent-mode"), "acceptEdits");
    choisir(b.controle_id("agent-tool-write"), "ask");
    let p = b.politique();
    assert_eq!(p.mode, PermissionMode::AcceptEdits);
    assert_eq!(p.tools[&ToolFamily::Write], ToolAccess::Ask);
    assert_eq!(liste_de(b.controle_id("agent-mode")).value, "acceptEdits");
    assert_eq!(liste_de(b.controle_id("agent-tool-write")).value, "ask");
}

#[test]
fn l_acces_d_un_dossier_ecrit_et_inherit_le_retire() {
    let b = banc();
    let cours = noeuds(&b).into_iter().find(|x| x.id == "Cours").unwrap();
    choisir(cours.control.clone().unwrap(), "read");
    assert_eq!(
        b.politique().folders.get("Cours"),
        Some(&FolderAccess::Read)
    );
    let cours = noeuds(&b).into_iter().find(|x| x.id == "Cours").unwrap();
    choisir(cours.control.unwrap(), "inherit");
    assert!(b.politique().folders.is_empty());
}

// ── les listes (commandes, domaines, hors coffre) ───────────────────────────

#[test]
fn une_commande_permise_s_ajoute_par_son_brouillon_et_se_retire() {
    let b = banc();
    let liste = b.liste("Commandes permises (même si le terminal est « Demander »)");
    let ajout = liste.items.last().unwrap().clone();
    let fragment_core::SettingGroupItem::Item(ajout) = ajout else {
        panic!("la ligne d'ajout est un item")
    };
    let (champ, bouton) = (&ajout.controls[0], &ajout.controls[1]);
    let SettingControl::Text(champ) = champ else {
        panic!("champ")
    };
    let SettingControl::ExtraButton(bouton) = bouton else {
        panic!("bouton")
    };
    // Un brouillon vide n'ajoute rien.
    (bouton.on_click)();
    assert!(b.politique().shell_rules.is_empty());
    (champ.on_change.clone().unwrap())("  git status*  ".into());
    (bouton.on_click)();
    assert_eq!(b.politique().shell_rules, ["git status*"]);
    // Retirer.
    let liste = b.liste("Commandes permises (même si le terminal est « Demander »)");
    cliquer_retirer(&liste.items[0], 0);
    assert!(b.politique().shell_rules.is_empty());
}

#[test]
fn les_domaines_web_tous_ou_une_liste() {
    let b = banc();
    assert!(matches!(b.politique().web_domains, WebDomains::Tous));
    // Tous : pas de ligne d'ajout.
    assert_eq!(b.liste("Domaines web").items.len(), 1);
    let fragment_core::SettingGroupItem::Control(l0) = b.liste("Domaines web").items[0].clone()
    else {
        panic!("bascule")
    };
    assert_eq!(l0.base.id.as_deref(), Some("agent-web-all"));
    let SettingControl::Toggle(t) = l0.control else {
        panic!("bascule")
    };
    assert!(t.value);
    (t.on_change.unwrap())(false);
    assert_eq!(b.politique().web_domains, WebDomains::Liste(Vec::new()));

    let liste = b.liste("Domaines web");
    let fragment_core::SettingGroupItem::Item(ajout) = liste.items.last().unwrap().clone() else {
        panic!("ajout")
    };
    let (SettingControl::Text(champ), SettingControl::ExtraButton(bouton)) =
        (&ajout.controls[0], &ajout.controls[1])
    else {
        panic!("champ et bouton")
    };
    (champ.on_change.clone().unwrap())("wikipedia.org".into());
    (bouton.on_click)();
    assert_eq!(
        b.politique().web_domains,
        WebDomains::Liste(vec!["wikipedia.org".into()])
    );
    let liste = b.liste("Domaines web");
    cliquer_retirer(&liste.items[1], 0);
    assert_eq!(b.politique().web_domains, WebDomains::Liste(Vec::new()));
}

#[test]
fn un_dossier_hors_du_coffre_s_ajoute_en_lecture_change_d_acces_et_se_retire() {
    let b = banc();
    let liste = b.liste("Hors du coffre");
    let fragment_core::SettingGroupItem::Item(ajout) = liste.items.last().unwrap().clone() else {
        panic!("ajout")
    };
    let (SettingControl::Text(champ), SettingControl::ExtraButton(bouton)) =
        (&ajout.controls[0], &ajout.controls[1])
    else {
        panic!("champ et bouton")
    };
    (champ.on_change.clone().unwrap())("D:/Documents".into());
    (bouton.on_click)();
    let p = b.politique();
    assert_eq!(p.outside[0].path, "D:/Documents");
    assert_eq!(p.outside[0].access, FolderAccess::Read);

    let liste = b.liste("Hors du coffre");
    let fragment_core::SettingGroupItem::Item(ligne) = &liste.items[0] else {
        panic!("ligne")
    };
    choisir(ligne.controls[0].clone(), "write");
    assert_eq!(b.politique().outside[0].access, FolderAccess::Write);
    let liste = b.liste("Hors du coffre");
    cliquer_retirer(&liste.items[0], 1);
    assert!(b.politique().outside.is_empty());
}

// ── « Toujours dans ce coffre » ─────────────────────────────────────────────

#[test]
fn toujours_dans_ce_coffre_les_regles_retenues_se_listent_et_se_retirent() {
    let b = banc();
    // Vide : le message.
    assert!(
        messages_vides(&b)
            .iter()
            .any(|m| m.starts_with("Aucune. Une carte"))
    );
    // Une carte a retenu une règle (le service a écrit dans le même fichier).
    let mut p = open_vault_policy();
    p.permanent.push(PermanentRule {
        family: ToolFamily::WebFetch,
        pattern: Some("fr.wikipedia.org".into()),
    });
    p.permanent.push(PermanentRule {
        family: ToolFamily::Shell,
        pattern: None,
    });
    let autre = Reglages::new(b.stockage.clone());
    autre.poser_politique(&p).unwrap();

    // ★ L'onglet relit le fichier à chaque construction : la règle est là sans rien faire.
    let groupe = b
        .defs()
        .into_iter()
        .find_map(|d| match d {
            SettingDefinition::Group(g) if g.name == "Toujours autorisé dans ce coffre" => Some(g),
            _ => None,
        })
        .unwrap();
    let noms: Vec<String> = groupe
        .items
        .iter()
        .map(|i| match i {
            SettingDefinition::Item(i) => i.name.clone(),
            _ => panic!("item"),
        })
        .collect();
    assert_eq!(
        noms,
        [
            "Lire des pages web — fr.wikipedia.org",
            "Commandes du terminal — tout"
        ]
    );
    let SettingDefinition::Item(premier) = &groupe.items[0] else {
        panic!("item")
    };
    let SettingControl::ExtraButton(retirer) = &premier.controls[0] else {
        panic!("bouton")
    };
    (retirer.on_click)();
    let reste = b.politique().permanent;
    assert_eq!(reste.len(), 1);
    assert_eq!(reste[0].family, ToolFamily::Shell);
}

// ── les réglages généraux ───────────────────────────────────────────────────

#[test]
fn le_modele_se_nettoie_et_un_champ_vide_revient_au_defaut() {
    let b = banc();
    let m = texte_de(b.controle("Modèle"));
    assert_eq!(m.placeholder.as_deref(), Some("claude-opus-5-5"));
    (m.on_change.unwrap())("  claude-sonnet-5-5 ".into());
    assert_eq!(b.ctx.reglages.modele(), "claude-sonnet-5-5");
    assert_eq!(texte_de(b.controle("Modèle")).value, "claude-sonnet-5-5");
    (texte_de(b.controle("Modèle")).on_change.unwrap())("   ".into());
    assert_eq!(b.ctx.reglages.modele(), "claude-opus-5-5");
}

#[test]
fn ton_nom_vide_laisse_le_compte_et_la_cle_disparait_du_fichier() {
    let b = banc();
    let n = texte_de(b.controle("Ton nom"));
    assert_eq!(n.value, "");
    assert_eq!(n.placeholder.as_deref(), Some("Julie"));
    (n.on_change.unwrap())(" Jul ".into());
    assert_eq!(b.fichier()["userName"], json!("Jul"));
    assert_eq!(b.ctx.reglages.nom_saisi().as_deref(), Some("Jul"));
    (texte_de(b.controle("Ton nom")).on_change.unwrap())("".into());
    assert!(!b.fichier().contains_key("userName"));
}

#[test]
fn le_moteur_claude_n_ecrit_rien_un_autre_moteur_s_ecrit_et_reste_affiche() {
    let b = banc();
    assert_eq!(liste_de(b.controle("Moteur")).value, "claude");
    choisir(b.controle("Moteur"), "pi");
    assert_eq!(b.ctx.reglages.harness().as_deref(), Some("pi"));
    // Un moteur non chargé ici reste affiché par son id.
    let d = liste_de(b.controle("Moteur"));
    assert_eq!(d.value, "pi");
    assert!(d.options.iter().any(|(v, l)| v == "pi" && l == "pi"));
    choisir(b.controle("Moteur"), "claude");
    assert!(!b.fichier().contains_key("harness"));
}

#[test]
fn claude_code_automatique_ou_celui_de_fragment() {
    let b = banc();
    assert_eq!(liste_de(b.controle("Claude Code")).value, "auto");
    choisir(b.controle("Claude Code"), "fragment");
    assert_eq!(b.ctx.reglages.moteur().as_deref(), Some("fragment"));
    choisir(b.controle("Claude Code"), "auto");
    assert!(!b.fichier().contains_key("engine"));
}

#[test]
fn la_barre_de_l_agent_replie_par_defaut_ouverte_s_ecrit() {
    let b = banc();
    assert_eq!(liste_de(b.controle("Barre de l’agent")).value, "collapsed");
    choisir(b.controle("Barre de l’agent"), "open");
    assert_eq!(b.fichier()["barStart"], json!("open"));
    choisir(b.controle("Barre de l’agent"), "collapsed");
    assert!(!b.fichier().contains_key("barStart"));
}

#[test]
fn la_cle_api_s_ecrit_au_stockage_des_secrets_nettoyee_et_pas_au_data_json() {
    let b = banc();
    let SettingControl::Secret(s) = b.controle("Clé API Anthropic") else {
        panic!("secret")
    };
    assert_eq!(s.key, CLE_SECRET_API);
    (s.on_change.unwrap())("  sk-ant-test  ".into());
    assert_eq!(
        b.secrets.get(CLE_SECRET_API).as_deref(),
        Some("sk-ant-test")
    );
    assert!(!format!("{:?}", b.fichier()).contains("sk-ant"));
}

// ── l'orbe choisie, relue par la trace ──────────────────────────────────────

#[test]
fn l_orbe_choisie_dans_les_reglages_remplace_l_automatique() {
    let b = banc();
    let d = liste_de(b.controle("Orbe"));
    assert_eq!(d.value, "auto");
    let valeurs: Vec<&str> = d.options.iter().map(|(v, _)| v.as_str()).collect();
    assert_eq!(valeurs.len(), 10);
    assert_eq!(valeurs[0], "auto");
    assert_eq!(d.options[6].1, "Tresse");
    assert_eq!(choix_orbe(&b.ctx.reglages), OrbeChoix::Auto);

    choisir(b.controle("Orbe"), "weaving");
    assert_eq!(b.fichier()["orb"], json!("weaving"));
    assert_eq!(liste_de(b.controle("Orbe")).value, "weaving");
    let choix = choix_orbe(&b.ctx.reglages);
    assert_eq!(choix, OrbeChoix::Etat(OrbState::Weaving));

    // La trace d'un tour en cours lit ce choix : l'orbe ne suit plus l'étape.
    let builder = TraceBuilder::new(0);
    let mut etats = EtatsTrace::new();
    let mut poignee = Poignee::new(etats.pour("m1"), builder.snapshot(), choix, false, 0);
    assert_eq!(poignee.vue(0).orbe, Some(OrbState::Weaving));

    // Revenir à « Automatique » retire la clé et relâche l'orbe.
    choisir(b.controle("Orbe"), "auto");
    assert!(!b.fichier().contains_key("orb"));
    let choix = choix_orbe(&b.ctx.reglages);
    assert_eq!(choix, OrbeChoix::Auto);
    poignee.definir_orbe(choix, 10);
    assert_eq!(poignee.vue(10).orbe, Some(OrbState::Solving));
}

// ── l'onglet et le fichier ──────────────────────────────────────────────────

#[test]
fn les_cles_des_autres_taches_survivent_a_une_sauvegarde_de_l_onglet() {
    let b = banc();
    b.ctx
        .reglages
        .poser("quick", json!({ "lasso": false }))
        .unwrap();
    choisir(b.controle_id("agent-mode"), "plan");
    assert_eq!(b.fichier()["quick"], json!({ "lasso": false }));
    assert_eq!(b.fichier()["policy"]["mode"], json!("plan"));
}

#[test]
fn l_onglet_agent_du_rail_porte_l_icone_et_les_definitions() {
    let b = banc();
    let onglet = onglet_agent(&b.ctx);
    assert_eq!(onglet.id, "agent");
    assert_eq!(onglet.nom, "Agent");
    assert!(onglet.definitions.is_some());
}
