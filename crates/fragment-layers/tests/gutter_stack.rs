// Port de app/src/ui/core/layers/gutterStack.test.ts

use fragment_layers::gutter_stack::{
    ArrangeOptions, ClusterItem, GUTTER_GAP_MAX, GUTTER_GAP_MIN, GutterItem, RoomOptions,
    arrange_gutter, cluster_gutter, compute_rooms, gutter_gap,
};

/// PRNG à graine (mulberry32) : des cas aléatoires mais rejouables. Mêmes
/// opérations 32 bits que le TS (`Math.imul` = `wrapping_mul`).
struct Prng(u32);

impl Prng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let seed = self.0;
        let mut t = (seed ^ (seed >> 15)).wrapping_mul(1 | seed);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

/// `Math.round` pour les valeurs positives des tests.
fn round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn it(top: f64, height: f64) -> GutterItem {
    GutterItem { top, height }
}

fn avec(gap: f64) -> ArrangeOptions {
    ArrangeOptions {
        gap,
        ..ArrangeOptions::default()
    }
}

/// Aucun recouvrement (écart ≥ gap) et ordre du texte gardé.
fn verifier(items: &[GutterItem], tops: &[f64], gap: f64) {
    let mut ordre: Vec<usize> = (0..items.len()).collect();
    ordre.sort_by(|&a, &b| items[a].top.total_cmp(&items[b].top).then(a.cmp(&b)));
    for j in 1..ordre.len() {
        let (a, b) = (ordre[j - 1], ordre[j]);
        assert!(tops[b] >= tops[a] + items[a].height + gap - 1e-6);
    }
}

// ── arrangeGutter ───────────────────────────────────────────────────────────

#[test]
fn laisse_en_place_ce_qui_ne_se_recouvre_pas_un_widget_seul_reste_sur_son_ancre() {
    assert_eq!(
        arrange_gutter(&[it(0.0, 50.0), it(100.0, 50.0)], ArrangeOptions::default()),
        vec![0.0, 100.0]
    );
    assert_eq!(
        arrange_gutter(&[it(42.0, 300.0)], ArrangeOptions::default()),
        vec![42.0]
    );
}

#[test]
fn deux_widgets_en_conflit_se_partagent_le_decalage_au_plus_pres_des_deux_ancres() {
    // Voulus 0 et 20, hauteur 50, gap 10 : il manque 40 → 20 chacun.
    assert_eq!(
        arrange_gutter(&[it(0.0, 50.0), it(20.0, 50.0)], avec(10.0)),
        vec![-20.0, 40.0]
    );
}

#[test]
fn min_top_on_ne_monte_pas_au_dessus_du_document_le_reste_descend() {
    let opts = ArrangeOptions {
        gap: 10.0,
        min_top: 0.0,
        ..ArrangeOptions::default()
    };
    assert_eq!(
        arrange_gutter(&[it(0.0, 50.0), it(20.0, 50.0)], opts),
        vec![0.0, 60.0]
    );
}

#[test]
fn epingle_exact_sur_son_ancre_les_autres_s_ecartent_au_dessus_et_en_dessous() {
    let items = [it(0.0, 100.0), it(10.0, 40.0), it(30.0, 20.0)];
    let opts = ArrangeOptions {
        pinned: Some(1),
        gap: 10.0,
        ..ArrangeOptions::default()
    };
    let tops = arrange_gutter(&items, opts);
    assert_eq!(tops[1], 10.0);
    assert_eq!(tops[0], 10.0 - 10.0 - 100.0); // juste au-dessus
    assert_eq!(tops[2], 10.0 + 40.0 + 10.0); // juste en dessous
    // Sans épingle, le grand (premier) ne monterait pas autant.
    assert!(arrange_gutter(&items, avec(10.0))[0] > -100.0);
}

#[test]
fn epingle_pres_du_haut_ceux_d_avant_ne_sortent_pas_par_le_haut_l_epingle_descend_d_autant() {
    let opts = ArrangeOptions {
        pinned: Some(1),
        gap: 10.0,
        min_top: 0.0,
    };
    let tops = arrange_gutter(&[it(0.0, 100.0), it(10.0, 40.0)], opts);
    assert_eq!(tops, vec![0.0, 110.0]);
}

