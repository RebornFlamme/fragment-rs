//! Tests headless de la palette et des modales : le plugin (épingles, récentes, persistances), la
//! portée clavier de la modale, et la chaîne complète sur la couche (ouverture, filtrage, flèches,
//! Entrée, Échap) dans une fenêtre de test GPUI.
//!
//! Noms d'après `CommandPalettePlugin.test.ts` et `command-palette.spec.ts` de Fragment ; les
//! données (commandes « Zebra alpha/beta ») sont réécrites.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use fragment_core::{
    Command, CommandHost, CommandManager, EditorContext, FuzzyMatch, HotkeyManager, InputEvent,
    KeyEvent, Keymap, Modifier, Modifiers,
};
use fragment_ui::modal::{CoucheModale, Fermeture, PorteeModale};
use fragment_ui::palette::{OptionsPalette, PalettePlugin, SourcePalette, StockagePalette};
use fragment_ui::suggest::{ListeSuggestions, filtrer_flou};
use gpui_kit::{Entity, TestAppContext, VisualTestContext};

// ── Les doublures ─────────────────────────────────────────────────────────────

/// Un stockage qui compte quelle porte est écrite, et quand.
#[derive(Default)]
struct Espion {
    recents: RefCell<Vec<Vec<String>>>,
    options: RefCell<Vec<OptionsPalette>>,
}

impl StockagePalette for Espion {
    fn sauver_recents(&self, recents: &[String]) {
        self.recents.borrow_mut().push(recents.to_vec());
    }
    fn sauver_options(&self, options: &OptionsPalette) {
        self.options.borrow_mut().push(options.clone());
    }
}

/// Une commande témoin avec son compteur d'exécutions.
fn temoin(id: &str, nom: &str) -> (Rc<Command>, Rc<Cell<u32>>) {
    let compteur = Rc::new(Cell::new(0));
    let c = compteur.clone();
    let commande = Command::new(id, nom).with_callback(move || c.set(c.get() + 1));
    (Rc::new(commande), compteur)
}

fn source(commandes: Vec<Rc<Command>>) -> SourcePalette {
    SourcePalette {
        commandes: Rc::new(move || commandes.clone()),
        raccourcis: Rc::new(|_| Vec::new()),
        executer: Rc::new(|c, _| match &c.callback {
            Some(cb) => {
                cb();
                true
            }
            None => false,
        }),
    }
}

fn palette(commandes: Vec<Rc<Command>>) -> (Rc<PalettePlugin>, Rc<Espion>) {
    let espion = Rc::new(Espion::default());
    let plugin = PalettePlugin::new(source(commandes), espion.clone());
    (Rc::new(plugin), espion)
}

fn noms(plugin: &PalettePlugin) -> Vec<String> {
    plugin.commandes().iter().map(|c| c.name.clone()).collect()
}

// ── CommandPalettePlugin.test.ts : chaque persistance écrit sa propre donnée ──

#[test]
fn executer_une_commande_n_ecrit_que_les_recents_pas_data_json() {
    let (go, compteur) = temoin("z:go", "Go");
    let (plugin, espion) = palette(vec![go.clone()]);

    plugin.executer(&go, None);

    assert_eq!(compteur.get(), 1);
    assert_eq!(*espion.recents.borrow(), vec![vec!["z:go".to_owned()]]);
    // Le cœur d'A28 : les épinglés n'ont pas bougé, donc aucune écriture des préférences.
    assert!(espion.options.borrow().is_empty());
}

#[test]
fn basculer_un_epinglage_ecrit_data_json_et_lui_seul_declenche_cette_ecriture() {
    let (go, _) = temoin("z:go", "Go");
    let (plugin, espion) = palette(vec![go]);

    plugin.basculer_epingle("z:go");
    assert!(plugin.est_epinglee("z:go"));
    assert_eq!(
        *espion.options.borrow(),
        vec![OptionsPalette {
            epingles: vec!["z:go".to_owned()]
        }]
    );

    plugin.basculer_epingle("z:go"); // re-bascule : désépingle
    assert!(!plugin.est_epinglee("z:go"));
    assert_eq!(espion.options.borrow().len(), 2);
    assert!(espion.recents.borrow().is_empty());
}

// ── command-palette.spec.ts : l'ordre et la disponibilité ─────────────────────

