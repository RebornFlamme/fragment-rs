// Port de app/src/ui/views/annotation/AnnotationSession.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Une interaction d'annotation : l'outil actif, le calque actif, le style, les
//  calques masqués et l'HISTORIQUE LOCAL de cette instance. Ni moteur d'état
//  générique, ni second cycle de vie.
//
//  ★ Transposition : en TS la session lit les registres `annotation:*@1` du
//    RegistryHub et monte des enfants `Component` (extensions). Ici les registres
//    sont une structure simple ([`AnnotationRegistries`]) et l'HÔTE (fragment-ui)
//    appelle [`AnnotationSession::registres_changed`] quand un registre change et
//    [`AnnotationSession::document_changed`] sur `view-document-change`. Les
//    extensions (enfants Component) et `geometry()` (lit la surface) restent à
//    fragment-ui — voir parite.toml.
// ═══════════════════════════════════════════════════════════════════════════

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use crate::source::{
    AnnotationEdit, AnnotationSource, Calque, DEFAULT_CALQUE, HistoryEntry, SourceError,
    SourceEvent, Stroke, StrokeInput, SurfaceId, map_history,
};

/// Une session ne garde que les 200 dernières entrées.
const HISTORIQUE_MAX: usize = 200;

/// Ce qu'une session sait de son hôte : la CLÉ du document (lue à l'usage : la vue
/// peut en changer), son preset, sa surface.
pub struct AnnotationContext {
    pub doc: RefCell<Option<String>>,
    pub preset: RefCell<Option<String>>,
    pub surface: SurfaceId,
}

impl AnnotationContext {
    pub fn new(doc: Option<&str>, preset: Option<&str>, surface: SurfaceId) -> Self {
        Self {
            doc: RefCell::new(doc.map(str::to_string)),
            preset: RefCell::new(preset.map(str::to_string)),
            surface,
        }
    }
}

/// `annotation:tools@1` : un outil. (`begin(gesture)` reste dans fragment-ui.)
#[derive(Clone, Debug, PartialEq)]
pub struct AnnotationTool {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub order: Option<i32>,
    pub section: Option<String>,
}

impl AnnotationTool {
    pub fn new(id: &str, label: &str, icon: &str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: icon.into(),
            order: None,
            section: None,
        }
    }
}

/// `annotation:kinds@1` : le rendu d'un trait (défaut : au-dessus, opaque, largeur ×1).
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeKind {
    pub above: bool,
    pub opacity: f64,
    pub width_factor: f64,
}

impl Default for StrokeKind {
    fn default() -> Self {
        Self {
            above: true,
            opacity: 1.0,
            width_factor: 1.0,
        }
    }
}

/// `annotation:filters@1` : `(trait, session) -> garder ?`.
pub type AnnotationFilter = Rc<dyn Fn(&Stroke, &AnnotationSession) -> bool>;

/// `annotation:presets@1` : une instance préconfigurée (outils permis, filtre).
#[derive(Clone)]
pub struct AnnotationPreset {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub tools: Option<Vec<String>>,
    pub filter: Option<AnnotationFilter>,
}

impl AnnotationPreset {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: None,
            tools: None,
            filter: None,
        }
    }
}

/// Les registres de l'annotation, en liste ordonnée par inscription.
///
/// ★ Politique du plugin annotations (`contributions.ts`) : à clé égale, la DERNIÈRE
///   valeur inscrite l'emporte ; ici une inscription remplace la précédente de même id.
#[derive(Default)]
pub struct AnnotationRegistries {
    tools: RefCell<Vec<AnnotationTool>>,
    kinds: RefCell<Vec<(String, StrokeKind)>>,
    presets: RefCell<Vec<AnnotationPreset>>,
    filters: RefCell<Vec<(u64, AnnotationFilter)>>,
    prochain: Cell<u64>,
}

impl AnnotationRegistries {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_tool(&self, tool: AnnotationTool) {
        let mut t = self.tools.borrow_mut();
        t.retain(|x| x.id != tool.id);
        t.push(tool);
    }

    pub fn remove_tool(&self, id: &str) {
        self.tools.borrow_mut().retain(|x| x.id != id);
    }

    pub fn register_kind(&self, tool: &str, kind: StrokeKind) {
        let mut k = self.kinds.borrow_mut();
        k.retain(|(id, _)| id != tool);
        k.push((tool.to_string(), kind));
    }

