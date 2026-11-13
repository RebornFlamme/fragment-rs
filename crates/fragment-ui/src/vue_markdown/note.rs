// Port de app/src/ui/views/markdown/MarkdownView.ts (le document de la vue : texte, sélection,
// lecture seule, historique d'annulation, la façade `Editor` que reçoivent les commandes).
//! Le document d'une vue Markdown, sans GPUI : la vérité que la vue lit et écrit (sauvegarde,
//! relecture du disque, commandes `editor:*`, collage), et que le montage recopie dans l'éditeur
//! du live preview à chaque révision.
//!
//! ★ Le document est PARTAGÉ (`Rc<RefCell<Note>>`) : la couche de rendu ne reçoit qu'un
//!   `&dyn View` immuable, et c'est pourtant elle qui voit les frappes. Les deux côtés écrivent
//!   donc ici, chacun avec sa porte : [`Note::poser_texte`] (la vue : chargement, disque,
//!   commande) monte la révision que le montage doit recopier ; [`Note::frappe`] (le montage :
//!   l'éditeur a changé) ne la monte pas, l'éditeur a déjà ce texte.
//! ★ Positions en unités de code UTF-16 (ADR-004), comme le contrat `Editor`.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_core::workspace::vue_fichier::SAVE_DELAY_MS;
use fragment_text::{
    DocumentSurface, Editor, EditorPosition, EditorRange, EditorTransactionSpec, Rect, TextSurface,
    utf16_len, utf16_to_byte,
};

/// Le document partagé entre la vue (le modèle) et son montage (le rendu).
pub type NotePartagee = Rc<RefCell<Note>>;

/// Une entrée d'historique : le texte et la sélection d'avant une transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Instantane {
    texte: String,
    selection: (usize, usize),
}

/// Le document de la vue.
#[derive(Debug, Default)]
pub struct Note {
    texte: String,
    /// `(anchor, head)` en UTF-16.
    selection: (usize, usize),
    lecture_seule: bool,
    focus: bool,
    /// ★ L'état neuf par document : rien n'est révélé tant que l'utilisateur n'a pas mis son
    ///   curseur dans la note. Remis à faux à chaque chargement neuf.
    a_eu_le_focus: bool,
    /// Montée à chaque texte posé par la vue : le montage recopie quand elle change.
    revision: u64,
    /// La dernière pose était un chargement neuf (l'éditeur repart sans historique).
    neuf: bool,
    annuler: Vec<Instantane>,
    retablir: Vec<Instantane>,
    /// L'échéance de la sauvegarde différée (horloge de l'hôte, ms).
    echeance: Option<u64>,
}

impl Note {
    /// Un document vide, partagé.
    pub fn partagee() -> NotePartagee {
        Rc::new(RefCell::new(Self::default()))
    }

    pub fn texte(&self) -> &str {
        &self.texte
    }

    /// La sélection `(anchor, head)` en UTF-16.
    pub fn selection(&self) -> (usize, usize) {
        self.selection
    }

    pub fn lecture_seule(&self) -> bool {
        self.lecture_seule
    }

    pub fn a_le_focus(&self) -> bool {
        self.focus
    }

