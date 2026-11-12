//! Tests headless de l'hôte des widgets de bloc et de la gouttière de repli (`fr-t3-widgets-blocs`).
//!
//! Même plateforme que `tests/editeur.rs` : GPUI de test, système de texte RÉEL (DirectWrite), DM
//! Sans enregistrée. Les noms suivent les cas de la spec e2e `editor-hell` de Fragment (« un clic sur
//! chaque ligne SOUS un bloc », « repli imbriqué », « fenêtre étroite », « menu Insert puis ligne
//! suivante cliquable ») et deux points laissés par #64 (rangée de hauteur 0 des lignes suivantes
//! d'un bloc « à la place », blocs atomiques au clavier).
//!
//! Les rendus que d'autres tâches mesurent (tableau, Properties, maths, embed) prennent ici des
//! hauteurs FIXES par le `Mesureur` : l'hôte est testé seul.
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_markdown::blocs::{BlocCalcule, ContenuBloc};
use fragment_theme::Mode;
use fragment_ui::editeur::{EditeurLive, OptionsEditeur};
use fragment_ui::md_blocs::{HoteBlocs, Mesureur};
use fragment_ui::{polices, theme_global};
use gpui_kit::base::input::EditorState;
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Modifiers, MouseMoveEvent, Point,
    TestAppContext, TestDispatcher, Window, WindowBounds, WindowOptions, point, px, size,
};

/// ★ L'identifiant d'élément de l'éditeur du fork (`EDITOR_ELEMENT_ID` de son `state.rs`).
const ID_EDITEUR: &str = "input-state";

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
        eprintln!("BLOCS|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
        std::process::exit(101);
    });
}

/// Les hauteurs des rendus des autres tâches, fixes ici.
fn mesureur() -> Mesureur {
    Rc::new(|b: &BlocCalcule, _| {
        Some(match b.contenu {
            ContenuBloc::Proprietes { .. } => 104.0,
            ContenuBloc::Tableau { .. } => 96.0,
            ContenuBloc::Maths { .. } => 72.0,
            ContenuBloc::Embed { .. } => 128.0,
            ContenuBloc::Html { .. } => 40.0,
            _ => return None,
        })
    })
}

struct Banc {
    /// ★ JAMAIS détruit : la destruction d'un `TestAppContext` monté sur DirectWrite ne rend pas la
    /// main sous Windows (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    hote: Entity<HoteBlocs>,
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
    fn new(texte: &str, l: f32, h: f32) -> Banc {
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
                        size: size(px(l), px(h)),
                    })),
                    ..Default::default()
                },
                cx,
                move |window, cx| {
                    cx.new(|cx| {
                        HoteBlocs::new(
                            &texte,
                            OptionsEditeur::default(),
                            Some(mesureur()),
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
        for _ in 0..4 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.fenetre, |_, window, cx| window.render_frame(cx))
                .unwrap();
        }
        self.cx.run_until_parked();
    }

    fn agir<R>(&mut self, f: impl FnOnce(&mut Window, &mut App, &Entity<EditorState>) -> R) -> R {
        let etat = self.etat.clone();
        let r = self
            .cx
            .update_window(self.fenetre, |_, window, cx| f(window, cx, &etat))
            .unwrap();
        self.stabiliser();
        r
    }

    fn texte(&mut self) -> String {
        let e = self.editeur.clone();
        self.cx.update(|cx| e.read(cx).texte().to_owned())
    }

    fn geometrie(&mut self) -> fragment_ui::md_blocs::Geometrie {
        let h = self.hote.clone();
        self.cx.update(|cx| h.read(cx).geometrie())
    }

    /// La ligne logique dont le texte est exactement `t`.
    fn ligne(&mut self, t: &str) -> usize {
        self.texte()
            .split('\n')
            .position(|l| l == t)
            .unwrap_or_else(|| panic!("pas de ligne « {t} »"))
    }

    /// Le texte de la ligne du curseur.
    fn ligne_du_curseur(&mut self) -> String {
        let texte = self.texte();
        let o = self.cx.update(|cx| self.etat.read(cx).cursor());
        let debut = texte[..o].rfind('\n').map_or(0, |i| i + 1);
        let fin = texte[o..].find('\n').map_or(texte.len(), |i| o + i);
        texte[debut..fin].to_owned()
    }

    /// Un clic à `p` (coordonnées FENÊTRE).
    fn cliquer(&mut self, p: Point<gpui_kit::Pixels>) {
        self.agir(|window, cx, _| {
            let origine = window.find(ID_EDITEUR).bounds().origin;
            window.click_at(ID_EDITEUR, p - origine, cx);
        });
    }

    /// Un clic au milieu (en hauteur) de la ligne `l`, à `dx` du début du texte.
    fn cliquer_ligne(&mut self, l: usize, dx: f32) {
        let g = self.geometrie();
        let le = *g
            .ligne(l)
            .unwrap_or_else(|| panic!("ligne {l} absente de l'écran : {:?}", g.lignes));
        let x = g.gauche + fragment_ui::editeur::constantes::debut_texte(g.largeur) + dx;
        self.cliquer(point(px(x), px(le.haut + le.hauteur / 2.0)));
    }

    fn survoler(&mut self, p: Point<gpui_kit::Pixels>) {
        let f = self.fenetre;
        self.cx
            .update_window(f, |_, window, cx| {
                window.dispatch_event(
                    gpui_kit::PlatformInput::MouseMove(MouseMoveEvent {
                        position: p,
                        pressed_button: None,
                        modifiers: Modifiers::default(),
                    }),
                    cx,
                );
            })
            .unwrap();
        self.stabiliser();
    }

    fn placer(&mut self, o: usize) {
        self.agir(|window, cx, etat| {
            etat.update(cx, |s, cx| {
                s.focus(window, cx);
                s.set_selected_range(o..o, cx);
            });
        });
    }
}