    pub fn register_preset(&self, preset: AnnotationPreset) {
        let mut p = self.presets.borrow_mut();
        p.retain(|x| x.id != preset.id);
        p.push(preset);
    }

    pub fn remove_preset(&self, id: &str) {
        self.presets.borrow_mut().retain(|x| x.id != id);
    }

    /// Inscrit un filtre ; rend son jeton pour [`AnnotationRegistries::remove_filter`].
    pub fn add_filter(&self, f: impl Fn(&Stroke, &AnnotationSession) -> bool + 'static) -> u64 {
        let id = self.prochain.get();
        self.prochain.set(id + 1);
        self.filters.borrow_mut().push((id, Rc::new(f)));
        id
    }

    pub fn remove_filter(&self, id: u64) {
        self.filters.borrow_mut().retain(|(i, _)| *i != id);
    }

    fn preset(&self, id: &str) -> Option<AnnotationPreset> {
        self.presets.borrow().iter().find(|p| p.id == id).cloned()
    }
}

/// Le style de dessin courant.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingStyle {
    pub color: String,
    pub width: f64,
}

/// Une modification partielle du style.
#[derive(Clone, Debug, Default)]
pub struct StyleChange {
    pub color: Option<String>,
    pub width: Option<f64>,
}

struct State {
    tool: Option<String>,
    calque: String,
    style: DrawingStyle,
    hidden: HashSet<String>,
    past: Vec<HistoryEntry>,
    future: Vec<HistoryEntry>,
    current_file: Option<String>,
}

/// Une interaction d'annotation.
pub struct AnnotationSession {
    pub context: Rc<AnnotationContext>,
    source: Rc<AnnotationSource>,
    registres: Rc<AnnotationRegistries>,
    state: RefCell<State>,
    listeners: RefCell<Vec<(u64, Rc<dyn Fn()>)>>,
    prochain: Cell<u64>,
    source_sub: Cell<Option<u64>>,
    new_id: Box<dyn Fn() -> String>,
    /// `true` si cette session a posé le branchement de la surface (voir `bind`).
    surface_liee: Cell<bool>,
}

impl AnnotationSession {
    /// Une session. `new_id` fabrique l'id d'un trait (UUID en production).
    pub fn new(
        context: Rc<AnnotationContext>,
        source: Rc<AnnotationSource>,
        registres: Rc<AnnotationRegistries>,
        new_id: impl Fn() -> String + 'static,
    ) -> Rc<Self> {
        Rc::new(Self {
            context,
            source,
            registres,
            state: RefCell::new(State {
                tool: None,
                calque: DEFAULT_CALQUE.to_string(),
                style: DrawingStyle {
                    color: "#e93147".into(),
                    width: 4.0,
                },
                hidden: HashSet::new(),
                past: Vec::new(),
                future: Vec::new(),
                current_file: None,
            }),
            listeners: RefCell::new(Vec::new()),
            prochain: Cell::new(0),
            source_sub: Cell::new(None),
            new_id: Box::new(new_id),
            surface_liee: Cell::new(false),
        })
    }

    /// La CLÉ du document (lue à l'usage : la vue peut en changer).
    pub fn doc(&self) -> Option<String> {
        self.context.doc.borrow().clone()
    }

    pub fn active_tool(&self) -> Option<String> {
        self.state.borrow().tool.clone()
    }

    pub fn active_calque(&self) -> String {
        self.state.borrow().calque.clone()
    }

    pub fn style(&self) -> DrawingStyle {
        self.state.borrow().style.clone()
    }

    /// Abonnement à l'événement `change` ; rend le jeton pour [`AnnotationSession::off`].
    pub fn on_change(&self, cb: impl Fn() + 'static) -> u64 {
        let id = self.prochain.get();
        self.prochain.set(id + 1);
        self.listeners.borrow_mut().push((id, Rc::new(cb)));
        id
    }

    pub fn off(&self, id: u64) {
        self.listeners.borrow_mut().retain(|(i, _)| *i != id);
    }

    fn preset(&self) -> Option<AnnotationPreset> {
        let id = self.context.preset.borrow().clone()?;
        self.registres.preset(&id)
    }

    fn a_un_preset(&self) -> bool {
        self.context.preset.borrow().is_some()
    }

