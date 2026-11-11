//! Tests headless de l'élément éditeur du live preview et de sa surface de texte (`fr-t3-surface`).
//!
//! Plateforme de test de GPUI, mais avec le système de texte RÉEL de la plateforme (DirectWrite
//! sous Windows) et DM Sans enregistrée : les largeurs sont celles des glyphes, pas la chasse fixe
//! du `NoopTextSystem` des spikes S1 / S1b. Aucun pixel n'est comparé ici (c'est le travail des
//! scènes de T3) : on vérifie la mise en page, la souris, le clavier et le contrat `TextSurface`.
//!
//! Les noms suivent les cas du bead (masquage sans trou, titre plus grand, clic juste après un
//! titre, ← → franchissent un masque, coordonnées aller-retour, 10 000 lignes) et deux cas de la
//! spec e2e `editor-hell` (note ouverte, titres 1 à 6). Les lignes `SURFACE|…` (`--nocapture`)
//! donnent les temps CI.
//!
//! Windows seulement (les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Instant;

use fragment_text::editor::{DocumentSurface as _, EditorPosition, TextSurface as _};
use fragment_theme::{Mode, Theme};
use fragment_ui::editeur::constantes::{INTERLIGNE_TEXTE, MARGE_CONTENU_VERTICALE, debut_texte};
use fragment_ui::editeur::{EditeurLive, OptionsEditeur, style_projete};
use fragment_ui::{polices, theme_global};
use gpui_kit::base::input::EditorState;
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Hsla, Point, ScrollDelta,
    TestAppContext, TestDispatcher, Window, WindowBounds, WindowOptions, point, px, size,
};

const LARGEUR: f32 = 1000.;
const HAUTEUR: f32 = 600.;

struct Banc {
    cx: TestAppContext,
    fenetre: AnyWindowHandle,
    editeur: Entity<EditeurLive>,
    etat: Entity<EditorState>,
}

impl Banc {
    fn new(texte: &str, options: OptionsEditeur) -> Banc {
        Banc::avec_taille(texte, options, LARGEUR, HAUTEUR)
    }

    fn avec_taille(texte: &str, options: OptionsEditeur, l: f32, h: f32) -> Banc {
        let cx = TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            current_platform(false).text_system(),
        );
        polices::enregistrer(cx.text_system()).unwrap();
        cx.update(gpui_kit::init);
        cx.update(|cx| theme_global::installer(cx, Mode::Sombre));
        let texte = texte.to_owned();
        let (fenetre, editeur) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(l), px(h)),
                    })),
                    ..Default::default()
                },
                cx,
                move |window, cx| cx.new(|cx| EditeurLive::new(&texte, options, window, cx)),
            )
            .unwrap()
        });
        let etat = cx.update(|cx| editeur.read(cx).etat().clone());
        let mut b = Banc {
            cx,
            fenetre,
            editeur,
            etat,
        };
        b.stabiliser();
        b
    }

    /// Plusieurs images (la colonne se centre sur la largeur connue après la première) et les
    /// effets en attente (observations, événements).
    fn stabiliser(&mut self) {
        for _ in 0..3 {
            self.cx.run_until_parked();
            self.cx
                .update_window(self.fenetre, |_, window, cx| window.render_frame(cx))
                .unwrap();
        }
        self.cx.run_until_parked();
    }

    /// Une action dans la fenêtre, puis stabilisation.
    fn agir<R>(&mut self, f: impl FnOnce(&mut Window, &mut App, &Entity<EditorState>) -> R) -> R {
        let etat = self.etat.clone();
        let r = self
            .cx
            .update_window(self.fenetre, |_, window, cx| f(window, cx, &etat))
            .unwrap();
        self.stabiliser();
        r
    }

    fn lire<R>(&mut self, f: impl FnOnce(&EditeurLive, &EditorState, &App) -> R) -> R {
        let (editeur, etat) = (self.editeur.clone(), self.etat.clone());
        self.cx.update(|cx| f(editeur.read(cx), etat.read(cx), cx))
    }

    fn bornes(&mut self, o: usize) -> Bounds<gpui_kit::Pixels> {
        self.lire(|_, e, _| e.range_to_bounds(&(o..o + 1)).unwrap())
    }

    /// Un clic à `p` (coordonnées FENÊTRE).
    fn cliquer(&mut self, p: Point<gpui_kit::Pixels>) {
        self.agir(|window, cx, etat| {
            let origine = window.find(("input", etat.entity_id())).bounds().origin;
            window.click_at(("input", etat.entity_id()), p - origine, cx);
        });
    }

    fn placer(&mut self, o: usize) {
        self.agir(|window, cx, etat| {
            etat.update(cx, |s, cx| {
                s.focus(window, cx);
                s.set_selected_range(o..o, cx);
            });
        });
    }

    fn touche(&mut self, k: &str) {
        self.agir(|window, cx, _| window.press(k, cx));
    }

    fn curseur(&mut self) -> usize {
        self.lire(|_, e, _| e.cursor())
    }
}

