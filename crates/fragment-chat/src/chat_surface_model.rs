// Port de app/src/ui/views/chat/chatSurfaceModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  L'espace de positions d'une CONVERSATION vue comme un document
//  (chat-comme-document.md) — pur, testé ; ChatSurface (fragment-ui) y branche la vue.
//
//  Une UNITÉ = une part de texte d'un message (un message peut en avoir
//  plusieurs, séparées par des appels d'outils). Positions à PAS FIXE, comme
//  les pages du PDF :
//
//      pos = message × MESSAGE_STRIDE + part × PART_STRIDE + caractère
//
//  ★ POURQUOI des pas fixes : un message qui s'écrit (flux) ne change que les
//    positions DANS sa part — rien ne bouge ailleurs, aucune ancre à remapper
//    dans les autres messages. Le pas ne survit pas à tout (un rechargement
//    peut regrouper les parts, préfixer l'historique) : l'ancre porte aussi sa
//    CITATION, relocalisée dans le texte joint (`surface_text_of`).
//
//  ★ « caractère » = unité de code UTF-16 (ADR-004), comme `String.length` : les longueurs
//    passent par `fragment_text::utf16::utf16_len`.
//
//  Le modèle de LIGNES de TextSurface : une ligne = une unité (comme une page
//  du PDF). `getLine(n)` rend tout le texte de l'unité.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;

use fragment_text::text_delta::{map_through_delta, text_delta};
use fragment_text::utf16::utf16_len;
use fragment_text::{Assoc, EditorPosition, MappedPos, SurfaceText};

pub const PART_STRIDE: usize = 1_000_000;
pub const MESSAGE_STRIDE: usize = 1_000 * PART_STRIDE;
/// Entre deux unités dans le texte joint : un paragraphe.
pub const UNIT_SEPARATOR: &str = "\n\n";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitSpec {
    /// Rang du message dans la conversation, rang de la part dans le message.
    pub msg: usize,
    pub part: usize,
    /// Identité stable d'une synchro à l'autre : `<id du message>#<part>`.
    pub key: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub msg: usize,
    pub part: usize,
    pub key: String,
    pub text: String,
    pub base: usize,
    /// Longueur de `text` en unités de code UTF-16 (ADR-004).
    pub len: usize,
}

pub fn base_of(msg: usize, part: usize) -> usize {
    msg * MESSAGE_STRIDE + part * PART_STRIDE
}

/// Les unités, dans l'ordre du document.
pub fn units_of(specs: &[UnitSpec]) -> Vec<Unit> {
    let mut units: Vec<Unit> = specs
        .iter()
        .map(|s| Unit {
            msg: s.msg,
            part: s.part,
            key: s.key.clone(),
            text: s.text.clone(),
            base: base_of(s.msg, s.part),
            len: utf16_len(&s.text),
        })
        .collect();
    units.sort_by_key(|u| u.base);
    units
}

/// Le dernier rang dont la base est ≤ pos (`None` si aucun).
fn rang_avant(units: &[Unit], pos: usize) -> Option<usize> {
    units.partition_point(|u| u.base <= pos).checked_sub(1)
}

/// L'unité qui CONTIENT pos (bornes incluses), ou `None`.
pub fn unit_at(units: &[Unit], pos: usize) -> Option<usize> {
    let i = rang_avant(units, pos)?;
    (pos <= units[i].base + units[i].len).then_some(i)
}

/// La fin du document (fin de la dernière unité).
pub fn end_of(units: &[Unit]) -> usize {
    units.last().map_or(0, |u| u.base + u.len)
}

/// Position → (ligne, caractère). Dans un trou : début de l'unité suivante ; au-delà : la fin.
pub fn offset_to_pos(units: &[Unit], pos: usize) -> EditorPosition {
    if units.is_empty() {
        return EditorPosition { line: 0, ch: 0 };
    }
    if let Some(i) = unit_at(units, pos) {
        return EditorPosition {
            line: i,
            ch: pos - units[i].base,
        };
    }
    let suivante = rang_avant(units, pos).map_or(0, |i| i + 1);
    if suivante < units.len() {
        return EditorPosition {
            line: suivante,
            ch: 0,
        };
    }
    let last = units.len() - 1;
    EditorPosition {
        line: last,
        ch: units[last].len,
    }
}

