// Port de app/src/ui/views/chat/ChatSurface.ts
//! La conversation vue comme une surface de document : une `TextSurface` en LECTURE SEULE, au même
//! titre que le PDF. On y lit le texte, on y mesure des passages, on y pose des calques
//! (surlignages, fils de la marge, annotation) ; on n'y écrit pas (`as_editor` = `None`).
//!
//! - Les POSITIONS sont celles du modèle pur `fragment_chat::chat_surface_model` : un pas fixe par
//!   message et par part (`message × 1e9 + part × 1e6 + caractère`). Une ligne = une part de texte.
//! - La GÉOMÉTRIE est demandée à l'éditeur en lecture seule de chaque part (celui que le fil monte,
//!   `chat_fil::TexteLecture`) ; le repère des rectangles est le PLAN du document (`ID_PLAN` de
//!   la liste), c'est-à-dire là où les calques se posent et défilent avec les messages.
//! - [`ChatSurface`] est l'entité qui tient les unités, émet les [`EditorChange`] et observe le
//!   plan ; [`SurfaceConversation`] est l'emprunt (comme `EditeurLive::surface`) qui répond au
//!   contrat `DocumentSurface` / `TextSurface` le temps d'une lecture.
//!
//! ★ `sync` après CHAQUE mise à jour du fil : un changement de texte part en `doc_changed`
//!   d'origine `External` (le texte est REÇU, pas écrit ici) — un calque remappe ses ancres mais
//!   n'émet aucun suivi, sinon chaque vue ouverte sur la conversation le publierait.
//! ★ `reset` (une autre conversation va s'afficher) n'émet AUCUN `doc_changed` : les ancres de
//!   l'ancienne conversation n'ont rien à remapper, les calques l'apprennent par le changement de
//!   document de la vue.
//! ★ Le PLAN est observé, pas seulement la colonne : la barre des conversations qui s'ouvre ou la
//!   marge de lecture DÉPLACENT la colonne dans le plan sans en changer la taille ; les marqueurs,
//!   en coordonnées du plan, doivent alors être redessinés ([`plan_observe`]).
//! ★ Pas de DOM, donc pas de sélection native qui traverse les messages : la sélection est celle de
//!   l'éditeur de part qui a (ou a eu en dernier) le focus. Un focus posé AILLEURS (le champ, la
//!   carte d'origine, un fil imbriqué) l'annule — c'est la règle « bornée à la colonne ».

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use fragment_chat::chat_session::LiveMessage;
use fragment_chat::chat_surface_model::{
    Unit, UnitSpec, diff_units, end_of, offset_to_pos, pos_to_offset, surface_text_of, unit_at,
    units_of,
};
use fragment_text::editor::{
    Assoc, ChangeOrigin, DocumentSurface, EditorChange, EditorPosition, EditorRange, FullText,
    MappedPos, Rect, TextSurface,
};
use fragment_text::utf16::{byte_to_utf16, utf16_len, utf16_to_byte};
use gpui_kit::{
    App, Bounds, Context, Entity, EventEmitter, IntoElement, ParentElement as _, Pixels,
    Styled as _, WeakEntity, Window, canvas, div, px,
};

use super::garde::GardeChamp;
use crate::chat_fil::ListeMessages;
use crate::editeur::EditeurLive;

/// Un changement sans texte : la géométrie a pu bouger.
fn changement_de_geometrie() -> EditorChange {
    EditorChange {
        doc_changed: false,
        selection_changed: false,
        viewport_changed: true,
        map_pos: Box::new(|pos, _| MappedPos {
            pos,
            deleted: false,
        }),
        origin: None,
    }
}

/// Le texte devient lisible, ou il a été reçu : un changement de document d'origine externe.
fn changement_recu(map_pos: impl Fn(usize, Assoc) -> MappedPos + 'static) -> EditorChange {
    EditorChange {
        doc_changed: true,
        selection_changed: false,
        viewport_changed: true,
        map_pos: Box::new(map_pos),
        origin: Some(ChangeOrigin::External),
    }
}

/// Ce que la surface sait de la place des choses à l'écran, relevé à chaque image par
/// [`plan_observe`].
#[derive(Default)]
struct Geometrie {
    /// Le plan dans la fenêtre (px logiques) ; `None` tant qu'il n'a pas été peint.
    plan: Option<Bounds<Pixels>>,
    /// La signature de la dernière image : le plan et la boîte de chaque part, relative au plan.
    signature: Vec<f32>,
    /// La clé de l'unité dont l'éditeur porte la sélection de la conversation.
    selection_de: Option<String>,
}