fn pres(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.
}

// ─── Cas du bead ───

/// `#` et `**` masqués hors curseur SANS TROU : le texte suivant prend leur place ; au curseur, la
/// source revient. La source n'est jamais modifiée.
#[test]
fn masquage_sans_trou() {
    let texte = "# Titre en **gras**\n\nun second paragraphe";
    let mut b = Banc::new(texte, OptionsEditeur::default());
    let debut = b.bornes(0).origin.x;
    let t = b.bornes(2).origin.x;
    assert_eq!(
        t, debut,
        "« T » au bord du texte : aucun trou à la place de « # »"
    );
    // `gras` suit `en ` sans les deux astérisques (« en » s'arrête en 11, « g » est en 13).
    let apres_en = b
        .lire(|_, e, _| e.range_to_bounds(&(10..11)).unwrap())
        .right();
    assert!(pres(f32::from(b.bornes(13).origin.x), f32::from(apres_en)));
    assert_eq!(b.lire(|_, e, _| e.value().to_string()), texte);

    // Le curseur dans la ligne : la source se révèle, le texte se décale.
    b.placer(5);
    let t_revele = b.bornes(2).origin.x;
    assert!(t_revele > debut, "révélé : « # » reprend sa place");
    assert!(!b.lire(|e, _, _| e.arguments_fork().unwrap().masques.contains(&(0..2))));
    assert_eq!(b.lire(|_, e, _| e.value().to_string()), texte);
}

/// Un H1 est plus HAUT (1,9 em × 1,25) et plus GROS que le corps ; la suite est décalée d'autant.
#[test]
fn titre_plus_grand() {
    let texte = "# Titre\nTitre\nfin";
    let mut b = Banc::new(texte, OptionsEditeur::default());
    let lh = b.lire(|_, e, _| f32::from(e.line_height().unwrap()));
    assert!(
        pres(lh, 16. * INTERLIGNE_TEXTE),
        "interligne du corps : {lh}"
    );
    let titre = b.bornes(2);
    let corps = b.lire(|_, e, _| e.range_to_bounds(&(8..13)).unwrap());
    let fin = b.bornes(14);
    let h1 = 1.9 * 16. * 1.25;
    assert!(
        pres(f32::from(titre.size.height), h1),
        "{:?}",
        titre.size.height
    );
    assert!(pres(f32::from(corps.origin.y - titre.origin.y), h1));
    assert!(pres(f32::from(fin.origin.y - corps.origin.y), lh));
    let largeur_titre = b.lire(|_, e, _| e.range_to_bounds(&(2..7)).unwrap().size.width);
    println!(
        "SURFACE|titre|h1={:?} largeur_titre={largeur_titre:?} largeur_corps={:?}",
        titre.size.height, corps.size.width
    );
    assert!(
        f32::from(largeur_titre) > 1.5 * f32::from(corps.size.width),
        "le titre est en gros caractères"
    );
}

/// Un clic juste SOUS un titre tombe dans la ligne suivante ; juste au-dessus de son bas, dans le
/// titre ; à droite de son texte, à sa fin.
#[test]
fn clic_juste_apres_un_titre() {
    let texte = "# Titre\ncorps\nfin";
    let mut b = Banc::new(texte, OptionsEditeur::default());
    let titre = b.bornes(2);
    let bas = titre.origin.y + titre.size.height;
    b.cliquer(point(titre.origin.x + px(4.), bas + px(2.)));
    let c = b.curseur();
    assert!((8..13).contains(&c), "sous le titre : dans « corps » ({c})");
    b.cliquer(point(titre.origin.x + px(4.), bas - px(2.)));
    let c = b.curseur();
    assert!(c <= 7, "au bas du titre : dans le titre ({c})");
    // À droite du texte du titre, sur sa rangée : la fin du titre.
    let fin_titre = b
        .lire(|_, e, _| e.range_to_bounds(&(6..7)).unwrap())
        .top_right();
    b.cliquer(point(fin_titre.x + px(40.), fin_titre.y + px(4.)));
    assert_eq!(b.curseur(), 7);
}