#[test]
fn epingle_ceux_qui_n_ont_pas_de_conflit_ne_bougent_pas() {
    let opts = ArrangeOptions {
        pinned: Some(1),
        ..ArrangeOptions::default()
    };
    assert_eq!(
        arrange_gutter(&[it(0.0, 10.0), it(100.0, 10.0), it(300.0, 10.0)], opts),
        vec![0.0, 100.0, 300.0]
    );
}

#[test]
fn rend_les_hauts_dans_l_ordre_de_l_entree_a_ancre_egale_l_ordre_d_entree_tranche() {
    assert_eq!(
        arrange_gutter(
            &[it(100.0, 10.0), it(300.0, 20.0)],
            ArrangeOptions::default()
        ),
        vec![100.0, 300.0]
    );
    let t = arrange_gutter(&[it(10.0, 10.0), it(10.0, 10.0)], avec(0.0));
    assert_eq!(t[1], t[0] + 10.0);
}

#[test]
fn proprietes_graines_jamais_de_recouvrement_ordre_garde_independant_de_l_ordre_d_entree() {
    for seed in 1..=300_u32 {
        let mut r = Prng(seed);
        let n = 1 + (r.next() * 9.0).floor() as usize;
        let items: Vec<GutterItem> = (0..n)
            .map(|_| {
                let top = round(r.next() * 600.0);
                let height = 10.0 + round(r.next() * 200.0);
                it(top, height)
            })
            .collect();
        let gap = round(r.next() * 12.0);
        let pinned = if r.next() < 0.5 {
            Some((r.next() * n as f64).floor() as usize)
        } else {
            None
        };
        let tops = arrange_gutter(
            &items,
            ArrangeOptions {
                pinned,
                gap,
                ..ArrangeOptions::default()
            },
        );
        verifier(&items, &tops, gap);
        if let Some(p) = pinned {
            assert_eq!(tops[p], items[p].top);
        }
        // Avec un haut de document : rien au-dessus, et toujours aucun recouvrement.
        let bornes = arrange_gutter(
            &items,
            ArrangeOptions {
                pinned,
                gap,
                min_top: 0.0,
            },
        );
        verifier(&items, &bornes, gap);
        for t in &bornes {
            assert!(*t >= 0.0);
        }

        // Mélange de l'entrée : même placement par item (ancres distinctes).
        let mut distincts: Vec<u64> = items.iter().map(|i| i.top.to_bits()).collect();
        distincts.sort_unstable();
        distincts.dedup();
        if distincts.len() != n {
            continue;
        }
        // ★ Le TS mélange par `sort(() => r() - 0.5)` (ordre dépendant du moteur) ;
        //   ici un Fisher-Yates à la même graine : la propriété testée est la même.
        let mut perm: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = (r.next() * (i + 1) as f64).floor() as usize;
            perm.swap(i, j);
        }
        let melange: Vec<GutterItem> = perm.iter().map(|&i| items[i]).collect();
        let pinned2 = pinned.and_then(|p| perm.iter().position(|&i| i == p));
        let tops2 = arrange_gutter(
            &melange,
            ArrangeOptions {
                pinned: pinned2,
                gap,
                ..ArrangeOptions::default()
            },
        );
        for (j, &i) in perm.iter().enumerate() {
            assert!((tops2[j] - tops[i]).abs() < 5e-7, "graine {seed}");
        }
    }
}

#[test]
fn optimal_deplacer_une_grappe_d_un_pixel_n_ameliore_jamais_le_cout() {
    let cout = |items: &[GutterItem], tops: &[f64]| -> f64 {
        items
            .iter()
            .zip(tops)
            .map(|(it, t)| (t - it.top).powi(2))
            .sum()
    };
    for seed in 1..=100_u32 {
        let mut r = Prng(seed);
        let items: Vec<GutterItem> = (0..6)
            .map(|_| {
                let top = round(r.next() * 300.0);
                let height = 20.0 + round(r.next() * 80.0);
                it(top, height)
            })
            .collect();
        let tops = arrange_gutter(&items, avec(8.0));
        let base = cout(&items, &tops);
        for d in [-1.0, 1.0] {
            let decales: Vec<f64> = tops.iter().map(|t| t + d).collect();
            assert!(cout(&items, &decales) >= base - 1e-6);
        }
    }
}