/// (ligne, caractère) → position, bornée à l'unité.
pub fn pos_to_offset(units: &[Unit], p: EditorPosition) -> usize {
    if units.is_empty() {
        return 0;
    }
    let line = p.line.min(units.len() - 1);
    let u = &units[line];
    u.base + p.ch.min(u.len)
}

/// Le texte joint (unités séparées par un paragraphe) et sa carte vers les positions.
pub fn surface_text_of(units: &[Unit]) -> SurfaceText {
    let mut starts: Vec<usize> = Vec::with_capacity(units.len());
    let mut text = String::new();
    let mut longueur = 0usize;
    for (k, u) in units.iter().enumerate() {
        if k > 0 {
            text.push_str(UNIT_SEPARATOR);
            longueur += utf16_len(UNIT_SEPARATOR);
        }
        starts.push(longueur);
        text.push_str(&u.text);
        longueur += u.len;
    }
    // Ce que les deux cartes retiennent (elles doivent être `'static`).
    let carte: Vec<(usize, usize)> = units.iter().map(|u| (u.base, u.len)).collect();
    let starts_a = starts.clone();
    let carte_a = carte.clone();
    let units_b: Vec<Unit> = units.to_vec();

    SurfaceText::new(
        text,
        // index → position : le dernier rang dont le début (dans le texte) est ≤ i.
        move |i| {
            if carte_a.is_empty() {
                return 0;
            }
            let k = starts_a.partition_point(|&s| s <= i).saturating_sub(1);
            let (base, len) = carte_a[k];
            base + (i - starts_a[k]).min(len)
        },
        // position → index.
        move |pos| {
            if units_b.is_empty() {
                return 0;
            }
            let p = offset_to_pos(&units_b, pos);
            starts[p.line] + p.ch
        },
    )
}

/// D'une synchro à la suivante : a-t-on changé, et où va chaque ancienne position.
/// Les unités se suivent par leur CLÉ (message + part) — un historique chargé
/// avant les tours en direct DÉCALE leurs rangs, il ne les supprime pas.
#[derive(Clone, Debug)]
pub struct UnitsDiff {
    pub changed: bool,
    prev: Vec<Unit>,
    next: Vec<Unit>,
    par_cle: HashMap<String, usize>,
}

pub fn diff_units(prev: &[Unit], next: &[Unit]) -> UnitsDiff {
    let par_cle = next
        .iter()
        .enumerate()
        .map(|(i, u)| (u.key.clone(), i))
        .collect();
    let changed = prev.len() != next.len()
        || prev
            .iter()
            .zip(next)
            .any(|(a, b)| a.key != b.key || a.base != b.base || a.text != b.text);
    UnitsDiff {
        changed,
        prev: prev.to_vec(),
        next: next.to_vec(),
        par_cle,
    }
}

impl UnitsDiff {
    /// Le point le plus proche qui survit, pour une position perdue.
    fn repli(&self, pos: usize) -> usize {
        if self.next.is_empty() {
            return 0;
        }
        let debut = rang_avant(&self.prev, pos).map_or(0, |i| i + 1);
        for u in &self.prev[debut..] {
            if let Some(&n) = self.par_cle.get(&u.key) {
                return self.next[n].base;
            }
        }
        end_of(&self.next)
    }

