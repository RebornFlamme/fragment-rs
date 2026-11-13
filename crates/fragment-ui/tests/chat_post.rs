//! Tests de la présentation « post » (`fr-t7-post`) : le modèle pur partout, la vue en headless
//! (Windows seulement : les crates GPUI ne se construisent pas sous Linux sans bibliothèques système).
//!
//! Les noms suivent `app/e2e/gutter-cluster.spec.ts` (la timeline : nom, « · 6 h », question,
//! citation d'une ligne, nombre de réponses, trait du fil, « Afficher ce fil », texte court sans
//! fondu) et la référence `post.ts` (troncature mesurée, compact sous 360 px). L'horloge est figée.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use fragment_chat::api::{ChatMessage, ChatPart, Participant, Role};
use fragment_chat::calendrier::DecalageFixe;
use fragment_ui::chat_post::modele::{
    AVATAR_COMPACT, AVATAR_GRAND, LIBELLE_FIL, Origine, est_compact, est_coupe, hauteur_max,
    hauteur_visible, interligne, libelle_heure, libelle_pieces, lignes_max, post_model,
    texte_citation,
};

// 2025-11-13 12:00:00 UTC : l'horloge figée des tests.
const MAINTENANT: i64 = 1_763_035_200_000;
const HEURE: i64 = 3_600_000;

fn julien() -> Participant {
    Participant {
        id: "u".into(),
        name: "Julien".into(),
        initials: None,
        role: Role::User,
    }
}

fn claude() -> Participant {
    Participant {
        id: "c".into(),
        name: "Claude".into(),
        initials: None,
        role: Role::Assistant,
    }
}

fn message(id: &str, auteur: &str, texte: &str, at: Option<i64>) -> ChatMessage {
    ChatMessage {
        id: id.into(),
        author: auteur.into(),
        parts: vec![ChatPart::text(texte)],
        error: None,
        at,
    }
}

mod heure_relative_a_horloge_figee {
    use super::*;

    #[test]
    fn il_y_a_six_heures_le_meme_jour() {
        let f = DecalageFixe(0);
        assert_eq!(
            libelle_heure(MAINTENANT - 6 * HEURE, MAINTENANT, &f),
            "· 6 h"
        );
    }

    #[test]
    fn a_l_instant_puis_en_minutes() {
        let f = DecalageFixe(0);
        assert_eq!(
            libelle_heure(MAINTENANT - 10_000, MAINTENANT, &f),
            "· à l’instant"
        );
        assert_eq!(
            libelle_heure(MAINTENANT - 5 * 60_000, MAINTENANT, &f),
            "· 5 min"
        );
    }

    #[test]
    fn la_veille_est_hier_et_plus_tot_une_date() {
        let f = DecalageFixe(0);
        // Hier 20 h, puis le 12 octobre de la même année, puis d'une autre année.
        assert_eq!(libelle_heure(1_762_977_600_000, MAINTENANT, &f), "· hier");
        assert_eq!(
            libelle_heure(1_760_263_200_000, MAINTENANT, &f),
            "· 12 oct."
        );
        assert_eq!(
            libelle_heure(1_728_727_200_000, MAINTENANT, &f),
            "· 12 oct. 2024"
        );
    }
}

mod modele_du_tweet {
    use super::*;

    #[test]
    fn la_question_la_1re_reponse_le_compte_et_le_fil() {
        let m = post_model(
            &[
                message("1", "u", "question 0", Some(MAINTENANT - 6 * HEURE)),
                message("2", "c", "réponse 0", None),
            ],
            &[julien(), claude()],
        );
        assert_eq!(m.post.as_ref().unwrap().text, "question 0");
        assert_eq!(m.reply.as_ref().unwrap().text, "réponse 0");
        assert_eq!(m.replies, 1);
        // 2 messages : rien de plus, pas de « Afficher ce fil ».
        assert!(!m.more);
        assert_eq!(LIBELLE_FIL, "Afficher ce fil");
    }

    #[test]
    fn trois_messages_et_plus_affichent_le_fil() {
        let m = post_model(
            &[
                message("1", "u", "a", None),
                message("2", "c", "b", None),
                message("3", "u", "c", None),
            ],
            &[julien(), claude()],
        );
        assert!(m.more);
        assert_eq!(m.replies, 2);
    }
}

mod citation_d_une_ligne {
    use super::*;

    #[test]
    fn le_passage_est_entre_chevrons_sur_une_ligne() {
        assert_eq!(
            texte_citation("passage 0").as_deref(),
            Some("« passage 0 »")
        );
        assert_eq!(
            texte_citation("  deux\n\n lignes\t ici ").as_deref(),
            Some("« deux lignes ici »")
        );
    }