// ── computeRooms ────────────────────────────────────────────────────────────

/// Des widgets simulés : flexible (se plie à sa place), rigide, à seuil (compact sous 80).
type Widget = Box<dyn Fn(f64) -> f64>;

fn flex(naturelle: f64) -> Widget {
    Box::new(move |room| naturelle.min(room))
}

fn rigide(h: f64) -> Widget {
    Box::new(move |_| h)
}

fn seuil(complet: f64, compact: f64) -> Widget {
    Box::new(move |room| if room < 80.0 { compact } else { complet })
}

fn lancer(
    tops: &[f64],
    widgets: &[Widget],
    pinned: Option<usize>,
    cap: f64,
) -> (Vec<f64>, Vec<f64>) {
    let opts = RoomOptions {
        pinned,
        gap: 8.0,
        cap,
    };
    let r = compute_rooms(tops, opts, |i, room| widgets[i](room));
    (r.rooms, r.heights)
}

const INF: f64 = f64::INFINITY;

#[test]
fn place_ecart_jusqu_a_l_ancre_suivante_moins_gap_le_dernier_illimitee() {
    let (rooms, heights) = lancer(
        &[100.0, 300.0, 600.0],
        &[flex(50.0), flex(50.0), flex(500.0)],
        None,
        INF,
    );
    assert_eq!(rooms, vec![192.0, 292.0, INF]);
    assert_eq!(heights, vec![50.0, 50.0, 500.0]);
}

#[test]
fn ancres_proches_le_widget_a_seuil_se_compacte_de_lui_meme_le_suivant_garde_sa_place() {
    let (rooms, heights) = lancer(
        &[100.0, 160.0, 400.0],
        &[seuil(200.0, 40.0), seuil(200.0, 40.0), seuil(200.0, 40.0)],
        None,
        INF,
    );
    assert_eq!(rooms, vec![52.0, 232.0, INF]);
    assert_eq!(heights, vec![40.0, 200.0, 200.0]);
}

#[test]
fn actif_place_plafonnee_le_suivant_pousse_voit_sa_place_reduite_et_se_compacte() {
    let tops = [100.0, 160.0, 400.0];
    let (rooms, heights) = lancer(
        &tops,
        &[seuil(200.0, 40.0), seuil(200.0, 40.0), seuil(200.0, 40.0)],
        Some(0),
        700.0,
    );
    assert_eq!(rooms[0], 700.0);
    assert_eq!(heights[0], 200.0);
    // B poussé à 100 + 200 + 8 = 308 → place = 400 − 308 − 8 = 84 (≥ 80 : complet)
    assert_eq!(rooms[1], 84.0);
    // Un actif plus grand : B passe sous le seuil.
    let (rooms2, heights2) = lancer(
        &tops,
        &[seuil(240.0, 40.0), seuil(200.0, 40.0), seuil(200.0, 40.0)],
        Some(0),
        700.0,
    );
    assert_eq!(rooms2[1], 44.0);
    assert_eq!(heights2[1], 40.0);
}

#[test]
fn un_widget_rigide_au_dessus_reduit_la_place_du_suivant() {
    let tops = [0.0, 100.0, 500.0];
    let (sans, _) = lancer(&tops, &[rigide(10.0), flex(300.0), flex(10.0)], None, INF);
    let (avec, _) = lancer(&tops, &[rigide(200.0), flex(300.0), flex(10.0)], None, INF);
    assert_eq!(sans[1], 392.0);
    assert_eq!(avec[1], 500.0 - 208.0 - 8.0);
}

#[test]
fn rend_dans_l_ordre_de_l_entree_quel_que_soit_l_ordre_des_ancres() {
    let (rooms, _) = lancer(&[300.0, 100.0], &[flex(10.0), flex(10.0)], None, INF);
    assert_eq!(rooms, vec![INF, 192.0]);
}

