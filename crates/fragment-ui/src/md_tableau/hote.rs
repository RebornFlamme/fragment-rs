// Port du branchement du tableau dans l'éditeur hôte d'app/src/ui/views/markdown/table/tableField.ts
// (tableField, tableCursorBridge, tableKeymap, clicSousTableauFinal, saisieSousTableau) et de
// `sortirApresTableau` (TableWidget.ts).
//! L'hôte des tableaux : il ENVELOPPE un [`HoteBlocs`] et pose un [`Tableau`] sur chaque bloc
//! tableau de la note.
//!
//! - la hauteur : le `Mesureur` de l'hôte des blocs rend celle que chaque tableau a mesurée (à
//!   défaut, une ligne de texte par rangée) ; la rangée du tableau la prend, ses autres lignes sont
//!   nulles (bloc « à la place », jamais révélé : on édite dans les cellules) ;
//! - la place : sur la ligne du tableau, dans la boîte des blocs (`debut_texte` − marge de ligne) ;
//! - le pont curseur : une sélection de l'hôte posée DANS un tableau (insert-table, undo…) devient
//!   le focus de la cellule sous elle ; le tableau ENTIER sélectionné prend un contour ;
//! - le clavier de l'hôte : ↑ / ↓ depuis la ligne voisine entrent dans le tableau ; Retour arrière au
//!   début de la ligne qui le suit (Suppr à la fin de celle qui le précède) le SÉLECTIONNE, l'appui
//!   suivant le supprime ;
//! - un tableau qui TERMINE la note : un clic dessous crée la ligne qui manque ; taper sur la ligne
//!   vide collée sous un tableau part sur la ligne d'après (`separer_du_tableau`, la règle GFM).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use fragment_markdown::blocs::{BlocCalcule, ContenuBloc};
use fragment_markdown::commandes::insertion::separer_du_tableau;
use fragment_maths::moteur::Moteur;
use fragment_text::editor::EditorChange;
use fragment_text::utf16::{byte_to_utf16, utf16_len, utf16_to_byte};
use gpui_kit::base::input::{Backspace, Delete, Escape, MoveDown, MoveUp};
use gpui_kit::{
    AppContext as _, Context, Entity, EntityInputHandler as _, Focusable as _,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    ParentElement as _, Render, Styled as _, Subscription, Window, canvas, div, px,
};

use super::geometrie::Reglages;
use super::widget::{EvenementTableau, OptionsTableau, Tableau};
use crate::editeur::constantes::{MARGE_LIGNE_DROITE, MARGE_LIGNE_GAUCHE, debut_texte, fin_texte};
use crate::editeur::{EditeurLive, OptionsEditeur};
use crate::md_blocs::{HoteBlocs, Mesureur};
use crate::theme_global::theme_ou_defaut;

const LF: u8 = b'\n';

/// L'hôte des tableaux d'une note (une entité GPUI).
pub struct HoteTableaux {
    blocs: Entity<HoteBlocs>,
    editeur: Entity<EditeurLive>,
    tableaux: Vec<Entity<Tableau>>,
    /// La hauteur mesurée de chaque tableau, par sa première ligne.
    hauteurs: Rc<RefCell<HashMap<usize, f32>>>,
    options: OptionsTableau,
    /// Levé par un tableau qui écrit dans l'hôte : le pont curseur ignore le changement suivant.
    ecriture: Rc<Cell<bool>>,
    _abonnements: Vec<Subscription>,
}

impl HoteTableaux {
    /// Un éditeur de note sur `texte`, avec ses blocs et ses tableaux. `autres` mesure les blocs qui
    /// ne sont pas des tableaux (Properties, HTML, embeds…).
    pub fn new(
        texte: &str,
        options_editeur: OptionsEditeur,
        options: OptionsTableau,
        autres: Option<Mesureur>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let hauteurs: Rc<RefCell<HashMap<usize, f32>>> = Rc::default();
        let reglages = Reglages::du_theme(&theme_ou_defaut(cx), options.taille_texte);
        let h = hauteurs.clone();
        let mesureur: Mesureur = Rc::new(move |b: &BlocCalcule, taille| {
            if let ContenuBloc::Tableau { source } = &b.contenu {
                return Some(
                    h.borrow()
                        .get(&b.premiere_ligne)
                        .copied()
                        .unwrap_or_else(|| Tableau::hauteur_estimee(source, &reglages)),
                );
            }
            autres.as_ref().and_then(|f| f(b, taille))
        });
        let options_editeur = OptionsEditeur {
            lecture_seule: options.lecture_seule,
            taille_texte: options.taille_texte,
            ..options_editeur
        };
        let blocs = cx.new(|cx| HoteBlocs::new(texte, options_editeur, Some(mesureur), window, cx));
        let editeur = blocs.read(cx).editeur().clone();
        let abonnements = vec![cx.subscribe_in(&editeur, window, Self::sur_changement)];
        let mut h = HoteTableaux {
            blocs,
            editeur,
            tableaux: Vec::new(),
            hauteurs,
            options,
            ecriture: Rc::new(Cell::new(false)),
            _abonnements: abonnements,
        };
        h.synchroniser(window, cx);
        h
    }