    /// Les outils permis, triés par `order` (tri stable).
    pub fn tools(&self) -> Vec<AnnotationTool> {
        // Un preset demandé mais absent : aucun outil.
        if self.a_un_preset() && self.preset().is_none() {
            return Vec::new();
        }
        let allowed = self.preset().and_then(|p| p.tools);
        let mut tools: Vec<AnnotationTool> = self
            .registres
            .tools
            .borrow()
            .iter()
            .filter(|t| allowed.as_ref().is_none_or(|a| a.contains(&t.id)))
            .cloned()
            .collect();
        tools.sort_by_key(|t| t.order.unwrap_or(0));
        tools
    }

    pub fn kind(&self, tool: &str) -> StrokeKind {
        self.registres
            .kinds
            .borrow()
            .iter()
            .find(|(id, _)| id == tool)
            .map(|(_, k)| k.clone())
            .unwrap_or_default()
    }

    pub fn strokes(&self) -> Vec<Stroke> {
        self.doc()
            .map(|d| self.source.strokes(&d))
            .unwrap_or_default()
    }

    /// Les calques, triés par `order` puis `id`.
    ///
    /// ★ Le TS départage par `localeCompare` ; ici l'ordre des octets (UTF-8). Les ids
    ///   sont des UUID / clés fractionnaires ASCII : même résultat.
    pub fn calques(&self) -> Vec<Calque> {
        let mut c = self
            .doc()
            .map(|d| self.source.calques(&d))
            .unwrap_or_default();
        c.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        c
    }

    pub fn visible_strokes(&self) -> Vec<Stroke> {
        let filters: Vec<AnnotationFilter> = self
            .registres
            .filters
            .borrow()
            .iter()
            .map(|(_, f)| f.clone())
            .collect();
        let preset = self.preset();
        if self.a_un_preset() && preset.is_none() {
            return Vec::new();
        }
        self.strokes()
            .into_iter()
            .filter(|s| {
                self.is_calque_visible(&s.calque)
                    && preset
                        .as_ref()
                        .and_then(|p| p.filter.as_ref())
                        .is_none_or(|f| f(s, self))
                    && filters.iter().all(|f| f(s, self))
            })
            .collect()
    }

    pub fn is_calque_visible(&self, id: &str) -> bool {
        !self.state.borrow().hidden.contains(id)
            && (id == DEFAULT_CALQUE || self.calques().iter().any(|c| c.id == id))
    }

    pub fn set_calque_visible(&self, id: &str, visible: bool) {
        {
            let mut st = self.state.borrow_mut();
            if visible {
                st.hidden.remove(id);
            } else {
                st.hidden.insert(id.to_string());
            }
        }
        self.changed();
    }

    pub fn set_active_calque(&self, id: &str) {
        let existe = id == DEFAULT_CALQUE || self.calques().iter().any(|c| c.id == id);
        self.state.borrow_mut().calque = if existe { id.into() } else { DEFAULT_CALQUE.into() };
        self.changed();
    }

    /// Arme un outil (ignoré s'il n'est pas permis).
    pub fn set_tool(&self, id: Option<&str>) {
        let permis = id.filter(|i| self.tools().iter().any(|t| t.id == *i));
        self.state.borrow_mut().tool = permis.map(str::to_string);
        self.changed();
    }

    pub fn set_style(&self, style: StyleChange) {
        if let Some(w) = style.width {
            if !w.is_finite() || w <= 0.0 {
                return;
            }
        }
        {
            let mut st = self.state.borrow_mut();
            if let Some(c) = style.color {
                st.style.color = c;
            }
            if let Some(w) = style.width {
                st.style.width = w;
            }
        }
        self.changed();
    }

    pub fn apply(&self, edits: &[AnnotationEdit]) -> Result<(), SourceError> {
        self.check_file();
        let Some(doc) = self.doc() else { return Ok(()) };
        if edits.is_empty() {
            return Ok(());
        }
        let entry = self.source.commit(&doc, edits)?;
        {
            let mut st = self.state.borrow_mut();
            st.past.push(entry);
            if st.past.len() > HISTORIQUE_MAX {
                st.past.remove(0);
            }
            st.future.clear();
        }
        self.changed();
        Ok(())
    }

