// Port de app/src/ui/views/annotation/AnnotationSource.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Données partagées d'un document d'annotation. Les sessions possèdent
//  l'interaction et leur historique ; la SOURCE possède les traits, les calques,
//  la pile d'annulation du `clear/erase/add`, et le remappage à travers l'édition.
//  Indexée par CLÉ de document (note, PDF, conversation…).
// ═══════════════════════════════════════════════════════════════════════════

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use fragment_artefacts::Origin;
use fragment_core::move_key;
use fragment_text::editor::{Assoc, EditorChange, MappedPos};
use fragment_text::map_anchor::{TextAnchored, map_anchor};

use crate::geometry::Point;

/// Le calque par défaut (toujours visible, toujours présent).
pub const DEFAULT_CALQUE: &str = "default";

/// Un trait. `pos` : unités de code UTF-16 (ADR-004).
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    pub id: String,
    pub pos: usize,
    pub points: Vec<Point>,
    pub color: String,
    pub width: f64,
    pub tool: String,
    pub calque: String,
}

/// Un calque d'annotation.
#[derive(Clone, Debug, PartialEq)]
pub struct Calque {
    pub id: String,
    pub name: String,
    /// Clé de tri fractionnaire (chaîne comparée telle quelle).
    pub order: String,
    pub color: Option<String>,
}

/// Une écriture : une valeur FINALE par entité (`None` = suppression).
#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationEdit {
    Stroke { id: String, value: Option<Stroke> },
    Calque { id: String, value: Option<Calque> },
}

impl AnnotationEdit {
    /// `"stroke:<id>"` / `"calque:<id>"` : la clé d'unicité d'un lot.
    pub fn key(&self) -> String {
        match self {
            AnnotationEdit::Stroke { id, .. } => format!("stroke:{id}"),
            AnnotationEdit::Calque { id, .. } => format!("calque:{id}"),
        }
    }
    pub fn id(&self) -> &str {
        match self {
            AnnotationEdit::Stroke { id, .. } | AnnotationEdit::Calque { id, .. } => id,
        }
    }
}

/// Une entrée d'historique : l'état avant / après d'un lot d'écritures.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct HistoryEntry {
    pub before: Vec<AnnotationEdit>,
    pub after: Vec<AnnotationEdit>,
}

/// Un trait à créer (`id` et `calque` facultatifs).
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeInput {
    pub id: Option<String>,
    pub pos: usize,
    pub points: Vec<Point>,
    pub color: String,
    pub width: f64,
    pub tool: String,
    pub calque: Option<String>,
}

/// Erreurs de la source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// `Annotation edit id mismatch` : l'id de la valeur n'est pas celui de l'écriture.
    IdIncoherent,
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceError::IdIncoherent => f.write_str("Annotation edit id mismatch"),
        }
    }
}

impl std::error::Error for SourceError {}

/// La FORME de l'ancre d'un trait — l'affaire de l'annotation, pas du cœur : un
/// trait est un POINT du texte (`pos`), ses points de tracé lui sont relatifs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AncreDuTrait {
    pub pos: usize,
}

impl TextAnchored for AncreDuTrait {
    fn span(&self) -> Option<(usize, Option<usize>)> {
        Some((self.pos, None))
    }
    fn with_span(self, pos: usize, _to: Option<usize>) -> Self {
        AncreDuTrait { pos }
    }
}

/// La fonction de remappage d'un changement d'éditeur.
pub type MapFn<'a> = &'a dyn Fn(usize, Assoc) -> MappedPos;

/// Remappe un trait par la RÈGLE DU CŒUR (`map_anchor`). La POLITIQUE est celle de
/// l'annotation : un trait dont le texte a disparu est retiré (`None`).
fn remapper_trait(stroke: &Stroke, map_pos: MapFn<'_>) -> Option<Stroke> {
    let m = map_anchor(AncreDuTrait { pos: stroke.pos }, map_pos);
    if m.deleted {
        return None;
    }
    Some(Stroke {
        pos: m.anchor.pos,
        ..stroke.clone()
    })
}