/// En lecture seule, rien n'est révélé : ← → franchissent la plage masquée `# ` d'un seul pas.
#[test]
fn gauche_droite_franchissent_un_masque() {
    let texte = "ab\n# Titre";
    let options = OptionsEditeur {
        lecture_seule: true,
        ..OptionsEditeur::default()
    };
    let mut b = Banc::new(texte, options);
    b.placer(2);
    assert_eq!(
        b.lire(|e, _, _| e.arguments_fork().unwrap().masques.clone()),
        vec![(3..5)]
    );
    b.touche("right");
    assert_eq!(b.curseur(), 3);
    b.touche("right");
    assert_eq!(b.curseur(), 5, "un pas franchit « # »");
    b.touche("right");
    assert_eq!(b.curseur(), 6);
    b.touche("left");
    b.touche("left");
    assert_eq!(b.curseur(), 3, "et de même à rebours");
}

/// `coords_at_pos` → `pos_at_coords` revient à la même position (UTF-16), à l'écran ; hors écran,
/// la ligne est retrouvée par la géométrie des rangées ; le repère est celui du document.
#[test]
fn coordonnees_aller_retour() {
    let mut texte = String::from("# Titre 😀 un\n\nPremier **gras** et `code`.\n");
    for i in 0..200 {
        texte.push_str(&format!("Ligne {i} é\n"));
        if i % 50 == 0 {
            texte.push_str(&format!("## Section {i}\n"));
        }
    }
    let mut b = Banc::new(&texte, OptionsEditeur::default());

    // Le repère : (0, 0) = coin du contenu, marge haute comprise.
    let r0 = b.lire(|e, _, cx| e.surface(cx).coords_at_pos(0).unwrap());
    assert!(pres(r0.top as f32, MARGE_CONTENU_VERTICALE), "{r0:?}");
    assert!(pres(r0.left as f32, debut_texte(LARGEUR)), "{r0:?}");
    let (haut3, r3) = b.lire(|e, _, cx| {
        let s = e.surface(cx);
        let p = s.pos_to_offset(EditorPosition { line: 3, ch: 0 });
        (s.haut_de_ligne(3), s.coords_at_pos(p).unwrap())
    });
    assert!(pres(r3.top as f32, haut3), "{} contre {haut3}", r3.top);

    // Aller-retour : chaque position d'un échantillon (UTF-16 ; « 😀 » en compte deux).
    let positions: Vec<usize> = vec![0, 2, 8, 11, 12, 16, 25, 30, 40];
    for p in positions {
        let (client, retour) = b.lire(|e, etat, cx| {
            let s = e.surface(cx);
            let o = fragment_text::utf16::utf16_to_byte(e.texte(), p).unwrap();
            let bornes = etat.range_to_bounds(&(o..o)).unwrap();
            let c = bornes.center();
            let retour = s.pos_at_coords(f64::from(f32::from(c.x)), f64::from(f32::from(c.y)));
            ((c.x, c.y), retour)
        });
        // Une position masquée (`**`) se confond avec sa voisine visible : même x.
        let attendu_ou_voisin = retour.map(|r| r.abs_diff(p) <= 2);
        assert_eq!(
            attendu_ou_voisin,
            Some(true),
            "{p} → {retour:?} via {client:?}"
        );
    }

    // Hors écran : la ligne 150 est bien au-delà de la fenêtre (600 px).
    let (haut, attendu, trouve) = b.lire(|e, _, cx| {
        let s = e.surface(cx);
        let l = 150;
        let haut = s.haut_de_ligne(l);
        let r0 = s.coords_at_pos(0).unwrap();
        let origine_y = r0.top;
        // Coordonnée client = document − (document de 0 − client de 0).
        let client0 = e.etat().read(cx).range_to_bounds(&(0..0)).unwrap().origin.y;
        let y = f64::from(haut) + 2. - origine_y + f64::from(f32::from(client0));
        (
            haut,
            s.pos_to_offset(EditorPosition { line: l, ch: 0 }),
            s.pos_at_coords(100., y),
        )
    });
    assert!(haut > HAUTEUR * 2., "la ligne 150 est hors écran ({haut})");
    assert_eq!(
        trouve,
        Some(attendu),
        "hors écran : le début de la ligne visée"
    );

    // Le reste du contrat, en UTF-16.
    b.lire(|e, _, cx| {
        let s = e.surface(cx);
        assert_eq!(s.get_line(0), "# Titre 😀 un");
        assert_eq!(s.offset_to_pos(11), EditorPosition { line: 0, ch: 11 });
        assert_eq!(s.offset_to_pos(14), EditorPosition { line: 1, ch: 0 });
        assert_eq!(s.line_count(), e.lignes().len());
        let t = s.get_text();
        assert_eq!(t.text().map(|t| t.text.as_str()), Some(e.texte()));
        assert!(!s.coords_for_range(15, 30).is_empty());
    });
    b.placer(fragment_text::utf16::utf16_to_byte(&texte, 12).unwrap());
    assert_eq!(
        b.lire(|e, _, cx| e.surface(cx).get_selection()),
        fragment_text::editor::EditorRange { from: 12, to: 12 }
    );
}

