// Port de app/src/ui/views/annotation/AnnotationPersistence.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le pont annotation ↔ journal d'artefacts : un record par trait
//  (`annotation:stroke`), un par calque (`annotation:calque`), et la migration de
//  l'ancien format `annotation:doc`. Le journal reste indépendant des instances.
//
//  ★ Transposition : en TS la classe est un `Component` qui écoute l'`ArtefactManager`,
//    le workspace et les documents, et relit en `async`. Ici, seule la PARTIE PURE
//    est portée — décodage, encodage en `Op`, migration, et le compteur de
//    RÉVISIONS qui invalide une lecture lente. L'orchestration (abonnements,
//    `queueMicrotask`, `await artefacts.ready`) revient à l'hôte, qui boucle :
//    `revision = revisions.get(f)` → lire → si `revisions.perimee(f, revision)`
//    recommencer, sinon [`lire_fichier`] puis `source.hydrate`.
// ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;

use fragment_artefacts::anchor::{SurfaceText, resolve_anchor};
use fragment_artefacts::{
    ArtefactScope, DurableAnchor, Op, Origin, Rec, ResolutionStatus, SetOp, WidgetAnchor,
};
use serde_json::{Value, json};

use crate::geometry::Point;
use crate::source::{AnnotationEdit, Calque, DEFAULT_CALQUE, Stroke};

pub const STROKE_TYPE: &str = "annotation:stroke";
pub const CALQUE_TYPE: &str = "annotation:calque";
pub const LEGACY_TYPE: &str = "annotation:doc";

/// Le type d'un record est-il l'un des trois que l'annotation lit ?
pub fn est_type_annotation(type_: Option<&str>) -> bool {
    matches!(type_, Some(STROKE_TYPE | CALQUE_TYPE | LEGACY_TYPE))
}

/// Les révisions par document : toute arrivée, suppression, édition locale ou
/// renommage invalide l'intégralité d'une lecture en cours (ne ressuscite pas un
/// trait supprimé, ne perd pas une édition locale).
#[derive(Default, Debug)]
pub struct Revisions {
    par_fichier: HashMap<String, u64>,
}

impl Revisions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Incrémente (créé à 1) et rend la nouvelle révision.
    pub fn bump(&mut self, file: &str) -> u64 {
        let r = self.par_fichier.entry(file.to_string()).or_insert(0);
        *r += 1;
        *r
    }

    pub fn get(&self, file: &str) -> Option<u64> {
        self.par_fichier.get(file).copied()
    }

    /// Une lecture démarrée à `revision` est-elle périmée ?
    pub fn perimee(&self, file: &str, revision: Option<u64>) -> bool {
        self.get(file) != revision
    }

    /// Les documents suivis (ceux qu'un événement du journal peut concerner).
    pub fn fichiers(&self) -> Vec<String> {
        self.par_fichier.keys().cloned().collect()
    }

    pub fn oublier(&mut self, file: &str) {
        self.par_fichier.remove(file);
    }

    pub fn vider(&mut self) {
        self.par_fichier.clear();
    }
}

fn est_fini_positif(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|w| w.is_finite() && *w > 0.0)
}

