// Port de app/src/ui/views/chat/components/thread.ts (`texte`, le montage markdown d'un part) et du
// bloc `.chat-markdown` de thread.css.
//! Le texte d'un part : l'éditeur de la note en LECTURE SEULE, mis à jour pendant le flux.
//!
//! ★ Le texte d'un message — de TOUS les messages, utilisateur compris — passe par le pipeline de
//! la note ([`EditeurLive`]) : titres, listes, gras, code identiques. Pendant le flux l'éditeur est
//! MIS À JOUR ([`TexteLecture::mettre_a_jour`]), jamais recréé : l'entité reste la même, c'est elle
//! que la surface de document de la conversation (`fr-t7-surface`) lit.
//!
//! Écart assumé (dette suivie, voir la PR) : les hôtes de blocs de la note (`HoteBlocs`,
//! `HoteHtml` : tableaux, maths en bloc, fonds de code) posent leur géométrie sur la colonne de
//! lecture de 720 px (`debut_texte`), pas sur une colonne de chat. Le fil monte donc l'éditeur
//! seul, aux marges de `.chat-markdown .cm-content` (`padding: 0`) ; la vue chat (`fr-t7-vue`)
//! l'enveloppera de l'hôte complet quand la colonne sera paramétrable.

use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window,
    div, px,
};

use crate::editeur::{EditeurLive, OptionsEditeur};

/// L'interligne d'un message : `.chat-message { line-height: 1.55 }` (thread.css:44).
pub const INTERLIGNE_MESSAGE: f32 = 1.55;

/// La taille du texte d'un message : celle de la note, `--font-text-size` (défaut 16 px), décision
/// de l'utilisateur (thread.css:38 à 43).
pub const TAILLE_TEXTE_DEFAUT: f32 = 16.0;

/// Les options de l'éditeur d'un message : lecture seule, sans fond, sans marge
/// (`.chat-markdown .cm-content { padding: 0 }`, thread.css), à la taille de la note.
#[must_use]
pub fn options_message(taille_texte: f32) -> OptionsEditeur {
    OptionsEditeur {
        lecture_seule: true,
        fond_transparent: true,
        taille_texte,
        marges: Some([0.0; 4]),
        interligne: Some(INTERLIGNE_MESSAGE),
        ..OptionsEditeur::default()
    }
}

/// La hauteur du contenu d'un éditeur : le bas de sa dernière ligne (rangées de retour à la ligne,
/// hauteur dictée et bloc posé dessous compris), mesuré sur la mise en page de l'image précédente.
#[must_use]
pub fn hauteur_du_contenu(e: &EditeurLive, cx: &gpui_kit::App) -> f32 {
    let lignes = e.lignes().len();
    // Les marges d'un message sont nulles (`options_message`).
    let marge_basse = 0.0;
    if lignes == 0 {
        return e.interligne() + marge_basse;
    }
    let l = lignes - 1;
    let base = e.interligne();
    let haut = e.surface(cx).haut_de_ligne(l);
    let rangees = e
        .etat()
        .read(cx)
        .display_map()
        .buffer_line_to_display_row_range(l)
        .map_or(1, |r| r.len().max(1));
    let (hauteur, bloc) = e.arguments_fork().map_or((base, 0.0), |f| {
        let cherche = |liste: &[(usize, f32)]| {
            liste
                .binary_search_by_key(&l, |&(n, _)| n)
                .ok()
                .map(|i| liste[i].1)
        };
        (
            cherche(&f.hauteurs).unwrap_or(base),
            cherche(&f.blocs).unwrap_or(0.0),
        )
    });
    // Le nombre de rangées d'une ligne reste très en dessous de 2^24.
    #[allow(clippy::cast_precision_loss)]
    let r = rangees as f32;
    haut + r * hauteur + bloc + marge_basse
}

/// Un part de texte rendu : l'éditeur en lecture seule, à la hauteur de son contenu.
pub struct TexteLecture {
    editeur: Entity<EditeurLive>,
    /// La hauteur posée à l'image précédente : tant qu'elle bouge, on redemande une image.
    derniere: f32,
    /// Combien de fois le texte a été remplacé (les tests vérifient « mis à jour, pas recréé »).
    misajours: usize,
}

impl TexteLecture {
    /// Un éditeur en lecture seule sur `texte`.
    pub fn new(
        texte: &str,
        taille_texte: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let options = options_message(taille_texte);
        let editeur = cx.new(|cx| EditeurLive::new(texte, options, window, cx));
        TexteLecture {
            editeur,
            derniere: 0.0,
            misajours: 0,
        }
    }

    /// L'éditeur (lecture seule) : la surface de document de la conversation le lit.
    #[must_use]
    pub fn editeur(&self) -> &Entity<EditeurLive> {
        &self.editeur
    }

    /// Le texte affiché.
    #[must_use]
    pub fn texte(&self, cx: &gpui_kit::App) -> String {
        self.editeur.read(cx).texte().to_owned()
    }

    /// Combien de fois le texte a été remplacé par [`Self::mettre_a_jour`].
    #[must_use]
    pub fn misajours(&self) -> usize {
        self.misajours
    }

    /// Met le texte à jour EN PLACE (le flux : un jeton de plus). Rien si le texte est le même.
    ///
    /// L'éditeur du fork ne remplace pas une plage hors édition de l'utilisateur ; `replace_all`
    /// est l'écriture programmatique, et l'élément en tire UN changement (préfixe et suffixe
    /// communs, `changement_par_morceaux`) : la projection est recalculée par région, pas en
    /// entier.
    pub fn mettre_a_jour(&mut self, texte: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.editeur.read(cx).texte() == texte {
            return;
        }
        let etat = self.editeur.read(cx).etat().clone();
        etat.update(cx, |s, cx| s.replace_all(texte.to_owned(), window, cx));
        self.misajours += 1;
        cx.notify();
    }
}

impl Render for TexteLecture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let h = hauteur_du_contenu(self.editeur.read(cx), cx);
        if (h - self.derniere).abs() > 0.25 {
            // La mise en page vient de changer (retour à la ligne, texte plus long) : une image de
            // plus pour que la hauteur rattrape le contenu.
            self.derniere = h;
            window.request_animation_frame();
        }
        div()
            .w_full()
            .min_w(px(0.0))
            .h(px(h))
            .child(self.editeur.clone())
    }
}