    /// Pose le moteur de maths partagé : maths en bloc de la note, maths en ligne des cellules.
    pub fn poser_moteur_maths(
        &mut self,
        moteur: Rc<Moteur>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.blocs
            .update(cx, |b, cx| b.poser_moteur_maths(moteur.clone(), cx));
        self.options.moteur = Some(moteur);
        // Les cellules existantes n'ont pas le calque : on reconstruit les tableaux.
        self.tableaux.clear();
        self.synchroniser(window, cx);
    }

    #[must_use]
    pub fn blocs(&self) -> &Entity<HoteBlocs> {
        &self.blocs
    }

    #[must_use]
    pub fn editeur(&self) -> &Entity<EditeurLive> {
        &self.editeur
    }

    /// Les tableaux de la note, dans l'ordre du document.
    #[must_use]
    pub fn tableaux(&self) -> &[Entity<Tableau>] {
        &self.tableaux
    }

    /// Les blocs tableau de la dernière projection.
    fn blocs_tableau(&self, cx: &Context<Self>) -> Vec<BlocCalcule> {
        self.blocs
            .read(cx)
            .blocs()
            .into_iter()
            .filter(|b| matches!(b.contenu, ContenuBloc::Tableau { .. }))
            .collect()
    }

    /// Un tableau par bloc tableau : ceux qui existent sont mis à jour (en place si possible), les
    /// nouveaux créés, ceux en trop retirés.
    fn synchroniser(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let blocs = self.blocs_tableau(cx);
        self.tableaux.truncate(blocs.len());
        for (i, b) in blocs.iter().enumerate() {
            let ContenuBloc::Tableau { source } = &b.contenu else {
                continue;
            };
            if let Some(t) = self.tableaux.get(i) {
                t.update(cx, |t, cx| {
                    t.mettre_a_jour(source, b.source, b.premiere_ligne, window, cx);
                });
                continue;
            }
            let (hote, options, ecriture) = (
                self.editeur.clone(),
                self.options.clone(),
                self.ecriture.clone(),
            );
            let (source, plage, ligne) = (source.clone(), b.source, b.premiere_ligne);
            let t = cx
                .new(|cx| Tableau::new(hote, &source, plage, ligne, options, ecriture, window, cx));
            self._abonnements
                .push(cx.subscribe(&t, |h, t, ev: &EvenementTableau, cx| {
                    let EvenementTableau::Hauteur(v) = *ev;
                    let ligne = t.read(cx).premiere_ligne();
                    let avant = h.hauteurs.borrow_mut().insert(ligne, v);
                    if avant.is_none_or(|a| (a - v).abs() > 0.01) {
                        h.editeur.update(cx, |e, cx| e.reprojeter_maintenant(cx));
                        cx.notify();
                    }
                }));
            self.tableaux.push(t);
        }
    }

    fn sur_changement(
        &mut self,
        _e: &Entity<EditeurLive>,
        ch: &EditorChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if ch.doc_changed {
            self.synchroniser(window, cx);
        }
        let de_nous = self.ecriture.replace(false);
        self.pont(de_nous, window, cx);
        cx.notify();
    }

    /// La sélection de l'hôte (ancre, tête) en UTF-16, et le texte.
    fn selection(&self, cx: &Context<Self>) -> (String, usize, usize) {
        let s = self.editeur.read(cx).etat().read(cx);
        let texte = s.value().to_string();
        let r = s.selected_range();
        let (a, h) = if s.is_selection_reversed() {
            (r.end, r.start)
        } else {
            (r.start, r.end)
        };
        let (a, h) = (byte_to_utf16(&texte, a), byte_to_utf16(&texte, h));
        (texte, a, h)
    }

    /// Le pont curseur : contour du tableau entier sélectionné ; une sélection posée DANS un tableau
    /// (et pas par lui) donne le focus à la cellule sous elle.
    fn pont(&mut self, de_nous: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (_, a, h) = self.selection(cx);
        let (de, a_) = (a.min(h), a.max(h));
        for t in self.tableaux.clone() {
            let p = t.read(cx).plage().utf16;
            let entier = de <= p.from && a_ >= p.to && de != a_;
            t.update(cx, |t, cx| t.poser_selectionne(entier, cx));
            if de_nous || entier || h < p.from || h > p.to {
                continue;
            }
            t.update(cx, |t, cx| {
                t.focaliser_a_l_offset(a.saturating_sub(p.from), h - p.from, window, cx);
            });
        }
    }