/// Remappe une entrée d'historique ; un trait supprimé disparaît des deux côtés.
pub fn map_history(entry: &HistoryEntry, map_pos: MapFn<'_>) -> HistoryEntry {
    let mut deleted: Vec<String> = Vec::new();
    let mut mapped = |edits: &[AnnotationEdit]| -> Vec<AnnotationEdit> {
        let mut out = Vec::new();
        for e in edits {
            match e {
                AnnotationEdit::Stroke {
                    id,
                    value: Some(value),
                } => match remapper_trait(value, map_pos) {
                    Some(v) => out.push(AnnotationEdit::Stroke {
                        id: id.clone(),
                        value: Some(v),
                    }),
                    None => deleted.push(id.clone()),
                },
                other => out.push(other.clone()),
            }
        }
        out
    };
    let before = mapped(&entry.before);
    let after = mapped(&entry.after);
    let garde = |edits: Vec<AnnotationEdit>| -> Vec<AnnotationEdit> {
        edits
            .into_iter()
            .filter(|e| !deleted.iter().any(|d| d == e.id()))
            .collect()
    };
    HistoryEntry {
        before: garde(before),
        after: garde(after),
    }
}

/// Ce que la source annonce à ses abonnés (les `trigger` du TS).
pub enum SourceEvent<'a> {
    /// Des écritures à journaliser (`write`).
    Write {
        file: &'a str,
        edits: &'a [AnnotationEdit],
        origin: Origin,
    },
    /// Les données du document ont changé (`change`).
    Change { file: &'a str },
    /// Les positions viennent d'être remappées (`mapped`) : les historiques des sessions suivent.
    Mapped { file: &'a str, map_pos: MapFn<'a> },
    /// Le texte a changé hors de cette vue (`external`) : relire le journal.
    External { file: &'a str },
}

type Listener = Rc<dyn Fn(&SourceEvent<'_>)>;

#[derive(Default)]
struct Doc {
    strokes: Vec<Stroke>,
    calques: Vec<Calque>,
}

/// Identifiant d'une surface (le `WeakMap<DocumentSurface>` du TS : ici une clé fournie par l'hôte).
pub type SurfaceId = u64;

/// Données partagées du document. Les sessions possèdent l'interaction et leur historique.
#[derive(Default)]
pub struct AnnotationSource {
    documents: RefCell<HashMap<String, Doc>>,
    undo_stacks: RefCell<HashMap<String, Vec<HistoryEntry>>>,
    redo_stacks: RefCell<HashMap<String, Vec<HistoryEntry>>>,
    surfaces: RefCell<HashMap<SurfaceId, usize>>,
    listeners: RefCell<Vec<(u64, Listener)>>,
    next_listener: Cell<u64>,
}

/// Plafond de l'historique (200 entrées).
pub const HISTORIQUE_MAX: usize = 200;

impl AnnotationSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// S'abonne à la source ; rend l'identifiant pour [`AnnotationSource::off`].
    pub fn on(&self, cb: impl Fn(&SourceEvent<'_>) + 'static) -> u64 {
        let id = self.next_listener.get();
        self.next_listener.set(id + 1);
        self.listeners.borrow_mut().push((id, Rc::new(cb)));
        id
    }

    pub fn off(&self, id: u64) {
        self.listeners.borrow_mut().retain(|(i, _)| *i != id);
    }

    fn trigger(&self, event: &SourceEvent<'_>) {
        // Copie : un écouteur peut s'abonner / se désabonner pendant l'émission.
        let copie: Vec<Listener> = self
            .listeners
            .borrow()
            .iter()
            .map(|(_, l)| l.clone())
            .collect();
        for l in copie {
            l(event);
        }
    }

    pub fn strokes(&self, file: &str) -> Vec<Stroke> {
        self.documents
            .borrow()
            .get(file)
            .map(|d| d.strokes.clone())
            .unwrap_or_default()
    }

    pub fn calques(&self, file: &str) -> Vec<Calque> {
        self.documents
            .borrow()
            .get(file)
            .map(|d| d.calques.clone())
            .unwrap_or_default()
    }

    /// Le document (chargé) qui porte ce trait.
    pub fn doc_of(&self, stroke_id: &str) -> Option<String> {
        self.documents
            .borrow()
            .iter()
            .find(|(_, d)| d.strokes.iter().any(|s| s.id == stroke_id))
            .map(|(f, _)| f.clone())
    }

    /// La valeur COURANTE de l'entité visée par `edit` (`None` si absente).
    pub fn value(&self, file: &str, edit: &AnnotationEdit) -> AnnotationEdit {
        match edit {
            AnnotationEdit::Stroke { id, .. } => AnnotationEdit::Stroke {
                id: id.clone(),
                value: self.strokes(file).into_iter().find(|s| &s.id == id),
            },
            AnnotationEdit::Calque { id, .. } => AnnotationEdit::Calque {
                id: id.clone(),
                value: self.calques(file).into_iter().find(|c| &c.id == id),
            },
        }
    }

    /// Une valeur finale par entité : pas de doubles écritures LWW dans la même Tx.
    ///
    /// ★ Comme `new Map(...)` : la DERNIÈRE valeur gagne, à la place de la PREMIÈRE occurrence.
    pub fn commit(
        &self,
        file: &str,
        edits: &[AnnotationEdit],
    ) -> Result<HistoryEntry, SourceError> {
        let mut ordre: Vec<String> = Vec::new();
        let mut par_cle: HashMap<String, AnnotationEdit> = HashMap::new();
        for e in edits {
            let k = e.key();
            if !par_cle.contains_key(&k) {
                ordre.push(k.clone());
            }
            par_cle.insert(k, e.clone());
        }
        let after: Vec<AnnotationEdit> = ordre.iter().filter_map(|k| par_cle.remove(k)).collect();
        let before: Vec<AnnotationEdit> = after.iter().map(|e| self.value(file, e)).collect();
        for e in &after {
            let incoherent = match e {
                AnnotationEdit::Stroke { id, value: Some(v) } => &v.id != id,
                AnnotationEdit::Calque { id, value: Some(v) } => &v.id != id,
                _ => false,
            };
            if incoherent {
                return Err(SourceError::IdIncoherent);
            }
        }
        {
            let mut docs = self.documents.borrow_mut();
            let doc = docs.entry(file.to_string()).or_default();
            for e in &after {
                match e {
                    AnnotationEdit::Stroke { id, value } => {
                        doc.strokes.retain(|s| &s.id != id);
                        if let Some(v) = value {
                            doc.strokes.push(v.clone());
                        }
                    }
                    AnnotationEdit::Calque { id, value } => {
                        doc.calques.retain(|c| &c.id != id);
                        if let Some(v) = value {
                            doc.calques.push(v.clone());
                        }
                    }
                }
            }
        }
        if !after.is_empty() {
            self.trigger(&SourceEvent::Write {
                file,
                edits: &after,
                origin: Origin::User,
            });
            self.trigger(&SourceEvent::Change { file });
        }
        Ok(HistoryEntry { before, after })
    }

    /// Ajoute un trait et le range dans l'historique de la SOURCE.
    pub fn add(
        &self,
        file: &str,
        input: StrokeInput,
        nouvel_id: impl FnOnce() -> String,
    ) -> Stroke {
        let stroke = Stroke {
            id: input.id.unwrap_or_else(nouvel_id),
            pos: input.pos,
            points: input.points,
            color: input.color,
            width: input.width,
            tool: input.tool,
            calque: input.calque.unwrap_or_else(|| DEFAULT_CALQUE.to_string()),
        };
        let edit = AnnotationEdit::Stroke {
            id: stroke.id.clone(),
            value: Some(stroke.clone()),
        };
        // Un id cohérent par construction : l'erreur ne peut pas se produire ici.
        if let Ok(entry) = self.commit(file, &[edit]) {
            self.remember(file, entry);
        }
        stroke
    }

    pub fn erase(&self, file: &str, id: &str) {
        if self.strokes(file).iter().any(|s| s.id == id) {
            let edit = AnnotationEdit::Stroke {
                id: id.to_string(),
                value: None,
            };
            if let Ok(entry) = self.commit(file, &[edit]) {
                self.remember(file, entry);
            }
        }
    }

    pub fn clear(&self, file: &str) {
        let edits: Vec<AnnotationEdit> = self
            .strokes(file)
            .into_iter()
            .map(|s| AnnotationEdit::Stroke {
                id: s.id,
                value: None,
            })
            .collect();
        if let Ok(entry) = self.commit(file, &edits) {
            self.remember(file, entry);
        }
    }

    pub fn can_undo(&self, file: &str) -> bool {
        self.undo_stacks
            .borrow()
            .get(file)
            .is_some_and(|s| !s.is_empty())
    }

    pub fn can_redo(&self, file: &str) -> bool {
        self.redo_stacks
            .borrow()
            .get(file)
            .is_some_and(|s| !s.is_empty())
    }

    pub fn undo(&self, file: &str) -> bool {
        self.travel(file, true)
    }

    pub fn redo(&self, file: &str) -> bool {
        self.travel(file, false)
    }

    fn remember(&self, file: &str, entry: HistoryEntry) {
        let mut undo = self.undo_stacks.borrow_mut();
        let pile = undo.entry(file.to_string()).or_default();
        pile.push(entry);
        if pile.len() > HISTORIQUE_MAX {
            pile.remove(0);
        }
        self.redo_stacks.borrow_mut().remove(file);
    }

    fn travel(&self, file: &str, undo: bool) -> bool {
        let entry = {
            let mut stacks = if undo {
                self.undo_stacks.borrow_mut()
            } else {
                self.redo_stacks.borrow_mut()
            };
            stacks.get_mut(file).and_then(|s| s.pop())
        };
        let Some(entry) = entry else {
            return false;
        };
        let edits = if undo { &entry.before } else { &entry.after };
        let _ = self.commit(file, edits);
        let mut vers = if undo {
            self.redo_stacks.borrow_mut()
        } else {
            self.undo_stacks.borrow_mut()
        };
        vers.entry(file.to_string()).or_default().push(entry);
        true
    }

    /// Remplace les données du document par celles du journal.
    pub fn hydrate(&self, file: &str, strokes: &[Stroke], calques: Option<&[Calque]>) {
        {
            let mut docs = self.documents.borrow_mut();
            let calques = match calques {
                Some(c) => c.to_vec(),
                None => docs
                    .get(file)
                    .map(|d| d.calques.clone())
                    .unwrap_or_default(),
            };
            docs.insert(
                file.to_string(),
                Doc {
                    strokes: strokes.to_vec(),
                    calques,
                },
            );
        }
        self.trigger(&SourceEvent::Change { file });
    }

    /// Un seul abonnement par surface, quel que soit le nombre d'instances.
    /// Rend `true` si c'est le PREMIER (l'hôte branche alors `surface.on_change`
    /// vers [`AnnotationSource::on_surface_change`]).
    pub fn bind(&self, surface: SurfaceId) -> bool {
        let mut s = self.surfaces.borrow_mut();
        let refs = s.entry(surface).or_insert(0);
        *refs += 1;
        *refs == 1
    }

    /// Rend `true` si c'était la DERNIÈRE référence (l'hôte débranche alors la surface).
    pub fn unbind(&self, surface: SurfaceId) -> bool {
        let mut s = self.surfaces.borrow_mut();
        match s.get_mut(&surface) {
            Some(refs) if *refs > 1 => {
                *refs -= 1;
                false
            }
            Some(_) => {
                s.remove(&surface);
                true
            }
            None => false,
        }
    }

    /// Nombre de surfaces branchées.
    pub fn surfaces_liees(&self) -> usize {
        self.surfaces.borrow().len()
    }

    /// Le changement d'une surface liée : remappe (local) ou annonce (`External`).
    pub fn on_surface_change(&self, file: Option<&str>, change: &EditorChange) {
        let Some(file) = file else { return };
        if !change.doc_changed {
            return;
        }
        if change.origin.is_none() {
            self.remap(file, &*change.map_pos);
        } else {
            self.trigger(&SourceEvent::External { file });
        }
    }

    pub fn remap(&self, file: &str, map_pos: MapFn<'_>) {
        let strokes = self.strokes(file);
        let edits: Vec<AnnotationEdit> = strokes
            .iter()
            .map(|s| AnnotationEdit::Stroke {
                id: s.id.clone(),
                value: remapper_trait(s, map_pos),
            })
            .collect();
        {
            let mut docs = self.documents.borrow_mut();
            let doc = docs.entry(file.to_string()).or_default();
            doc.strokes = edits
                .iter()
                .filter_map(|e| match e {
                    AnnotationEdit::Stroke { value: Some(v), .. } => Some(v.clone()),
                    _ => None,
                })
                .collect();
        }
        for stacks in [&self.undo_stacks, &self.redo_stacks] {
            let mut st = stacks.borrow_mut();
            if let Some(entries) = st.get_mut(file) {
                *entries = entries.iter().map(|e| map_history(e, map_pos)).collect();
            }
        }
        self.trigger(&SourceEvent::Mapped { file, map_pos });
        self.trigger(&SourceEvent::Write {
            file,
            edits: &edits,
            origin: Origin::Follow,
        });
        self.trigger(&SourceEvent::Change { file });
    }

    /// Un document (ou tout ce qui est sous un dossier) a été renommé : ses données suivent.
    pub fn rename(&self, old_base: &str, new_base: &str) {
        fn deplacer<T>(map: &mut HashMap<String, T>, old_base: &str, new_base: &str) {
            let cles: Vec<String> = map.keys().cloned().collect();
            for key in cles {
                if let Some(moved) = move_key(&key, old_base, new_base)
                    && let Some(v) = map.remove(&key)
                {
                    map.insert(moved.to_string(), v);
                }
            }
        }
        deplacer(&mut self.documents.borrow_mut(), old_base, new_base);
        deplacer(&mut self.undo_stacks.borrow_mut(), old_base, new_base);
        deplacer(&mut self.redo_stacks.borrow_mut(), old_base, new_base);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::p;

    #[test]
    fn un_renommage_deplace_les_donnees_et_l_historique_sous_la_nouvelle_cle() {
        let src = AnnotationSource::new();
        let input = |id: &str| StrokeInput {
            id: Some(id.into()),
            pos: 3,
            points: vec![p(0.0, 0.0)],
            color: "x".into(),
            width: 1.0,
            tool: "t".into(),
            calque: None,
        };
        src.add("dossier/a.md", input("s1"), || "inutile".into());
        src.add("autre/b.md", input("s2"), || "inutile".into());
        src.rename("dossier", "renomme");
        assert!(src.strokes("dossier/a.md").is_empty());
        assert_eq!(src.strokes("renomme/a.md").len(), 1);
        assert!(src.can_undo("renomme/a.md"));
        assert_eq!(src.strokes("autre/b.md").len(), 1);
    }

    #[test]
    fn deux_ecritures_du_meme_id_ne_gardent_que_la_derniere() {
        let src = AnnotationSource::new();
        let s = |c: &str| Stroke {
            id: "s".into(),
            pos: 1,
            points: vec![p(0.0, 0.0)],
            color: c.into(),
            width: 2.0,
            tool: "crayon".into(),
            calque: DEFAULT_CALQUE.into(),
        };
        let e = |c: &str| AnnotationEdit::Stroke {
            id: "s".into(),
            value: Some(s(c)),
        };
        let h = src.commit("a.md", &[e("blue"), e("green")]).ok();
        assert_eq!(src.strokes("a.md")[0].color, "green");
        assert_eq!(h.map(|h| h.after.len()), Some(1));
    }

    #[test]
    fn un_id_incoherent_est_refuse_sans_rien_ecrire() {
        let src = AnnotationSource::new();
        let stroke = Stroke {
            id: "autre".into(),
            pos: 1,
            points: vec![],
            color: "x".into(),
            width: 1.0,
            tool: "t".into(),
            calque: DEFAULT_CALQUE.into(),
        };
        let r = src.commit(
            "a.md",
            &[AnnotationEdit::Stroke {
                id: "s".into(),
                value: Some(stroke),
            }],
        );
        assert_eq!(r, Err(SourceError::IdIncoherent));
        assert!(src.strokes("a.md").is_empty());
    }

    #[test]
    fn un_trait_dont_le_texte_a_disparu_est_retire_au_remappage() {
        let src = AnnotationSource::new();
        src.add(
            "a.md",
            StrokeInput {
                id: Some("s".into()),
                pos: 10,
                points: vec![p(0.0, 0.0)],
                color: "x".into(),
                width: 1.0,
                tool: "t".into(),
                calque: None,
            },
            || "inutile".into(),
        );
        src.remap("a.md", &|pos, _| MappedPos {
            pos: pos.min(3),
            deleted: true,
        });
        assert!(src.strokes("a.md").is_empty());
    }

    #[test]
    fn la_surface_n_est_liee_qu_une_fois() {
        let src = AnnotationSource::new();
        assert!(src.bind(7));
        assert!(!src.bind(7));
        assert!(!src.unbind(7));
        assert_eq!(src.surfaces_liees(), 1);
        assert!(src.unbind(7));
        assert_eq!(src.surfaces_liees(), 0);
    }
}
