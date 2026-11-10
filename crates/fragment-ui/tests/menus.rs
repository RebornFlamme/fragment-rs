//! Tests de `notice`, `menu`, `tooltip` et `toolbar` (tâche `fr-t1-menus`).
//!
//! Les cas de `tooltip.test.ts` et de `Toolbar.test.ts` portent le nom du test d'origine, en
//! snake_case sans accents ; les comportements du menu et des notices ont chacun le leur. Les
//! modèles ne touchent pas à GPUI : ces tests tournent sans fenêtre ni GPU. Chaque test s'exécute
//! sur son propre fil, donc sur ses propres magasins de surfaces flottantes.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use fragment_core::{KeyEvent, Keymap};
use fragment_ui::menu::{self, Menu};
use fragment_ui::notice::Notice;
use fragment_ui::toolbar::{BarreOutils, Enfant, Forme};
use fragment_ui::tooltip::{
    self, Cible, DELAI_DEFAUT_MS, Options, Placement, Rect, Survol, Tooltip,
};

// ── Menu ────────────────────────────────────────────────────────────────────

fn menu_a_sections() -> Menu {
    let mut m = Menu::new();
    m.add_sections(&["edition", "danger"]);
    m.add_item(|i| {
        i.set_title("Supprimer").set_section("danger");
    });
    m.add_item(|i| {
        i.set_title("Copier").set_section("edition");
    });
    m.add_item(|i| {
        i.set_title("Autre").set_section("zzz");
    });
    m.add_item(|i| {
        i.set_title("Coller").set_section("edition");
    });
    m
}

#[test]
fn menu_range_par_section_declaree_puis_les_autres_dans_l_ordre_de_1re_apparition() {
    assert_eq!(
        menu_a_sections().forme(),
        ["Copier", "Coller", "|", "Supprimer", "|", "Autre"]
    );
}

#[test]
fn menu_sans_section_garde_son_ordre_et_ses_separateurs_manuels() {
    let mut m = Menu::new();
    m.add_item(|i| {
        i.set_title("x");
    });
    m.add_separator();
    m.add_item(|i| {
        i.set_title("y");
    });
    assert_eq!(m.forme(), ["x", "|", "y"]);
}

#[test]
fn menu_en_mode_sections_retire_les_separateurs_manuels() {
    let mut m = Menu::new();
    m.add_sections(&["a", "vide", "b"]);
    m.add_item(|i| {
        i.set_title("x").set_section("a");
    });
    m.add_separator();
    m.add_item(|i| {
        i.set_title("y").set_section("b");
    });
    assert_eq!(m.forme(), ["x", "|", "y"]);
}

#[test]
fn menu_une_entree_desactivee_ne_reagit_pas_au_clic_et_le_menu_reste_ouvert() {
    let appels = Rc::new(Cell::new(0));
    let a = appels.clone();
    let mut m = Menu::new();
    m.add_item(|i| {
        i.set_title("Renommer")
            .set_disabled(true)
            .on_click(move || a.set(a.get() + 1));
    });
    m.show_at_position(10.0, 10.0, &Keymap::new());
    assert!(!menu::cliquer(0, 0));
    assert_eq!(appels.get(), 0);
    assert!(menu::menu_ouvert());
    menu::fermer_menu();
}

#[test]
fn menu_un_clic_ferme_toute_la_chaine_avant_d_executer_l_action() {
    let vu_ouvert = Rc::new(Cell::new(true));
    let v = vu_ouvert.clone();
    let mut m = Menu::new();
    m.add_item(|i| {
        i.set_title("Trier par");
        i.set_submenu().add_item(|s| {
            s.set_title("Nom")
                .on_click(move || v.set(menu::menu_ouvert()));
        });
    });
    m.show_at_position(10.0, 10.0, &Keymap::new());
    assert!(menu::survoler(0, 0));
    assert_eq!(menu::profondeur_ouverte(), 1);
    assert!(menu::cliquer(1, 0));
    assert!(!menu::menu_ouvert());
    assert!(!vu_ouvert.get(), "l'action part APRÈS la fermeture");
}

