// Port de app/src/ui/views/chat/components/post.ts (`renderPost`, `ligne`) et de post.css.
//! La présentation « post » d'une conversation : un tweet, à la lettre de X.
//!
//! ```text
//!   (J)  Julien · 2 h
//!    │   la question (Markdown, 6 lignes)
//!    │   [ document · « le passage » ]      une ligne de citation
//!    │   (bulle) 3
//!   (C)  Claude · 2 h                       la 1re réponse, reliée par le trait du fil
//!        la réponse (4 lignes)
//!        Afficher ce fil
//! ```
//!
//! Le texte passe par l'éditeur de la note en lecture seule (le même pipeline que le fil), coupé à
//! la hauteur de ses lignes. ★ Le fondu de la dernière ligne ne se pose QUE si le texte dépasse :
//! on mesure la hauteur du contenu, pas le nombre de caractères. Le mode compact (≤ 360 px de
//! contenu) se décide sur la largeur MESURÉE du post.
//!
//! Rien n'est branché ici : qui héberge le post (la grappe) donne ses [`Rappels`] au montage — le
//! clic, « Aller à », le menu ⋯. Sans rappel, le coin n'est pas dessiné.

use std::rc::Rc;

use fragment_chat::api::Participant;
use fragment_chat::calendrier::Fuseau;
use fragment_theme::{Couleur, Theme};
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, canvas, div, px, svg,
};

use super::modele::{
    FONDU, LIBELLE_FIL, LIBELLE_VIDE, Origine, PostMessage, PostModel, cote_avatar, est_compact,
    est_coupe, hauteur_visible, interligne, libelle_heure, libelle_pieces, texte_citation,
};
use crate::chat_fil::modele::{ContenuAvatar, contenu_avatar, oklch_vers_rgb};
use crate::chat_fil::texte::hauteur_du_contenu;
use crate::echelle::{Echelle, u};
use crate::editeur::{EditeurLive, OptionsEditeur};
use crate::icones;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Un rappel de l'hôte : appelé au clic, sans rien savoir du chat.
pub type Rappel = Rc<dyn Fn(&mut Window, &mut App)>;

/// Ce que l'hôte branche au montage. Absent : le geste ne fait rien (et son bouton n'est pas dessiné).
#[derive(Clone, Default)]
pub struct Rappels {
    /// Un clic sur le post : le fil en grand.
    pub ouvrir: Option<Rappel>,
    /// « Aller à » (icône seule) : le membre à sa place.
    pub aller_a: Option<Rappel>,
    /// ⋯ : le menu du membre.
    pub menu: Option<Rappel>,
}

/// Ce que le post sait de son contexte.
#[derive(Clone)]
pub struct DepsPost {
    /// L'identité du post (ids des éléments).
    pub id: SharedString,
    /// La source de la conversation : document et passage. `None` : pas de ligne de citation.
    pub origine: Option<Origine>,
    /// L'horloge de l'hôte, en ms : figée en test, jamais lue ici.
    pub maintenant: i64,
    pub fuseau: Rc<dyn Fuseau>,
    /// Le fond sous le post : le fondu s'efface VERS lui (jeton du thème, choisi par l'hôte).
    pub fond: Couleur,
}

/// Un corps de texte : l'éditeur de la note en lecture seule, à la taille du mode.
struct Corps {
    editeur: Entity<EditeurLive>,
    /// La hauteur du contenu relevée à l'image précédente.
    derniere: f32,
}

impl Corps {
    fn new(texte: &str, compact: bool, window: &mut Window, cx: &mut App) -> Self {
        let taille = if compact { 14.0 } else { 15.0 };
        let options = OptionsEditeur {
            lecture_seule: true,
            fond_transparent: true,
            taille_texte: taille,
            marges: Some([0.0; 4]),
            // L'interligne se pose en multiple de la taille : 20 / 15, 18 / 14.
            interligne: Some(interligne(compact) / taille),
            ..OptionsEditeur::default()
        };
        let editeur = cx.new(|cx| EditeurLive::new(texte, options, window, cx));
        Corps {
            editeur,
            derniere: 0.0,
        }
    }