#[test]
fn proprietes_graines_aucune_place_negative_deux_calculs_identiques() {
    for seed in 1..=200_u32 {
        let mut r = Prng(seed);
        let n = 1 + (r.next() * 8.0).floor() as usize;
        let tops: Vec<f64> = (0..n).map(|_| round(r.next() * 800.0)).collect();
        let w: Vec<Widget> = (0..n)
            .map(|_| {
                if r.next() < 0.5 {
                    flex(20.0 + round(r.next() * 300.0))
                } else {
                    seuil(150.0, 30.0)
                }
            })
            .collect();
        let pinned = if r.next() < 0.5 {
            Some((r.next() * n as f64).floor() as usize)
        } else {
            None
        };
        let a = lancer(&tops, &w, pinned, 600.0);
        let b = lancer(&tops, &w, pinned, 600.0);
        assert_eq!(a, b);
        for room in &a.0 {
            assert!(*room >= 0.0);
        }
    }
}

// ── gutterGap ───────────────────────────────────────────────────────────────

#[test]
fn gutter_gap_proportionnel_entre_les_bornes() {
    assert!((gutter_gap(300.0) - 30.0).abs() < 1e-9);
    assert!((gutter_gap(450.0) - 45.0).abs() < 1e-9);
    assert!(gutter_gap(400.0) > gutter_gap(300.0));
}

#[test]
fn gutter_gap_borne_marge_etroite_le_minimum_12_tres_large_le_maximum() {
    assert_eq!(gutter_gap(60.0), GUTTER_GAP_MIN);
    assert_eq!(gutter_gap(5000.0), GUTTER_GAP_MAX);
}

#[test]
fn gutter_gap_marge_nulle_negative_ou_illisible_le_minimum() {
    for m in [0.0, -40.0, f64::NAN, f64::INFINITY] {
        assert_eq!(gutter_gap(m), GUTTER_GAP_MIN);
    }
}

// ── clusterGutter ───────────────────────────────────────────────────────────

fn it3(top: f64, compact: bool) -> ClusterItem {
    ClusterItem { top, compact }
}

#[test]
fn les_reduits_voisins_rayon_font_une_grappe_un_entier_reste_seul_sans_la_couper() {
    assert_eq!(
        cluster_gutter(
            &[
                it3(0.0, true),
                it3(10.0, true),
                it3(20.0, true),
                it3(30.0, false),
                it3(40.0, true)
            ],
            24.0
        ),
        vec![vec![0, 1, 2], vec![3], vec![4]]
    );
    assert_eq!(
        cluster_gutter(&[it3(0.0, true), it3(5.0, false), it3(10.0, true)], 24.0),
        vec![vec![0, 2], vec![1]]
    );
}

#[test]
fn le_rayon_se_compte_depuis_le_debut_de_la_grappe_pas_de_chaine_infinie() {
    assert_eq!(
        cluster_gutter(
            &[
                it3(0.0, true),
                it3(20.0, true),
                it3(40.0, true),
                it3(60.0, true)
            ],
            24.0
        ),
        vec![vec![0, 1], vec![2, 3]]
    );
}

#[test]
fn ordre_du_texte_quel_que_soit_l_ordre_d_entree_chaque_item_dans_exactement_une_grappe_graines() {
    for seed in 1..=200_u32 {
        let mut r = Prng(seed);
        let n = 1 + (r.next() * 12.0).floor() as usize;
        let items: Vec<ClusterItem> = (0..n)
            .map(|_| {
                let top = round(r.next() * 200.0);
                it3(top, r.next() < 0.7)
            })
            .collect();
        let g = cluster_gutter(&items, 24.0);
        let mut tous: Vec<usize> = g.iter().flatten().copied().collect();
        tous.sort_unstable();
        assert_eq!(tous, (0..items.len()).collect::<Vec<_>>());
        for m in &g {
            let tops: Vec<f64> = m.iter().map(|&i| items[i].top).collect();
            let mut tries = tops.clone();
            tries.sort_by(f64::total_cmp);
            assert_eq!(tops, tries);
            if m.len() > 1 {
                assert!(m.iter().all(|&i| items[i].compact));
                assert!(tops[tops.len() - 1] - tops[0] <= 24.0);
            }
        }
    }
}