#[test]
fn menu_survoler_une_autre_entree_replie_le_sous_menu() {
    let mut m = Menu::new();
    m.add_item(|i| {
        i.set_title("Trier par");
        i.set_submenu().add_item(|s| {
            s.set_title("Nom");
        });
    });
    m.add_item(|i| {
        i.set_title("Copier");
    });
    m.show_at_position(10.0, 10.0, &Keymap::new());
    assert!(menu::survoler(0, 0));
    assert_eq!(menu::profondeur_ouverte(), 1);
    assert!(!menu::survoler(0, 0), "re-survoler ne change rien");
    assert!(menu::survoler(0, 1));
    assert_eq!(menu::profondeur_ouverte(), 0);
    menu::fermer_menu();
}

#[test]
fn menu_se_ferme_a_echap_par_la_portee_echap_et_la_depile() {
    let keymap = Keymap::new();
    let fermetures = Rc::new(Cell::new(0));
    let f = fermetures.clone();
    let mut m = Menu::new();
    m.on_hide(move || f.set(f.get() + 1));
    m.add_item(|i| {
        i.set_title("Copier");
    });
    m.show_at_position(10.0, 10.0, &keymap);
    assert!(menu::menu_ouvert());
    assert!(
        keymap.scope() != keymap.get_root_scope(),
        "la portée du menu est au sommet"
    );
    assert_eq!(
        keymap.handle_key_event(&KeyEvent::new("Escape")),
        Some(false)
    );
    assert!(!menu::menu_ouvert());
    assert_eq!(fermetures.get(), 1);
    assert!(keymap.scope() == keymap.get_root_scope(), "portée dépilée");
}

#[test]
fn menu_ouvert_masque_les_raccourcis_globaux() {
    let keymap = Keymap::new();
    let globaux = Rc::new(Cell::new(0));
    let g = globaux.clone();
    keymap.root_scope().register(None, Some("p"), move |_, _| {
        g.set(g.get() + 1);
        Some(false)
    });
    let mut m = Menu::new();
    m.add_item(|i| {
        i.set_title("Copier");
    });
    m.show_at_position(10.0, 10.0, &keymap);
    assert_eq!(keymap.handle_key_event(&KeyEvent::new("p")), None);
    assert_eq!(
        globaux.get(),
        0,
        "le raccourci global ne part pas derrière le menu"
    );
    menu::fermer_menu();
    assert_eq!(keymap.handle_key_event(&KeyEvent::new("p")), Some(false));
    assert_eq!(globaux.get(), 1);
}

// ── Notice ──────────────────────────────────────────────────────────────────

#[test]
fn notice_persistante_ne_part_qu_au_clic_et_s_empile_dans_l_ordre() {
    Notice::tout_fermer();
    let a = Notice::new("Première", 0);
    let _b = Notice::new("Deuxième", 0);
    Notice::purger(Instant::now() + Duration::from_secs(3600));
    assert_eq!(Notice::messages(), ["Première", "Deuxième"]);
    a.hide();
    a.hide();
    assert_eq!(Notice::messages(), ["Deuxième"]);
    assert!(!a.est_affichee());
    Notice::tout_fermer();
}

#[test]
fn notice_temporaire_disparait_a_son_echeance() {
    Notice::tout_fermer();
    let _n = Notice::new("Copié", 5000);
    Notice::purger(Instant::now() + Duration::from_millis(4000));
    assert_eq!(Notice::messages(), ["Copié"]);
    Notice::purger(Instant::now() + Duration::from_millis(6000));
    assert!(Notice::messages().is_empty());
}