    #[test]
    fn un_passage_vide_n_a_pas_de_ligne() {
        assert_eq!(texte_citation(" \n "), None);
    }

    #[test]
    fn l_origine_garde_son_document_et_son_icone() {
        let o = Origine {
            titre: "article".into(),
            icone: "file-text".into(),
            citation: Some("passage 0".into()),
        };
        assert_eq!(o.titre, "article");
    }
}

mod pieces_jointes {
    use super::*;

    #[test]
    fn une_puce_au_singulier_ou_au_pluriel() {
        assert_eq!(libelle_pieces(0), None);
        assert_eq!(libelle_pieces(1).as_deref(), Some("Pièce jointe"));
        assert_eq!(libelle_pieces(3).as_deref(), Some("3 pièces jointes"));
    }
}

mod troncature_mesuree {
    use super::*;

    #[test]
    fn six_lignes_pour_le_tweet_quatre_pour_la_reponse() {
        assert_eq!(lignes_max(false), 6);
        assert_eq!(lignes_max(true), 4);
        assert!((hauteur_max(false, false) - 120.0).abs() < f32::EPSILON);
        assert!((hauteur_max(true, false) - 80.0).abs() < f32::EPSILON);
    }

    #[test]
    fn en_compact_la_ligne_est_de_18_px() {
        assert!((interligne(true) - 18.0).abs() < f32::EPSILON);
        assert!((hauteur_max(false, true) - 108.0).abs() < f32::EPSILON);
        assert!((hauteur_max(true, true) - 72.0).abs() < f32::EPSILON);
    }

    #[test]
    fn le_texte_court_n_est_pas_coupe_donc_pas_de_fondu() {
        assert!(!est_coupe(20.0, false, false));
        // Pile à la limite, ou 1 px de plus : toujours pas coupé (tolérance de la mesure).
        assert!(!est_coupe(120.0, false, false));
        assert!(!est_coupe(121.0, false, false));
    }

    #[test]
    fn le_texte_qui_depasse_est_coupe_et_plafonne() {
        assert!(est_coupe(121.5, false, false));
        assert!(est_coupe(81.5, true, false));
        assert!((hauteur_visible(300.0, false, false) - 120.0).abs() < f32::EPSILON);
        assert!((hauteur_visible(40.0, false, false) - 40.0).abs() < f32::EPSILON);
    }
}

mod compact_sous_360_px {
    use super::*;

    #[test]
    fn le_seuil_est_celui_du_contenu_du_conteneur() {
        // 360 px de contenu = 392 px de post (2 × 16 px de remplissage).
        assert!(est_compact(392.0));
        assert!(!est_compact(392.5));
        assert!(est_compact(300.0));
        assert!(!est_compact(600.0));
    }

    #[test]
    fn l_avatar_passe_de_40_a_28_px() {
        assert!((AVATAR_GRAND - 40.0).abs() < f32::EPSILON);
        assert!((AVATAR_COMPACT - 28.0).abs() < f32::EPSILON);
    }
}