/// La surface de document d'une conversation (une entité GPUI, émettrice d'[`EditorChange`]).
pub struct ChatSurface {
    liste: Entity<ListeMessages>,
    units: Vec<Unit>,
    /// L'éditeur de chaque unité (même ordre que `units`) ; `None` : texte rendu par un tiers
    /// (`chat:text@1`), sans géométrie connue.
    editeurs: Vec<Option<Entity<EditeurLive>>>,
    ready: bool,
    /// Le fil est masqué par la vue (historique ouvert, accueil) : rien n'est dessiné.
    masque: bool,
    geo: Rc<RefCell<Geometrie>>,
    /// Une notification de géométrie est déjà prévue dans ce cycle d'effets.
    mesure_prevue: Rc<Cell<bool>>,
    /// Le nombre de `request_update` reçus (le « poke » des calques).
    demandes: Cell<u64>,
    garde: GardeChamp,
}

impl EventEmitter<EditorChange> for ChatSurface {}

impl ChatSurface {
    /// Une surface sur la liste des messages du fil (vide, texte pas encore lisible).
    #[must_use]
    pub fn new(liste: Entity<ListeMessages>) -> Self {
        ChatSurface {
            liste,
            units: Vec::new(),
            editeurs: Vec::new(),
            ready: false,
            masque: false,
            geo: Rc::default(),
            mesure_prevue: Rc::default(),
            demandes: Cell::new(0),
            garde: GardeChamp::default(),
        }
    }

    /// La liste des messages lue par la surface.
    #[must_use]
    pub fn liste(&self) -> &Entity<ListeMessages> {
        &self.liste
    }

    /// La garde du champ : une frappe tapée dans un champ de la vue lui appartient.
    #[must_use]
    pub fn garde_champ(&self) -> GardeChamp {
        self.garde.clone()
    }

    /// Les unités courantes (une par part de texte, dans l'ordre du document).
    #[must_use]
    pub fn unites(&self) -> &[Unit] {
        &self.units
    }

    /// Le texte est-il lisible (le transcript a été chargé) ?
    #[must_use]
    pub fn prete(&self) -> bool {
        self.ready
    }

    /// Le nombre de `request_update` reçus.
    #[must_use]
    pub fn demandes_de_mise_a_jour(&self) -> u64 {
        self.demandes.get()
    }

    /// L'emprunt qui répond au contrat de surface, le temps d'une lecture.
    #[must_use]
    pub fn surface<'a>(&'a self, cx: &'a App) -> SurfaceConversation<'a> {
        SurfaceConversation { s: self, cx }
    }

    // ── synchronisation avec le fil ─────────────────────────────────────────

    /// Relit les unités du fil, après chaque `ListeMessages::mettre_a_jour`. `messages` est ce que
    /// le fil vient de recevoir : il donne le texte des parts rendus par un tiers, que la surface ne
    /// sait pas lire dans leur vue. `loaded` : le transcript est chargé.
    pub fn sync(&mut self, loaded: bool, messages: &[LiveMessage], cx: &mut Context<Self>) {
        let par_id: HashMap<&str, &LiveMessage> =
            messages.iter().map(|m| (m.id.as_str(), m)).collect();
        let entrees = self.liste.read(cx).entrees(cx);
        let mut specs: Vec<UnitSpec> = Vec::new();
        let mut editeurs: HashMap<String, Option<Entity<EditeurLive>>> = HashMap::new();
        for (msg, (id, parts)) in entrees.into_iter().enumerate() {
            for p in parts {
                if p.kind != "text" {
                    continue;
                }
                // ★ L'identité d'une unité : le message, puis l'id du part s'il en a un, sinon son
                //   rang. Par le rang seul, retirer un part AVANT (la trace vide d'un tour, à sa fin)
                //   ferait « disparaître » la réponse, et un fil ancré dedans serait détaché.
                let key = format!(
                    "{id}#{}",
                    p.id.clone().unwrap_or_else(|| p.index.to_string())
                );
                let text = match &p.editeur {
                    Some(e) => e.read(cx).texte().to_owned(),
                    None => par_id
                        .get(id.as_str())
                        .and_then(|m| m.parts.get(p.index))
                        .and_then(|part| part.text.clone())
                        .unwrap_or_default(),
                };
                editeurs.insert(key.clone(), p.editeur.clone());
                specs.push(UnitSpec {
                    msg,
                    part: p.index,
                    key,
                    text,
                });
            }
        }
        let next = units_of(&specs);
        let diff = diff_units(&self.units, &next);
        self.editeurs = next
            .iter()
            .map(|u| editeurs.get(&u.key).cloned().flatten())
            .collect();
        self.units = next;

        if !self.ready && loaded {
            // Le texte devient LISIBLE (`get_text` ne rend plus `NotYet`) : les ancres en attente
            // se résolvent, sans rien remapper.
            self.ready = true;
            cx.emit(changement_recu(|pos, _| MappedPos {
                pos,
                deleted: false,
            }));
            cx.notify();
        } else if self.ready && diff.changed {
            cx.emit(changement_recu(move |pos, assoc| diff.map_pos(pos, assoc)));
            cx.notify();
        } else {
            // Le texte n'a pas bougé : la géométrie, peut-être.
            self.planifier_mesure(cx);
        }
    }