#[test]
fn une_commande_executee_remonte_en_tete_a_l_ouverture_suivante() {
    let (alpha, _) = temoin("t:alpha", "Alpha");
    let (zz, _) = temoin("t:zz", "Zz tout en bas");
    let (plugin, _) = palette(vec![zz.clone(), alpha]);

    // Sans requête, l'ordre est alphabétique : « Zz » est loin.
    assert_eq!(noms(&plugin), vec!["Alpha", "Zz tout en bas"]);
    plugin.executer(&zz, None);
    // Récente : elle passe devant, avant toute frappe.
    assert_eq!(noms(&plugin), vec!["Zz tout en bas", "Alpha"]);
}

#[test]
fn les_epinglees_passent_avant_les_recentes_dans_l_ordre_choisi() {
    let (a, _) = temoin("t:a", "A");
    let (b, _) = temoin("t:b", "B");
    let (c, _) = temoin("t:c", "C");
    let (plugin, _) = palette(vec![a, b.clone(), c]);
    plugin.basculer_epingle("t:c");
    plugin.basculer_epingle("t:a");
    plugin.executer(&b, None);
    assert_eq!(noms(&plugin), vec!["C", "A", "B"]);
}

#[test]
fn les_recentes_sont_plafonnees() {
    let (a, _) = temoin("t:a", "A");
    let (plugin, _) = palette(vec![a.clone()]);
    for i in 0..150 {
        let (x, _) = temoin(&format!("t:{i}"), "X");
        plugin.executer(&x, None);
    }
    plugin.executer(&a, None);
    assert_eq!(plugin.recents().len(), 100);
    assert_eq!(plugin.recents()[0], "t:a");
}

/// Un hôte sans éditeur ni raccourcis.
struct Hote;

impl CommandHost for Hote {
    fn set_last_event(&self, _evt: Option<InputEvent>) {}
    fn active_editor(&self) -> Option<EditorContext> {
        None
    }
    fn hotkey_manager(&self) -> Option<Rc<HotkeyManager>> {
        None
    }
}

#[test]
fn une_commande_d_editeur_disparait_quand_aucun_editeur_n_a_le_focus() {
    let hote: Rc<dyn CommandHost> = Rc::new(Hote);
    let gestionnaire = CommandManager::new(Rc::downgrade(&hote));
    let _ = gestionnaire.add_command(
        Command::new("temoin:gras", "Zebra gras").with_editor_callback(|_| {}),
        "t",
    );
    let _ = gestionnaire.add_command(
        Command::new("temoin:saine", "Zebra saine").with_callback(|| {}),
        "t",
    );
    let source = SourcePalette::depuis_gestionnaires(gestionnaire, None);
    let plugin = PalettePlugin::new(source, Rc::new(Espion::default()));
    assert_eq!(noms(&plugin), vec!["Zebra saine"]);
}

#[test]
fn une_commande_dont_check_callback_leve_est_masquee_la_palette_survit() {
    let hote: Rc<dyn CommandHost> = Rc::new(Hote);
    let gestionnaire = CommandManager::new(Rc::downgrade(&hote));
    let _ = gestionnaire.add_command(
        Command::new("temoin:casse", "Zebra cassée").with_check_callback(|_| panic!("boum")),
        "t",
    );
    let _ = gestionnaire.add_command(
        Command::new("temoin:saine", "Zebra saine").with_callback(|| {}),
        "t",
    );
    let source = SourcePalette::depuis_gestionnaires(gestionnaire, None);
    let plugin = PalettePlugin::new(source, Rc::new(Espion::default()));
    assert_eq!(noms(&plugin), vec!["Zebra saine"]);
}

fn id_selectionne(l: &ListeSuggestions<FuzzyMatch<Rc<Command>>>) -> Option<String> {
    l.selectionnee().map(|m| m.item.id.clone())
}

#[test]
fn le_filtre_isole_les_temoins_et_les_fleches_naviguent() {
    let (alpha, _) = temoin("temoin:alpha", "Zebra alpha");
    let (beta, _) = temoin("temoin:beta", "Zebra beta");
    let (autre, _) = temoin("temoin:autre", "Autre chose");
    let (plugin, _) = palette(vec![beta, autre, alpha]);

    let trouvees = filtrer_flou("zebra", plugin.commandes(), |c| c.name.clone());
    let titres: Vec<_> = trouvees.iter().map(|m| m.item.name.as_str()).collect();
    assert_eq!(titres, vec!["Zebra alpha", "Zebra beta"]);

    let mut liste = ListeSuggestions::new(100);
    liste.remplacer(trouvees);
    assert_eq!(id_selectionne(&liste).as_deref(), Some("temoin:alpha"));
    liste.deplacer(1);
    assert_eq!(id_selectionne(&liste).as_deref(), Some("temoin:beta"));
    liste.deplacer(-1);
    assert_eq!(id_selectionne(&liste).as_deref(), Some("temoin:alpha"));
}