    pub fn add_stroke(&self, input: StrokeInput) -> Result<Stroke, SourceError> {
        let stroke = Stroke {
            id: input.id.unwrap_or_else(|| (self.new_id)()),
            pos: input.pos,
            points: input.points,
            color: input.color,
            width: input.width,
            tool: input.tool,
            calque: input
                .calque
                .unwrap_or_else(|| self.state.borrow().calque.clone()),
        };
        self.apply(&[AnnotationEdit::Stroke {
            id: stroke.id.clone(),
            value: Some(stroke.clone()),
        }])?;
        Ok(stroke)
    }

    pub fn can_undo(&self) -> bool {
        !self.state.borrow().past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.state.borrow().future.is_empty()
    }

    pub fn undo(&self) -> bool {
        self.travel(true)
    }

    pub fn redo(&self) -> bool {
        self.travel(false)
    }

    fn travel(&self, undo: bool) -> bool {
        self.check_file();
        let entry = {
            let mut st = self.state.borrow_mut();
            if undo { st.past.pop() } else { st.future.pop() }
        };
        let (Some(entry), Some(doc)) = (entry, self.doc()) else {
            return false;
        };
        // ★ Une annulation locale n'écrase pas une modification tierce plus récente :
        //   on ne rejoue que les entités encore dans l'état que CETTE entrée avait posé.
        let expected = if undo { &entry.after } else { &entry.before };
        let cible = if undo { &entry.before } else { &entry.after };
        let changes: Vec<AnnotationEdit> = cible
            .iter()
            .filter(|e| {
                expected
                    .iter()
                    .find(|p| p.key() == e.key())
                    .is_some_and(|previous| {
                        valeur(&self.source.value(&doc, e)) == valeur(previous)
                    })
            })
            .cloned()
            .collect();
        if !changes.is_empty() {
            let _ = self.source.commit(&doc, &changes);
            let keys: HashSet<String> = changes.iter().map(AnnotationEdit::key).collect();
            let garde = |edits: &[AnnotationEdit]| -> Vec<AnnotationEdit> {
                edits
                    .iter()
                    .filter(|e| keys.contains(&e.key()))
                    .cloned()
                    .collect()
            };
            let nouvelle = HistoryEntry {
                before: garde(&entry.before),
                after: garde(&entry.after),
            };
            let mut st = self.state.borrow_mut();
            if undo {
                st.future.push(nouvelle);
            } else {
                st.past.push(nouvelle);
            }
        }
        self.changed();
        true
    }

    /// Changement de document : l'historique, les masquages et le calque actif n'ont plus de sens.
    fn check_file(&self) {
        let doc = self.doc();
        let mut st = self.state.borrow_mut();
        if st.current_file == doc {
            return;
        }
        st.current_file = doc;
        st.past.clear();
        st.future.clear();
        st.hidden.clear();
        st.calque = DEFAULT_CALQUE.to_string();
    }

    fn changed(&self) {
        self.check_file();
        let calque = self.state.borrow().calque.clone();
        if calque != DEFAULT_CALQUE && !self.calques().iter().any(|c| c.id == calque) {
            self.state.borrow_mut().calque = DEFAULT_CALQUE.to_string();
        }
        let copie: Vec<Rc<dyn Fn()>> = self.listeners.borrow().iter().map(|(_, l)| l.clone()).collect();
        for l in copie {
            l();
        }
    }

    /// `view-document-change` de la vue de cette session.
    pub fn document_changed(&self) {
        self.changed();
    }

    /// Un registre `annotation:*` a changé : désarme un outil retiré, relit tout à chaud.
    pub fn registres_changed(&self) {
        let tool = self.active_tool();
        if let Some(t) = tool {
            if !self.tools().iter().any(|x| x.id == t) {
                self.state.borrow_mut().tool = None;
            }
        }
        self.changed();
    }

