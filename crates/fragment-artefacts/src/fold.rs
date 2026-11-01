// Port de app/src/ui/core/artefacts/fold.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le pli (§14.4) — LA fonction pure qui applique une transaction à l'index.
//  Plier tout le journal donne l'état courant.
//
//  Pour chaque groupe de champs (contenu, scope, ancre, tombe), la valeur qui
//  porte la plus grande estampille l'emporte (LWW). Trois propriétés, trois
//  conséquences (invariant §19.1-3) :
//    - commutatif : les segments se lisent dans n'importe quel ordre ;
//    - idempotent : relire une ligne déjà lue ne change rien ;
//    - associatif : plier un segment compacté = plier les segments qu'il remplace.
//
//  ★ Le code est celui du PDF, à la lettre. Seule addition : `local`, qui n'est
//    pas un groupe arbitré mais l'appartenance du record au journal de l'appareil
//    (posée à la création, jamais retirée).
// ═══════════════════════════════════════════════════════════════════════════

use crate::types::{Index, Op, Rec, Tx};

/// `t` est-elle plus récente que l'estampille courante (absente = oui) ?
pub fn newer(t: &str, cur: Option<&str>) -> bool {
    match cur {
        None => true,
        Some(cur) => t > cur,
    }
}

/// Un record est vivant si son contenu est plus récent que sa tombe.
pub fn alive(r: &Rec) -> bool {
    match (&r.content_t, &r.del_t) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(c), Some(d)) => c > d,
    }
}