    fn hauteur(&self, cx: &App) -> f32 {
        hauteur_du_contenu(self.editeur.read(cx), cx)
    }
}

/// La vue du post.
pub struct PostVue {
    modele: PostModel,
    deps: DepsPost,
    rappels: Rappels,
    tweet: Option<Corps>,
    reponse: Option<Corps>,
    /// Les éditeurs ont été construits pour ce mode (on les refait quand il change).
    compact_construit: bool,
    /// La largeur du post, mesurée à la peinture (px de conception).
    largeur: Option<f32>,
    coupe: [bool; 2],
}

impl PostVue {
    /// Un post sur `modele`, au montage de l'hôte (qui donne ses `rappels`).
    pub fn new(
        modele: PostModel,
        deps: DepsPost,
        rappels: Rappels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut v = PostVue {
            modele,
            deps,
            rappels,
            tweet: None,
            reponse: None,
            compact_construit: false,
            largeur: None,
            coupe: [false; 2],
        };
        v.construire(false, window, cx);
        v
    }

    fn construire(&mut self, compact: bool, window: &mut Window, cx: &mut App) {
        self.compact_construit = compact;
        let corps = |m: &Option<PostMessage>, window: &mut Window, cx: &mut App| {
            m.as_ref()
                .filter(|m| !m.text.is_empty())
                .map(|m| Corps::new(&m.text, compact, window, cx))
        };
        self.tweet = corps(&self.modele.post, window, cx);
        self.reponse = corps(&self.modele.reply, window, cx);
    }

    /// Remplace le modèle (la conversation a avancé) : les corps sont reconstruits.
    pub fn poser_modele(&mut self, modele: PostModel, window: &mut Window, cx: &mut Context<Self>) {
        self.modele = modele;
        let c = self.compact();
        self.construire(c, window, cx);
        cx.notify();
    }

    /// Change l'horloge de l'hôte.
    pub fn poser_maintenant(&mut self, maintenant: i64, cx: &mut Context<Self>) {
        self.deps.maintenant = maintenant;
        cx.notify();
    }

    /// Change le fond vers lequel le fondu s'efface (l'hôte le fait au survol).
    pub fn poser_fond(&mut self, fond: Couleur, cx: &mut Context<Self>) {
        self.deps.fond = fond;
        cx.notify();
    }

    /// Pose la largeur du post : l'hôte qui la connaît d'avance (et les tests).
    pub fn poser_largeur(&mut self, largeur: f32, cx: &mut Context<Self>) {
        if self.largeur != Some(largeur) {
            self.largeur = Some(largeur);
            cx.notify();
        }
    }

    /// Le mode compact : décidé sur la largeur mesurée (large tant qu'on ne l'a pas).
    #[must_use]
    pub fn compact(&self) -> bool {
        self.largeur.is_some_and(est_compact)
    }

    /// Le texte du tweet est-il coupé (fondu posé) ? Dernier constat de la peinture.
    #[must_use]
    pub fn tweet_coupe(&self) -> bool {
        self.coupe[0]
    }

    /// Le texte de la réponse est-il coupé ?
    #[must_use]
    pub fn reponse_coupee(&self) -> bool {
        self.coupe[1]
    }

    /// La hauteur du contenu du tweet (avant coupe), si le tweet a du texte.
    #[must_use]
    pub fn hauteur_tweet(&self, cx: &App) -> Option<f32> {
        self.tweet.as_ref().map(|c| c.hauteur(cx))
    }

    /// Le modèle affiché.
    #[must_use]
    pub fn modele(&self) -> &PostModel {
        &self.modele
    }

