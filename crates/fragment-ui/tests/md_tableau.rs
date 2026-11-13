//! Tests headless des tableaux du live preview (`fr-t3-tableaux`).
//!
//! Même plateforme que `tests/md_blocs.rs` : GPUI de test, système de texte RÉEL (DirectWrite), DM
//! Sans enregistrée. Les noms suivent les cas de la spec e2e `markdown-table` de Fragment (et deux de
//! `editor-hell` : undo croisé cellule / texte, taper juste sous un tableau final). La SOURCE de la
//! note est vérifiée à chaque étape, comme là-bas. Les fixtures sont réécrites.
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_core::Keymap;
use fragment_markdown::decorations::GenreWidget;
use fragment_markdown::editor_transforms::inserer_tableau;
use fragment_text::EditorRange;
use fragment_text::utf16::utf16_to_byte;
use fragment_theme::Mode;
use fragment_ui::editeur::{EditeurLive, OptionsEditeur};
use fragment_ui::md_tableau::widget::{LIBELLE_AJOUT_COLONNE, LIBELLE_AJOUT_LIGNE};
use fragment_ui::md_tableau::{Bande, GeoTableau, HoteTableaux, OptionsTableau, Tableau};
use fragment_ui::{menu, polices, theme_global};
use gpui_kit::base::input::EditorState;
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, EntityInputHandler as _, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, Point,
    TestAppContext, TestDispatcher, Window, WindowBounds, WindowOptions, point, px, size,
};

/// ★ Borne chaque test (voir `tests/editeur.rs`) : un test figé échoue en 5 min, nommé.
const DELAI_TEST: Duration = Duration::from_secs(300);