pub fn fold(index: &mut Index, tx: &Tx) {
    for op in &tx.ops {
        let id = op.id();
        let r = index.entry(id.to_string()).or_insert_with(|| Rec::new(id));

        match op {
            Op::Del { .. } => {
                if newer(&tx.t, r.del_t.as_deref()) {
                    r.del_t = Some(tx.t.clone());
                }
            }
            Op::Set(op) => {
                if op.type_.is_some() && newer(&tx.t, r.content_t.as_deref()) {
                    r.type_ = op.type_.clone();
                    r.v = op.v;
                    r.state = op.state.clone();
                    r.head = op.head.clone();
                    r.content_t = Some(tx.t.clone());
                }
                if op.scope.is_some() && newer(&tx.t, r.scope_t.as_deref()) {
                    r.scope = op.scope.clone();
                    r.scope_t = Some(tx.t.clone());
                }
                if op.anchor.is_some() && newer(&tx.t, r.anchor_t.as_deref()) {
                    r.anchor = op.anchor.clone();
                    r.anchor_t = Some(tx.t.clone());
                }
                if op.local {
                    r.local = true;
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Port de app/src/ui/core/artefacts/fold.test.ts
//
//  Les preuves du pli (§19.3) — tests de propriété, transactions tirées au
//  hasard par un générateur à graine (reproductible, sans dépendance). ★ Mêmes
//  graines, même PRNG (mulberry32), mêmes tirages dans le même ordre que le TS.
// ═══════════════════════════════════════════════════════════════════════════
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::clock::{date_utc, format_stamp};
    use crate::journal::compact::{RETENTION_MS, compact_txs, content_stamps_of};
    use crate::types::{ArtefactScope, Origin, SetOp, WidgetAnchor};
    use serde_json::json;

    /// mulberry32 : un PRNG à graine.
    pub(crate) fn prng(seed: u32) -> impl FnMut() -> f64 {
        let mut a = seed;
        move || {
            a = a.wrapping_add(0x6d2b_79f5);
            let mut t = a;
            t = (t ^ (t >> 15)).wrapping_mul(t | 1);
            t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
            f64::from(t ^ (t >> 14)) / 4_294_967_296.0
        }
    }

    /// `Math.floor(rand() * n)`.
    fn pick(rand: &mut impl FnMut() -> f64, n: usize) -> usize {
        (rand() * n as f64).floor() as usize
    }

    const IDS: [&str; 4] = ["a", "b", "c", "d"];
    const WRITERS: [&str; 3] = ["k3f9-b71", "p2x8-e02", "zz00-001"];
    fn t0() -> i64 {
        date_utc(2026, 8, 29, 17, 20)
    }

    /// Un journal aléatoire : des transactions de plusieurs écrivains, estampilles uniques.
    fn random_journal(rand: &mut impl FnMut() -> f64, n: usize, t0: i64) -> Vec<Tx> {
        let mut txs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..n {
            let writer = WRITERS[pick(rand, WRITERS.len())];
            let t = loop {
                let t = format_stamp(t0 + pick(rand, 5000) as i64, pick(rand, 4) as u32, writer);
                if seen.insert(t.clone()) {
                    break t;
                }
            };
            let mut ops = Vec::new();
            let n_ops = 1 + pick(rand, 3);
            for _ in 0..n_ops {
                let id = IDS[pick(rand, IDS.len())];
                let r = rand();
                if r < 0.15 {
                    ops.push(Op::del(id));
                    continue;
                }
                let mut op = SetOp::new(id);
                if r < 0.6 {
                    op.type_ = Some("test:x".into());
                    op.v = Some(1);
                    op.state = Some(json!({ "n": pick(rand, 100) }));
                }
                if rand() < 0.4 {
                    op.scope = Some(ArtefactScope::file(if rand() < 0.5 {
                        "a.md"
                    } else {
                        "b.md"
                    }));
                }
                if rand() < 0.5 {
                    op.anchor = Some(Some(WidgetAnchor::document(pick(rand, 500) as i64).into()));
                }
                ops.push(Op::Set(op));
            }
            txs.push(Tx {
                t,
                o: Origin::User,
                ops,
            });
        }
        txs
    }

    pub(crate) fn fold_all(txs: &[Tx]) -> Index {
        let mut index = Index::new();
        for tx in txs {
            fold(&mut index, &tx.clone());
        }
        index
    }

    fn shuffle(arr: &[Tx], rand: &mut impl FnMut() -> f64) -> Vec<Tx> {
        let mut a = arr.to_vec();
        for i in (1..a.len()).rev() {
            let j = pick(rand, i + 1);
            a.swap(i, j);
        }
        a
    }

    mod le_pli_proprietes_14_4_19_3 {
        use super::*;

        #[test]
        fn commutatif_les_memes_transactions_dans_n_importe_quel_ordre_donnent_le_meme_index() {
            for seed in 1..=60 {
                let mut rand = prng(seed);
                let txs = random_journal(&mut rand, 40, t0());
                let reference = fold_all(&txs);
                for _ in 0..5 {
                    assert_eq!(
                        fold_all(&shuffle(&txs, &mut rand)),
                        reference,
                        "graine {seed}"
                    );
                }
            }
        }

        #[test]
        fn idempotent_plier_deux_fois_donne_le_meme_index_que_plier_une_fois() {
            for seed in 100..=140 {
                let mut rand = prng(seed);
                let txs = random_journal(&mut rand, 30, t0());
                let once = fold_all(&txs);
                let mut twice = txs.clone();
                twice.extend(shuffle(&txs, &mut rand));
                assert_eq!(fold_all(&twice), once, "graine {seed}");
            }
        }

        #[test]
        fn associatif_l_index_plie_apres_compaction_egale_l_index_plie_avant() {
            for seed in 200..=260 {
                let mut rand = prng(seed);
                let txs = random_journal(&mut rand, 50, t0());
                // Un « appareil » compacte la moitié de ses lignes ; le reste du journal ne bouge pas.
                let cut = txs.len() / 2;
                let sources = &txs[..cut];
                let rest = &txs[cut..];
                let index = fold_all(&txs);
                let mut kept =
                    compact_txs(sources, &index, &content_stamps_of(&txs), t0() + 10_000);
                kept.extend_from_slice(rest);
                assert_eq!(fold_all(&kept), index, "graine {seed}");
            }
        }
    }

    mod le_pli_l_arbitrage_par_groupe_figure_27 {
        use super::*;

        fn s(sec: i64, w: &str) -> String {
            format_stamp(t0() + sec * 1000, 0, w)
        }
        const K: &str = "k3f9-b71";
        const P: &str = "p2x8-e02";

        fn tx(t: String, o: Origin, ops: Vec<Op>) -> Tx {
            Tx { t, o, ops }
        }

        #[test]
        fn lww_par_groupe_ordre_d_arrivee_quelconque_l_ancienne_valeur_ignoree() {
            let mut index = Index::new();
            fold(
                &mut index,
                &tx(
                    s(14, K),
                    Origin::User,
                    vec![SetOp::new("X").content("t", 1, json!("lait")).into()],
                ),
            );
            fold(
                &mut index,
                &tx(
                    s(3, K),
                    Origin::User,
                    vec![SetOp::new("X").scope(ArtefactScope::file("note.md")).into()],
                ),
            );
            fold(
                &mut index,
                &tx(
                    s(12, P),
                    Origin::User,
                    vec![SetOp::new("X").content("t", 1, json!("pain")).into()],
                ),
            );
            fold(
                &mut index,
                &tx(
                    s(10, K),
                    Origin::Follow,
                    vec![SetOp::new("X").anchor(WidgetAnchor::document(140)).into()],
                ),
            );
            fold(&mut index, &tx(s(9, P), Origin::User, vec![Op::del("X")]));
            let x = &index["X"];
            assert_eq!(x.state, Some(json!("lait")));
            assert_eq!(x.anchor().map(|a| a.anchor.pos()), Some(Some(140)));
            assert_eq!(x.del_t.as_deref(), Some(s(9, P).as_str()));
            assert!(alive(x)); // contenu …14 > tombe …09
        }

        #[test]
        fn une_tombe_plus_recente_que_le_contenu_le_record_n_est_plus_vivant_un_nouveau_set_le_restaure()
         {
            let mut index = Index::new();
            fold(
                &mut index,
                &tx(
                    s(1, K),
                    Origin::User,
                    vec![SetOp::new("Y").content("t", 1, json!(1)).into()],
                ),
            );
            fold(&mut index, &tx(s(2, K), Origin::User, vec![Op::del("Y")]));
            assert!(!alive(&index["Y"]));
            fold(
                &mut index,
                &tx(
                    s(3, K),
                    Origin::User,
                    vec![SetOp::new("Y").content("t", 1, json!(2)).into()],
                ),
            );
            assert!(alive(&index["Y"]));
        }
    }

    mod la_compaction_ce_qui_part_ce_qui_reste_15_9 {
        use super::*;

        fn s(ms: i64) -> String {
            format_stamp(ms, 0, "k3f9-b71")
        }
        const DAY: i64 = 24 * 60 * 60 * 1000;

        fn tx(t: String, o: Origin, ops: Vec<Op>) -> Tx {
            Tx { t, o, ops }
        }
        fn content(id: &str, state: &str) -> Op {
            SetOp::new(id).content("t", 1, json!(state)).into()
        }
        fn anchor(id: &str, pos: i64) -> Op {
            SetOp::new(id).anchor(WidgetAnchor::document(pos)).into()
        }

        #[test]
        fn retire_les_ancres_remplacees_et_le_contenu_remplace_depuis_plus_de_90_jours_garde_courant_et_tombes()
         {
            let t0 = t0();
            let now = t0 + 200 * DAY;
            let txs = vec![
                tx(s(t0), Origin::User, vec![content("X", "v1")]), // remplacé il y a > 90 j
                tx(s(t0 + DAY), Origin::Follow, vec![anchor("X", 120)]),
                tx(s(t0 + 2 * DAY), Origin::User, vec![content("X", "v2")]), // remplacé il y a 12 j
                tx(s(now - 12 * DAY), Origin::User, vec![content("X", "v3")]), // courant
                tx(s(now - 11 * DAY), Origin::Follow, vec![anchor("X", 140)]),
                tx(s(t0 + 3 * DAY), Origin::User, vec![Op::del("Z")]),
            ];
            let index = fold_all(&txs);
            let kept = compact_txs(&txs, &index, &content_stamps_of(&txs), now);
            let ops: Vec<&Op> = kept.iter().flat_map(|tx| tx.ops.iter()).collect();
            let states: Vec<&serde_json::Value> = ops
                .iter()
                .filter_map(|op| op.as_set().and_then(|s| s.state.as_ref()))
                .collect();
            assert_eq!(states, vec![&json!("v2"), &json!("v3")]);
            let anchors = ops
                .iter()
                .filter(|op| op.as_set().is_some_and(|s| s.anchor.is_some()))
                .count();
            assert_eq!(anchors, 1);
            assert!(ops.iter().any(|op| op.is_del() && op.id() == "Z"));
        }

        #[test]
        fn un_record_supprime_voit_son_contenu_purge_apres_la_retention_la_tombe_reste() {
            let t0 = t0();
            let now = t0 + RETENTION_MS + 2 * DAY;
            let txs = vec![
                tx(s(t0), Origin::User, vec![content("X", "gros")]),
                tx(s(t0 + DAY), Origin::User, vec![Op::del("X")]),
            ];
            let kept = compact_txs(&txs, &fold_all(&txs), &content_stamps_of(&txs), now);
            let ops: Vec<Op> = kept.into_iter().flat_map(|tx| tx.ops).collect();
            assert_eq!(ops, vec![Op::del("X")]);
        }
    }
}
