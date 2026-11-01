// Port de app/src/ui/views/chat/components/historyModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le modèle (pur) de la barre latérale du cowork : l'historique groupé par
//  jour, le filtre de la recherche, la salutation de l'accueil.
//
//  ★ Le TS lit l'heure LOCALE (`Date`) : ici le fuseau est un paramètre
//    ([`Fuseau`], voir `calendrier.rs`), `now` et les instants sont en ms.
// ═══════════════════════════════════════════════════════════════════════════

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::api::ChatConversationInfo;
use crate::calendrier::{Fuseau, JOUR_MS, date_courte, date_longue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryGroup {
    pub label: String,
    pub items: Vec<ChatConversationInfo>,
}

/// L'étiquette du jour de `t` vu depuis `now` : Aujourd'hui, Hier, « 10 oct. » (année si autre).
pub fn day_label(t: i64, now: i64, fuseau: &dyn Fuseau) -> String {
    let auj = fuseau.debut_du_jour(now);
    let jour = fuseau.debut_du_jour(t);
    if jour >= auj {
        return "Aujourd'hui".to_string();
    }
    // ★ Par les minuits (pas « − 24 h ») : un jour de changement d'heure dure 23 ou 25 h.
    if jour >= fuseau.debut_du_jour(auj - JOUR_MS / 2) {
        return "Hier".to_string();
    }
    let c = fuseau.civil(t);
    if c.annee == fuseau.civil(now).annee {
        date_courte(c)
    } else {
        date_longue(c)
    }
}

/// Les conversations groupées par jour, la plus récente d'abord (groupes et items).
pub fn group_by_day(
    items: &[ChatConversationInfo],
    now: i64,
    fuseau: &dyn Fuseau,
) -> Vec<HistoryGroup> {
    let mut tries: Vec<&ChatConversationInfo> = items.iter().collect();
    // Tri stable, comme `Array.prototype.sort`.
    tries.sort_by_key(|it| std::cmp::Reverse(it.updated_at));
    let mut groupes: Vec<HistoryGroup> = Vec::new();
    for it in tries {
        let label = day_label(it.updated_at, now, fuseau);
        match groupes.last_mut() {
            Some(dernier) if dernier.label == label => dernier.items.push(it.clone()),
            _ => groupes.push(HistoryGroup {
                label,
                items: vec![it.clone()],
            }),
        }
    }
    groupes
}

/// Minuscules, sans accents : « Écrire » ≈ « ecrire ».
fn plier(s: &str) -> String {
    s.nfd()
        .filter(|&c| !is_combining_mark(c))
        .collect::<String>()
        .to_lowercase()
}

/// Les conversations dont le titre contient `query` (casse et accents ignorés).
pub fn filter_by_title(items: &[ChatConversationInfo], query: &str) -> Vec<ChatConversationInfo> {
    let q = plier(query.trim());
    if q.is_empty() {
        return items.to_vec();
    }
    items
        .iter()
        .filter(|it| {
            let titre = if it.title.is_empty() {
                "Sans titre"
            } else {
                &it.title
            };
            plier(titre).contains(&q)
        })
        .cloned()
        .collect()
}

/// La salutation de l'accueil, selon l'heure locale (0 à 23) : « Bonjour, Julien ».
pub fn salutation(heure: u8, name: &str) -> String {
    let mot = match heure {
        5..=11 => "Bonjour",
        12..=17 => "Bon après-midi",
        _ => "Bonsoir",
    };
    let n = name.trim();
    if n.is_empty() {
        mot.to_string()
    } else {
        format!("{mot}, {n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendrier::{DecalageFixe, instant};

    const FUSEAU: DecalageFixe = DecalageFixe(0);

    // 12 oct. 2026, 15 h.
    fn now() -> i64 {
        instant(&FUSEAU, 2026, 10, 12, 15)
    }

    fn at(y: i32, m: u8, d: u8, h: u8) -> i64 {
        instant(&FUSEAU, y, m, d, h)
    }

    fn info(id: &str, title: &str, updated_at: i64) -> ChatConversationInfo {
        ChatConversationInfo {
            id: id.into(),
            title: title.into(),
            updated_at,
        }
    }

    fn ids(v: &[ChatConversationInfo]) -> Vec<&str> {
        v.iter().map(|x| x.id.as_str()).collect()
    }

    mod day_label_tests {
        use super::*;

        #[test]
        fn aujourd_hui_hier_puis_la_date() {
            let l = |t| day_label(t, now(), &FUSEAU);
            assert_eq!(l(at(2026, 10, 12, 0)), "Aujourd'hui");
            assert_eq!(l(at(2026, 10, 11, 23)), "Hier");
            assert_eq!(l(at(2026, 10, 11, 0)), "Hier");
            assert!(l(at(2026, 10, 10, 10)).starts_with("10 oct"));
            assert!(l(at(2025, 10, 10, 10)).contains("2025"));
        }
    }

    mod group_by_day_tests {
        use super::*;

        #[test]
        fn groupe_et_trie_le_plus_recent_d_abord() {
            let g = group_by_day(
                &[
                    info("a", "A", at(2026, 10, 10, 10)),
                    info("b", "B", at(2026, 10, 12, 9)),
                    info("c", "C", at(2026, 10, 12, 14)),
                    info("d", "D", at(2026, 10, 11, 10)),
                ],
                now(),
                &FUSEAU,
            );
            let labels: Vec<String> = g
                .iter()
                .map(|x| x.label.chars().take(6).collect())
                .collect();
            assert_eq!(labels, vec!["Aujour", "Hier", "10 oct"]);
            assert_eq!(ids(&g[0].items), vec!["c", "b"]);
        }

        #[test]
        fn vide_aucun_groupe() {
            assert_eq!(group_by_day(&[], now(), &FUSEAU), vec![]);
        }
    }

    mod filter_by_title_tests {
        use super::*;

        fn items() -> Vec<ChatConversationInfo> {
            vec![
                info("1", "Écrire un mail", 0),
                info("2", "Planning", 0),
                info("3", "", 0),
            ]
        }

        #[test]
        fn ignore_casse_et_accents() {
            assert_eq!(ids(&filter_by_title(&items(), "ecrire")), vec!["1"]);
            assert_eq!(ids(&filter_by_title(&items(), "PLAN")), vec!["2"]);
        }

        #[test]
        fn requete_vide_tout_sans_titre_cherchable() {
            assert_eq!(filter_by_title(&items(), "  ").len(), 3);
            assert_eq!(ids(&filter_by_title(&items(), "sans")), vec!["3"]);
        }
    }

    mod salutation_tests {
        use super::*;

        #[test]
        fn selon_l_heure_bornes_comprises() {
            assert_eq!(salutation(4, "Julien"), "Bonsoir, Julien");
            assert_eq!(salutation(5, "Julien"), "Bonjour, Julien");
            assert_eq!(salutation(12, "Julien"), "Bon après-midi, Julien");
            assert_eq!(salutation(18, "Julien"), "Bonsoir, Julien");
        }

        #[test]
        fn sans_nom() {
            assert_eq!(salutation(9, "  "), "Bonjour");
        }
    }
}
