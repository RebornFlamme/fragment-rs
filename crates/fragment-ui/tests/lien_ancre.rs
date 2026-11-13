//! Tests du lien pointillé d'ancre (tâche `fr-t5-lien-ancre`).
//!
//! Les quatre cas portent le nom de ceux de `anchor-link.spec.ts` (e2e) en snake_case, sur une FAUSSE
//! surface écrite ici : les rectangles d'entrée sont donnés à la main, aucun test ne touche à GPUI.
//! La géométrie du tracé (`trace`) est testée dans `fragment-annotation`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::Cell;
use std::rc::Rc;

use fragment_annotation::anchor_link::Point;
use fragment_ui::lien_ancre::dessin::pointilles;
use fragment_ui::lien_ancre::modele::{COUDE, ENTREE};
use fragment_ui::lien_ancre::{
    Ancre, CibleLien, EtatLien, Liens, Marqueur, Rect, SurfaceLien, bouts_du_lien, lien_voulu,
};

/// Une surface de test : un passage = une ligne de 12 px de haut, qui finit à `5 * (to)` ; le
/// point d'ancrage est au même x.
struct Fausse {
    echelle: f64,
}

impl SurfaceLien for Fausse {
    fn rectangles_passage(&self, from: usize, to: usize) -> Vec<Rect> {
        vec![Rect {
            left: from as f64 * 5.0,
            top: 100.0,
            right: to as f64 * 5.0,
            bottom: 112.0,
        }]
    }
    fn coords_au_point(&self, pos: usize) -> Option<Rect> {
        Some(Rect {
            left: pos as f64 * 5.0,
            top: 100.0,
            right: pos as f64 * 5.0 + 5.0,
            bottom: 112.0,
        })
    }
    fn rendu(&self, _pos: usize) -> bool {
        true
    }
    fn echelle(&self) -> f64 {
        self.echelle
    }
}

fn surface() -> Fausse {
    Fausse { echelle: 1.0 }
}

fn passage(pos: usize, to: usize) -> Ancre {
    Ancre {
        pos,
        to: Some(to),
        dy: 0.0,
        viewport: false,
    }
}

/// Un widget dans la marge : bord gauche à `gauche`, rangée d'icônes de 26 px à `top + 6`.
fn widget(gauche: f64, top: f64) -> CibleLien {
    CibleLien {
        boite: Rect {
            left: gauche,
            top,
            right: gauche + 320.0,
            bottom: top + 200.0,
        },
        icones: Some(Rect {
            left: gauche + 10.0,
            top: top + 6.0,
            right: gauche + 100.0,
            bottom: top + 32.0,
        }),
        cache: false,
    }
}

fn etat_survole(ancre: Ancre) -> EtatLien {
    EtatLien {
        ancre: Some(ancre),
        survole: true,
        ..EtatLien::default()
    }
}

// ─── anchor-link.spec.ts ───

#[test]
fn au_survol_il_continue_le_soulignement_horizontal_oblique_horizontal_jusqu_au_milieu_des_icones_parti_au_depart_du_pointeur()
 {
    let s = surface();
    let ancre = passage(10, 16);
    assert!(lien_voulu(&etat_survole(ancre)));
    let cible = widget(600.0, 40.0);
    let liens = Liens::nouveau();
    let (a, c) = (ancre, cible);
    liens.poser("w1", move || {
        bouts_du_lien(&Fausse { echelle: 1.0 }, &a, Some(&c))
    });

    let tous = liens.liens();
    assert_eq!(tous.len(), 1);
    let pts = &tous[0].points;
    let fin = s.rectangles_passage(10, 16)[0];
    // Départ : au bout du soulignement, à son niveau (1 px au-dessus du pied).
    assert_eq!(
        pts[0],
        Point {
            x: fin.right,
            y: fin.bottom - 1.0
        }
    );
    // Arrivée : le bord gauche du widget, au milieu de sa rangée d'icônes.
    let dernier = *pts.last().unwrap();
    assert_eq!(
        dernier,
        Point {
            x: 600.0,
            y: 40.0 + 6.0 + 13.0
        }
    );
    // Hauteurs différentes : quatre points, premier et dernier morceaux horizontaux.
    assert_eq!(pts.len(), 4);
    assert_eq!(pts[1].y, pts[0].y);
    assert_eq!(pts[2].y, dernier.y);
    // Le coude et l'entrée sont à 40 et 12 px avant le widget.
    assert_eq!(pts[1].x, 600.0 - COUDE);
    assert_eq!(pts[2].x, 600.0 - ENTREE);
    // Le pointeur part : le lien est retiré.
    assert!(liens.retirer("w1"));
    assert!(liens.liens().is_empty());
    assert!(!lien_voulu(&EtatLien {
        survole: false,
        ..etat_survole(ancre)
    }));
}