    /// L'utilisateur a-t-il déjà mis son curseur dans CE document ?
    pub fn a_eu_le_focus(&self) -> bool {
        self.a_eu_le_focus
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// La dernière pose était-elle un chargement neuf ?
    pub fn neuf(&self) -> bool {
        self.neuf
    }

    /// Le nombre d'entrées d'annulation (headless : l'historique du document).
    pub fn entrees_annulables(&self) -> usize {
        self.annuler.len()
    }

    /// Pose le texte depuis la vue. `neuf` : chargement d'un fichier (historique vidé, curseur au
    /// début, état neuf) ; sinon relecture du disque, le curseur suit le texte.
    pub fn poser_texte(&mut self, texte: &str, neuf: bool) {
        if neuf {
            self.annuler.clear();
            self.retablir.clear();
            self.selection = (0, 0);
            self.a_eu_le_focus = false;
            self.focus = false;
            self.echeance = None;
        } else {
            let (a, h) = self.selection;
            self.selection = (
                super::relecture::position_apres(&self.texte, texte, a),
                super::relecture::position_apres(&self.texte, texte, h),
            );
        }
        texte.clone_into(&mut self.texte);
        self.neuf = neuf;
        self.revision += 1;
    }

    /// Vide le document (le fichier quitte la vue).
    pub fn vider(&mut self) {
        self.poser_texte("", true);
    }

    /// Le montage rapporte une frappe : le texte et la sélection de l'éditeur. Rend vrai si le
    /// texte a changé (une sauvegarde est alors à demander).
    pub fn frappe(&mut self, texte: &str, selection: (usize, usize)) -> bool {
        self.selection = selection;
        if self.texte == texte {
            return false;
        }
        texte.clone_into(&mut self.texte);
        true
    }

    /// Le montage rapporte le focus. Le premier focus du document lève l'état neuf.
    pub fn poser_focus(&mut self, focus: bool) {
        self.focus = focus;
        if focus {
            self.a_eu_le_focus = true;
        }
    }

    pub fn poser_lecture_seule(&mut self, valeur: bool) {
        self.lecture_seule = valeur;
    }

    /// `requestSave` : la sauvegarde aura lieu `SAVE_DELAY_MS` après la DERNIÈRE demande.
    pub fn demander_sauvegarde(&mut self, maintenant_ms: u64) {
        self.echeance = Some(maintenant_ms + SAVE_DELAY_MS);
    }

    pub fn echeance(&self) -> Option<u64> {
        self.echeance
    }

    /// L'échéance est-elle passée ? Si oui, elle est consommée : l'appelant écrit.
    pub fn sauvegarde_due(&mut self, maintenant_ms: u64) -> bool {
        match self.echeance {
            Some(e) if maintenant_ms >= e => {
                self.echeance = None;
                true
            }
            _ => false,
        }
    }

    /// Une sauvegarde faite annule l'échéance en attente.
    pub fn sauvegarde_faite(&mut self) {
        self.echeance = None;
    }

    /// Applique une transaction (positions de l'AVANT, sélection de l'APRÈS). Refusée en lecture
    /// seule. Rend vrai si le texte a changé.
    pub fn appliquer(&mut self, spec: &EditorTransactionSpec) -> bool {
        if self.lecture_seule {
            return false;
        }
        let change = !spec.changes.is_empty();
        if change {
            self.annuler.push(self.instantane());
            self.retablir.clear();
            let mut utf16: Vec<u16> = self.texte.encode_utf16().collect();
            let mut changes = spec.changes.clone();
            // Toutes exprimées dans l'avant : de la fin vers le début.
            changes.sort_by_key(|c| std::cmp::Reverse(c.from));
            for c in &changes {
                let fin = c.to.unwrap_or(c.from).min(utf16.len());
                let debut = c.from.min(fin);
                utf16.splice(debut..fin, c.insert.encode_utf16());
            }
            self.texte = String::from_utf16_lossy(&utf16);
            self.revision += 1;
            self.neuf = false;
        }
        let n = utf16_len(&self.texte);
        if let Some(s) = &spec.selection {
            self.selection = (s.anchor.min(n), s.head.unwrap_or(s.anchor).min(n));
            if !change {
                self.revision += 1;
                self.neuf = false;
            }
        }
        change
    }

    /// Ctrl+Z sur le document (headless ; monté, l'éditeur a son propre historique).
    pub fn annuler(&mut self) -> bool {
        self.echanger(true)
    }

    /// Ctrl+Y.
    pub fn retablir(&mut self) -> bool {
        self.echanger(false)
    }

    fn echanger(&mut self, arriere: bool) -> bool {
        if self.lecture_seule {
            return false;
        }
        let pris = if arriere {
            self.annuler.pop()
        } else {
            self.retablir.pop()
        };
        let Some(i) = pris else { return false };
        let courant = self.instantane();
        if arriere {
            self.retablir.push(courant);
        } else {
            self.annuler.push(courant);
        }
        self.texte = i.texte;
        self.selection = i.selection;
        self.revision += 1;
        self.neuf = false;
        true
    }

    fn instantane(&self) -> Instantane {
        Instantane {
            texte: self.texte.clone(),
            selection: self.selection,
        }
    }
}

/// La façade `Editor` du document : ce que reçoivent les commandes `editor:*`, le menu
/// contextuel et le collage. Chaque transaction appliquée est signalée par `sur_changement`
/// (la vue y demande la sauvegarde).
#[derive(Clone)]
pub struct EditeurNote {
    note: NotePartagee,
    sur_changement: Rc<dyn Fn()>,
}

impl EditeurNote {
    pub fn new(note: NotePartagee, sur_changement: Rc<dyn Fn()>) -> Self {
        Self {
            note,
            sur_changement,
        }
    }