/// La note de la spec, réécrite (ADR-007) : un exemplaire de chaque bloc, suivi d'une ligne sonde.
const BLOCS: &str = "---\nauteur: moi\nmots:\n  - x\n---\nsous les propriétés\n# Un titre\n\
sous le titre\n| P | Q |\n| --- | --- |\n| 7 | 8 |\n\nsous la grille\n```rust\nlet u = 3;\n\n\
let v = 4;\n```\nsous le code\n$$\n\\sqrt{2}\n$$\nsous la formule\n> une citation\nsous la citation\n\
> [!tip] Astuce\n> dedans\n\nsous l'encadré\n***\nsous le trait\n![[photo.png]]\nsous la photo\n\
- un point\n- [ ] à faire\nla fin";

const SONDES: [&str; 12] = [
    "sous les propriétés",
    "sous le titre",
    "sous la grille",
    "let u = 3;",
    "let v = 4;",
    "sous le code",
    "sous la formule",
    "sous la citation",
    "sous l'encadré",
    "sous le trait",
    "sous la photo",
    "la fin",
];

// ─── Cas de la spec e2e `editor-hell` ───

/// « un clic sur chaque ligne SOUS un bloc pose le curseur sur cette ligne », et la ligne VIDE du
/// bloc de code. La source n'est pas modifiée.
#[test]
fn un_clic_sur_chaque_ligne_sous_un_bloc_pose_le_curseur_sur_cette_ligne() {
    let mut b = Banc::new(BLOCS, 1000., 1600.);
    for t in SONDES {
        let l = b.ligne(t);
        b.cliquer_ligne(l, 20.);
        assert_eq!(b.ligne_du_curseur(), t, "clic sur « {t} »");
    }
    let vide = b.ligne("let u = 3;") + 1;
    b.cliquer_ligne(vide, 20.);
    assert_eq!(b.ligne_du_curseur(), "");
    assert_eq!(b.texte(), BLOCS);
}

/// « fenêtre étroite : aucun défilement horizontal, clics toujours justes » (600 px, la largeur
/// minimale de la fenêtre).
#[test]
fn fenetre_etroite_aucun_defilement_horizontal_clics_toujours_justes() {
    let mut b = Banc::new(BLOCS, 600., 1800.);
    let g = b.geometrie();
    // Le texte tient dans la colonne : le fork ne défile jamais en largeur.
    let defilement = b.cx.update(|cx| b.etat.read(cx).scroll_offset());
    assert_eq!(defilement.x, px(0.));
    assert!(g.largeur <= 600.);
    for t in ["sous la grille", "sous le code", "la fin"] {
        let l = b.ligne(t);
        b.cliquer_ligne(l, 10.);
        assert_eq!(b.ligne_du_curseur(), t, "clic sur « {t} »");
    }
}

/// « repli imbriqué : replier un ## cache son ###, le déplier le rend ». Le chevron n'apparaît
/// qu'au survol de la ligne, et le clic passe par lui.
#[test]
fn repli_imbrique_replier_un_h2_cache_son_h3_le_deplier_le_rend() {
    let doc = "## A\ntexte A\n### A1\ntexte A1\n## B\ntexte B";
    let mut b = Banc::new(doc, 1000., 600.);
    let plier = |b: &mut Banc| {
        let g = b.geometrie();
        let le = *g.ligne(0).unwrap();
        b.survoler(point(px(g.gauche + 400.), px(le.haut + le.hauteur / 2.)));
        let g = b.geometrie();
        let (c, (x, y, cote)) = *g
            .chevrons
            .iter()
            .find(|(c, _)| c.ligne == 0)
            .expect("chevron visible au survol");
        assert_eq!(c.ligne, 0);
        b.cliquer(point(px(x + cote / 2.), px(y + cote / 2.)));
    };
    plier(&mut b);
    let g = b.geometrie();
    assert!(
        g.ligne(3).is_none(),
        "« texte A1 » est caché : {:?}",
        g.lignes
    );
    assert!(g.ligne(2).is_none(), "« ### A1 » est caché");
    assert!(g.ligne(5).is_some(), "« texte B » reste visible");
    assert!(b.cx.update(|cx| b.hote.read(cx).ligne_repliee(0)));
    // Replié : le chevron reste, même sans survol.
    assert!(g.chevrons.iter().any(|(c, _)| c.ligne == 0 && c.replie));
    plier(&mut b);
    let g = b.geometrie();
    assert!(g.ligne(3).is_some(), "« texte A1 » revient");
    assert_eq!(b.texte(), doc);
}