#[test]
fn notice_set_message_remplace_le_texte() {
    Notice::tout_fermer();
    let n = Notice::new("Avant", 0);
    n.set_message("Après");
    assert_eq!(Notice::messages(), ["Après"]);
    Notice::tout_fermer();
}

// ── Toolbar.test.ts : les SECTIONS de la barre ──────────────────────────────

/// Les sections affichées, dans l'ordre : `[id, [libellés]]`, et `|` pour un séparateur.
fn forme(barre: &BarreOutils) -> Vec<Result<(String, Vec<String>), &'static str>> {
    barre
        .modele()
        .forme()
        .into_iter()
        .map(|f| match f {
            Forme::Sep => Err("|"),
            Forme::Section(id, libelles) => Ok((id, libelles)),
        })
        .collect()
}

fn section(id: &str, libelles: &[&str]) -> Result<(String, Vec<String>), &'static str> {
    Ok((
        id.to_owned(),
        libelles.iter().map(|s| (*s).to_owned()).collect(),
    ))
}

#[test]
fn range_par_section_declaree_puis_les_autres_dans_l_ordre_de_1re_apparition() {
    let mut bar = BarreOutils::new();
    bar.add_sections(&["tool", "style"]);
    bar.add_item(
        |i| {
            i.set_tooltip("Rouge").set_section("style");
        },
        None::<fn()>,
    );
    bar.add_item(
        |i| {
            i.set_tooltip("Ask").set_section("agent");
        },
        None::<fn()>,
    );
    bar.add_item(
        |i| {
            i.set_tooltip("Crayon").set_section("tool");
        },
        None::<fn()>,
    );
    bar.add_item(
        |i| {
            i.set_tooltip("Gomme").set_section("tool");
        },
        None::<fn()>,
    );
    assert_eq!(
        forme(&bar),
        [
            section("tool", &["Crayon", "Gomme"]),
            Err("|"),
            section("style", &["Rouge"]),
            Err("|"),
            section("agent", &["Ask"]),
        ]
    );
}

#[test]
fn pas_de_separateur_pour_une_section_declaree_vide_les_separateurs_manuels_sont_retires() {
    let mut bar = BarreOutils::new();
    bar.add_sections(&["a", "vide", "b"]);
    bar.add_item(
        |i| {
            i.set_tooltip("x").set_section("a");
        },
        None::<fn()>,
    );
    bar.add_separator();
    bar.add_item(
        |i| {
            i.set_tooltip("y").set_section("b");
        },
        None::<fn()>,
    );
    assert_eq!(
        forme(&bar),
        [section("a", &["x"]), Err("|"), section("b", &["y"])]
    );
}

#[test]
fn opt_in_sans_section_la_barre_garde_ses_separateurs_manuels() {
    let mut bar = BarreOutils::new();
    bar.add_item(
        |i| {
            i.set_tooltip("x");
        },
        None::<fn()>,
    );
    bar.add_separator();
    bar.add_item(
        |i| {
            i.set_tooltip("y");
        },
        None::<fn()>,
    );
    assert_eq!(
        forme(&bar),
        [section("", &["x"]), Err("|"), section("", &["y"])]
    );
}

#[test]
fn un_item_ajoute_apres_l_affichage_est_range_la_poignee_reste_derniere() {
    let mut bar = BarreOutils::new();
    bar.add_sections(&["tool", "style"]);
    bar.add_item(
        |i| {
            i.set_tooltip("Rouge").set_section("style");
        },
        None::<fn()>,
    );
    assert!(bar.modele_mut().show());
    bar.add_item(
        |i| {
            i.set_tooltip("Crayon").set_section("tool");
        },
        None::<fn()>,
    );
    assert_eq!(
        forme(&bar),
        [
            section("tool", &["Crayon"]),
            Err("|"),
            section("style", &["Rouge"])
        ]
    );
    assert_eq!(bar.modele().enfants().last(), Some(&Enfant::Poignee));
}