    /// L'heure du tweet, « · 2 h ».
    #[must_use]
    pub fn heure_tweet(&self) -> Option<String> {
        let at = self.modele.post.as_ref()?.at?;
        Some(libelle_heure(at, self.deps.maintenant, &*self.deps.fuseau))
    }

    // ─── Le dessin ───

    /// Une rangée (le tweet ou la réponse) : la gouttière (avatar, trait du fil) et la colonne
    /// (nom · heure, corps, pièces jointes, puis `extras`).
    fn rangee(
        &self,
        m: &PostMessage,
        reponse: bool,
        coupe: bool,
        extras: Vec<AnyElement>,
        t: &Theme,
        cx: &App,
    ) -> gpui_kit::Div {
        let compact = self.compact();
        let p = &t.palette;
        let (gouttiere, ecart) = if compact { (28.0, 8.0) } else { (40.0, 12.0) };
        let trait_fil = p.text_normal.melange(16.0, Couleur::TRANSPARENT);
        let mut gout = div()
            .flex()
            .flex_col()
            .items_center()
            .flex_none()
            .w(u(cx, gouttiere));
        if let Some(a) = &m.author {
            gout = gout.child(avatar_post(a, compact, t, cx));
        }
        if !reponse && self.modele.reply.is_some() {
            // Le trait du fil : de sous l'avatar jusqu'à l'avatar de la réponse (marge négative en
            // bas : il passe sous le haut de la rangée suivante).
            gout = gout.child(
                div()
                    .flex_1()
                    .w(u(cx, 2.0))
                    .mt(u(cx, t.forme.size_4_1))
                    .mb(u(cx, -t.forme.size_4_1))
                    .rounded(u(cx, t.forme.radius_hair))
                    .bg(couleur(trait_fil)),
            );
        }
        let mut tete = div()
            .flex()
            .flex_row()
            .items_baseline()
            .gap(u(cx, t.forme.size_4_1))
            .min_w(px(0.0))
            // La place des boutons de l'hôte (⋯…).
            .pr(u(cx, 28.0))
            .whitespace_nowrap()
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .font_weight(gpui_kit::FontWeight(f32::from(
                        u16::try_from(t.typo.font_bold).unwrap_or(700),
                    )))
                    .child(
                        m.author
                            .as_ref()
                            .map(|a| a.name.clone())
                            .unwrap_or_default(),
                    ),
            );
        if let Some(at) = m.at {
            tete = tete.child(
                div()
                    .flex_none()
                    .text_color(couleur(p.text_muted))
                    .child(libelle_heure(at, self.deps.maintenant, &*self.deps.fuseau)),
            );
        }
        let mut main = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .pb(u(cx, t.forme.size_4_3))
            .child(tete);
        let corps = if reponse { &self.reponse } else { &self.tweet };
        if let Some(c) = corps {
            main = main.child(self.texte(c, reponse, coupe, t, cx));
        }
        if let Some(l) = libelle_pieces(m.attachments) {
            main = main.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .self_start()
                    .gap(u(cx, t.forme.size_2_3))
                    .mt(u(cx, t.forme.size_4_2))
                    .px(u(cx, t.forme.size_2_5))
                    .py(u(cx, t.forme.size_4_1))
                    .border(px(t.forme.border_width))
                    .border_color(couleur(trait_fil))
                    .rounded(px(999.0))
                    .text_color(couleur(p.text_muted))
                    .text_size(u(cx, t.typo.font_ui_medium))
                    .child(
                        svg()
                            .path(icones::chemin("paperclip"))
                            .size(u(cx, t.forme.icon_xs))
                            .flex_none(),
                    )
                    .child(l),
            );
        }
        for e in extras {
            main = main.child(e);
        }
        div()
            .flex()
            .flex_row()
            .gap(u(cx, ecart))
            .child(gout)
            .child(main)
    }

    /// Le corps : l'éditeur coupé à ses lignes, le fondu seulement s'il dépasse.
    fn texte(&self, c: &Corps, reponse: bool, coupe: bool, t: &Theme, cx: &App) -> AnyElement {
        let compact = self.compact();
        let h = c.hauteur(cx);
        let visible = hauteur_visible(h, reponse, compact);
        let mut boite = div()
            .relative()
            .mt(u(cx, t.forme.size_2_1))
            .w_full()
            .min_w(px(0.0))
            .h(px(visible))
            .overflow_hidden()
            .child(div().w_full().h(px(h)).child(c.editeur.clone()));
        if coupe {
            let fond = couleur(self.deps.fond);
            let mut transparent = fond;
            transparent.a = 0.0;
            boite = boite.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(FONDU.min(visible)))
                    .bg(gpui_kit::linear_gradient(
                        180.0,
                        gpui_kit::linear_color_stop(transparent, 0.0),
                        gpui_kit::linear_color_stop(fond, 1.0),
                    )),
            );
        }
        boite.into_any_element()
    }

    /// La ligne de citation : UNE ligne encadrée (document en gras, passage en gris).
    fn citation(&self, o: &Origine, t: &Theme, cx: &App) -> AnyElement {
        let compact = self.compact();
        let p = &t.palette;
        let mut l = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(u(cx, t.forme.size_2_3))
            .min_w(px(0.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .mt(u(cx, t.forme.size_2_5))
            .border(px(t.forme.border_width))
            .border_color(couleur(p.text_normal.melange(16.0, Couleur::TRANSPARENT)))
            .text_size(u(cx, t.typo.font_ui_medium))
            .line_height(u(cx, 16.0));
        l = if compact {
            l.px(u(cx, t.forme.size_2_5))
                .py(u(cx, t.forme.size_2_3))
                .rounded(u(cx, t.forme.radius_l))
        } else {
            l.px(u(cx, t.forme.size_4_3))
                .py(u(cx, t.forme.size_4_2))
                .rounded(u(cx, t.forme.radius_xl))
        };
        l = l.child(
            svg()
                .path(icones::chemin(&o.icone))
                .size(u(cx, t.forme.icon_xs))
                .flex_none()
                .text_color(couleur(p.text_muted)),
        );
        // Étroit : le nom du document disparaît, le passage reste.
        if !compact {
            l = l.child(
                div()
                    .flex_none()
                    .max_w(gpui_kit::relative(0.45))
                    .overflow_hidden()
                    .text_ellipsis()
                    .font_weight(gpui_kit::FontWeight(f32::from(
                        u16::try_from(t.typo.font_semibold).unwrap_or(600),
                    )))
                    .child(o.titre.clone()),
            );
        }
        if let Some(q) = o.citation.as_deref().and_then(texte_citation) {
            l = l.child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_color(couleur(p.text_muted))
                    .child(q),
            );
        }
        l.into_any_element()
    }

    /// La barre d'actions : le nombre de réponses, en gris.
    fn actions(&self, t: &Theme, cx: &App) -> AnyElement {
        let compact = self.compact();
        div()
            .flex()
            .flex_row()
            .gap(u(
                cx,
                if compact {
                    t.forme.size_2_9
                } else {
                    t.forme.size_4_7
                },
            ))
            .mt(u(cx, t.forme.size_2_5))
            .text_color(couleur(t.palette.text_muted))
            .text_size(u(cx, t.typo.font_ui_medium))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(u(cx, t.forme.size_2_3))
                    .child(
                        svg()
                            .path(icones::chemin("message-circle"))
                            .size(u(cx, 17.0))
                            .flex_none(),
                    )
                    .child(self.modele.replies.to_string()),
            )
            .into_any_element()
    }

    /// « Afficher ce fil » : le lien de la 1re réponse quand d'autres messages suivent.
    fn afficher_le_fil(&self, t: &Theme, cx: &App) -> AnyElement {
        div()
            .mt(u(cx, t.forme.size_2_3))
            .text_color(couleur(t.palette.color_accent))
            .text_size(u(cx, t.typo.font_ui_normal))
            .child(LIBELLE_FIL)
            .into_any_element()
    }

    /// Le coin haut-droit : « Aller à » (icône seule), puis ⋯ — branchés par l'hôte.
    fn coin(&self, t: &Theme, cx: &App) -> Option<gpui_kit::Div> {
        if self.rappels.aller_a.is_none() && self.rappels.menu.is_none() {
            return None;
        }
        let accent = t.palette.color_accent;
        let bouton = |nom: &str, icone: &str, libelle: &str, taille: f32, rappel: Rappel| {
            div()
                .id(ElementId::from(SharedString::from(format!(
                    "{}-{nom}",
                    self.deps.id
                ))))
                .role(Role::Button)
                .aria_label(libelle.to_owned())
                .flex()
                .items_center()
                .justify_center()
                .size(u(cx, 26.0))
                .rounded(px(999.0))
                .cursor_pointer()
                .text_color(couleur(t.palette.text_muted))
                .hover(move |s| {
                    s.text_color(couleur(accent))
                        .bg(couleur(accent.melange(12.0, Couleur::TRANSPARENT)))
                })
                .active(move |s| s.bg(couleur(accent.melange(22.0, Couleur::TRANSPARENT))))
                .child(
                    svg()
                        .path(icones::chemin(icone))
                        .size(u(cx, taille))
                        .flex_none(),
                )
                // Un clic ici ne navigue pas : le post ne le voit pas.
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    rappel(window, cx);
                })
        };
        let mut coin = div()
            .absolute()
            .top(u(cx, 8.0))
            .right(u(cx, 8.0))
            .flex()
            .items_center()
            .gap(u(cx, t.forme.size_2_1));
        if let Some(r) = &self.rappels.aller_a {
            coin = coin.child(bouton("aller-a", "locate", "Aller à", 15.0, r.clone()));
        }
        if let Some(r) = &self.rappels.menu {
            coin = coin.child(bouton(
                "plus",
                "ellipsis",
                "Plus d’options",
                17.0,
                r.clone(),
            ));
        }
        Some(coin)
    }
}