    /// L'éditeur hôte a-t-il le focus clavier ?
    fn hote_focalise(&self, window: &Window, cx: &Context<Self>) -> bool {
        self.editeur
            .read(cx)
            .etat()
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
    }

    /// Le tableau dont la colonne / ligne est choisie, s'il y en a un.
    fn tableau_choisi(&self, cx: &Context<Self>) -> Option<Entity<Tableau>> {
        self.tableaux
            .iter()
            .find(|t| t.read(cx).choix().is_some())
            .cloned()
    }

    /// Suppr / Retour arrière : la colonne / ligne choisie, sinon (curseur de l'hôte au bord d'une
    /// ligne voisine d'un tableau) la sélection du tableau entier.
    fn effacer(&mut self, avant: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(t) = self.tableau_choisi(cx) {
            return t.update(cx, |t, cx| t.supprimer_choix(window, cx));
        }
        if !self.hote_focalise(window, cx) {
            return false;
        }
        let (texte, a, h) = self.selection(cx);
        if a != h {
            return false;
        }
        let u: Vec<u16> = texte.encode_utf16().collect();
        let debut = u[..h]
            .iter()
            .rposition(|&c| c == u16::from(b'\n'))
            .map_or(0, |i| i + 1);
        let fin = u[h..]
            .iter()
            .position(|&c| c == u16::from(b'\n'))
            .map_or(u.len(), |i| h + i);
        let voisin = if avant {
            (h == debut && debut > 0).then(|| debut - 1)
        } else {
            (h == fin && fin < u.len()).then(|| fin + 1)
        };
        let Some(v) = voisin else { return false };
        let Some(t) = self.tableaux.iter().find(|t| {
            let p = t.read(cx).plage().utf16;
            if avant { p.to == v } else { p.from == v }
        }) else {
            return false;
        };
        let p = t.read(cx).plage().octets;
        let etat = self.editeur.read(cx).etat().clone();
        etat.update(cx, |s, cx| s.set_selected_range(p.from..p.to, cx));
        true
    }

    /// ↑ / ↓ depuis la ligne VISUELLE qui touche un tableau : on entre dans sa première / dernière
    /// ligne.
    fn entrer(&mut self, bas: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.hote_focalise(window, cx) {
            return false;
        }
        let (texte, a, h) = self.selection(cx);
        if a != h {
            return false;
        }
        let u: Vec<u16> = texte.encode_utf16().collect();
        let debut = u[..h]
            .iter()
            .rposition(|&c| c == u16::from(b'\n'))
            .map_or(0, |i| i + 1);
        let fin = u[h..]
            .iter()
            .position(|&c| c == u16::from(b'\n'))
            .map_or(u.len(), |i| h + i);
        let Some(voisine) = (if bas {
            Some(fin + 1).filter(|&v| v <= u.len())
        } else {
            debut.checked_sub(1)
        }) else {
            return false;
        };
        let Some(t) = self
            .tableaux
            .iter()
            .find(|t| {
                let p = t.read(cx).plage().utf16;
                if bas {
                    p.from == voisine
                } else {
                    p.to == voisine
                }
            })
            .cloned()
        else {
            return false;
        };
        // Seulement depuis la rangée visuelle qui borde le tableau (la ligne peut se replier).
        let s = self.editeur.read(cx).etat().read(cx);
        let o = |x: usize| utf16_to_byte(&texte, x).unwrap_or(texte.len());
        let ici = s.range_to_bounds(&(o(h)..o(h)));
        let bord = s
            .range_to_bounds(&(o(if bas { fin } else { debut })..o(if bas { fin } else { debut })));
        if let (Some(i), Some(b)) = (ici, bord)
            && (f32::from(i.origin.y) - f32::from(b.origin.y)).abs() > 2.0
        {
            return false;
        }
        t.update(cx, |t, cx| {
            if bas {
                t.focaliser(0, 0, Some(0), None, window, cx);
            } else {
                t.focaliser_derniere_ligne(window, cx);
            }
        });
        true
    }