/// Décode un record `annotation:stroke` (ou un trait de l'ancien format).
///
/// `None` si l'état est invalide (pas de points, point non fini, couleur/outil non
/// chaîne, largeur ≤ 0), si l'ancre manque ou est négative, ou si elle est ORPHELINE.
pub fn decode_stroke(
    id: &str,
    anchor: Option<&DurableAnchor>,
    state: &Value,
    text: Option<&dyn SurfaceText>,
) -> Option<Stroke> {
    let obj = state.as_object()?;
    let points_json = obj.get("points")?.as_array().filter(|a| !a.is_empty())?;
    let mut points = Vec::with_capacity(points_json.len());
    for p in points_json {
        let o = p.as_object()?;
        points.push(Point {
            dx: o.get("dx")?.as_f64()?,
            dy: o.get("dy")?.as_f64()?,
        });
    }
    let color = obj.get("color")?.as_str()?.to_string();
    let tool = obj.get("tool")?.as_str()?.to_string();
    let width = est_fini_positif(obj.get("width"))?;
    let anchor = anchor?;
    // `'pos' in rec.anchor` : une ancre viewport n'a pas de `pos` → refusée.
    let pos = anchor.anchor.pos().filter(|p| *p >= 0)?;
    let resolved = resolve_anchor(anchor, text);
    if resolved.status == ResolutionStatus::Orphan {
        return None;
    }
    let pos = resolved.anchor.pos().unwrap_or(pos);
    Some(Stroke {
        id: id.to_string(),
        pos: usize::try_from(pos).ok()?,
        points,
        color,
        width,
        tool,
        calque: obj
            .get("calque")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_CALQUE)
            .to_string(),
    })
}

/// Décode un record `annotation:calque`.
pub fn decode_calque(id: &str, state: &Value) -> Option<Calque> {
    let obj = state.as_object()?;
    Some(Calque {
        id: id.to_string(),
        name: obj.get("name")?.as_str()?.to_string(),
        order: obj.get("order")?.as_str()?.to_string(),
        color: obj.get("color").and_then(Value::as_str).map(str::to_string),
    })
}

/// L'ancre durable d'un trait à `pos` (citation relevée sur la surface si elle a du texte).
pub type FabriqueAncre<'a> = &'a dyn Fn(usize) -> DurableAnchor;

/// Une écriture → une opération du journal (`operation` du TS).
///
/// ★ Un trait est rangé SANS son `id` ni son `pos` : l'id est celui du record, la
///   position vit dans l'ANCRE (qui porte aussi la citation pour la retrouver).
pub fn operation(file: &str, edit: &AnnotationEdit, ancre: FabriqueAncre<'_>) -> Op {
    let scope = ArtefactScope::file(file);
    match edit {
        AnnotationEdit::Stroke { id, value: None } | AnnotationEdit::Calque { id, value: None } => {
            Op::del(id)
        }
        AnnotationEdit::Calque {
            id,
            value: Some(c),
        } => {
            let mut state = json!({ "id": c.id, "name": c.name, "order": c.order });
            if let (Some(color), Some(o)) = (&c.color, state.as_object_mut()) {
                o.insert("color".into(), json!(color));
            }
            SetOp::new(id)
                .content(CALQUE_TYPE, 1, state)
                .scope(scope)
                .into()
        }
        AnnotationEdit::Stroke {
            id,
            value: Some(s),
        } => {
            let state = json!({
                "points": s.points,
                "color": s.color,
                "width": s.width,
                "tool": s.tool,
                "calque": s.calque,
            });
            SetOp::new(id)
                .content(STROKE_TYPE, 1, state)
                .scope(scope)
                .anchor(ancre(s.pos))
                .into()
        }
    }
}

/// Les opérations d'une écriture de la source (`write` du TS).
///
/// ★ Origine `follow` (le trait a suivi le texte) : on ne réécrit QUE l'ancre
///   (`{ set, anchor }`), pas le tracé — sinon chaque frappe réécrirait tous les traits.
pub fn ops_d_ecriture(
    file: &str,
    edits: &[AnnotationEdit],
    origin: Origin,
    ancre: FabriqueAncre<'_>,
) -> Vec<Op> {
    edits
        .iter()
        .map(|e| match e {
            AnnotationEdit::Stroke {
                id,
                value: Some(s),
            } if origin == Origin::Follow => SetOp::new(id).anchor(ancre(s.pos)).into(),
            _ => operation(file, e, ancre),
        })
        .collect()
}

/// Un record du journal et son état déjà lu.
#[derive(Clone, Debug)]
pub struct Enregistrement {
    pub rec: Rec,
    pub state: Option<Value>,
}

