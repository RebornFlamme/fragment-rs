//! Tests headless du HTML dans le Markdown (`fr-t3-html-rendu`, ADR-012).
//!
//! Un `#[test]` par cas de la spec e2e `markdown-html` de Fragment (ses cinq premiers : les trois
//! réponses de l'agent vont à T7), même nom en snake_case. La note est RÉÉCRITE pour ce dépôt
//! (ADR-007) : mêmes ingrédients (div stylée, SVG animé à la couleur du thème, `details`, pièges,
//! commentaire, balises en ligne, lien `javascript:`), autres mots et autres valeurs.
//!
//! Même plateforme que `tests/md_blocs.rs` : GPUI de test, système de texte RÉEL (DirectWrite).
//! Windows seulement.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fragment_markdown::syntaxe::modele_rangees::lignes_de;
use fragment_theme::{Mode, Theme};
use fragment_ui::editeur::constantes::debut_texte;
use fragment_ui::editeur::{EditeurLive, OptionsEditeur};
use fragment_ui::md_html::arbre::{self, Noeud};
use fragment_ui::md_html::{BlocHtml, HoteHtml, svg};
use fragment_ui::{polices, theme_global};
use gpui_kit::base::input::EditorState;
use gpui_kit::platform::current_platform;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Entity, Hsla, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, Point, TestAppContext,
    TestDispatcher, Window, WindowBounds, WindowOptions, point, px, size,
};

/// ★ Borne chaque test (voir `tests/editeur.rs`) : un test figé échoue en 5 min, nommé.
const DELAI_TEST: Duration = Duration::from_secs(300);

/// La note de la spec, réécrite.
const NOTE: &str = "# Balises

<div class=\"cadre\" style=\"color: rgb(180, 20, 20); border: 1px solid\">Cadre rouge</div>

<svg id=\"anime\" width=\"120\" height=\"30\" viewBox=\"0 0 120 30\"><circle id=\"point\" cx=\"15\" cy=\"15\" r=\"6\" style=\"fill: var(--color-accent)\"><animate attributeName=\"cx\" values=\"15;105;15\" dur=\"2s\" repeatCount=\"indefinite\"/></circle></svg>

<details id=\"pli\"><summary id=\"titre-pli\">Déplier</summary>Texte replié</details>

<p id=\"leurre\" onclick=\"window.__x = 1\">Leurre</p>
<script>window.__y = 1</script>

<!-- une note cachée -->

Ligne avec <span style=\"color: rgb(10, 140, 10)\">verte</span>, <mark>marquée</mark>, <u>soulignée</u>, CO<sub>2</sub> puis<br>suite.

Un <a id=\"ancre\" href=\"javascript:window.__z=1\">lien douteux</a> là.

Fin.";

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
        eprintln!("HTML|bloque|{nom} dépasse {DELAI_TEST:?} : arrêt du processus");
        std::process::exit(101);
    });
}