    pub fn note(&self) -> &NotePartagee {
        &self.note
    }

    fn lignes(&self) -> Vec<String> {
        self.note
            .borrow()
            .texte
            .split('\n')
            .map(str::to_owned)
            .collect()
    }
}

impl DocumentSurface for EditeurNote {
    fn coords_at_pos(&self, _: usize) -> Option<Rect> {
        None
    }
    fn pos_at_coords(&self, _: f64, _: f64) -> Option<usize> {
        None
    }
    fn viewport_range(&self) -> EditorRange {
        EditorRange {
            from: 0,
            to: utf16_len(&self.note.borrow().texte),
        }
    }
    fn rendered_ranges(&self) -> Vec<EditorRange> {
        vec![self.viewport_range()]
    }
    fn request_update(&self) {}
    fn as_text(&self) -> Option<&dyn TextSurface> {
        Some(self)
    }
}

impl TextSurface for EditeurNote {
    fn get_line(&self, n: usize) -> String {
        self.lignes().into_iter().nth(n).unwrap_or_default()
    }
    fn line_count(&self) -> usize {
        self.lignes().len()
    }
    fn offset_to_pos(&self, offset: usize) -> EditorPosition {
        let mut reste = offset;
        let lignes = self.lignes();
        let derniere = lignes.len().saturating_sub(1);
        for (i, l) in lignes.iter().enumerate() {
            let n = utf16_len(l);
            if reste <= n || i == derniere {
                return EditorPosition {
                    line: i,
                    ch: reste.min(n),
                };
            }
            reste -= n + 1;
        }
        EditorPosition::default()
    }
    fn pos_to_offset(&self, pos: EditorPosition) -> usize {
        let lignes = self.lignes();
        let mut offset = 0;
        for l in lignes.iter().take(pos.line) {
            offset += utf16_len(l) + 1;
        }
        let n = lignes.get(pos.line).map_or(0, |l| utf16_len(l));
        offset + pos.ch.min(n)
    }
    fn get_selection(&self) -> EditorRange {
        let (a, h) = self.note.borrow().selection;
        EditorRange {
            from: a.min(h),
            to: a.max(h),
        }
    }
    fn coords_for_range(&self, _: usize, _: usize) -> Vec<Rect> {
        Vec::new()
    }
    fn as_editor(&self) -> Option<&dyn Editor> {
        Some(self)
    }
}

impl Editor for EditeurNote {
    fn get_value(&self) -> String {
        self.note.borrow().texte.clone()
    }
    fn get_range(&self, from: usize, to: usize) -> String {
        let n = self.note.borrow();
        let de = utf16_to_byte(&n.texte, from).unwrap_or(n.texte.len());
        let a = utf16_to_byte(&n.texte, to).unwrap_or(n.texte.len());
        n.texte.get(de.min(a)..a).unwrap_or_default().to_owned()
    }
    fn transaction(&self, spec: EditorTransactionSpec) {
        let change = self.note.borrow_mut().appliquer(&spec);
        if change {
            (self.sur_changement)();
        }
    }
    fn focus(&self) {
        self.note.borrow_mut().poser_focus(true);
    }
    fn has_focus(&self) -> bool {
        self.note.borrow().focus
    }
}
