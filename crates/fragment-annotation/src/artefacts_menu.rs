// Port de app/src/ui/views/artefacts/menuModel.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  « Calques ▸ Artefacts » — la LISTE des artefacts d'un document (pur, testé).
//  Les mêmes records que ceux que le calque monte (ancrés, kind connu, version
//  lisible), masqués compris : c'est d'ici qu'on les réaffiche.
//
//  ★ Crate choisie : fragment-annotation, la plus proche des quatre de cette tâche
//    (le menu « Calques » est celui des calques d'annotation et d'artefacts). Le
//    modèle ne dépend que de `fragment-artefacts` (Rec, ArtefactKind).
// ═══════════════════════════════════════════════════════════════════════════

use std::panic::{AssertUnwindSafe, catch_unwind};

use fragment_artefacts::{ArtefactKind, Rec};

/// Une ligne du menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtefactRow {
    pub id: String,
    pub label: String,
    pub icon: String,
    /// Le passage d'ancrage, court (citation) — distingue deux artefacts de même nom.
    pub passage: Option<String>,
    pub hidden: bool,
    /// Le calque auquel il appartient (`describe().layer`, défaut Artefacts).
    pub layer: String,
}

/// Le calque des artefacts : celui d'un objet qui n'en déclare pas d'autre.
pub const ARTEFACTS_LAYER: &str = "artefacts";

/// Un record qu'un calque Artefacts monterait (hors masquage) — la règle de `ViewReconciler.run`.
pub fn is_mountable(r: &Rec, kind: Option<&ArtefactKind>) -> bool {
    r.anchor().is_some()
        && r.type_.as_deref().is_some_and(|t| !t.is_empty())
        && kind.is_some_and(|k| r.v.unwrap_or(1) <= k.version.unwrap_or(1))
}

/// Le nom, l'icône et le calque d'un artefact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub label: String,
    pub icon: String,
    pub layer: String,
}

/// Le nom d'un artefact : `describe` du kind, sinon sa description (1re phrase), sinon son type.
pub fn label_of(r: &Rec, kind: Option<&ArtefactKind>) -> Label {
    // ★ Un kind qui lève (panique) ne casse pas le menu.
    let d = kind
        .and_then(|k| k.describe.as_ref())
        .and_then(|f| catch_unwind(AssertUnwindSafe(|| f(r.state.as_ref()))).ok())
        .flatten();
    let layer = d
        .as_ref()
        .and_then(|d| d.layer.clone())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| ARTEFACTS_LAYER.to_string());
    if let Some(d) = d.filter(|d| !d.label.is_empty()) {
        return Label {
            label: d.label,
            icon: d.icon.unwrap_or_else(|| "box".into()),
            layer,
        };
    }
    let desc = kind
        .and_then(|k| k.description.as_deref())
        .and_then(|d| d.split(['.', '(', ':']).next())
        .map(|p| trim_js(p).to_string())
        .filter(|d| !d.is_empty());
    Label {
        label: desc
            .or_else(|| r.type_.clone().filter(|t| !t.is_empty()))
            .unwrap_or_else(|| r.id.clone()),
        icon: "box".into(),
        layer,
    }
}

/// Les blancs de `\s` / `trim()` de JS (Unicode « espace » + BOM).
fn est_blanc(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}

fn trim_js(s: &str) -> &str {
    s.trim_matches(est_blanc)
}

/// Longueur par défaut de [`short_quote`].
pub const CITATION_MAX: usize = 28;

/// Une citation courte, blancs repliés, coupée à un MOT (la citation d'une ancre est
/// une fenêtre de 32 caractères : elle finit souvent au milieu d'un mot).
///
/// `max` compte en unités UTF-16 comme le `length` du TS (ADR-004).
pub fn short_quote(text: Option<&str>, max: usize) -> Option<String> {
    let replie: String = {
        let mut out = String::new();
        let mut dans_blanc = false;
        for c in text.unwrap_or("").chars() {
            if est_blanc(c) {
                dans_blanc = true;
            } else {
                if dans_blanc {
                    out.push(' ');
                }
                dans_blanc = false;
                out.push(c);
            }
        }
        // `replace(/\s+/g, ' ').trim()` : un blanc de tête devient « » après trim.
        trim_js(&out).to_string()
    };
    if replie.is_empty() {
        return None;
    }
    let unites: Vec<u16> = replie.encode_utf16().collect();
    if unites.len() <= max {
        return Some(replie);
    }
    let mut cut = unites[..max].to_vec();
    if let Some(espace) = cut.iter().rposition(|u| *u == u16::from(b' ')) {
        if espace > max / 2 {
            cut.truncate(espace);
        }
    }
    let cut = String::from_utf16_lossy(&cut);
    // « garder.… » → « garder… »
    let cut = cut.trim_end_matches(|c: char| est_blanc(c) || ".,;:!?…".contains(c));
    Some(format!("{cut}…"))
}