    /// Une autre conversation va s'afficher : tout repart de zéro, SANS `doc_changed`.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.units.clear();
        self.editeurs.clear();
        self.ready = false;
        self.geo.borrow_mut().selection_de = None;
        self.notifier(cx);
    }

    /// Le fil est masqué (historique ouvert, accueil) ou de nouveau montré.
    pub fn poser_masque(&mut self, masque: bool, cx: &mut Context<Self>) {
        if self.masque != masque {
            self.masque = masque;
            self.notifier(cx);
        }
    }

    /// La géométrie a pu bouger (réagencement, défilement) : calques et widgets suivent.
    pub fn notifier(&mut self, cx: &mut Context<Self>) {
        cx.emit(changement_de_geometrie());
        cx.notify();
    }

    /// Une notification de géométrie, au plus une par cycle d'effets : un flux qui ne change que la
    /// mise en page ne fait pas relire toute leur géométrie aux calques à chaque jeton.
    fn planifier_mesure(&mut self, cx: &mut Context<Self>) {
        if self.mesure_prevue.replace(true) {
            return;
        }
        let prevue = self.mesure_prevue.clone();
        let moi = cx.entity().downgrade();
        cx.defer(move |cx| {
            prevue.set(false);
            let _ = moi.update(cx, |s, cx| s.notifier(cx));
        });
    }

    /// Relevé d'une image (appelé par [`plan_observe`] au peint) : le plan, la boîte de chaque
    /// part, l'éditeur qui porte la sélection, le champ qui a le focus. Vrai si la géométrie a
    /// changé depuis l'image précédente.
    fn relever(&self, plan: Bounds<Pixels>, window: &Window, cx: &App) -> bool {
        self.garde.actualiser(window);
        let mut geo = self.geo.borrow_mut();
        geo.plan = Some(plan);
        let mut signature = vec![
            f32::from(plan.origin.x),
            f32::from(plan.origin.y),
            f32::from(plan.size.width),
            f32::from(plan.size.height),
        ];
        let mut focalisee: Option<String> = None;
        for (u, e) in self.units.iter().zip(&self.editeurs) {
            let Some(e) = e else { continue };
            let e = e.read(cx);
            let b = e.etat().read(cx).input_bounds();
            signature.extend([
                f32::from(b.origin.x - plan.origin.x),
                f32::from(b.origin.y - plan.origin.y),
                f32::from(b.size.width),
                f32::from(b.size.height),
            ]);
            if e.a_le_focus() {
                focalisee = Some(u.key.clone());
            }
        }
        match focalisee {
            Some(k) => geo.selection_de = Some(k),
            // Le focus est AILLEURS (le champ, la carte d'origine, un fil) : la sélection de la
            // conversation tombe. Sans focus du tout (un clic sur une barre), elle reste.
            None if window.focused(cx).is_some() => geo.selection_de = None,
            None => {}
        }
        let change = signature != geo.signature;
        geo.signature = signature;
        change
    }

    // ── lectures internes ───────────────────────────────────────────────────

    fn plan(&self) -> Option<Bounds<Pixels>> {
        self.geo.borrow().plan
    }

    /// Un rectangle de la FENÊTRE → le repère du plan.
    fn vers_plan(&self, b: Bounds<Pixels>) -> Rect {
        let (ox, oy) = self.plan().map_or((0.0, 0.0), |p| {
            (f32::from(p.origin.x), f32::from(p.origin.y))
        });
        let gauche = f64::from(f32::from(b.origin.x) - ox);
        let haut = f64::from(f32::from(b.origin.y) - oy);
        Rect {
            left: gauche,
            top: haut,
            right: gauche + f64::from(f32::from(b.size.width)),
            bottom: haut + f64::from(f32::from(b.size.height)),
        }
    }
}