#[test]
fn ancre_a_un_point_une_zone_entouree_rien_de_surligne_pas_de_lien_au_survol() {
    let point = Ancre {
        pos: 10,
        to: Some(10),
        dy: 0.0,
        viewport: false,
    };
    assert!(!lien_voulu(&etat_survole(point)));
    let sans_fin = Ancre { to: None, ..point };
    assert!(!lien_voulu(&etat_survole(sans_fin)));
    // Ancré à la fenêtre : jamais de lien non plus, même avec un passage.
    let fenetre = Ancre {
        viewport: true,
        ..passage(10, 16)
    };
    assert!(!lien_voulu(&etat_survole(fenetre)));
    assert!(bouts_du_lien(&surface(), &fenetre, Some(&widget(600.0, 40.0))).is_none());
}

#[test]
fn le_widget_bouge_sans_que_le_texte_bouge_marge_1_4_3_4_le_lien_le_suit() {
    let ancre = passage(10, 16);
    let gauche = Rc::new(Cell::new(600.0_f64));
    let liens = Liens::nouveau();
    let g = Rc::clone(&gauche);
    liens.poser("w1", move || {
        bouts_du_lien(
            &Fausse { echelle: 1.0 },
            &ancre,
            Some(&widget(g.get(), 40.0)),
        )
    });
    let avant = liens.liens();
    assert_eq!(avant[0].points.last().unwrap().x, 600.0);
    // Rien n'a bougé : le suivi ne demande rien.
    let demandes = liens.redessins_demandes();
    assert!(!liens.suivre());
    assert_eq!(liens.redessins_demandes(), demandes);
    // La marge s'élargit : le widget se déplace sans que la surface se redessine.
    gauche.set(480.0);
    assert!(liens.suivre());
    assert_eq!(liens.redessins_demandes(), demandes + 1);
    let apres = liens.liens();
    assert_eq!(apres[0].points.last().unwrap().x, 480.0);
    assert_eq!(apres[0].points[0], avant[0].points[0]); // le départ reste au bout du passage
    // Une fois redessiné, plus rien à suivre ; la table vide arrête le suivi.
    assert!(!liens.suivre());
    liens.retirer("w1");
    assert!(!liens.suivre());
}

#[test]
fn une_grappe_survolee_un_lien_par_membre_chacun_du_bout_de_son_passage_jusqu_aux_bulles() {
    let bulles = widget(600.0, 40.0);
    let liens = Liens::nouveau();
    // Trois membres rangés dans la grappe : chacun a son passage (décalé de 2) et est survolé par elle.
    for i in 0..3_usize {
        let ancre = passage(10 + i * 2, 16 + i);
        let membre = EtatLien {
            ancre: Some(ancre),
            cache: true,
            survole_par_sa_grappe: true,
            ..EtatLien::default()
        };
        assert!(lien_voulu(&membre));
        liens.poser(&format!("m{i}"), move || {
            bouts_du_lien(&Fausse { echelle: 1.0 }, &ancre, Some(&bulles))
        });
    }
    let tous = liens.liens();
    assert_eq!(tous.len(), 3);
    for (i, l) in tous.iter().enumerate() {
        assert_eq!(l.points.last().unwrap().x, 600.0);
        assert_eq!(l.points[0].x, (16 + i) as f64 * 5.0); // le bout de SON passage
    }
    // La grappe n'a pas de lien propre ; les membres rangés ne sont pas survolés par eux-mêmes.
    let grappe = EtatLien {
        presente_une_grappe: true,
        ..etat_survole(passage(10, 16))
    };
    assert!(!lien_voulu(&grappe));
    let range = EtatLien {
        cache: true,
        ..etat_survole(passage(10, 16))
    };
    assert!(!lien_voulu(&range));
    // Le pointeur part : tous les liens sont retirés.
    for i in 0..3 {
        liens.retirer(&format!("m{i}"));
    }
    assert!(liens.liens().is_empty());
}