/// Les lignes du menu, dans l'ordre du texte ; un ancrage viewport à la fin.
pub fn artefact_rows<'k>(
    recs: &[Rec],
    kind_of: &dyn Fn(&str) -> Option<&'k ArtefactKind>,
    hidden: &dyn Fn(&str) -> bool,
) -> Vec<ArtefactRow> {
    let mut rows: Vec<(ArtefactRow, i64)> = Vec::new();
    for r in recs {
        let kind = r.type_.as_deref().and_then(kind_of);
        if !is_mountable(r, kind) {
            continue;
        }
        let Label { label, icon, layer } = label_of(r, kind);
        let Some(a) = r.anchor() else { continue };
        rows.push((
            ArtefactRow {
                id: r.id.clone(),
                label,
                icon,
                passage: short_quote(a.quote.as_ref().map(|q| q.exact.as_str()), CITATION_MAX),
                hidden: hidden(&r.id),
                layer,
            },
            // Number.MAX_SAFE_INTEGER pour une ancre sans `pos` (viewport).
            a.anchor.pos().unwrap_or(9_007_199_254_740_991),
        ));
    }
    // L'ordre du texte ; à égalité (même passage), l'ordre d'arrivée (tri stable).
    rows.sort_by_key(|(_, pos)| *pos);
    rows.into_iter().map(|(row, _)| row).collect()
}