#[test]
fn clear_oublie_les_sections_declarees_l_ouvreur_les_redeclare() {
    let mut bar = BarreOutils::new();
    bar.add_sections(&["b", "a"]);
    bar.add_item(
        |i| {
            i.set_tooltip("x").set_section("a");
        },
        None::<fn()>,
    );
    bar.clear();
    bar.add_item(
        |i| {
            i.set_tooltip("y");
        },
        None::<fn()>,
    );
    bar.add_separator();
    bar.add_item(
        |i| {
            i.set_tooltip("z");
        },
        None::<fn()>,
    );
    assert_eq!(
        forme(&bar),
        [section("", &["y"]), Err("|"), section("", &["z"])]
    );
}

#[test]
fn le_clic_d_un_item_de_groupe_l_arme_sans_fermer_la_barre() {
    let appels = Rc::new(Cell::new(0));
    let a = appels.clone();
    let mut bar = BarreOutils::new();
    bar.add_item(
        |i| {
            i.set_tooltip("Crayon").set_group("outil").set_active(true);
        },
        None::<fn()>,
    );
    bar.add_item(
        |i| {
            i.set_tooltip("Gomme").set_group("outil");
        },
        Some(move || a.set(a.get() + 1)),
    );
    bar.show_at_position(5.0, 5.0);
    assert!(fragment_ui::toolbar::barre_ouverte());
    assert!(fragment_ui::toolbar::cliquer(1));
    assert_eq!(appels.get(), 1);
    assert!(
        fragment_ui::toolbar::barre_ouverte(),
        "la barre reste ouverte"
    );
    fragment_ui::toolbar::fermer_barre();
    assert!(!fragment_ui::toolbar::barre_ouverte());
}

// ── tooltip.test.ts : l'infobulle de survol ─────────────────────────────────

fn bouton(survol: &mut Survol, id: u64) {
    survol.ajouter_cible(
        id,
        Rect {
            left: 100.0,
            top: 100.0,
            width: 40.0,
            height: 20.0,
        },
    );
}

#[test]
fn le_texte_est_l_aria_label_title_retire_placement_en_data() {
    let mut s = Survol::new();
    bouton(&mut s, 1);
    s.set_tooltip(
        1,
        "Calques",
        Options {
            placement: Some(Placement::Right),
            delai_ms: None,
        },
    );
    let c = s.cible(1).cloned().unwrap_or_default();
    assert_eq!(c.texte, "Calques");
    assert_eq!(c.placement.nom(), "right");
}

#[test]
fn apparait_apres_le_delai_300_ms_par_defaut_part_au_depart_du_pointeur() {
    let mut s = Survol::new();
    bouton(&mut s, 1);
    s.set_tooltip(1, "Calques", Options::default());
    let t0 = Instant::now();
    s.pointer_enter(1, t0);
    s.poll(t0 + Duration::from_millis(DELAI_DEFAUT_MS - 1));
    assert_eq!(s.bulle().shown_on(), None);
    s.poll(t0 + Duration::from_millis(DELAI_DEFAUT_MS));
    assert_eq!(s.bulle().shown_on(), Some(1));
    assert_eq!(s.bulle().texte(), "Calques");
    assert_eq!(s.bulle().placement().nom(), "bottom");
    s.pointer_leave();
    assert_eq!(s.bulle().shown_on(), None);
}

#[test]
fn part_a_l_appui_un_delai_propre_est_respecte() {
    let mut s = Survol::new();
    bouton(&mut s, 1);
    s.set_tooltip(
        1,
        "X",
        Options {
            placement: None,
            delai_ms: Some(50),
        },
    );
    let t0 = Instant::now();
    s.pointer_enter(1, t0);
    s.poll(t0 + Duration::from_millis(50));
    assert_eq!(s.bulle().shown_on(), Some(1));
    s.pointer_down();
    assert_eq!(s.bulle().shown_on(), None);
}