struct Banc {
    /// ★ JAMAIS détruit (voir `tests/editeur.rs`).
    cx: ManuallyDrop<TestAppContext>,
    fenetre: AnyWindowHandle,
    hote: Entity<HoteHtml>,
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
                        size: size(px(1000.), px(1400.)),
                    })),
                    ..Default::default()
                },
                cx,
                move |window, cx| {
                    cx.new(|cx| HoteHtml::new(&texte, OptionsEditeur::default(), None, window, cx))
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
        for _ in 0..5 {
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

    fn lire<R>(&mut self, f: impl FnOnce(&App) -> R) -> R {
        self.cx.update(|cx| f(cx))
    }

    fn texte(&mut self) -> String {
        let e = self.editeur.clone();
        self.lire(|cx| e.read(cx).texte().to_owned())
    }

    /// Le texte VISIBLE de la ligne qui commence par `debut` : sa source moins ce que le fork masque.
    fn ligne_visible(&mut self, debut: &str) -> String {
        let texte = self.texte();
        let e = self.editeur.clone();
        let masques = self.lire(|cx| {
            e.read(cx)
                .arguments_fork()
                .map(|f| f.masques.clone())
                .unwrap_or_default()
        });
        let l = lignes_de(&texte)
            .into_iter()
            .find(|l| l.texte(&texte).starts_with(debut))
            .unwrap_or_else(|| panic!("pas de ligne « {debut} »"));
        let (de, a) = (l.octets.from, l.octets.to);
        let mut s = String::new();
        let mut o = de;
        for m in masques.iter().filter(|m| m.end > de && m.start < a) {
            if m.start > o {
                s.push_str(&texte[o..m.start]);
            }
            o = o.max(m.end);
        }
        if o < a {
            s.push_str(&texte[o..a]);
        }
        s
    }

    /// Tout le texte visible de la note.
    fn visible(&mut self) -> String {
        let texte = self.texte();
        let debuts: Vec<String> = texte
            .split('\n')
            .filter(|l| !l.is_empty())
            .map(|l| l.chars().take(12).collect())
            .collect();
        debuts
            .iter()
            .map(|d| self.ligne_visible(d))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn widgets(&mut self) -> Vec<Entity<BlocHtml>> {
        let h = self.hote.clone();
        self.lire(|cx| h.read(cx).widgets().to_vec())
    }

    /// Le widget rendu qui contient l'élément d'`id` ou de classe donnés (`#x`, `.x`).
    fn widget(&mut self, sel: &str) -> Option<Entity<BlocHtml>> {
        let ws = self.widgets();
        ws.into_iter().find(|w| {
            self.lire(|cx| {
                let a = w.read(cx).arbre();
                let mut trouve = false;
                arbre::parcourir(a, &mut |e| {
                    trouve |= match sel.split_at(1) {
                        ("#", id) => e.attribut("id") == Some(id),
                        (".", c) => e.a_la_classe(c),
                        _ => false,
                    };
                });
                trouve
            })
        })
    }

    fn placer(&mut self, o: usize) {
        self.agir(|window, cx, etat| {
            etat.update(cx, |s, cx| {
                s.focus(window, cx);
                s.set_selected_range(o..o, cx);
            });
        });
    }

    fn placer_sur(&mut self, motif: &str) {
        let o = self.texte().find(motif).expect("motif");
        self.placer(o);
    }

    fn souris(&mut self, ev: PlatformInput) {
        self.agir(|window, cx, _| {
            window.dispatch_event(ev, cx);
        });
    }

    fn survoler(&mut self, p: Point<gpui_kit::Pixels>) {
        self.souris(PlatformInput::MouseMove(MouseMoveEvent {
            position: p,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }));
    }

    fn appuyer(&mut self, p: Point<gpui_kit::Pixels>) {
        self.souris(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: p,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
    }

    fn cliquer(&mut self, p: Point<gpui_kit::Pixels>) {
        self.survoler(p);
        self.appuyer(p);
        self.souris(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: p,
            modifiers: Modifiers::default(),
            click_count: 1,
        }));
    }

    /// Le texte de la ligne du curseur.
    fn ligne_du_curseur(&mut self) -> String {
        let texte = self.texte();
        let e = self.etat.clone();
        let o = self.lire(|cx| e.read(cx).cursor());
        let debut = texte[..o].rfind('\n').map_or(0, |i| i + 1);
        let fin = texte[o..].find('\n').map_or(texte.len(), |i| o + i);
        texte[debut..fin].to_owned()
    }

    /// Le bord gauche (fenêtre) du premier caractère de la ligne `debut`.
    fn bord_du_texte(&mut self, debut: &str) -> f32 {
        let o = self.texte().find(debut).expect("ligne");
        let e = self.etat.clone();
        self.lire(|cx| f32::from(e.read(cx).range_to_bounds(&(o..o)).expect("boîte").origin.x))
    }

    /// Un clic au milieu de la ligne `debut`, à 12 px du début du texte.
    fn cliquer_ligne(&mut self, debut: &str) {
        let texte = self.texte();
        let l = texte
            .split('\n')
            .position(|l| l.starts_with(debut))
            .expect("ligne");
        let h = self.hote.clone();
        let g = self.lire(|cx| h.read(cx).blocs().read(cx).geometrie());
        let le = *g.ligne(l).expect("ligne visible");
        let x = g.gauche + debut_texte(g.largeur) + 12.0;
        self.cliquer(point(px(x), px(le.haut + le.hauteur / 2.0)));
    }
}

fn hsla(c: fragment_theme::Couleur) -> Hsla {
    theme_global::couleur(c).into()
}

/// « blocs rendus : div stylée, SVG ANIMÉ (couleur du thème), details qui s'ouvre ; rien du HTML
/// brut ».
#[test]
fn blocs_rendus_div_stylee_svg_anime_couleur_du_theme_details_qui_s_ouvre_rien_du_html_brut() {
    let mut b = Banc::new(NOTE);
    b.placer(0);
    // La div : rendue, à sa couleur, son texte ; aucune balise visible.
    let div = b.widget(".cadre").expect("bloc .cadre rendu");
    let (cadre, couleur_cadre, boite) = b.lire(|cx| {
        let w = div.read(cx);
        let mut e = None;
        arbre::parcourir(w.arbre(), &mut |x| {
            if x.a_la_classe("cadre") {
                e = Some(x.clone());
            }
        });
        let e = e.expect("élément");
        let c = fragment_ui::md_html::bloc::couleur_appliquee(&Theme::sombre(), &e, 16.0);
        let boite = w.geometrie().elements.get(".cadre").copied();
        (e, c, boite)
    });
    assert_eq!(
        couleur_cadre,
        Some(fragment_theme::Couleur::octets(180, 20, 20, 255))
    );
    assert_eq!(cadre.texte(), "Cadre rouge");
    let visible = b.visible();
    assert!(!visible.contains("<div"), "{visible}");
    assert!(
        !visible.contains("<svg") && !visible.contains("<details"),
        "{visible}"
    );
    // Le rendu commence au bord du texte, au pixel.
    let boite = boite.expect("boîte de .cadre peinte");
    let bord = b.bord_du_texte("Fin.");
    assert!(
        (f32::from(boite.origin.x) - bord).abs() < 1.0,
        "bloc à {:?}, texte à {bord}",
        boite.origin.x
    );
    // Le SVG tourne : sa position animée change avec le temps, et l'horloge avance.
    let anime = b.widget("#anime").expect("SVG rendu");
    let (est_anime, svg_el, t) = b.lire(|cx| {
        let w = anime.read(cx);
        let Some(Noeud::Element(e)) = w.arbre().first().cloned() else {
            panic!("svg")
        };
        (w.anime(), e, w.temps())
    });
    assert!(est_anime);
    assert!(t > 0.0);
    let theme = Theme::sombre();
    let a = svg::source_a(&theme, &svg_el, 0.0);
    let z = svg::source_a(&theme, &svg_el, 0.5);
    assert!(a.contains("cx=\"15\""), "{a}");
    assert!(z.contains("cx=\"60\""), "{z}");
    // La couleur du thème (variable de l'app) s'applique.
    let accent = theme.palette.color_accent;
    assert!(
        a.contains(&format!("rgb({}, {}, {})", accent.r, accent.g, accent.b)),
        "{a}"
    );
    assert!(svg::rasteriser(&a, 1.0).is_some());
    // details : interactif, sans révéler la source.
    let det = b.widget("#pli").expect("details rendu");
    assert_eq!(b.lire(|cx| det.read(cx).details_ouvert("pli")), Some(false));
    let boite = b
        .lire(|cx| det.read(cx).geometrie().elements.get("#pli").copied())
        .expect("boîte du details");
    let avant = b.texte();
    b.cliquer(point(boite.origin.x + px(8.), boite.origin.y + px(6.)));
    assert_eq!(b.lire(|cx| det.read(cx).details_ouvert("pli")), Some(true));
    let det = b.widget("#pli").expect("toujours rendu");
    assert!(b.lire(|cx| det.read(cx).details_ouvert("pli")) == Some(true));
    assert_eq!(b.texte(), avant);
    assert!(!b.visible().contains("<details"));
}

/// « ASSAINI : <script> ne tourne pas, onclick et javascript: neutralisés ».
#[test]
fn assaini_script_ne_tourne_pas_onclick_et_javascript_neutralises() {
    let mut b = Banc::new(NOTE);
    let leurre = b.widget("#leurre").expect("bloc du leurre rendu");
    let (p, scripts) = b.lire(|cx| {
        let a = leurre.read(cx).arbre();
        let mut n = 0;
        arbre::parcourir(a, &mut |e| n += usize::from(e.nom_bas() == "script"));
        (arbre::par_id(a, "leurre").cloned().expect("p"), n)
    });
    assert_eq!(p.texte(), "Leurre");
    assert_eq!(p.attribut("onclick"), None);
    assert_eq!(scripts, 0);
    // Aucun widget ne porte de script, ni le texte du script.
    for w in b.widgets() {
        let src = b.lire(|cx| {
            let mut s = String::new();
            arbre::parcourir(w.read(cx).arbre(), &mut |e| s.push_str(&e.nom_bas()));
            s
        });
        assert!(!src.contains("script"));
    }
    // Le lien en ligne : son texte balisé par un `a` SANS href.
    let h = b.hote.clone();
    let el = b.lire(|cx| h.read(cx).en_ligne());
    let texte = b.texte();
    let lien = el
        .marques
        .iter()
        .find(|m| m.plage.texte(&texte) == "lien douteux")
        .expect("lien balisé");
    assert_eq!(lien.balise.tag_name, "a");
    assert!(
        lien.balise
            .attributes
            .iter()
            .any(|(n, v)| n == "id" && v == "ancre")
    );
    assert!(lien.balise.attributes.iter().all(|(n, _)| n != "href"));
    assert_eq!(b.ligne_visible("Un "), "Un lien douteux là.");
    // Un clic sur le leurre ne fait rien d'autre que de rester sur le rendu.
    let boite = b
        .lire(|cx| leurre.read(cx).geometrie().elements.get("#leurre").copied())
        .expect("boîte");
    b.cliquer(boite.center());
    assert_eq!(b.texte(), NOTE);
    assert!(b.widget("#leurre").is_some());
}

/// « commentaire caché ; en ligne : span stylé, mark, u, sub, br rendus, balises cachées ».
#[test]
fn commentaire_cache_en_ligne_span_style_mark_u_sub_br_rendus_balises_cachees() {
    let mut b = Banc::new(NOTE);
    assert!(!b.visible().contains("note cachée"));
    let l = b.ligne_visible("Ligne avec");
    assert!(!l.contains('<'), "{l}");
    assert_eq!(l, "Ligne avec verte, marquée, soulignée, CO2 puissuite.");
    let texte = b.texte();
    let h = b.hote.clone();
    let el = b.lire(|cx| h.read(cx).en_ligne());
    let marques = b.lire(|cx| h.read(cx).marques().to_vec());
    let style_de = |mot: &str| {
        let o = texte.find(&format!(">{mot}<")).expect("mot") + 1;
        marques
            .iter()
            .find(|d| d.range == (o..o + mot.len()))
            .unwrap_or_else(|| panic!("pas de marque sur « {mot} »"))
            .style
    };
    assert_eq!(
        style_de("verte").color,
        Some(hsla(fragment_theme::Couleur::octets(10, 140, 10, 255)))
    );
    let theme = Theme::sombre();
    assert_eq!(
        style_de("marquée").background_color,
        Some(hsla(theme.palette.text_highlight_bg))
    );
    assert!(style_de("soulignée").underline.is_some());
    let sub = el
        .marques
        .iter()
        .find(|m| m.plage.texte(&texte) == "2")
        .expect("sub");
    assert_eq!(sub.balise.tag_name, "sub");
    assert_eq!(el.entiers.iter().filter(|e| e.nom == "br").count(), 1);
}

/// « la SOURCE au curseur (bloc et en ligne), le rendu revient en sortant ; le bouton </> révèle
/// le bloc ».
#[test]
fn la_source_au_curseur_bloc_et_en_ligne_le_rendu_revient_en_sortant_le_bouton_revele_le_bloc() {
    let mut b = Banc::new(NOTE);
    b.placer_sur("Cadre rouge");
    assert!(b.ligne_visible("<div").contains("<div class=\"cadre\""));
    assert!(b.widget(".cadre").is_none());
    b.placer_sur("verte</span>");
    assert!(
        b.ligne_visible("Ligne avec")
            .contains("<span style=\"color: rgb(10, 140, 10)\">verte</span>")
    );
    b.placer(0);
    assert!(b.widget(".cadre").is_some());
    assert!(!b.ligne_visible("Ligne avec").contains("<span"));
    // Le bouton « </> » du coin : visible au survol, il ramène la source.
    let w = b.widget(".cadre").expect("bloc");
    let boite = b.lire(|cx| w.read(cx).geometrie().boite);
    b.survoler(boite.center());
    let bouton = b
        .lire(|cx| w.read(cx).geometrie().bouton)
        .expect("bouton « </> »");
    b.appuyer(bouton.center());
    assert!(b.ligne_visible("<div").contains("<div class=\"cadre\""));
    assert!(b.widget(".cadre").is_none());
    assert!(b.ligne_du_curseur().starts_with("<div class=\"cadre\""));
}

/// « éditer autour d'un bloc HTML : frappe et annulation, le rendu suit ».
#[test]
fn editer_autour_d_un_bloc_html_frappe_et_annulation_le_rendu_suit() {
    let mut b = Banc::new(NOTE);
    // Un CLIC sous les blocs HTML tombe sur la bonne ligne (hauteurs mesurées justes).
    b.cliquer_ligne("Fin.");
    assert_eq!(b.ligne_du_curseur(), "Fin.");
    b.agir(|window, cx, _| window.press("end", cx));
    b.agir(|window, cx, _| window.input(" Ajout.", cx));
    assert_eq!(b.ligne_du_curseur(), "Fin. Ajout.");
    assert!(b.widget(".cadre").is_some());
    b.agir(|window, cx, _| window.press("ctrl-z", cx));
    assert_eq!(b.ligne_du_curseur(), "Fin.");
    assert_eq!(b.texte(), NOTE);
    assert!(b.widget(".cadre").is_some());
}
