// Port de app/src/ui/views/chat/scroll/jump.ts (et du calcul de l'état dans ChatView.ts,
// `majDefilement`), styles de scroll.css (`.chat-jump-anchor`, `.chat-jump`).
//! La PASTILLE « ↓ » : dit ce qui se passe SOUS l'écran — une réponse qui s'écrit, des messages
//! arrivés, ou seulement le plus récent loin dessous. Posée sur une ancre de hauteur nulle au-dessus
//! du champ : elle n'a aucun effet sur la mise en page. Un clic : le plus récent, puis on suit.

use gpui_kit::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, svg,
};

use crate::echelle::u;
use crate::flottant::{ombrer, texte};
use crate::icones;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme};

/// L'état de la pastille (`data-kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EtatPastille {
    Cachee,
    /// Une réponse s'écrit (`streaming`).
    EnCours,
    /// N messages arrivés depuis la dernière lecture (`new`).
    Nouveaux(usize),
    /// Seulement loin du plus récent (`latest`).
    PlusRecent,
}

impl EtatPastille {
    /// La valeur de `data-kind`.
    #[must_use]
    pub fn genre(self) -> &'static str {
        match self {
            EtatPastille::Cachee => "hidden",
            EtatPastille::EnCours => "streaming",
            EtatPastille::Nouveaux(_) => "new",
            EtatPastille::PlusRecent => "latest",
        }
    }
}

/// Le libellé (et l'étiquette accessible) de la pastille.
#[must_use]
pub fn libelle_pastille(e: EtatPastille) -> String {
    match e {
        EtatPastille::Cachee => String::new(),
        EtatPastille::EnCours => "Réponse en cours".into(),
        EtatPastille::Nouveaux(1) => "1 nouveau message".into(),
        EtatPastille::Nouveaux(n) => format!("{n} nouveaux messages"),
        EtatPastille::PlusRecent => "Aller au plus récent".into(),
    }
}

/// L'état d'après le fil : cachée quand le bas est à l'écran ou le fil vide ; sinon la réponse en
/// cours prime sur les nouveaux messages, qui priment sur « le plus récent ».
#[must_use]
pub fn etat_pastille(
    bas_visible: bool,
    vide: bool,
    en_cours: bool,
    nouveaux: usize,
) -> EtatPastille {
    if bas_visible || vide {
        EtatPastille::Cachee
    } else if en_cours {
        EtatPastille::EnCours
    } else if nouveaux > 0 {
        EtatPastille::Nouveaux(nouveaux)
    } else {
        EtatPastille::PlusRecent
    }
}

/// Les messages NOUVEAUX : après le dernier lu, d'un autre auteur, et finis.
/// `messages` : `(de_moi, en_flux)` dans l'ordre du fil.
#[must_use]
pub fn nouveaux(messages: &[(bool, bool)], lus_jusqua: usize) -> usize {
    messages
        .iter()
        .skip(lus_jusqua)
        .filter(|(moi, flux)| !moi && !flux)
        .count()
}

/// Le délai à l'écran au bout duquel un message est lu (ms).
pub const DELAI_LU_MS: f64 = 1000.0;

/// Ce qui est lu : tout ce qui a été vu à l'écran (le bas visible pendant [`DELAI_LU_MS`], la vue
/// montrée) ou suivi (envoi, bord vivant).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SuiviLus {
    /// `None` : pas encore initialisé (tout ce qui est là à l'ouverture est lu).
    lus_jusqua: Option<usize>,
    /// Le bas est visible depuis cet instant (ms) avec des non-lus.
    depuis: Option<f64>,
}

impl SuiviLus {
    /// Une observation : `suivi` = mode autre que `free`. Rend le nombre de messages lus.
    pub fn observer(
        &mut self,
        n_messages: usize,
        suivi: bool,
        bas_visible: bool,
        vue_montree: bool,
        maintenant: f64,
    ) -> usize {
        let lus = *self.lus_jusqua.get_or_insert(n_messages);
        if vue_montree && suivi {
            self.lus_jusqua = Some(n_messages);
            self.depuis = None;
        } else if vue_montree && bas_visible && lus < n_messages {
            match self.depuis {
                None => self.depuis = Some(maintenant),
                Some(t) if maintenant - t >= DELAI_LU_MS => {
                    self.lus_jusqua = Some(n_messages);
                    self.depuis = None;
                }
                Some(_) => {}
            }
        } else {
            self.depuis = None;
        }
        self.lus_jusqua.unwrap_or(n_messages)
    }

    /// Un délai à attendre avant de réobserver (le minuteur « lu après 1 s »), s'il y en a un.
    #[must_use]
    pub fn reste(&self, maintenant: f64) -> Option<f64> {
        self.depuis.map(|t| (t + DELAI_LU_MS - maintenant).max(0.0))
    }
}

/// L'identifiant de la pastille.
pub const ID_PASTILLE: &str = "chat-jump";

/// La pastille : une ancre de hauteur nulle, la pastille en dépasse vers le haut, centrée, à 8 px
/// au-dessus (`bottom: --size-4-2`). Padding 4 / 12, écart 6, icône 16, `--font-ui-small`,
/// `--background-primary-alt`, bordure `--chat-border` (= `--tab-outline-color`), rayon 999,
/// `--shadow-s` ; fond `--menu-bg` au survol.
pub fn pastille(
    etat: EtatPastille,
    au_clic: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let ancre = div().relative().h(px(0.0)).w_full().flex().justify_center();
    if etat == EtatPastille::Cachee {
        return ancre;
    }
    let t = theme(cx);
    let p = &t.palette;
    let libelle = libelle_pastille(etat);
    let bouton = div()
        .id(ElementId::from(ID_PASTILLE))
        .role(Role::Button)
        .aria_label(libelle.clone())
        .flex()
        .items_center()
        .gap(u(cx, 6.0))
        .py(u(cx, 4.0))
        .px(u(cx, 12.0))
        .whitespace_nowrap()
        .font_family(FAMILLE_INTERFACE)
        .text_color(couleur(p.text_normal))
        .bg(couleur(p.background_primary_alt))
        .border(u(cx, t.forme.border_width))
        .border_color(couleur(p.tab_outline_color))
        .cursor_pointer()
        .hover({
            let fond = p.menu_bg;
            move |s| s.bg(couleur(fond))
        })
        // Sans voler le focus du champ.
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
            window.prevent_default();
        })
        .on_click(move |_, window, cx| au_clic(window, cx))
        .child(
            svg()
                .path(icones::chemin("arrow-down"))
                .size(u(cx, 16.0))
                .flex_none()
                .text_color(couleur(p.text_normal)),
        )
        .child(texte(div(), cx, t.typo.font_ui_small, 1.5).child(libelle));
    let bouton = ombrer(
        bouton,
        cx,
        &p.shadow_s,
        "--shadow-s",
        px(999.0),
        u(cx, t.forme.border_width),
    );
    ancre.child(
        div()
            .absolute()
            .bottom(u(cx, 8.0))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(bouton),
    )
}