/// L'avatar d'un participant à la taille du post : 40 px (28 en compact), sans ombre, l'icône à 20 px.
///
/// Même contenu que l'avatar du fil (une icône du registre, sinon des initiales sur une pastille
/// teintée) : seule la taille change (`chat_fil::primitives::avatar` n'a que 22 et 28 px).
fn avatar_post(p: &Participant, compact: bool, t: &Theme, cx: &App) -> AnyElement {
    let pal = &t.palette;
    let corps = if compact {
        t.typo.font_ui_small
    } else {
        t.typo.font_ui_large
    };
    let base = div()
        .id(ElementId::from(SharedString::from(format!(
            "chat-avatar-{}",
            p.id
        ))))
        .role(Role::Image)
        .aria_label(p.name.clone())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(u(cx, cote_avatar(compact)))
        .rounded(px(999.0))
        .font_family(FAMILLE_INTERFACE);
    let pastille = |l: f32, c: f32, teinte: u32| {
        // Les teintes sont des angles (≤ 360) : exacts en f32.
        #[allow(clippy::cast_precision_loss)]
        let [r, g, b] = oklch_vers_rgb(l, c, teinte as f32);
        Couleur::octets(r, g, b, 255)
    };
    match contenu_avatar(p, |n| icones::svg(n).is_some()) {
        ContenuAvatar::Icone(nom) => base
            .bg(couleur(pal.text_normal))
            .text_color(couleur(pal.workspace_bg))
            .child(
                svg()
                    .path(icones::chemin(&nom))
                    .size(u(cx, 20.0))
                    .flex_none(),
            )
            .into_any_element(),
        ContenuAvatar::Texte { texte, teinte } => base
            .bg(couleur(pastille(pal.avatar_bg_l, pal.avatar_bg_c, teinte)))
            .text_color(couleur(pastille(pal.avatar_fg_l, pal.avatar_fg_c, teinte)))
            .text_size(u(cx, corps))
            .font_weight(gpui_kit::FontWeight(f32::from(
                u16::try_from(t.typo.font_semibold).unwrap_or(600),
            )))
            .child(texte)
            .into_any_element(),
    }
}