/// 10 000 lignes (1 000 titres, du gras, des blocs de code) avec DirectWrite : pose du modèle,
/// défilement, frappe au milieu. Les temps sont imprimés (`SURFACE|10k|…`), pas comparés.
#[test]
fn dix_mille_lignes() {
    let mut texte = String::new();
    for i in 0..10_000 {
        match i % 20 {
            0 => texte.push_str(&format!("# Titre {i}\n")),
            5 => texte.push_str("```rust\n"),
            6 => texte.push_str("fn f(x: u32) -> u32 { x + 1 } // commentaire\n"),
            7 => texte.push_str("```\n"),
            3 => texte.push_str(&format!(
                "Ligne {i} avec **gras**, `code` et ==surligné==.\n"
            )),
            _ => texte.push_str(&format!(
                "Ligne {i} de texte lorem ipsum dolor sit amet, consectetur adipiscing\n"
            )),
        }
    }
    let t = Instant::now();
    let mut b = Banc::new(&texte, OptionsEditeur::default());
    let ouverture_ms = ms(t);
    let masques = b.lire(|e, _, _| e.arguments_fork().unwrap().masques.len());
    assert!(masques >= 1_000, "{masques}");

    let lh = b.lire(|_, e, _| e.line_height().unwrap());
    let mut pire = 0f64;
    let t_tout = Instant::now();
    for _ in 0..100 {
        let t = Instant::now();
        b.agir(|window, cx, etat| {
            window.scroll(
                ("input", etat.entity_id()),
                ScrollDelta::Pixels(point(px(0.), -lh * 3.)),
                cx,
            );
        });
        pire = pire.max(ms(t));
    }
    let defilement_ms = ms(t_tout) / 100.;
    let visibles = b.lire(|_, e, _| e.visible_row_range());
    assert!(
        visibles.as_ref().is_some_and(|r| r.start > 100),
        "{visibles:?}"
    );

    let milieu = texte.floor_char_boundary(texte.len() / 2);
    b.placer(milieu);
    let t_frappe = Instant::now();
    for _ in 0..20 {
        b.agir(|window, cx, _| window.input("x", cx));
    }
    let frappe_ms = ms(t_frappe) / 20.;
    let t_curseur = Instant::now();
    for _ in 0..20 {
        b.touche("down");
    }
    let curseur_ms = ms(t_curseur) / 20.;
    assert_eq!(b.lire(|e, _, _| e.texte().len()), texte.len() + 20);
    println!(
        "SURFACE|10k|octets={} masques={masques} ouverture_ms={ouverture_ms:.1} \
         defilement_moy_ms={defilement_ms:.2} defilement_max_ms={pire:.2} \
         frappe_moy_ms={frappe_ms:.2} curseur_moy_ms={curseur_ms:.2} visibles={visibles:?}",
        texte.len()
    );
    assert!(defilement_ms < 2000., "défilement absurdement lent");
}

// ─── Garde, thèmes, spec e2e ───