/// « menu Insert sur une ligne vide : chaque bloc inséré, puis la ligne suivante cliquable ». Le
/// menu est le travail de `fr-t3-commandes` : on insère ici ce qu'il insère.
#[test]
fn menu_insert_puis_la_ligne_suivante_cliquable() {
    for insere in [
        "---\n",
        "```\n\n```\n",
        "$$\n\\pi\n$$\n",
        "| a |\n| --- |\n| 1 |\n\n",
    ] {
        let mut b = Banc::new("début\n\nfin", 1000., 600.);
        b.placer("début\n".len());
        b.agir(|window, cx, _| window.input(insere, cx));
        // Hors curseur : le bloc se rend.
        b.placer(0);
        let l = b.ligne("fin");
        b.cliquer_ligne(l, 4.);
        assert_eq!(b.ligne_du_curseur(), "fin", "après {insere:?}");
    }
}

// ─── Points laissés par #64 (fr-t3-blocs-modele) ───

/// ★ Un bloc « à la place » masque toute sa source : hauteur dictée à la première ligne, 0 aux
/// suivantes. Le fork l'accepte : la ligne d'après commence juste sous le rendu.
#[test]
fn rangee_de_hauteur_nulle_pour_les_lignes_suivantes_d_un_bloc() {
    let doc = "avant\n$$\na+b\n$$\naprès";
    let mut b = Banc::new(doc, 1000., 600.);
    let g = b.geometrie();
    let math = *g.ligne(1).unwrap();
    assert!((math.hauteur - 72.).abs() < 0.05, "{math:?}");
    assert!(g.ligne(2).is_none() && g.ligne(3).is_none());
    let apres = *g.ligne(4).unwrap();
    assert!((apres.haut - (math.haut + 72.)).abs() < 0.05, "{apres:?}");
    // Et le fork pose bien le texte de « après » à cet endroit.
    let o = doc.find("après").unwrap();
    let y =
        b.cx.update(|cx| f32::from(b.etat.read(cx).range_to_bounds(&(o..o)).unwrap().origin.y));
    assert!(
        (y - apres.haut).abs() < 0.05,
        "fork {y} contre hôte {}",
        apres.haut
    );
}

/// Un bloc est ATOMIQUE : curseur dans la source (rendu dessous), Bas va d'un coup à la ligne qui
/// suit le rendu ; hors curseur, le rendu n'est qu'un arrêt.
#[test]
fn bloc_atomique_bas_traverse_le_rendu_d_un_coup() {
    let doc = "avant\n$$\na+b\n$$\naprès";
    let mut b = Banc::new(doc, 1000., 600.);
    // Sur le `$$` de fermeture : la source est révélée, le rendu est posé dessous.
    b.placer(doc.rfind("$$").unwrap());
    let fork =
        b.cx.update(|cx| b.editeur.read(cx).arguments_fork().unwrap().blocs.clone());
    assert_eq!(fork, [(3, 72.0)]);
    b.agir(|window, cx, _| window.press("down", cx));
    assert_eq!(b.ligne_du_curseur(), "après");
    // Hors curseur, Haut depuis « après » s'arrête UNE fois sur le bloc (sa première ligne).
    b.agir(|window, cx, _| window.press("up", cx));
    let o = b.cx.update(|cx| b.etat.read(cx).cursor());
    assert!(
        (doc.find("$$").unwrap()..=doc.rfind("$$").unwrap() + 2).contains(&o),
        "curseur {o} hors de la source du bloc"
    );
}

/// Un clic sur une formule rendue sélectionne son LaTeX (`selectInside`), délimiteurs exclus.
#[test]
fn clic_sur_un_bloc_de_maths_selectionne_le_latex() {
    let doc = "avant\n$$\na+b\n$$\naprès";
    let mut b = Banc::new(doc, 1000., 600.);
    let g = b.geometrie();
    let (boite, _, _) = g.maths[0];
    b.cliquer(boite.center());
    let r = b.cx.update(|cx| b.etat.read(cx).selected_range());
    assert_eq!(&doc[r.start.min(r.end)..r.start.max(r.end)], "\na+b\n");
}