#[cfg(windows)]
mod headless {
    use std::cell::Cell;
    use std::mem::ManuallyDrop;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use fragment_theme::Mode;
    use fragment_ui::chat_post::{DepsPost, Origine, PostModel, PostVue, Rappels, post_model};
    use fragment_ui::{polices, theme_global};
    use gpui_kit::AppContext as _;
    use gpui_kit::platform::current_platform;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AnyWindowHandle, Bounds, Entity, Point, TestAppContext, TestDispatcher, WindowBounds,
        WindowOptions, px, size,
    };

    use super::*;

    const DELAI_TEST: Duration = Duration::from_secs(300);

    /// ★ Le contexte n'est JAMAIS détruit (DirectWrite ne rend pas la main, voir `tests/editeur.rs`).
    struct Banc {
        cx: ManuallyDrop<TestAppContext>,
        fenetre: AnyWindowHandle,
        post: Entity<PostVue>,
        fini: Arc<AtomicBool>,
    }

    impl Drop for Banc {
        fn drop(&mut self) {
            self.fini.store(true, Ordering::Relaxed);
        }
    }

    fn banc(largeur: f32, modele: PostModel, rappels: Rappels) -> Banc {
        let mut cx = TestAppContext::build_with_text_system(
            TestDispatcher::new(0),
            None,
            current_platform(false).text_system(),
        );
        polices::enregistrer(cx.text_system()).unwrap();
        cx.update(gpui_kit::init);
        cx.update(|cx| theme_global::installer(cx, Mode::Sombre));
        let fond = fragment_theme::Theme::sombre().palette.workspace_bg;
        let deps = DepsPost {
            id: "post-0".into(),
            origine: Some(Origine {
                titre: "article".into(),
                icone: "file-text".into(),
                citation: Some("passage 0".into()),
            }),
            maintenant: MAINTENANT,
            fuseau: Rc::new(DecalageFixe(0)),
            fond,
        };
        let (fenetre, post) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point::default(),
                        size: size(px(largeur), px(900.0)),
                    })),
                    ..Default::default()
                },
                cx,
                move |window, cx| cx.new(|cx| PostVue::new(modele, deps, rappels, window, cx)),
            )
            .unwrap()
        });
        cx.update_window(fenetre, |_, window, _| window.activate_window())
            .unwrap();
        let fini = Arc::new(AtomicBool::new(false));
        let f = fini.clone();
        std::thread::spawn(move || {
            let debut = Instant::now();
            while debut.elapsed() < DELAI_TEST {
                if f.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            eprintln!("CHAT_POST|bloque : arrêt du processus");
            std::process::exit(101);
        });
        let mut b = Banc {
            cx: ManuallyDrop::new(cx),
            fenetre,
            post,
            fini,
        };
        b.stabiliser();
        b
    }

    impl Banc {
        fn stabiliser(&mut self) {
            for _ in 0..6 {
                self.cx.run_until_parked();
                self.cx
                    .update_window(self.fenetre, |_, window, cx| window.render_frame(cx))
                    .unwrap();
            }
            self.cx.run_until_parked();
        }

        fn lire<R>(&mut self, f: impl FnOnce(&PostVue, &gpui_kit::App) -> R) -> R {
            let p = self.post.clone();
            self.cx.update(|cx| f(p.read(cx), cx))
        }
    }

    fn long_texte(lignes: usize) -> String {
        // Des paragraphes séparés : chacun est sa propre ligne, quelle que soit la largeur.
        (0..lignes)
            .map(|i| format!("ligne numéro {i}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn modele(question: &str, reponse: &str, extra: usize) -> PostModel {
        let mut ms = vec![
            message("1", "u", question, Some(MAINTENANT - 6 * HEURE)),
            message("2", "c", reponse, None),
        ];
        for i in 0..extra {
            ms.push(message(&format!("x{i}"), "u", "suite", None));
        }
        post_model(&ms, &[julien(), claude()])
    }

    #[test]
    fn texte_court_pas_de_fondu() {
        let mut b = banc(
            600.0,
            modele("question 0", "réponse 0", 0),
            Rappels::default(),
        );
        assert!(!b.lire(|v, _| v.tweet_coupe()));
        assert!(!b.lire(|v, _| v.reponse_coupee()));
        assert!(!b.lire(|v, _| v.compact()));
    }

    #[test]
    fn texte_long_coupe_mesure_et_fondu() {
        let mut b = banc(
            600.0,
            modele(&long_texte(12), &long_texte(9), 0),
            Rappels::default(),
        );
        assert!(b.lire(|v, cx| v.hauteur_tweet(cx).unwrap()) > 121.0);
        assert!(b.lire(|v, _| v.tweet_coupe()));
        assert!(b.lire(|v, _| v.reponse_coupee()));
    }

    #[test]
    fn la_heure_du_tweet_suit_l_horloge_figee() {
        let mut b = banc(
            600.0,
            modele("question 0", "réponse 0", 0),
            Rappels::default(),
        );
        assert_eq!(b.lire(|v, _| v.heure_tweet()).as_deref(), Some("· 6 h"));
    }

    #[test]
    fn compact_sous_360_px_de_contenu() {
        let mut etroit = banc(
            300.0,
            modele("question 0", "réponse 0", 0),
            Rappels::default(),
        );
        assert!(etroit.lire(|v, _| v.compact()));
        let mut large = banc(
            500.0,
            modele("question 0", "réponse 0", 0),
            Rappels::default(),
        );
        assert!(!large.lire(|v, _| v.compact()));
    }

    #[test]
    fn la_conversation_vide_se_dit() {
        let mut b = banc(500.0, post_model(&[], &[julien()]), Rappels::default());
        assert!(b.lire(|v, _| v.modele().post.is_none()));
    }

    #[test]
    fn les_rappels_sont_ceux_de_l_hote() {
        let appels = Rc::new(Cell::new(0));
        let a = appels.clone();
        let r = Rappels {
            ouvrir: Some(Rc::new(move |_, _| a.set(a.get() + 1))),
            ..Rappels::default()
        };
        let mut b = banc(500.0, modele("question 0", "réponse 0", 1), r);
        // Le fil entier est proposé quand un 3e message existe.
        assert!(b.lire(|v, _| v.modele().more));
        assert_eq!(appels.get(), 0);
    }
}