/// Enveloppe le PLAN du document (la liste des messages, ou ce qui la contient sans marge) pour que
/// la surface en relève la place à chaque image — l'équivalent des deux `ResizeObserver` de
/// Fragment (colonne et plan). Un enfant invisible, posé sur toute la boîte, lit ses bornes au
/// peint (APRÈS les messages : leurs éditeurs ont déjà leur boîte de l'image) ; si quelque chose a
/// bougé, la surface notifie ses abonnés au cycle suivant.
pub fn plan_observe(surface: &Entity<ChatSurface>, plan: impl IntoElement) -> impl IntoElement {
    let faible: WeakEntity<ChatSurface> = surface.downgrade();
    div().relative().w_full().min_w(px(0.0)).child(plan).child(
        canvas(
            |_, _, _| {},
            move |bounds, (), window, cx| {
                let Some(s) = faible.upgrade() else { return };
                if s.read(cx).relever(bounds, window, cx) {
                    let s = s.downgrade();
                    cx.defer(move |cx| {
                        let _ = s.update(cx, |s, cx| s.planifier_mesure(cx));
                    });
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    )
}

/// La surface d'une conversation, empruntée le temps d'une lecture (`ChatSurface::surface`).
pub struct SurfaceConversation<'a> {
    s: &'a ChatSurface,
    cx: &'a App,
}

impl SurfaceConversation<'_> {
    fn editeur(&self, i: usize) -> Option<&EditeurLive> {
        self.s.editeurs.get(i)?.as_ref().map(|e| e.read(self.cx))
    }

    /// L'octet du caractère `ch` (UTF-16) dans le texte de l'éditeur.
    fn octet(e: &EditeurLive, ch: usize) -> usize {
        let t = e.texte();
        utf16_to_byte(t, ch.min(utf16_len(t))).unwrap_or(t.len())
    }

    /// La boîte (fenêtre) d'un caractère de l'unité `i`.
    fn bornes(&self, i: usize, ch: usize) -> Option<Bounds<Pixels>> {
        let e = self.editeur(i)?;
        let o = Self::octet(e, ch);
        e.etat().read(self.cx).range_to_bounds(&(o..o))
    }

    /// La position (dans l'unité `i`) sous un point de la fenêtre, si l'unité a un éditeur.
    fn dans_unite(&self, i: usize, x: f64, y: f64) -> Option<usize> {
        let e = self.editeur(i)?;
        let u = &self.s.units[i];
        let ch = e.surface(self.cx).pos_at_coords(x, y)?;
        Some(u.base + ch.min(u.len))
    }

    /// La boîte (fenêtre) de l'éditeur de l'unité `i`.
    fn boite(&self, i: usize) -> Option<Bounds<Pixels>> {
        Some(self.editeur(i)?.etat().read(self.cx).input_bounds())
    }
}

impl DocumentSurface for SurfaceConversation<'_> {
    fn coords_at_pos(&self, offset: usize) -> Option<Rect> {
        let i = unit_at(&self.s.units, offset)?;
        let ch = offset - self.s.units[i].base;
        let mut r = self.s.vers_plan(self.bornes(i, ch)?);
        r.right = r.left;
        Some(r)
    }

    fn pos_at_coords(&self, x: f64, y: f64) -> Option<usize> {
        let units = &self.s.units;
        // Dans la bande verticale d'une part : la position exacte, demandée à son éditeur.
        #[allow(clippy::cast_possible_truncation)]
        let (fx, fy) = (px(x as f32), px(y as f32));
        for i in 0..units.len() {
            if let Some(b) = self.boite(i)
                && b.origin.y <= fy
                && fy <= b.bottom()
                && let Some(p) = self.dans_unite(i, x, y)
            {
                return Some(p);
            }
        }
        // Ailleurs dans le plan (avatar, appel d'outil, entre deux messages) : le début de la part
        // qui SUIT le point ; après la dernière, la fin du document.
        if let Some(plan) = self.s.plan()
            && plan.contains(&gpui_kit::point(fx, fy))
        {
            let suivante = (0..units.len())
                .find(|&i| self.boite(i).is_some_and(|b| b.origin.y > fy))
                .map(|i| units[i].base);
            return Some(suivante.unwrap_or_else(|| end_of(units)));
        }
        // Hors du plan : la part la plus proche verticalement, à son début.
        let mut meilleure: Option<(f32, usize)> = None;
        for (i, u) in units.iter().enumerate() {
            let Some(b) = self.boite(i) else { continue };
            let d = if fy < b.origin.y {
                f32::from(b.origin.y - fy)
            } else if fy > b.bottom() {
                f32::from(fy - b.bottom())
            } else {
                0.0
            };
            if meilleure.is_none_or(|(m, _)| d < m) {
                meilleure = Some((d, u.base));
            }
        }
        meilleure.map(|(_, base)| base)
    }

    fn viewport_range(&self) -> EditorRange {
        EditorRange {
            from: 0,
            to: end_of(&self.s.units),
        }
    }

    fn rendered_ranges(&self) -> Vec<EditorRange> {
        // Le fil masqué, ou un plan sans boîte (un ancêtre replié) : rien n'est dessiné.
        let sans_boite = self
            .s
            .plan()
            .is_none_or(|p| p.size.width <= px(0.0) && p.size.height <= px(0.0));
        if self.s.masque || sans_boite {
            return Vec::new();
        }
        self.s
            .units
            .iter()
            .map(|u| EditorRange {
                from: u.base,
                to: u.base + u.len,
            })
            .collect()
    }

    fn request_update(&self) {
        self.s.demandes.set(self.s.demandes.get() + 1);
    }

    fn as_text(&self) -> Option<&dyn TextSurface> {
        Some(self)
    }
}