/// Un déplacement qui ne change pas le modèle ne le repose PAS (garde de comparaison) ; entrer dans
/// un titre le repose une fois.
#[test]
fn garde_de_comparaison() {
    let texte = "# Titre\n\nun paragraphe assez long\nfin";
    let mut b = Banc::new(texte, OptionsEditeur::default());
    b.placer(12);
    let poses = b.lire(|e, _, _| e.poses());
    b.touche("right");
    b.touche("right");
    b.touche("left");
    assert_eq!(
        b.lire(|e, _, _| e.poses()),
        poses,
        "même modèle : aucune pose"
    );
    b.placer(3);
    assert_eq!(
        b.lire(|e, _, _| e.poses()),
        poses + 1,
        "titre révélé : une pose"
    );
}

/// Le curseur prend `--text-normal` : visible (contraste fort avec le fond) dans les deux thèmes.
#[test]
fn caret_visible_dans_les_deux_themes() {
    let luminance = |c: Hsla| c.l;
    for theme in [Theme::sombre(), Theme::clair()] {
        let s = style_projete(&theme);
        assert_eq!(s.caret, s.foreground);
        assert!(s.caret.a > 0.99, "opaque");
        let ecart = (luminance(s.caret) - luminance(s.background)).abs();
        assert!(ecart > 0.4, "{:?} : écart de luminance {ecart}", theme.mode);
    }
}

/// Spec e2e `editor-hell` : « note ouverte : RIEN n'est révélé avant que l'utilisateur ait mis son
/// curseur dedans (la 1re ligne est rendue) ; puis comme avant ».
#[test]
fn note_ouverte_rien_n_est_revele_avant_le_premier_focus() {
    let texte = "# Titre en **gras**\n\nun second paragraphe";
    let mut b = Banc::new(texte, OptionsEditeur::default());
    let premiere = |b: &mut Banc| {
        b.lire(|e, _, _| {
            e.projection()
                .decorations
                .texte_visible(e.texte(), 0, 19, |_| String::new())
        })
    };
    assert!(!b.lire(|e, _, _| e.a_le_focus()));
    assert_eq!(premiere(&mut b), "Titre en gras");
    b.placer(25);
    assert_eq!(premiere(&mut b), "Titre en gras");
    b.placer(14);
    assert_eq!(premiere(&mut b), texte.lines().next().unwrap());
    // Perte du focus : la ligne du curseur reste en source.
    b.agir(|window, cx, _| window.blur(cx));
    assert!(premiere(&mut b).contains("**gras**"));
}

/// Spec e2e `editor-hell` : « titres 1 à 6 : marques cachées hors curseur, révélées au curseur »,
/// tailles décroissantes.
#[test]
fn titres_1_a_6_marques_cachees_hors_curseur_revelees_au_curseur() {
    let texte = (1..=6)
        .map(|k| format!("{} T{k}", "#".repeat(k)))
        .collect::<Vec<_>>()
        .join("\n")
        + "\nfin";
    let mut b = Banc::new(&texte, OptionsEditeur::default());
    b.placer(texte.len());
    let (masques, polices) = b.lire(|e, _, _| {
        let f = e.arguments_fork().unwrap();
        (f.masques.len(), f.polices.clone())
    });
    assert_eq!(masques, 6);
    for k in 1..6 {
        assert!(polices[k - 1].1 >= polices[k].1, "{polices:?}");
    }
    // Le curseur dans le titre 3 : sa source se révèle, les autres restent masquées.
    let l3 = texte.find("### ").unwrap();
    b.placer(l3 + 5);
    let masques = b.lire(|e, _, _| e.arguments_fork().unwrap().masques.clone());
    assert_eq!(masques.len(), 5);
    assert!(masques.iter().all(|m| m.start != l3));
}

/// Basculer le thème reprojette les couleurs des marques (le lien prend `link_color` du thème).
#[test]
fn les_marques_suivent_le_theme() {
    let texte = "voir [lien](https://exemple.org) ici\n\nfin";
    let mut b = Banc::new(texte, OptionsEditeur::default());
    let couleur_lien = |b: &mut Banc| {
        b.lire(|e, _, _| {
            e.projection()
                .marques
                .iter()
                .find(|(r, _)| r.start == 5)
                .and_then(|(_, s)| s.color)
        })
    };
    let sombre = couleur_lien(&mut b);
    b.agir(|_, cx, _| theme_global::basculer(cx, Mode::Clair));
    let clair = couleur_lien(&mut b);
    assert!(sombre.is_some() && clair.is_some());
    let attendu: Hsla = theme_global::couleur(Theme::clair().palette.link_color).into();
    assert_eq!(clair, Some(attendu));
}