impl Render for PostVue {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme_ou_defaut(cx);
        // Le mode a changé (la largeur mesurée passe le seuil) : les corps sont refaits à la bonne taille.
        let compact = self.compact();
        if compact != self.compact_construit {
            self.construire(compact, window, cx);
        }
        // La mise en page d'un éditeur n'est connue qu'après sa peinture : tant que la hauteur
        // bouge, une image de plus.
        let mut bouge = false;
        let mut coupe = [false; 2];
        for (i, corps) in [self.tweet.as_mut(), self.reponse.as_mut()]
            .into_iter()
            .enumerate()
        {
            let Some(c) = corps else { continue };
            let h = hauteur_du_contenu(c.editeur.read(cx), cx);
            if (h - c.derniere).abs() > 0.25 {
                c.derniere = h;
                bouge = true;
            }
            coupe[i] = est_coupe(h, i == 1, compact);
        }
        self.coupe = coupe;
        if bouge {
            window.request_animation_frame();
        }

        let p = &t.palette;
        let mut racine = div()
            .id(ElementId::from(self.deps.id.clone()))
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .min_w(px(0.0))
            .px(u(cx, t.forme.size_4_4))
            .pt(u(cx, t.forme.size_4_3))
            .pb(u(cx, t.forme.size_4_1))
            .font_family(FAMILLE_INTERFACE)
            .text_size(u(
                cx,
                if compact {
                    t.typo.font_ui_normal
                } else {
                    t.typo.font_ui_large
                },
            ))
            .line_height(u(cx, interligne(compact)))
            .text_color(couleur(p.text_normal));
        // La largeur du post, mesurée à la peinture : c'est elle qui décide du mode compact.
        let faible = cx.entity().downgrade();
        racine = racine.child(
            canvas(
                |_, _, _| {},
                move |bornes, (), _, cx| {
                    let l = f32::from(bornes.size.width) / Echelle::courante(cx).zoom();
                    let Some(v) = faible.upgrade() else { return };
                    if v.read(cx).largeur.is_none_or(|a| (a - l).abs() > 0.5) {
                        cx.defer(move |cx| {
                            v.update(cx, |v, cx| v.poser_largeur(l, cx));
                        });
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );

        let Some(post) = self.modele.post.clone() else {
            return racine
                .pb(u(cx, t.forme.size_4_3))
                .text_color(couleur(p.text_muted))
                .child(LIBELLE_VIDE)
                .into_any_element();
        };
        // Le tweet : sa citation d'origine (UNE ligne), puis les actions.
        let mut extras = Vec::new();
        if let Some(o) = &self.deps.origine {
            extras.push(self.citation(o, &t, cx));
        }
        extras.push(self.actions(&t, cx));
        racine = racine.child(self.rangee(&post, false, coupe[0], extras, &t, cx));
        if let Some(r) = self.modele.reply.clone() {
            let mut extras = Vec::new();
            if self.modele.more {
                extras.push(self.afficher_le_fil(&t, cx));
            }
            racine = racine.child(self.rangee(&r, true, coupe[1], extras, &t, cx));
        }
        if let Some(coin) = self.coin(&t, cx) {
            racine = racine.child(coin);
        }
        match self.rappels.ouvrir.clone() {
            Some(o) => racine
                .cursor_pointer()
                .on_click(move |_, window, cx| o(window, cx))
                .into_any_element(),
            None => racine.into_any_element(),
        }
    }
}