    /// Le `onload` : abonne la session à la source et branche la surface.
    /// Rend `true` si la surface n'était pas encore liée (l'hôte branche alors
    /// `surface.on_change` → [`AnnotationSource::on_surface_change`]).
    pub fn load(self: &Rc<Self>) -> bool {
        self.check_file();
        let premier = self.source.bind(self.context.surface);
        self.surface_liee.set(true);
        let faible: Weak<Self> = Rc::downgrade(self);
        let id = self.source.on(move |event| {
            let Some(session) = faible.upgrade() else { return };
            match event {
                SourceEvent::Change { file } if session.doc().as_deref() == Some(*file) => {
                    session.changed();
                }
                SourceEvent::Mapped { file, map_pos } if session.doc().as_deref() == Some(*file) => {
                    let mut st = session.state.borrow_mut();
                    st.past = st.past.iter().map(|e| map_history(e, *map_pos)).collect();
                    st.future = st.future.iter().map(|e| map_history(e, *map_pos)).collect();
                }
                _ => {}
            }
        });
        self.source_sub.set(Some(id));
        premier
    }

    /// Le `onunload`. Rend `true` si c'était la dernière session sur la surface.
    pub fn unload(&self) -> bool {
        if let Some(id) = self.source_sub.take() {
            self.source.off(id);
        }
        if self.surface_liee.replace(false) {
            self.source.unbind(self.context.surface)
        } else {
            false
        }
    }
}