// ─── Ajouts : bouts, échelle, marqueurs, pointillés ───

#[test]
fn widget_a_gauche_du_texte_le_lien_arrive_par_son_bord_droit() {
    let b = bouts_du_lien(&surface(), &passage(100, 106), Some(&widget(-400.0, 40.0))).unwrap();
    assert_eq!(b.to.x, -400.0 + 320.0);
    assert_eq!(b.elbow_x, b.to.x + COUDE);
    assert_eq!(b.entry_x, b.to.x + ENTREE);
}

#[test]
fn sans_rangee_d_icones_mesuree_le_lien_entre_a_19_px_du_haut() {
    let cible = CibleLien {
        icones: None,
        ..widget(600.0, 40.0)
    };
    let b = bouts_du_lien(&surface(), &passage(10, 16), Some(&cible)).unwrap();
    assert_eq!(b.to.y, 59.0);
}

#[test]
fn widget_cache_ou_de_largeur_nulle_ou_position_non_rendue_pas_de_bouts() {
    let a = passage(10, 16);
    assert!(bouts_du_lien(&surface(), &a, None).is_none());
    let cache = CibleLien {
        cache: true,
        ..widget(600.0, 40.0)
    };
    assert!(bouts_du_lien(&surface(), &a, Some(&cache)).is_none());
    let mut nul = widget(600.0, 40.0);
    nul.boite.right = nul.boite.left;
    assert!(bouts_du_lien(&surface(), &a, Some(&nul)).is_none());
}

#[test]
fn plan_zoome_le_soulignement_et_le_coude_gardent_leur_taille_d_ecran() {
    let zoom = Fausse { echelle: 2.0 };
    let b = bouts_du_lien(&zoom, &passage(10, 16), Some(&widget(600.0, 40.0))).unwrap();
    assert_eq!(b.from.y, 112.0 - 0.5); // 1 px écran = 0,5 px de plan
    assert_eq!(b.elbow_x, 600.0 - COUDE / 2.0);
    assert_eq!(b.entry_x, 600.0 - ENTREE / 2.0);
}

#[test]
fn sans_passage_le_depart_est_le_point_d_ancrage_decale_de_dy() {
    let a = Ancre {
        pos: 10,
        to: None,
        dy: 4.0,
        viewport: false,
    };
    let b = bouts_du_lien(&surface(), &a, Some(&widget(600.0, 40.0))).unwrap();
    assert_eq!(b.from, Point { x: 50.0, y: 104.0 });
}

#[test]
fn deux_marqueurs_de_meme_geometrie_arrondie_ont_la_meme_cle() {
    let p = |x, y| Point { x, y };
    let a = Marqueur::nouveau("w1", &[p(1.04, 2.0), p(9.0, 2.0)]);
    let b = Marqueur::nouveau("w1", &[p(1.0, 2.0), p(9.0, 2.0)]);
    assert_eq!(a, b);
    assert_eq!(a.cle, "w1|1,2 9,2");
    assert_ne!(a, Marqueur::nouveau("w2", &[p(1.0, 2.0), p(9.0, 2.0)]));
}

#[test]
fn pointilles_trois_de_trait_trois_de_blanc_sur_un_trait_droit() {
    let p = |x| Point { x, y: 0.0 };
    let d = pointilles(&[p(0.0), p(10.0)], 3.0, 3.0);
    let xs: Vec<(f64, f64)> = d.iter().map(|(a, b)| (a.x, b.x)).collect();
    assert_eq!(xs, vec![(0.0, 3.0), (6.0, 9.0)]);
}

#[test]
fn pointilles_la_phase_se_poursuit_d_un_sommet_a_l_autre() {
    let p = |x, y| Point { x, y };
    // 0..4 horizontal puis 4 vers le bas : le premier trait (3) finit à x=3, le blanc (3) va de 3 à 4
    // puis 2 de plus vers le bas, le trait suivant commence à y=2.
    let d = pointilles(&[p(0.0, 0.0), p(4.0, 0.0), p(4.0, 6.0)], 3.0, 3.0);
    assert_eq!(d[0], (p(0.0, 0.0), p(3.0, 0.0)));
    assert_eq!(d[1].0, p(4.0, 2.0));
    assert_eq!(d[1].1, p(4.0, 5.0));
    assert_eq!(d.len(), 2);
}