    /// La frappe dans l'hôte : sur une ligne vide collée sous un tableau, le texte part sur la ligne
    /// d'après (sinon GFM en ferait une rangée).
    fn frappe(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = &ev.keystroke.modifiers;
        if m.control || m.alt || m.platform || m.function {
            return false;
        }
        let Some(car) = ev.keystroke.key_char.clone().filter(|c| !c.is_empty()) else {
            return false;
        };
        if !self.hote_focalise(window, cx) {
            return false;
        }
        // Le cas rare d'abord, sans copier le texte : un curseur seul sur une ligne VIDE.
        {
            let s = self.editeur.read(cx).etat().read(cx);
            let r = s.selected_range();
            let t = self.editeur.read(cx).texte().as_bytes();
            let vide_avant = r.start == 0 || t.get(r.start - 1) == Some(&LF);
            let vide_apres = r.start == t.len() || t.get(r.start) == Some(&LF);
            if r.start != r.end || r.start == 0 || !vide_avant || !vide_apres {
                return false;
            }
        }
        let (texte, a, h) = self.selection(cx);
        if a != h {
            return false;
        }
        let insere = separer_du_tableau(&texte, h, &car);
        if insere == car {
            return false;
        }
        let etat = self.editeur.read(cx).etat().clone();
        etat.update(cx, |s, cx| {
            s.replace_text_in_range(Some(h..h), &insere, window, cx)
        });
        true
    }

    /// Un clic SOUS un tableau qui termine la note : la ligne qui manque est créée, le curseur y va.
    fn clic_sous_le_dernier(
        &mut self,
        ev: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if ev.button != MouseButton::Left || self.options.lecture_seule {
            return false;
        }
        let texte = self.editeur.read(cx).texte().to_owned();
        let long = utf16_len(&texte);
        let Some(t) = self.tableaux.last() else {
            return false;
        };
        let (plage, bloc) = {
            let t = t.read(cx);
            (t.plage(), t.geometrie().bloc)
        };
        let g = self.blocs.read(cx).geometrie();
        let y = ev.position.y;
        let dans_l_editeur = f32::from(ev.position.x) >= g.gauche
            && f32::from(ev.position.x) < g.gauche + g.largeur
            && f32::from(y) < g.haut + g.hauteur;
        if plage.utf16.to != long || !dans_l_editeur || y <= bloc.origin.y + bloc.size.height {
            return false;
        }
        let etat = self.editeur.read(cx).etat().clone();
        self.ecriture.set(true);
        etat.update(cx, |s, cx| {
            s.focus(window, cx);
            s.replace_text_in_range(Some(long..long), "\n", window, cx);
            let n = s.text().len();
            s.set_selected_range(n..n, cx);
        });
        true
    }
}

impl Render for HoteTableaux {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = self.blocs.read(cx).geometrie();
        let gauche = debut_texte(g.largeur) - MARGE_LIGNE_GAUCHE;
        let largeur = g.largeur - gauche - fin_texte(g.largeur) + MARGE_LIGNE_DROITE;
        let mut couche = div().absolute().top_0().left_0().size_full();
        for t in &self.tableaux {
            let ligne = t.read(cx).premiere_ligne();
            t.update(cx, |t, cx| t.poser_largeur(largeur.max(0.0), cx));
            let Some(le) = g.ligne(ligne) else { continue };
            if le.hauteur <= 0.0 {
                continue;
            }
            couche = couche.child(
                div()
                    .absolute()
                    .left(px(gauche))
                    .top(px(le.haut - g.haut))
                    .w(px(largeur.max(0.0)))
                    .h(px(le.hauteur))
                    .child(t.clone()),
            );
        }
        let entite = cx.entity();
        div()
            .relative()
            .size_full()
            .capture_action(cx.listener(|h, _: &Backspace, w, cx| {
                if h.effacer(true, w, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|h, _: &Delete, w, cx| {
                if h.effacer(false, w, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|h, _: &MoveDown, w, cx| {
                if h.entrer(true, w, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|h, _: &MoveUp, w, cx| {
                if h.entrer(false, w, cx) {
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|h, _: &Escape, _, cx| {
                if let Some(t) = h.tableau_choisi(cx) {
                    t.update(cx, |t, cx| t.effacer_choix(cx));
                    cx.stop_propagation();
                }
            }))
            .capture_key_down(cx.listener(|h, ev: &KeyDownEvent, w, cx| {
                // Sans éditeur focalisé (choix d'une poignée), les touches n'ont pas d'action.
                let touche = ev.keystroke.key.as_str();
                let pris = match touche {
                    "delete" | "backspace" => h
                        .tableau_choisi(cx)
                        .is_some_and(|t| t.update(cx, |t, cx| t.supprimer_choix(w, cx))),
                    "escape" => h.tableau_choisi(cx).is_some_and(|t| {
                        t.update(cx, |t, cx| t.effacer_choix(cx));
                        true
                    }),
                    _ => h.frappe(ev, w, cx),
                };
                if pris {
                    cx.stop_propagation();
                }
            }))
            .child(self.blocs.clone())
            .child(couche)
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, (), window, _| {
                        let h = entite.clone();
                        window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
                            if phase.capture()
                                && h.update(cx, |h, cx| h.clic_sous_le_dernier(ev, window, cx))
                            {
                                cx.stop_propagation();
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}