/// Les lignes d'UNE section du menu « Calques ». Un objet d'un calque que la vue ne
/// propose pas (inconnu, retiré — ex. l'ancien calque `reader`) tombe sous Artefacts :
/// sinon il serait impossible à retrouver et à supprimer.
pub fn rows_of_layer(
    rows: &[ArtefactRow],
    layer_id: &str,
    offered: &dyn Fn(&str) -> bool,
) -> Vec<ArtefactRow> {
    let section = |r: &ArtefactRow| {
        if offered(&r.layer) {
            r.layer.clone()
        } else {
            ARTEFACTS_LAYER.to_string()
        }
    };
    rows.iter()
        .filter(|r| section(r) == layer_id)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fragment_artefacts::{
        AnchorQuote, ArtefactScope, Described, DurableAnchor, GutterSide, WidgetAnchor,
    };
    use serde_json::{Value, json};

    fn kind(f: impl FnOnce(&mut ArtefactKind)) -> ArtefactKind {
        let mut k = ArtefactKind::new("p:k");
        f(&mut k);
        k
    }

    /// `pos` `None` = ancrage viewport.
    fn rec(id: &str, pos: Option<i64>, state: Option<Value>) -> Rec {
        let mut r = Rec::new(id);
        r.type_ = Some("p:k".into());
        r.state = state;
        r.anchor = Some(Some(match pos {
            None => DurableAnchor::from(WidgetAnchor::Viewport {
                x: 0.0,
                y: 0.0,
                from: None,
            }),
            Some(pos) => DurableAnchor {
                anchor: WidgetAnchor::Gutter {
                    side: GutterSide::Right,
                    pos,
                    to: None,
                    dy: 0.0,
                },
                quote: Some(AnchorQuote {
                    prefix: String::new(),
                    exact: format!("passage {id}"),
                    suffix: String::new(),
                }),
            },
        }));
        r
    }

    fn k_decrit() -> ArtefactKind {
        kind(|k| {
            k.describe = Some(Box::new(|s: Option<&Value>| {
                Some(Described {
                    label: s
                        .and_then(|s| s.get("t"))
                        .and_then(Value::as_str)
                        .unwrap_or("Sans nom")
                        .to_string(),
                    icon: Some("message-square".into()),
                    layer: None,
                })
            }))
        })
    }

    // ── artefactRows ──
    #[test]
    fn dans_l_ordre_du_texte_un_ancrage_viewport_a_la_fin_masques_marques() {
        let k = k_decrit();
        let kind_of = |t: &str| (t == "p:k").then_some(&k);
        let recs = [
            rec("b", Some(50), Some(json!({"t": "B"}))),
            rec("v", None, None),
            rec("a", Some(10), Some(json!({"t": "A"}))),
        ];
        let rows = artefact_rows(&recs, &kind_of, &|id| id == "b");
        let vue: Vec<(&str, &str, bool)> = rows
            .iter()
            .map(|r| (r.id.as_str(), r.label.as_str(), r.hidden))
            .collect();
        assert_eq!(
            vue,
            vec![("a", "A", false), ("b", "B", true), ("v", "Sans nom", false)]
        );
        assert_eq!(rows[0].icon, "message-square");
        assert_eq!(rows[0].passage.as_deref(), Some("passage a"));
    }

    #[test]
    fn comme_le_calque_sans_ancre_sans_type_kind_absent_ou_trop_ancien_absent() {
        let k = k_decrit();
        let kind_of = |t: &str| (t == "p:k").then_some(&k);
        let mut trait_ = Rec::new("trait");
        trait_.type_ = Some("p:k".into());
        trait_.scope = Some(ArtefactScope::file("n.md"));
        let mut inconnu = rec("inconnu", Some(2), None);
        inconnu.type_ = Some("x:y".into());
        let mut futur = rec("futur", Some(3), None);
        futur.v = Some(2);
        let recs = [rec("ok", Some(1), None), trait_, inconnu, futur];
        let rows = artefact_rows(&recs, &kind_of, &|_| false);
        assert_eq!(
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["ok"]
        );
    }

    // ── rowsOfLayer ──
    #[test]
    fn sous_son_calque_sans_calque_artefacts_calque_inconnu_artefacts_sinon_introuvable() {
        let k = kind(|k| {
            k.describe = Some(Box::new(|s: Option<&Value>| {
                Some(Described {
                    label: "x".into(),
                    icon: None,
                    layer: s
                        .and_then(|s| s.get("layer"))
                        .and_then(Value::as_str)
                        .map(String::from),
                })
            }))
        });
        let kind_of = |t: &str| (t == "p:k").then_some(&k);
        let recs = [
            rec("sans", Some(1), None),
            rec("agent", Some(2), Some(json!({"layer": "agent"}))),
            rec("vieux", Some(3), Some(json!({"layer": "reader"}))),
        ];
        let rows = artefact_rows(&recs, &kind_of, &|_| false);
        let offerts = |id: &str| id == "agent" || id == "artefacts";
        let ids = |v: Vec<ArtefactRow>| v.into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(ids(rows_of_layer(&rows, "agent", &offerts)), vec!["agent"]);
        assert_eq!(
            ids(rows_of_layer(&rows, "artefacts", &offerts)),
            vec!["sans", "vieux"]
        );
        assert_eq!(rows_of_layer(&rows, "reader", &offerts), vec![]);
    }

    // ── labelOf — repli ──
    #[test]
    fn describe_sinon_la_1re_phrase_de_la_description_sinon_le_type_un_describe_qui_leve_ne_casse_rien()
     {
        let a = rec("a", Some(0), None);
        let avec_desc = kind(|k| {
            k.description = Some("Un objet HTML jetable (HTML + CSS). Suite.".into());
        });
        assert_eq!(label_of(&a, Some(&avec_desc)).label, "Un objet HTML jetable");
        assert_eq!(label_of(&a, Some(&kind(|_| {}))).label, "p:k");
        let leve = kind(|k| k.describe = Some(Box::new(|_| panic!("x"))));
        assert_eq!(label_of(&a, Some(&leve)).label, "p:k");
    }

    // ── shortQuote ──
    #[test]
    fn blancs_replies_coupee_avec_points_de_suspension() {
        assert_eq!(short_quote(Some("  a\n b "), 28).as_deref(), Some("a b"));
        assert_eq!(
            short_quote(Some(&"x".repeat(40)), 10),
            Some(format!("{}…", "x".repeat(10)))
        );
        assert_eq!(
            short_quote(Some("Premier passage à garder.\n\nSecon"), 28).as_deref(),
            Some("Premier passage à garder…")
        );
        assert_eq!(short_quote(Some("   "), 28), None);
        assert_eq!(short_quote(None, 28), None);
    }
}