    /// Où va l'ancienne position `pos` (`assoc` : de quel côté elle colle à une insertion pile dessus).
    pub fn map_pos(&self, pos: usize, assoc: Assoc) -> MappedPos {
        if !self.changed {
            return MappedPos {
                pos,
                deleted: false,
            };
        }
        let Some(i) = unit_at(&self.prev, pos) else {
            return MappedPos {
                pos: self.repli(pos),
                deleted: false,
            };
        };
        let u = &self.prev[i];
        let Some(&n) = self.par_cle.get(&u.key) else {
            return MappedPos {
                pos: self.repli(pos),
                deleted: true,
            };
        };
        let n = &self.next[n];
        let ch = pos - u.base;
        if u.text == n.text {
            return MappedPos {
                pos: n.base + ch,
                deleted: false,
            };
        }
        let r = map_through_delta(text_delta(&u.text, &n.text), ch, assoc);
        MappedPos {
            pos: n.base + r.ch,
            deleted: r.deleted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(msg: usize, part: usize, text: &str, id: Option<&str>) -> UnitSpec {
        let id = id.map_or_else(|| format!("m{msg}"), str::to_string);
        UnitSpec {
            msg,
            part,
            key: format!("{id}#{part}"),
            text: text.to_string(),
        }
    }

    mod positions_a_pas_fixe {
        use super::*;

        fn units() -> Vec<Unit> {
            units_of(&[
                spec(0, 0, "Bonjour", None),
                spec(1, 0, "Réponse", None),
                spec(1, 2, "Suite", None),
            ])
        }

        #[test]
        fn base_message_x_message_stride_part_x_part_stride() {
            let units = units();
            let bases: Vec<_> = units.iter().map(|u| u.base).collect();
            assert_eq!(
                bases,
                vec![0, MESSAGE_STRIDE, MESSAGE_STRIDE + 2 * PART_STRIDE]
            );
            assert_eq!(base_of(3, 1), 3 * MESSAGE_STRIDE + PART_STRIDE);
        }

        #[test]
        fn unit_at_bornes_incluses_trous_exclus() {
            let units = units();
            assert_eq!(unit_at(&units, 7), Some(0));
            assert_eq!(unit_at(&units, 8), None);
            assert_eq!(unit_at(&units, MESSAGE_STRIDE + 3), Some(1));
            assert_eq!(end_of(&units), MESSAGE_STRIDE + 2 * PART_STRIDE + 5);
        }

        #[test]
        fn lignes_unites_aller_retour_position_ligne_ch() {
            let units = units();
            for p in [
                0,
                3,
                7,
                MESSAGE_STRIDE + 2,
                MESSAGE_STRIDE + 2 * PART_STRIDE + 5,
            ] {
                assert_eq!(pos_to_offset(&units, offset_to_pos(&units, p)), p);
            }
            // Un trou → début de la suivante.
            assert_eq!(offset_to_pos(&units, 50), EditorPosition { line: 1, ch: 0 });
            // Au-delà → la fin.
            assert_eq!(
                offset_to_pos(&units, 10 * MESSAGE_STRIDE),
                EditorPosition { line: 2, ch: 5 }
            );
            assert_eq!(pos_to_offset(&units, EditorPosition { line: 0, ch: 99 }), 7);
        }

        #[test]
        fn texte_joint_index_position_separateurs_compris() {
            let units = units();
            let t = surface_text_of(&units);
            assert_eq!(t.text, "Bonjour\n\nRéponse\n\nSuite");
            for p in [
                0,
                7,
                MESSAGE_STRIDE,
                MESSAGE_STRIDE + 7,
                MESSAGE_STRIDE + 2 * PART_STRIDE + 2,
            ] {
                assert_eq!(t.to_pos(t.to_index(p)), p);
            }
            // Dans le séparateur → fin de l'unité précédente.
            assert_eq!(t.to_pos(8), 7);
            // ★ Index en UTF-16 : « é » compte 1 comme en JS (le texte contient un accent).
            let idx = utf16_len("Bonjour\n\nRéponse\n\n");
            assert_eq!(t.to_index(t.to_pos(idx)), idx);
        }

        #[test]
        fn vide() {
            assert_eq!(offset_to_pos(&[], 5), EditorPosition { line: 0, ch: 0 });
            assert_eq!(surface_text_of(&[]).to_pos(3), 0);
        }
    }

    mod diff_units_tests {
        use super::*;

        fn avant() -> Vec<Unit> {
            units_of(&[spec(0, 0, "Question", None), spec(1, 0, "Répon", None)])
        }

        fn m(pos: usize, deleted: bool) -> MappedPos {
            MappedPos { pos, deleted }
        }

        #[test]
        fn rien_n_a_change() {
            let apres = units_of(&[spec(0, 0, "Question", None), spec(1, 0, "Répon", None)]);
            assert!(!diff_units(&avant(), &apres).changed);
        }

        #[test]
        fn flux_seul_le_message_qui_s_ecrit_bouge_et_seulement_apres_sa_fin() {
            let apres = units_of(&[spec(0, 0, "Question", None), spec(1, 0, "Réponse.", None)]);
            let d = diff_units(&avant(), &apres);
            assert!(d.changed);
            assert_eq!(d.map_pos(3, Assoc::After), m(3, false));
            assert_eq!(
                d.map_pos(MESSAGE_STRIDE + 2, Assoc::After),
                m(MESSAGE_STRIDE + 2, false)
            );
            assert_eq!(
                d.map_pos(MESSAGE_STRIDE + 5, Assoc::After),
                m(MESSAGE_STRIDE + 8, false)
            );
            assert_eq!(
                d.map_pos(MESSAGE_STRIDE + 5, Assoc::Before),
                m(MESSAGE_STRIDE + 5, false)
            );
        }

        #[test]
        fn historique_charge_avant_les_tours_en_direct_rebases_par_leur_cle() {
            let direct = units_of(&[spec(0, 0, "Q", Some("u1")), spec(1, 0, "R", Some("a1"))]);
            let apres = units_of(&[
                spec(0, 0, "Vieux", Some("m0")),
                spec(1, 0, "Q", Some("u1")),
                spec(2, 0, "R", Some("a1")),
            ]);
            let d = diff_units(&direct, &apres);
            assert_eq!(
                d.map_pos(MESSAGE_STRIDE, Assoc::After),
                m(2 * MESSAGE_STRIDE, false)
            );
        }

        #[test]
        fn reecriture_au_milieu_la_position_dans_le_passage_remplace_est_perdue() {
            let apres = units_of(&[spec(0, 0, "Quatre", None), spec(1, 0, "Répon", None)]);
            let d = diff_units(&avant(), &apres);
            assert!(d.map_pos(4, Assoc::After).deleted);
            assert_eq!(
                d.map_pos(MESSAGE_STRIDE + 1, Assoc::After),
                m(MESSAGE_STRIDE + 1, false)
            );
        }

        #[test]
        fn message_disparu_perdu_replie_sur_l_unite_suivante_qui_survit() {
            let apres = units_of(&[spec(1, 0, "Répon", None)]);
            let d = diff_units(&avant(), &apres);
            assert_eq!(d.map_pos(2, Assoc::After), m(MESSAGE_STRIDE, true));
        }

        #[test]
        fn propriete_toute_position_remappee_reste_dans_le_document_graines() {
            // ★ Le TS tire d'un LCG en flottants (précision perdue au-delà de 2^53) ; ici le même
            //   LCG en entiers 64 bits : la propriété testée ne dépend pas de la suite exacte.
            let mut seed: u64 = 42;
            let mut rand = move || {
                seed = (seed * 1_103_515_245 + 12_345) % (1 << 31);
                seed as f64 / (1u64 << 31) as f64
            };
            let mot = |r: f64| "abcdefghij"[..1 + (r * 9.0).floor() as usize].to_string();
            let mut prev = units_of(&[spec(0, 0, "abc", None)]);
            for step in 0..200 {
                let mut specs: Vec<UnitSpec> = prev
                    .iter()
                    .map(|u| UnitSpec {
                        msg: u.msg,
                        part: u.part,
                        key: u.key.clone(),
                        text: u.text.clone(),
                    })
                    .collect();
                let r = rand();
                let m1 = mot(rand());
                if r < 0.4 {
                    if let Some(dernier) = specs.last_mut() {
                        dernier.text.push_str(&m1);
                    }
                } else if r < 0.6 {
                    let n = specs.len();
                    specs.push(spec(n, 0, &m1, Some(&format!("n{step}"))));
                } else if r < 0.8 {
                    specs.insert(0, spec(0, 0, &m1, Some(&format!("h{step}"))));
                } else if let Some(premier) = specs.first_mut() {
                    // `mot + text.slice(1)` : le premier caractère est remplacé.
                    let reste: String = premier.text.chars().skip(1).collect();
                    premier.text = format!("{m1}{reste}");
                }
                for (i, s) in specs.iter_mut().enumerate() {
                    s.msg = i;
                }
                let next = units_of(&specs);
                let d = diff_units(&prev, &next);
                for u in &prev {
                    for ch in [0, u.len] {
                        let mp = d.map_pos(u.base + ch, Assoc::After);
                        assert!(mp.pos <= end_of(&next));
                    }
                }
                prev = next;
            }
        }
    }
}