/// La valeur finale d'une écriture, sérialisée pour comparaison (le `JSON.stringify` du TS).
fn valeur(edit: &AnnotationEdit) -> String {
    match edit {
        AnnotationEdit::Stroke { value, .. } => format!("{value:?}"),
        AnnotationEdit::Calque { value, .. } => format!("{value:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::p;
    use fragment_text::editor::{Assoc, EditorChange, MappedPos};
    use std::cell::Cell;

    struct Banc {
        context: Rc<AnnotationContext>,
        source: Rc<AnnotationSource>,
        registres: Rc<AnnotationRegistries>,
        session: Rc<AnnotationSession>,
        compteur: Rc<Cell<u32>>,
    }

    fn fabrique(
        context: &Rc<AnnotationContext>,
        source: &Rc<AnnotationSource>,
        registres: &Rc<AnnotationRegistries>,
        compteur: &Rc<Cell<u32>>,
    ) -> Rc<AnnotationSession> {
        let c = compteur.clone();
        let s = AnnotationSession::new(
            context.clone(),
            source.clone(),
            registres.clone(),
            move || {
                c.set(c.get() + 1);
                format!("id-{}", c.get())
            },
        );
        s.load();
        s
    }

    fn nouvelle(banc: &Banc) -> Rc<AnnotationSession> {
        fabrique(&banc.context, &banc.source, &banc.registres, &banc.compteur)
    }

    fn setup() -> Banc {
        let context = Rc::new(AnnotationContext::new(Some("a.md"), None, 1));
        let source = Rc::new(AnnotationSource::new());
        let registres = Rc::new(AnnotationRegistries::new());
        let compteur = Rc::new(Cell::new(0));
        let session = fabrique(&context, &source, &registres, &compteur);
        Banc {
            context,
            source,
            registres,
            session,
            compteur,
        }
    }

    fn input() -> StrokeInput {
        StrokeInput {
            id: None,
            pos: 10,
            points: vec![p(0.0, 0.0), p(8.0, 4.0)],
            color: "#f00".into(),
            width: 2.0,
            tool: "crayon".into(),
            calque: None,
        }
    }

    fn ids(s: &[Stroke]) -> Vec<String> {
        s.iter().map(|x| x.id.clone()).collect()
    }

    fn stroke_edit(s: &Stroke, couleur: &str) -> AnnotationEdit {
        AnnotationEdit::Stroke {
            id: s.id.clone(),
            value: Some(Stroke {
                color: couleur.into(),
                ..s.clone()
            }),
        }
    }

    #[test]
    fn isole_interaction_et_undo_par_instance_partage_les_donnees() {
        let banc = setup();
        let a = banc.session.clone();
        let b = nouvelle(&banc);
        a.set_style(StyleChange {
            color: Some("#00f".into()),
            width: None,
        });
        let one = a.add_stroke(input()).expect("a");
        let two = b.add_stroke(input()).expect("b");
        assert_ne!(b.style().color, "#00f");
        a.undo();
        assert_eq!(ids(&b.strokes()), vec![two.id.clone()]);
        a.redo();
        assert!(ids(&b.strokes()).contains(&one.id));
        b.undo();
        assert_eq!(ids(&a.strokes()), vec![one.id.clone()]);
        a.unload();
        b.unload();
    }

    #[test]
    fn un_upsert_est_inversible_les_ecritures_repetees_dans_un_lot_sont_normalisees() {
        let banc = setup();
        let session = banc.session.clone();
        let stroke = session.add_stroke(input()).expect("add");
        session
            .apply(&[stroke_edit(&stroke, "blue"), stroke_edit(&stroke, "green")])
            .expect("apply");
        assert_eq!(session.strokes()[0].color, "green");
        session.undo();
        assert_eq!(session.strokes()[0].color, "#f00");
        session.redo();
        assert_eq!(session.strokes()[0].color, "green");
        session.unload();
    }

    #[test]
    fn ne_remappe_qu_une_fois_une_surface_utilisee_par_plusieurs_instances_y_compris_l_historique()
    {
        let banc = setup();
        let a = banc.session.clone();
        let b = nouvelle(&banc);
        a.add_stroke(input()).expect("add");
        // Une seule liaison de surface, deux sessions.
        assert_eq!(banc.source.surfaces_liees(), 1);
        let change = EditorChange::doc(|pos, _: Assoc| MappedPos {
            pos: pos + 5,
            deleted: false,
        });
        // L'hôte n'a branché la surface qu'une fois : un seul appel.
        banc.source.on_surface_change(Some("a.md"), &change);
        assert_eq!(b.strokes()[0].pos, 15);
        a.undo();
        a.redo();
        assert_eq!(b.strokes()[0].pos, 15);
        assert!(!a.unload());
        assert_eq!(banc.source.surfaces_liees(), 1);
        assert!(b.unload());
        assert_eq!(banc.source.surfaces_liees(), 0);
    }

    #[test]
    fn desarme_un_outil_retire_et_relit_filtres_et_presets_a_chaud() {
        let banc = setup();
        let session = banc.session.clone();
        banc.registres
            .register_tool(AnnotationTool::new("custom", "Custom", "pencil"));
        session.set_tool(Some("custom"));
        assert_eq!(session.active_tool().as_deref(), Some("custom"));
        banc.registres.remove_tool("custom");
        session.registres_changed();
        assert_eq!(session.active_tool(), None);
        session.add_stroke(input()).expect("add");
        let filtre = banc.registres.add_filter(|_, _| false);
        assert_eq!(session.visible_strokes().len(), 0);
        banc.registres.remove_filter(filtre);
        assert_eq!(session.visible_strokes().len(), 1);
        *banc.context.preset.borrow_mut() = Some("agent".into());
        assert_eq!(session.visible_strokes().len(), 0);
        banc.registres
            .register_preset(AnnotationPreset::new("agent", "Agent"));
        assert_eq!(session.visible_strokes().len(), 1);
        banc.registres.remove_preset("agent");
        assert_eq!(session.visible_strokes().len(), 0);
        session.unload();
    }

    #[test]
    fn supprime_un_calque_sans_detruire_ses_traits_restaure_par_undo_et_nettoie_au_changement_de_fichier()
     {
        let banc = setup();
        let session = banc.session.clone();
        let calque = Calque {
            id: "c".into(),
            name: "Calque".into(),
            order: "i".into(),
            color: None,
        };
        session
            .apply(&[AnnotationEdit::Calque {
                id: "c".into(),
                value: Some(calque),
            }])
            .expect("calque");
        session.set_active_calque("c");
        session.add_stroke(input()).expect("add");
        session
            .apply(&[AnnotationEdit::Calque {
                id: "c".into(),
                value: None,
            }])
            .expect("suppression");
        assert_eq!(session.strokes().len(), 1);
        assert_eq!(session.visible_strokes().len(), 0);
        assert_eq!(session.active_calque(), "default");
        session.undo();
        assert_eq!(session.visible_strokes().len(), 1);
        *banc.context.doc.borrow_mut() = Some("b.md".into());
        session.document_changed();
        assert_eq!(session.strokes().len(), 0);
        assert!(!session.can_undo());
        session.unload();
    }

    #[test]
    fn une_annulation_ne_reecrit_pas_une_version_distante_plus_recente() {
        let banc = setup();
        let session = banc.session.clone();
        let stroke = session.add_stroke(input()).expect("add");
        banc.source.hydrate(
            "a.md",
            &[Stroke {
                color: "remote".into(),
                ..stroke
            }],
            None,
        );
        session.undo();
        assert_eq!(session.strokes()[0].color, "remote");
        assert!(!session.can_redo());
        session.unload();
    }
}