// ── La portée de la modale ────────────────────────────────────────────────────

#[test]
fn echap_ferme_et_est_consomme_les_raccourcis_globaux_remontent_puis_la_pile_se_rend() {
    let keymap = Keymap::new();
    let global = Rc::new(Cell::new(0));
    let g = global.clone();
    keymap
        .root_scope()
        .register(Some(&[Modifier::Ctrl]), Some("p"), move |_, _| {
            g.set(g.get() + 1);
            Some(false)
        });

    let fermeture = Fermeture::default();
    let portee = PorteeModale::ouvrir(&keymap, &fermeture);
    assert!(keymap.scope() == *portee.scope(), "la portée est au sommet");

    // Un raccourci global non reconnu par la modale remonte : la modale ne le gèle pas.
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::default()
    };
    assert_eq!(
        keymap.handle_key_event(&KeyEvent::new("p").with_mods(ctrl)),
        Some(false)
    );
    assert_eq!(global.get(), 1);
    assert!(!fermeture.demandee());

    // Échap ferme et est consommé.
    assert_eq!(
        keymap.handle_key_event(&KeyEvent::new("Escape")),
        Some(false)
    );
    assert!(fermeture.demandee());

    // Portée dépilée : la racine redevient le sommet.
    portee.fermer();
    assert!(keymap.scope() == *keymap.root_scope());
}

// ── La chaîne complète, dans une fenêtre de test ──────────────────────────────

fn peindre(vcx: &mut VisualTestContext) {
    // Deux images : la première ouvre la modale et lui donne le focus, la seconde l'inscrit dans
    // l'arbre de distribution des touches.
    for _ in 0..2 {
        vcx.update(|w, cx| {
            w.draw(cx).clear(cx);
        });
    }
}

fn fenetre(cx: &mut TestAppContext) -> (Entity<CoucheModale>, &mut VisualTestContext) {
    let (couche, vcx) = cx.add_window_view(|_, _| CoucheModale::new());
    peindre(vcx);
    (couche, vcx)
}

#[test]
fn ouvrir_taper_naviguer_entree_execute_et_la_palette_se_ferme() {
    let mut cx = TestAppContext::single();
    let (couche, vcx) = fenetre(&mut cx);
    let (alpha, n_alpha) = temoin("temoin:alpha", "Zebra alpha");
    let (beta, n_beta) = temoin("temoin:beta", "Zebra beta");
    let (plugin, _) = palette(vec![alpha, beta]);

    plugin.ouvrir();
    peindre(vcx);
    assert!(couche.read_with(&*vcx, |c, _| c.est_ouverte()));

    vcx.simulate_input("zebra");
    vcx.simulate_keystrokes("down");
    peindre(vcx);
    vcx.simulate_keystrokes("enter");
    peindre(vcx);

    assert!(
        !couche.read_with(&*vcx, |c, _| c.est_ouverte()),
        "la palette se ferme"
    );
    assert_eq!(
        (n_alpha.get(), n_beta.get()),
        (0, 1),
        "↓ a choisi la seconde"
    );
}

#[test]
fn echap_ferme_sans_rien_executer_et_une_seconde_ouverture_est_ignoree() {
    let mut cx = TestAppContext::single();
    let (couche, vcx) = fenetre(&mut cx);
    let (x, n) = temoin("temoin:x", "Témoin X");
    let (plugin, _) = palette(vec![x]);

    plugin.ouvrir();
    peindre(vcx);
    // Ouvrir encore la palette quand elle est ouverte ne fait rien de visible.
    plugin.ouvrir();
    peindre(vcx);
    assert!(couche.read_with(&*vcx, |c, _| c.est_ouverte()));

    vcx.simulate_keystrokes("escape");
    peindre(vcx);
    assert!(!couche.read_with(&*vcx, |c, _| c.est_ouverte()));
    assert_eq!(n.get(), 0);

    // Rouvrir marche.
    plugin.ouvrir();
    peindre(vcx);
    assert!(couche.read_with(&*vcx, |c, _| c.est_ouverte()));
}