fn chien_de_garde(fini: Arc<AtomicBool>) {
    let nom = std::thread::current().name().unwrap_or("?").to_owned();
    std::thread::spawn(move || {
        let debut = Instant::now();
        while debut.elapsed() < DELAI_TEST {
            if fini.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        eprintln!("TABLEAU|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
        std::process::exit(101);
    });
}

// Les notes de la spec, réécrites.
const NOTE: &str = "Avant\n\n| A | B | C |\n| --- | --- | --- |\n| 1 | 2 | 3 |\n\nAprès\n";
const FIN: &str = "Intro\n\n| A | B |\n| --- | --- |\n| 1 | 2 |";

struct Banc {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur DirectWrite ne rend pas la
    /// main sous Windows (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    hote: Entity<HoteTableaux>,
    editeur: Entity<EditeurLive>,
    etat: Entity<EditorState>,
    fini: Arc<AtomicBool>,
}

impl Drop for Banc {
    fn drop(&mut self) {
        self.fini.store(true, Ordering::Relaxed);
    }
}

impl Banc {
    fn new(texte: &str) -> Banc {
        let mut cx = TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            current_platform(false).text_system(),
        );
        polices::enregistrer(cx.text_system()).unwrap();
        cx.update(gpui_kit::init);
        cx.update(|cx| theme_global::installer(cx, Mode::Sombre));
        let texte = texte.to_owned();
        let (fenetre, hote) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(900.), px(700.)),
                    })),
                    ..Default::default()
                },
                cx,
                move |window, cx| {
                    let options = OptionsTableau {
                        taille_texte: 16.0,
                        lecture_seule: false,
                        keymap: Keymap::new(),
                        moteur: None,
                    };
                    cx.new(|cx| {
                        HoteTableaux::new(
                            &texte,
                            OptionsEditeur::default(),
                            options,
                            None,
                            window,
                            cx,
                        )
                    })
                },
            )
            .unwrap()
        });
        cx.update_window(fenetre, |_, window, _| window.activate_window())
            .unwrap();
        let editeur = cx.update(|cx| hote.read(cx).editeur().clone());
        let etat = cx.update(|cx| editeur.read(cx).etat().clone());
        let fini = Arc::new(AtomicBool::new(false));
        chien_de_garde(fini.clone());
        let mut b = Banc {
            cx: ManuallyDrop::new(cx),
            fenetre,
            hote,
            editeur,
            etat,
            fini,
        };
        b.stabiliser();
        b
    }

    fn stabiliser(&mut self) {
        for _ in 0..6 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.fenetre, |_, window, cx| window.render_frame(cx))
                .unwrap();
        }
        self.cx.run_until_parked();
    }

    fn agir<R>(&mut self, f: impl FnOnce(&mut Window, &mut App) -> R) -> R {
        let r = self
            .cx
            .update_window(self.fenetre, |_, window, cx| f(window, cx))
            .unwrap();
        self.stabiliser();
        r
    }

    fn lire<R>(&mut self, f: impl FnOnce(&App) -> R) -> R {
        self.cx.update(|cx| f(cx))
    }

    fn source(&mut self) -> String {
        let e = self.editeur.clone();
        self.lire(|cx| e.read(cx).texte().to_owned())
    }

    /// Les lignes `| … |` du source, découpées en cellules (comme la spec).
    fn grille(&mut self) -> Vec<Vec<String>> {
        self.source()
            .split('\n')
            .filter(|l| {
                let t = l.trim();
                t.starts_with('|')
                    && !t.trim_start_matches('|').trim_start().starts_with("---")
                    && !t.trim_start_matches('|').trim_start().starts_with(":-")
            })
            .map(|l| {
                let t = l.trim();
                t[1..t.len() - 1]
                    .split('|')
                    .map(|c| c.trim().to_owned())
                    .collect()
            })
            .collect()
    }

    fn nombre_de_tableaux(&mut self) -> usize {
        let h = self.hote.clone();
        self.lire(|cx| h.read(cx).tableaux().len())
    }

    fn tableau(&mut self) -> Entity<Tableau> {
        let h = self.hote.clone();
        self.lire(|cx| h.read(cx).tableaux()[0].clone())
    }

    fn geo(&mut self) -> GeoTableau {
        let t = self.tableau();
        self.lire(|cx| t.read(cx).geometrie())
    }

    fn cellule(&mut self, r: usize, c: usize) -> Entity<EditeurLive> {
        let t = self.tableau();
        self.lire(|cx| t.read(cx).cellule(r, c).unwrap())
    }

    fn texte_cellule(&mut self, r: usize, c: usize) -> String {
        let e = self.cellule(r, c);
        self.lire(|cx| e.read(cx).texte().to_owned())
    }

    fn focus_sur(&mut self, r: usize, c: usize) -> bool {
        let t = self.tableau();
        self.lire(|cx| t.read(cx).cellule_focalisee(cx)) == Some((r, c))
    }

    fn evenement(&mut self, ev: PlatformInput) {
        self.agir(|window, cx| {
            window.dispatch_event(ev, cx);
        });
    }

    fn bouger(&mut self, p: Point<Pixels>, bouton: Option<MouseButton>) {
        self.evenement(PlatformInput::MouseMove(MouseMoveEvent {
            position: p,
            pressed_button: bouton,
            modifiers: Modifiers::default(),
        }));
    }

    fn appuyer(&mut self, p: Point<Pixels>, bouton: MouseButton) {
        self.evenement(PlatformInput::MouseDown(MouseDownEvent {
            button: bouton,
            position: p,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
    }

    fn relacher(&mut self, p: Point<Pixels>, bouton: MouseButton) {
        self.evenement(PlatformInput::MouseUp(MouseUpEvent {
            button: bouton,
            position: p,
            modifiers: Modifiers::default(),
            click_count: 1,
        }));
    }

    fn cliquer(&mut self, p: Point<Pixels>, bouton: MouseButton) {
        self.bouger(p, None);
        self.appuyer(p, bouton);
        self.relacher(p, bouton);
    }

    /// Un clic dans le texte de la cellule (r, c).
    fn cliquer_cellule(&mut self, r: usize, c: usize) {
        let b = self.geo().cellule(r, c).unwrap();
        self.cliquer(b.center(), MouseButton::Left);
    }

    /// Survole la cellule (r, c) : les poignées de sa colonne et de sa ligne apparaissent.
    fn survoler_cellule(&mut self, r: usize, c: usize) {
        let b = self.geo().cellule(r, c).unwrap();
        self.bouger(b.center(), None);
    }

    /// Glisse la poignée (colonne / ligne) `index` jusqu'au point `vers`.
    fn glisser(&mut self, colonne: bool, index: usize, vers: Point<Pixels>) {
        if colonne {
            self.survoler_cellule(0, index);
        } else {
            self.survoler_cellule(index, 0);
        }
        let (i, b) = self.geo().poignee(colonne).expect("poignée visible");
        assert_eq!(i, index);
        let de = b.center();
        self.bouger(de, None);
        self.appuyer(de, MouseButton::Left);
        self.bouger(de + point(px(2.), px(2.)), Some(MouseButton::Left));
        for k in 1..=8 {
            let f = k as f32 / 8.0;
            let p = point(de.x + (vers.x - de.x) * f, de.y + (vers.y - de.y) * f);
            self.bouger(p, Some(MouseButton::Left));
        }
        self.relacher(vers, MouseButton::Left);
    }

    /// Un CLIC (sans glisser) sur la poignée : choisit la colonne / ligne.
    fn choisir_poignee(&mut self, colonne: bool, index: usize) {
        if colonne {
            self.survoler_cellule(1, index);
        } else {
            self.survoler_cellule(index, 0);
        }
        let (_, b) = self.geo().poignee(colonne).expect("poignée visible");
        self.cliquer(b.center(), MouseButton::Left);
    }

    /// Le clic droit sur la poignée `index` : les titres du menu ouvert.
    fn menu_poignee(&mut self, colonne: bool, index: usize) -> Vec<String> {
        if colonne {
            self.survoler_cellule(1, index);
        } else {
            self.survoler_cellule(index, 0);
        }
        let (_, b) = self.geo().poignee(colonne).expect("poignée visible");
        self.cliquer(b.center(), MouseButton::Right);
        titres(0)
    }

    fn cliquer_menu(&mut self, niveau: usize, titre: &str) {
        let i = menu::index_ouvert(niveau, titre).unwrap_or_else(|| panic!("pas de « {titre} »"));
        assert!(menu::cliquer(niveau, i));
        self.stabiliser();
    }

    fn touche(&mut self, k: &str) {
        self.agir(|window, cx| window.press(k, cx));
    }

    fn taper(&mut self, t: &str) {
        self.agir(|window, cx| window.input(t, cx));
    }

    /// Le focus et le curseur de l'hôte à `o` (octets).
    fn placer_hote(&mut self, o: usize) {
        let etat = self.etat.clone();
        self.agir(|window, cx| {
            etat.update(cx, |s, cx| {
                s.focus(window, cx);
                s.set_selected_range(o..o, cx);
            });
        });
    }

    fn curseur_hote(&mut self) -> usize {
        let etat = self.etat.clone();
        self.lire(|cx| etat.read(cx).cursor())
    }
}

/// Les titres du niveau ouvert, sans les séparateurs.
fn titres(niveau: usize) -> Vec<String> {
    menu::forme_ouverte(niveau)
        .into_iter()
        .filter(|t| t != "|")
        .collect()
}

// ─── markdown-table.spec.ts ───

#[test]
fn grille_rendu_frappe_tab_undo_plus_droite_bas_glisser_colonne_et_ligne() {
    let mut b = Banc::new(NOTE);
    // Rendu : un tableau, 2 × 3 cellules éditables.
    assert_eq!(b.nombre_de_tableaux(), 1);
    let t = b.tableau();
    assert_eq!(b.lire(|cx| t.read(cx).nombre_de_cellules()), 6);
    assert_eq!(b.texte_cellule(0, 0), "A");

    // Frappe dans (0, 0) : la source suit, le focus RESTE (pas de reconstruction).
    b.cliquer_cellule(0, 0);
    assert!(b.focus_sur(0, 0));
    b.touche("end");
    b.taper("xyz");
    assert_eq!(b.grille()[0][0], "Axyz");
    assert!(b.focus_sur(0, 0));

    // Ctrl+Z dans la cellule : l'historique de l'HÔTE annule la frappe (d'un coup).
    b.touche("ctrl-z");
    assert_eq!(b.grille()[0][0], "A");
    assert_eq!(b.texte_cellule(0, 0), "A");

    // Tab → la cellule suivante.
    b.cliquer_cellule(0, 0);
    b.touche("tab");
    assert!(b.focus_sur(0, 1));

    // « + » à droite → une 4e colonne.
    let bande = b.geo().bande(Bande::Colonne).unwrap();
    b.bouger(bande.center(), None);
    assert_eq!(
        b.lire(|cx| t.read(cx).bande_survolee()),
        Some(Bande::Colonne)
    );
    assert_eq!(LIBELLE_AJOUT_COLONNE, "Add column to the right");
    b.cliquer(bande.center(), MouseButton::Left);
    assert_eq!(b.grille()[0].len(), 4);

    // « + » en bas → une ligne de plus ; on la marque.
    let bande = b.geo().bande(Bande::Ligne).unwrap();
    b.bouger(bande.center(), None);
    assert_eq!(b.lire(|cx| t.read(cx).bande_survolee()), Some(Bande::Ligne));
    assert_eq!(LIBELLE_AJOUT_LIGNE, "Add row below");
    b.cliquer(bande.center(), MouseButton::Left);
    assert_eq!(b.grille().len(), 3);
    b.cliquer_cellule(2, 0);
    b.taper("Z");
    assert_eq!(b.grille()[2][0], "Z");

    // Glisser la colonne 2 (C) tout à gauche → C, A, B, ''.
    let g = b.geo().grille_fenetre();
    b.glisser(true, 2, g.origin + point(px(2.), px(10.)));
    assert_eq!(b.grille()[0], ["C", "A", "B", ""]);

    // Glisser la ligne 2 (Z…) juste sous l'en-tête → elle passe avant « 1 2 3 ».
    let r1 = b.geo().cellule(1, 0).unwrap();
    let g = b.geo().grille_fenetre();
    b.glisser(false, 2, point(g.origin.x + px(10.), r1.origin.y + px(2.)));
    assert_eq!(b.grille()[1][1], "Z");
    assert_eq!(b.grille()[2], ["3", "1", "2", ""]);

    // Le texte autour du tableau est intact.
    let src = b.source();
    assert!(src.starts_with("Avant\n\n|"), "{src:?}");
    assert!(src.ends_with("\n\nAprès\n"), "{src:?}");
}

#[test]
fn editor_insert_table_un_2x2_rendu_focus_dans_la_premiere_cellule() {
    let mut b = Banc::new("");
    b.placer_hote(0);
    let spec = inserer_tableau("", EditorRange { from: 0, to: 0 });
    let etat = b.etat.clone();
    b.agir(|window, cx| {
        etat.update(cx, |s, cx| {
            for c in &spec.changes {
                s.replace_text_in_range(
                    Some(c.from..c.to.unwrap_or(c.from)),
                    &c.insert,
                    window,
                    cx,
                );
            }
            if let Some(sel) = &spec.selection {
                let t = s.value().to_string();
                let a = utf16_to_byte(&t, sel.anchor).unwrap();
                let h = utf16_to_byte(&t, sel.head.unwrap_or(sel.anchor)).unwrap();
                s.set_selected_range(a.min(h)..a.max(h), cx);
            }
        });
    });
    assert_eq!(b.nombre_de_tableaux(), 1);
    let t = b.tableau();
    assert_eq!(b.lire(|cx| t.read(cx).nombre_de_cellules()), 4);
    assert!(b.focus_sur(0, 0));
    // Le tableau naît VIDE.
    assert!(b.grille().iter().flatten().all(String::is_empty));

    b.taper("Nom");
    assert_eq!(b.grille()[0][0], "Nom");
}

#[test]
fn trop_de_colonnes_le_bloc_defile_horizontalement() {
    let n = 12;
    let ligne = |f: &dyn Fn(usize) -> String| {
        format!("| {} |", (0..n).map(f).collect::<Vec<_>>().join(" | "))
    };
    let note = format!(
        "{}\n{}\n{}\n",
        ligne(&|i| format!("Colonne numéro {}", i + 1)),
        ligne(&|_| "---".to_owned()),
        ligne(&|i| format!("valeur {i}"))
    );
    let mut b = Banc::new(&note);
    assert_eq!(b.nombre_de_tableaux(), 1);
    let t = b.tableau();
    let defile = b.lire(|cx| t.read(cx).defilement().clone());
    assert!(
        f32::from(defile.max_offset().x) > 0.0,
        "la grille déborde : {:?}",
        defile.max_offset()
    );
    defile.set_offset(point(px(-200.), px(0.)));
    b.stabiliser();
    assert!(f32::from(defile.offset().x) < 0.0, "le bloc a défilé");
}

#[test]
fn tableau_en_fin_de_document_un_clic_dessous_cree_la_ligne_et_y_pose_le_curseur() {
    let mut b = Banc::new(FIN);
    assert_eq!(b.nombre_de_tableaux(), 1);
    let bloc = b.geo().bloc;
    b.cliquer(
        point(
            bloc.origin.x + px(40.),
            bloc.origin.y + bloc.size.height + px(8.),
        ),
        MouseButton::Left,
    );
    assert_eq!(b.source(), format!("{FIN}\n"));
    let long = b.source().len();
    assert_eq!(b.curseur_hote(), long);
}

#[test]
fn backspace_apres_un_tableau_1er_appui_le_selectionne_2e_le_supprime() {
    let mut b = Banc::new(NOTE);
    let avant = b.source();
    let fin = avant.find("| 1 | 2 | 3 |").unwrap() + "| 1 | 2 | 3 |".len();
    b.placer_hote(fin + 1);

    b.touche("backspace");
    let t = b.tableau();
    assert!(b.lire(|cx| t.read(cx).est_selectionne()));
    assert_eq!(b.source(), avant, "rien de supprimé");

    b.touche("backspace");
    assert_eq!(b.nombre_de_tableaux(), 0);
    assert!(!b.source().contains('|'));
}

#[test]
fn cellules_math_et_gras_rendus_hors_focus_source_revelee_au_focus() {
    let mut b = Banc::new("| $x^2$ | b |\n| --- | --- |\n| $y$ | **gras** |\n");
    assert_eq!(b.nombre_de_tableaux(), 1);
    let maths = |b: &mut Banc, r, c| {
        let e = b.cellule(r, c);
        b.lire(|cx| {
            e.read(cx)
                .projection()
                .decorations
                .widgets
                .iter()
                .filter(|w| matches!(w.genre, GenreWidget::Maths { .. }))
                .count()
        })
    };
    // Aucune cellule focalisée : tout est rendu, même en début de cellule.
    assert_eq!(maths(&mut b, 0, 0), 1);
    assert_eq!(maths(&mut b, 1, 0), 1);
    let gras = b.cellule(1, 1);
    let masques = b.lire(|cx| gras.read(cx).arguments_fork().unwrap().masques.len());
    assert!(masques > 0, "les ** sont masqués hors focus");

    // Curseur DANS la formule : la source se révèle, dans cette cellule seulement.
    let t = b.tableau();
    b.agir(|window, cx| t.update(cx, |t, cx| t.focaliser(0, 0, Some(2), None, window, cx)));
    assert_eq!(maths(&mut b, 0, 0), 0);
    assert_eq!(maths(&mut b, 1, 0), 1);
}

#[test]
fn menu_de_la_poignee_de_colonne_entrees_delete_align_tri() {
    let mut b = Banc::new(NOTE);
    let titres_ = b.menu_poignee(true, 0);
    assert_eq!(
        titres_,
        [
            "Sort by column (A to Z)",
            "Sort by column (Z to A)",
            "Add column to the left",
            "Add column to the right",
            "Move column right",
            "Align left",
            "Align center",
            "Align right",
            "Duplicate column",
            "Delete column",
        ]
    );
    b.cliquer_menu(0, "Align center");
    assert!(b.source().contains("| :-"), "{}", b.source());
    assert!(b.source().contains(":"));

    let titres_ = b.menu_poignee(true, 2);
    assert!(titres_.iter().any(|t| t == "Move column left"));
    assert!(!titres_.iter().any(|t| t == "Move column right"));
    b.cliquer_menu(0, "Delete column");
    assert_eq!(b.grille()[0], ["A", "B"]);

    b.menu_poignee(false, 1);
    b.cliquer_menu(0, "Duplicate row");
    assert_eq!(b.grille().len(), 3);
}

#[test]
fn clic_droit_sur_le_tableau_row_column_tris_dans_une_cellule_menu_d_edition_plus_tableau() {
    let mut b = Banc::new(NOTE);
    // Sur la BORDURE de la cellule (hors du texte) : le menu du tableau seul.
    let c = b.geo().cellule(1, 1).unwrap();
    b.cliquer(
        point(c.origin.x + px(0.5), c.origin.y + c.size.height - px(0.5)),
        MouseButton::Right,
    );
    assert!(menu::menu_ouvert());
    assert_eq!(
        titres(0),
        [
            "Row",
            "Column",
            "Sort by column (A to Z)",
            "Sort by column (Z to A)"
        ]
    );
    let row = menu::index_ouvert(0, "Row").unwrap();
    assert!(menu::survoler(0, row));
    b.cliquer_menu(1, "Add row below");
    assert_eq!(b.grille().len(), 3);

    // DANS le texte d'une cellule : le menu d'édition, puis les sections du tableau.
    let c = b.geo().cellule(1, 1).unwrap();
    b.cliquer(c.center(), MouseButton::Right);
    let t = titres(0);
    assert_eq!(t[..2], ["Add link", "Add external link"]);
    assert_eq!(
        t[t.len() - 4..],
        [
            "Row",
            "Column",
            "Sort by column (A to Z)",
            "Sort by column (Z to A)"
        ]
    );
    menu::fermer_menu();
}

#[test]
fn taper_sous_le_dernier_tableau_ne_cree_pas_une_ligne_de_tableau() {
    let mut b = Banc::new(FIN);
    let bloc = b.geo().bloc;
    b.cliquer(
        point(
            bloc.origin.x + px(40.),
            bloc.origin.y + bloc.size.height + px(8.),
        ),
        MouseButton::Left,
    );
    b.taper("suite");
    assert_eq!(b.source(), format!("{FIN}\n\nsuite"));
    assert_eq!(b.grille().len(), 2);
    let t = b.tableau();
    assert!(!b.lire(|cx| t.read(cx).source().contains("suite")));
}

#[test]
fn blocs_math_rendus_dans_les_cellules_au_milieu_et_dans_une_phrase() {
    let mut b = Banc::new("| a | x $$y^2$$ z |\n| --- | --- |\n| 1 | 2 |\n\nTexte $$z^3$$ fin.\n");
    assert_eq!(b.nombre_de_tableaux(), 1);
    let affichage = |b: &mut Banc, e: &Entity<EditeurLive>| {
        b.lire(|cx| {
            e.read(cx)
                .projection()
                .decorations
                .widgets
                .iter()
                .filter(|w| {
                    matches!(
                        w.genre,
                        GenreWidget::Maths {
                            affichage: true,
                            ..
                        }
                    )
                })
                .count()
        })
    };
    // Hors focus : au milieu de la cellule, une formule display dans le flux.
    let c = b.cellule(0, 1);
    assert_eq!(affichage(&mut b, &c), 1);
    // Dans la note aussi : « Texte $$z^3$$ fin. » (la source du tableau, masquée, n'en compte pas).
    let hote = b.editeur.clone();
    let debut = b.source().find("Texte").unwrap();
    let dans_la_phrase = b.lire(|cx| {
        hote.read(cx)
            .projection()
            .decorations
            .widgets
            .iter()
            .filter(|w| {
                matches!(
                    w.genre,
                    GenreWidget::Maths {
                        affichage: true,
                        ..
                    }
                )
            })
            .filter(|w| w.source.octets.from >= debut)
            .count()
    });
    assert_eq!(dans_la_phrase, 1);
    // Curseur dans la formule : la source se révèle.
    let t = b.tableau();
    b.agir(|window, cx| t.update(cx, |t, cx| t.focaliser(0, 1, Some(5), None, window, cx)));
    assert_eq!(affichage(&mut b, &c), 0);
    assert_eq!(b.texte_cellule(0, 1), "x $$y^2$$ z");
}

#[test]
fn suppr_backspace_sur_une_colonne_ou_une_ligne_selectionnee_la_supprime() {
    let mut b = Banc::new(NOTE);
    b.choisir_poignee(true, 1);
    let t = b.tableau();
    assert!(b.lire(|cx| t.read(cx).choix()).is_some());
    b.touche("delete");
    assert_eq!(b.grille()[0], ["A", "C"]);

    // En-tête : non supprimable, rien ne change.
    let avant = b.source();
    b.choisir_poignee(false, 0);
    b.touche("delete");
    assert_eq!(b.source(), avant);
    b.touche("escape");
    assert!(b.lire(|cx| t.read(cx).choix()).is_none());

    b.choisir_poignee(false, 1);
    b.touche("backspace");
    assert_eq!(b.grille().len(), 1);
    assert!(b.source().contains("Après"));
}

// ─── editor-hell.spec.ts ───

/// Undo croisé : une frappe dans le texte, puis dans une cellule ; Ctrl+Z (dans la cellule) annule
/// la cellule, puis le texte : un seul historique, celui de la note.
#[test]
fn undo_croise_cellule_et_texte_un_seul_historique() {
    let mut b = Banc::new(NOTE);
    b.placer_hote("Avant".len());
    b.taper("!");
    assert!(b.source().starts_with("Avant!\n"));
    b.cliquer_cellule(1, 1);
    b.touche("end");
    b.taper("q");
    assert_eq!(b.grille()[1][1], "2q");
    b.touche("ctrl-z");
    assert_eq!(b.grille()[1][1], "2");
    assert!(b.source().starts_with("Avant!\n"));
    b.touche("ctrl-z");
    assert!(b.source().starts_with("Avant\n"), "{:?}", b.source());
    assert_eq!(b.source(), NOTE);
}