#[test]
fn une_seule_bulle_de_survol_passer_d_un_bouton_a_l_autre_la_deplace() {
    let mut s = Survol::new();
    bouton(&mut s, 1);
    bouton(&mut s, 2);
    s.set_tooltip(1, "A", Options::default());
    s.set_tooltip(2, "B", Options::default());
    s.display_tooltip(1, None, Options::default());
    s.display_tooltip(2, None, Options::default());
    assert_eq!(s.bulle().shown_on(), Some(2));
    assert_eq!(s.bulle().texte(), "B");
}

#[test]
fn set_tooltip_sur_la_bulle_affichee_suit_le_nouveau_texte_texte_vide_plus_d_infobulle() {
    let mut s = Survol::new();
    bouton(&mut s, 1);
    s.display_tooltip(1, Some("Avant"), Options::default());
    s.set_tooltip(1, "Après", Options::default());
    assert_eq!(s.bulle().texte(), "Après");
    s.set_tooltip(1, "", Options::default());
    s.display_tooltip(1, None, Options::default());
    assert_eq!(s.bulle().shown_on(), None);
}

#[test]
fn cible_retiree_du_dom_rien_n_est_affiche() {
    let mut s = Survol::new();
    bouton(&mut s, 1);
    s.set_tooltip(1, "X", Options::default());
    s.retirer(1);
    s.display_tooltip(1, None, Options::default());
    assert_eq!(s.bulle().shown_on(), None);
}

#[test]
fn posee_a_cote_de_la_cible_bas_centree_ecart_6_bornee_a_4_px() {
    let cible = Rect {
        left: 100.0,
        top: 100.0,
        width: 40.0,
        height: 20.0,
    };
    // Une bulle de 0 × 0 : centrée sur la cible, 6 px dessous.
    assert_eq!(
        tooltip::position(cible, (0.0, 0.0), Placement::Bottom, (1280.0, 800.0)),
        (120.0, 126.0)
    );
    // Bornée à 4 px du bord de la fenêtre.
    assert_eq!(
        tooltip::position(cible, (400.0, 50.0), Placement::Left, (1280.0, 800.0)),
        (4.0, 85.0)
    );
    // À droite, collée au bord : la bulle raccourcit au lieu de revenir sur sa cible.
    assert_eq!(
        tooltip::largeur_max(cible, Placement::Right, 200.0),
        Some(50.0)
    );
    assert_eq!(tooltip::largeur_max(cible, Placement::Bottom, 200.0), None);
}

// ── Tooltip : la classe (extensible) ────────────────────────────────────────

fn cible_connectee() -> Cible {
    Cible {
        texte: "Survol".to_owned(),
        connectee: true,
        ..Cible::default()
    }
}

#[test]
fn une_instance_a_soi_ne_touche_pas_la_bulle_de_survol_et_reste() {
    let mut s = Survol::new();
    bouton(&mut s, 1);
    s.display_tooltip(1, Some("Survol"), Options::default());
    let mut mienne = Tooltip::new();
    mienne.show(1, &cible_connectee(), Some(Placement::Right));
    assert_eq!(s.bulle().shown_on(), Some(1));
    s.pointer_leave();
    assert_eq!(s.bulle().shown_on(), None);
    assert_eq!(
        mienne.shown_on(),
        Some(1),
        "pas de câblage de survol : elle reste"
    );
    assert_eq!(mienne.placement().nom(), "right");
    mienne.hide();
    assert_eq!(mienne.shown_on(), None);
}

#[test]
fn une_sous_classe_change_le_contenu_par_fill_et_garde_le_placement_de_la_base() {
    let mut riche = Tooltip::avec_contenu(|c| format!("<riche>{}", c.texte));
    riche.show(1, &cible_connectee(), None);
    assert_eq!(riche.texte(), "<riche>Survol");
    assert_eq!(riche.placement(), Placement::Bottom);
}