/// Ne garde que les records `v == 1` des trois types lus (les versions futures
/// restent INTACTES dans le journal : elles ne sont ni lues ni migrées).
pub fn filtrer_enregistrements(recs: Vec<Enregistrement>) -> Vec<Enregistrement> {
    recs.into_iter()
        .filter(|r| r.rec.v.unwrap_or(1) == 1 && est_type_annotation(r.rec.type_.as_deref()))
        .collect()
}

/// Ce que la relecture d'un document produit.
#[derive(Debug, Default)]
pub struct Lecture {
    pub strokes: Vec<Stroke>,
    pub calques: Vec<Calque>,
    /// Opérations de migration à répartir sur le journal (origine `migrate`).
    pub migration: Vec<Op>,
}

/// Reconstruit les traits et calques d'un document depuis ses records (le corps de
/// `refresh`). `existe(id)` dit si un record de ce nom existe déjà (une migration
/// relancée ne crée pas de doublon).
///
/// `None` : un trait de l'ancien format est invalide — la lecture est ABANDONNÉE, rien
/// n'est hydraté ni migré (« une migration partielle ne détruit jamais le record original »).
pub fn lire_fichier(
    recs: &[Enregistrement],
    text: Option<&dyn SurfaceText>,
    existe: &dyn Fn(&str) -> bool,
    file: &str,
    ancre: FabriqueAncre<'_>,
) -> Option<Lecture> {
    let mut lecture = Lecture::default();
    for r in recs {
        let state = r.state.clone().unwrap_or(Value::Null);
        match r.rec.type_.as_deref() {
            Some(STROKE_TYPE) => {
                if let Some(s) = decode_stroke(&r.rec.id, r.rec.anchor(), &state, text) {
                    lecture.strokes.push(s);
                }
            }
            Some(CALQUE_TYPE) => {
                if let Some(c) = decode_calque(&r.rec.id, &state) {
                    lecture.calques.push(c);
                }
            }
            _ => {
                // L'ancien format : `{ strokes: [{ id, pos, points… }] }`.
                let Some(anciens) = state
                    .as_object()
                    .and_then(|o| o.get("strokes"))
                    .and_then(Value::as_array)
                else {
                    continue;
                };
                let mut convertis = Vec::new();
                for old in anciens {
                    let o = old.as_object()?;
                    let ancien_id = match o.get("id") {
                        Some(Value::String(s)) => s.clone(),
                        Some(v) => v.to_string(),
                        None => "undefined".to_string(),
                    };
                    let id = format!("{}:{}", r.rec.id, ancien_id);
                    let pos = o.get("pos").and_then(Value::as_f64).filter(|p| p.is_finite())?;
                    let vivante = WidgetAnchor::document(pos as i64);
                    let stroke = decode_stroke(&id, Some(&vivante.into()), old, None)?;
                    convertis.push(stroke);
                }
                for stroke in convertis {
                    if existe(&stroke.id) {
                        continue;
                    }
                    lecture.migration.push(operation(
                        file,
                        &AnnotationEdit::Stroke {
                            id: stroke.id.clone(),
                            value: Some(stroke.clone()),
                        },
                        ancre,
                    ));
                    lecture.strokes.push(stroke);
                }
                lecture.migration.push(Op::del(&r.rec.id));
            }
        }
    }
    Some(lecture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::p;
    use fragment_artefacts::anchor::PlainText;

    fn ancre(pos: usize) -> DurableAnchor {
        WidgetAnchor::document(pos as i64).into()
    }

    fn etat() -> Value {
        json!({ "points": [{"dx": 0, "dy": 0}, {"dx": 8, "dy": 4}], "color": "#f00", "width": 2, "tool": "crayon" })
    }

    fn rec(id: &str, type_: &str, v: u32, pos: i64, state: Value) -> Enregistrement {
        let mut r = Rec::new(id);
        r.type_ = Some(type_.into());
        r.v = Some(v);
        r.anchor = Some(Some(WidgetAnchor::document(pos).into()));
        r.scope = Some(ArtefactScope::file("a.md"));
        Enregistrement {
            rec: r,
            state: Some(state),
        }
    }

    #[test]
    fn decode_un_trait_valide_et_met_le_calque_par_defaut() {
        let s = decode_stroke("s", Some(&ancre(10)), &etat(), None).expect("trait");
        assert_eq!(s.pos, 10);
        assert_eq!(s.calque, DEFAULT_CALQUE);
        assert_eq!(s.points[1], p(8.0, 4.0));
    }

    #[test]
    fn refuse_un_etat_invalide() {
        let sans_points = json!({ "points": [], "color": "x", "width": 1, "tool": "t" });
        assert!(decode_stroke("s", Some(&ancre(1)), &sans_points, None).is_none());
        let mut largeur = etat();
        largeur["width"] = json!(0);
        assert!(decode_stroke("s", Some(&ancre(1)), &largeur, None).is_none());
        let mut couleur = etat();
        couleur["color"] = json!(3);
        assert!(decode_stroke("s", Some(&ancre(1)), &couleur, None).is_none());
        assert!(decode_stroke("s", None, &etat(), None).is_none());
        assert!(decode_stroke("s", Some(&ancre(0)), &Value::Null, None).is_none());
        let viewport = DurableAnchor::from(WidgetAnchor::Viewport {
            x: 1.0,
            y: 2.0,
            from: None,
        });
        assert!(decode_stroke("s", Some(&viewport), &etat(), None).is_none());
    }

    #[test]
    fn un_trait_dont_la_citation_est_introuvable_est_orphelin() {
        let texte = PlainText::new("bonjour le monde");
        let mut a = ancre(3);
        a.quote = Some(fragment_artefacts::AnchorQuote {
            prefix: String::new(),
            exact: "zzzzzzzzzzzzzzzzzzzzzzzz".into(),
            suffix: String::new(),
        });
        assert!(decode_stroke("s", Some(&a), &etat(), Some(&texte)).is_none());
    }

    #[test]
    fn decode_un_calque_avec_couleur_facultative() {
        let c = decode_calque("c", &json!({"name": "Cours", "order": "i"})).expect("calque");
        assert_eq!((c.name.as_str(), c.color.as_deref()), ("Cours", None));
        let c = decode_calque("c", &json!({"name": "N", "order": "i", "color": "#fff"}));
        assert_eq!(c.and_then(|c| c.color), Some("#fff".to_string()));
        assert!(decode_calque("c", &json!({"name": "N"})).is_none());
    }

    #[test]
    fn un_trait_s_ecrit_sans_id_ni_pos_et_porte_son_ancre() {
        let stroke = Stroke {
            id: "s".into(),
            pos: 10,
            points: vec![p(0.0, 0.0)],
            color: "#f00".into(),
            width: 2.0,
            tool: "crayon".into(),
            calque: "c".into(),
        };
        let op = operation(
            "a.md",
            &AnnotationEdit::Stroke {
                id: "s".into(),
                value: Some(stroke),
            },
            &ancre,
        );
        let set = op.as_set().expect("set");
        assert_eq!(set.type_.as_deref(), Some(STROKE_TYPE));
        assert_eq!(set.v, Some(1));
        let state = set.state.clone().unwrap_or(Value::Null);
        assert!(state.get("id").is_none() && state.get("pos").is_none());
        assert_eq!(state["calque"], json!("c"));
        assert_eq!(
            set.anchor.as_ref().and_then(|a| a.as_ref()).and_then(|a| a.anchor.pos()),
            Some(10)
        );
    }

    #[test]
    fn une_suppression_devient_une_tombe() {
        let op = operation(
            "a.md",
            &AnnotationEdit::Calque {
                id: "c".into(),
                value: None,
            },
            &ancre,
        );
        assert_eq!(op, Op::del("c"));
    }

    #[test]
    fn l_origine_follow_n_ecrit_que_l_ancre() {
        let stroke = Stroke {
            id: "s".into(),
            pos: 15,
            points: vec![p(0.0, 0.0)],
            color: "x".into(),
            width: 1.0,
            tool: "t".into(),
            calque: DEFAULT_CALQUE.into(),
        };
        let edits = [AnnotationEdit::Stroke {
            id: "s".into(),
            value: Some(stroke),
        }];
        let ops = ops_d_ecriture("a.md", &edits, Origin::Follow, &ancre);
        let set = ops[0].as_set().expect("set");
        assert!(set.type_.is_none() && set.state.is_none() && set.scope.is_none());
        assert!(set.anchor.is_some());
        let ops = ops_d_ecriture("a.md", &edits, Origin::User, &ancre);
        assert!(ops[0].as_set().is_some_and(|s| s.state.is_some()));
    }

    #[test]
    fn les_revisions_perimment_une_lecture_lente() {
        let mut rev = Revisions::new();
        rev.bump("a.md");
        let debut = rev.get("a.md");
        assert!(!rev.perimee("a.md", debut));
        // Une édition locale pendant la lecture.
        rev.bump("a.md");
        assert!(rev.perimee("a.md", debut));
    }

    #[test]
    fn migre_le_format_annotation_doc_sans_doublon_et_laisse_les_versions_futures_intactes() {
        let ancien = json!({ "strokes": [{ "id": "s1", "pos": 10, "points": [{"dx": 0, "dy": 0}, {"dx": 8, "dy": 4}], "color": "#f00", "width": 2, "tool": "crayon" }] });
        let recs = vec![
            rec("legacy", LEGACY_TYPE, 1, 0, ancien),
            rec("future", STROKE_TYPE, 2, 10, etat()),
        ];
        let recs = filtrer_enregistrements(recs);
        assert_eq!(recs.len(), 1, "la version future n'est ni lue ni migrée");
        let l = lire_fichier(&recs, None, &|_| false, "a.md", &ancre).expect("lecture");
        assert_eq!(l.strokes.len(), 1);
        assert_eq!(l.strokes[0].id, "legacy:s1");
        // set du trait migré puis tombe de l'ancien record.
        assert_eq!(l.migration.len(), 2);
        assert_eq!(l.migration[0].id(), "legacy:s1");
        assert_eq!(l.migration[1], Op::del("legacy"));
        // Relancée après migration : le trait existe déjà, pas de doublon, tombe seule.
        let l = lire_fichier(&recs, None, &|id| id == "legacy:s1", "a.md", &ancre)
            .expect("relecture");
        assert!(l.strokes.is_empty());
        assert_eq!(l.migration, vec![Op::del("legacy")]);
    }

    #[test]
    fn une_migration_partielle_ne_detruit_jamais_le_record_original() {
        let ancien = json!({ "strokes": [
            { "id": "ok", "pos": 1, "points": [{"dx": 0, "dy": 0}], "color": "x", "width": 1, "tool": "t" },
            { "id": "ko", "pos": 2, "points": [], "color": "x", "width": 1, "tool": "t" },
        ] });
        let recs = vec![rec("legacy", LEGACY_TYPE, 1, 0, ancien)];
        assert!(lire_fichier(&recs, None, &|_| false, "a.md", &ancre).is_none());
    }

    #[test]
    fn lit_traits_et_calques_et_ignore_les_etats_invalides() {
        let recs = vec![
            rec("s", STROKE_TYPE, 1, 10, etat()),
            rec("mauvais", STROKE_TYPE, 1, 10, json!({"points": []})),
            rec("c", CALQUE_TYPE, 1, 0, json!({"name": "Cours", "order": "i"})),
        ];
        let l = lire_fichier(&recs, None, &|_| false, "a.md", &ancre).expect("lecture");
        assert_eq!(l.strokes.len(), 1);
        assert_eq!(l.calques.len(), 1);
        assert!(l.migration.is_empty());
    }
}