impl TextSurface for SurfaceConversation<'_> {
    fn get_line(&self, n: usize) -> String {
        self.s
            .units
            .get(n)
            .map(|u| u.text.clone())
            .unwrap_or_default()
    }

    fn line_count(&self) -> usize {
        self.s.units.len()
    }

    fn offset_to_pos(&self, offset: usize) -> EditorPosition {
        offset_to_pos(&self.s.units, offset)
    }

    fn pos_to_offset(&self, pos: EditorPosition) -> usize {
        pos_to_offset(&self.s.units, pos)
    }

    fn get_selection(&self) -> EditorRange {
        let geo = self.s.geo.borrow();
        let Some(cle) = geo.selection_de.as_deref() else {
            return EditorRange::default();
        };
        let Some(i) = self.s.units.iter().position(|u| u.key == cle) else {
            return EditorRange::default();
        };
        let Some(e) = self.editeur(i) else {
            return EditorRange::default();
        };
        let u = &self.s.units[i];
        let r = e.etat().read(self.cx).selected_range();
        let t = e.texte();
        let (a, b) = (
            byte_to_utf16(t, r.start.min(r.end)).min(u.len),
            byte_to_utf16(t, r.start.max(r.end)).min(u.len),
        );
        EditorRange {
            from: u.base + a,
            to: u.base + b,
        }
    }

    fn coords_for_range(&self, from: usize, to: usize) -> Vec<Rect> {
        let (debut, fin) = (from.min(to), from.max(to));
        let mut rects = Vec::new();
        for (i, u) in self.s.units.iter().enumerate() {
            let a = debut.max(u.base) - u.base;
            let b = fin.min(u.base + u.len).saturating_sub(u.base);
            if b <= a || debut > u.base + u.len {
                continue;
            }
            let Some(e) = self.editeur(i) else { continue };
            let s = e.surface(self.cx);
            // Les rectangles de l'éditeur sont dans SON repère de document : on les ramène à la
            // fenêtre par un caractère dont on connaît les deux (sa boîte et ses coordonnées).
            let (Some(fenetre), Some(doc)) = (self.bornes(i, a), s.coords_at_pos(a)) else {
                continue;
            };
            let dx = f64::from(f32::from(fenetre.origin.x)) - doc.left;
            let dy = f64::from(f32::from(fenetre.origin.y)) - doc.top;
            let plan = self.s.vers_plan(Bounds::default());
            for r in s.coords_for_range(a, b) {
                if r.right - r.left <= 0.0 || r.bottom - r.top <= 0.0 {
                    continue;
                }
                rects.push(Rect {
                    left: r.left + dx + plan.left,
                    top: r.top + dy + plan.top,
                    right: r.right + dx + plan.left,
                    bottom: r.bottom + dy + plan.top,
                });
            }
        }
        rects
    }

    fn get_text(&self) -> FullText {
        if !self.s.ready {
            return FullText::NotYet;
        }
        FullText::Text(surface_text_of(&self.s.units))
    }
}
