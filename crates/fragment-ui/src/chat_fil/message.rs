// Port de app/src/ui/views/chat/components/thread.ts (`renderMessage`) et de thread.css
// (`.chat-message*`, `.chat-typing`, `.chat-message-actions`).
//! Un message : son auteur au-dessus, ses parts, « ••• » tant qu'il attend, sa barre d'actions.
//!
//! - le texte d'un part passe par [`TexteLecture`] (l'éditeur de la note, lecture seule), sauf si
//!   une contribution `chat:text@1` s'applique ;
//! - un part non-texte est dessiné par son [`RenduDePart`] ; s'il rend une poignée vivante
//!   ([`PartVivant`]), chaque nouvelle version est un `mettre_a_jour`, pas un redessin ;
//! - l'identité d'un part est `message#part` : l'id du part s'il en a un, sinon son rang
//!   ([`PartRendue`]) — la surface de document de la conversation (`fr-t7-surface`) les lit.

use fragment_chat::api::{ChatMode, ChatPart, ChatSlotContext, Participant, Role};
use fragment_chat::chat_session::LiveMessage;
use gpui_kit::{
    AnyView, AppContext as _, ClipboardItem, Context, ElementId, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px, relative,
};

use super::modele::{
    EtatMessage, LIBELLE_ATTENTE, attend, auteur_de, avec_actions, etat, forme, peut_patcher,
    texte_a_copier,
};
use super::primitives::{Variante, avatar, bouton_icone};
use super::registre::{PartVivant, RegistreFil, rendu_de_part, rendu_de_texte, slots_applicables};
use super::texte::{INTERLIGNE_MESSAGE, TexteLecture};
use crate::echelle::u;
use crate::editeur::EditeurLive;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Ce que le fil sait de la conversation affichée (partagé par tous ses messages).
#[derive(Clone)]
pub struct DepsFil {
    pub participants: Vec<Participant>,
    pub registre: RegistreFil,
    pub fournisseur: String,
    pub conversation: String,
    pub mode: ChatMode,
    /// `--font-text-size` : la taille du texte d'un message est celle de la note.
    pub taille_texte: f32,
}

impl DepsFil {
    /// Le contexte que reçoivent les contributions pour ce message.
    #[must_use]
    pub fn contexte(&self, m: &LiveMessage) -> ChatSlotContext {
        ChatSlotContext {
            provider: self.fournisseur.clone(),
            conversation: self.conversation.clone(),
            mode: self.mode,
            message: Some(m.message.clone()),
            streaming: m.streaming,
        }
    }
}

/// Une part rendue, telle que la surface de document de la vue le lit.
#[derive(Clone)]
pub struct PartRendue {
    pub index: usize,
    /// L'id du part (`ChatPart.id`), s'il en a un : son identité quand les parts d'avant changent.
    pub id: Option<String>,
    pub kind: String,
    /// L'éditeur (lecture seule) d'un texte rendu en markdown ; absent : un part non-texte, ou un
    /// texte rendu par `chat:text@1`.
    pub editeur: Option<Entity<EditeurLive>>,
}

enum Rendu {
    /// Du texte par l'éditeur de la note.
    Texte {
        id: Option<String>,
        hote: Entity<TexteLecture>,
    },
    /// Du texte par une contribution `chat:text@1`.
    TexteTiers { id: Option<String>, vue: AnyView },
    /// Un part non-texte ; `None` : personne ne sait le rendre (invisible).
    Part {
        id: Option<String>,
        kind: String,
        vivant: Option<Box<dyn PartVivant>>,
    },
}

impl Rendu {
    fn detruire(&mut self, cx: &mut gpui_kit::App) {
        if let Rendu::Part {
            vivant: Some(v), ..
        } = self
        {
            v.detruire(cx);
        }
    }
}

/// Un message rendu (une entité GPUI).
pub struct MessageVue {
    id: String,
    auteur: Participant,
    deps: DepsFil,
    forme: String,
    etat: EtatMessage,
    erreur: Option<String>,
    attend: bool,
    actions: bool,
    texte_copie: String,
    message: LiveMessage,
    rendus: Vec<Rendu>,
    /// Combien de fois le message a été mis à jour en place.
    patches: usize,
}

impl MessageVue {
    /// Dessine un message.
    pub fn new(
        m: &LiveMessage,
        deps: DepsFil,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let auteur = auteur_de(&deps.participants, &m.author);
        let mut v = MessageVue {
            id: m.id.clone(),
            auteur,
            deps,
            forme: forme(m),
            etat: etat(m),
            erreur: m.error.clone(),
            attend: attend(m),
            actions: false,
            texte_copie: String::new(),
            message: m.clone(),
            rendus: Vec::new(),
            patches: 0,
        };
        let ctx = v.deps.contexte(m);
        for part in &m.parts {
            let r = v.dessiner(part, &ctx, window, cx);
            v.rendus.push(r);
        }
        v.majeter(m);
        v
    }

    /// Ce qui suit l'état : « ••• », la barre d'actions, l'erreur.
    fn majeter(&mut self, m: &LiveMessage) {
        self.etat = etat(m);
        self.erreur = m.error.clone();
        self.attend = attend(m);
        self.actions = avec_actions(m, self.auteur.role);
        self.texte_copie = texte_a_copier(m);
        self.message = m.clone();
    }

    fn dessiner(
        &self,
        part: &ChatPart,
        ctx: &ChatSlotContext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Rendu {
        let id = part.id.clone();
        if part.kind == "text" {
            let texte = part.text.clone().unwrap_or_default();
            if let Some(r) = rendu_de_texte(&self.deps.registre, ctx) {
                let vue = r.rendre(&texte, ctx, window, cx);
                return Rendu::TexteTiers { id, vue };
            }
            let taille = self.deps.taille_texte;
            let hote = cx.new(|cx| TexteLecture::new(&texte, taille, window, cx));
            return Rendu::Texte { id, hote };
        }
        let vivant = rendu_de_part(&self.deps.registre, &part.kind)
            .and_then(|r| r.creer(part, ctx, window, cx));
        Rendu::Part {
            id,
            kind: part.kind.clone(),
            vivant,
        }
    }

    /// Met à jour EN PLACE si la forme n'a pas changé (le texte en flux, la fin du tour) ; sinon
    /// rend `false` et l'appelant reconstruit.
    pub fn mettre_a_jour(
        &mut self,
        suivant: &LiveMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !peut_patcher(&self.forme, self.rendus.len(), suivant, self.actions) {
            return false;
        }
        let ctx = self.deps.contexte(suivant);
        for (i, part) in suivant.parts.iter().enumerate() {
            if part.kind == "text" {
                let texte = part.text.clone().unwrap_or_default();
                match &mut self.rendus[i] {
                    Rendu::Texte { hote, .. } => {
                        let hote = hote.clone();
                        hote.update(cx, |h, cx| h.mettre_a_jour(&texte, window, cx));
                        continue;
                    }
                    Rendu::TexteTiers { .. } => {}
                    Rendu::Part { .. } => {}
                }
                let neuf = self.dessiner(part, &ctx, window, cx);
                self.rendus[i] = neuf;
            } else {
                // Un part qui vit est mis à jour ; sinon (ou si la mise à jour refuse) il est
                // détruit et redessiné.
                let mis_a_jour = match &mut self.rendus[i] {
                    Rendu::Part {
                        vivant: Some(v), ..
                    } => v.mettre_a_jour(part, &ctx, window, cx),
                    _ => false,
                };
                if !mis_a_jour {
                    self.rendus[i].detruire(cx);
                    let neuf = self.dessiner(part, &ctx, window, cx);
                    self.rendus[i] = neuf;
                }
            }
        }
        self.forme = forme(suivant);
        self.majeter(suivant);
        self.patches += 1;
        cx.notify();
        true
    }

    /// Libère les parts vivants (le message quitte le fil ou va être reconstruit).
    pub fn detruire(&mut self, cx: &mut gpui_kit::App) {
        for r in &mut self.rendus {
            r.detruire(cx);
        }
    }

    /// L'id du message.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Le participant qui l'a écrit.
    #[must_use]
    pub fn auteur(&self) -> &Participant {
        &self.auteur
    }

    /// Son état (`data-state`).
    #[must_use]
    pub fn etat(&self) -> EtatMessage {
        self.etat
    }

    /// « ••• » est montré (réponse en flux sans aucun part).
    #[must_use]
    pub fn en_attente(&self) -> bool {
        self.attend
    }

    /// La barre d'actions est posée.
    #[must_use]
    pub fn avec_actions(&self) -> bool {
        self.actions
    }

    /// L'erreur affichée, s'il y en a une.
    #[must_use]
    pub fn erreur(&self) -> Option<&str> {
        self.erreur.as_deref()
    }

    /// Combien de fois il a été mis à jour en place.
    #[must_use]
    pub fn patches(&self) -> usize {
        self.patches
    }

    /// Les parts rendus, dans l'ordre.
    #[must_use]
    pub fn parts(&self, cx: &gpui_kit::App) -> Vec<PartRendue> {
        self.rendus
            .iter()
            .enumerate()
            .map(|(index, r)| match r {
                Rendu::Texte { id, hote } => PartRendue {
                    index,
                    id: id.clone(),
                    kind: "text".into(),
                    editeur: Some(hote.read(cx).editeur().clone()),
                },
                Rendu::TexteTiers { id, .. } => PartRendue {
                    index,
                    id: id.clone(),
                    kind: "text".into(),
                    editeur: None,
                },
                Rendu::Part { id, kind, .. } => PartRendue {
                    index,
                    id: id.clone(),
                    kind: kind.clone(),
                    editeur: None,
                },
            })
            .collect()
    }
}

impl Render for MessageVue {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        let p = &theme.palette;
        let utilisateur = self.auteur.role == Role::User;

        // La racine : un article par message, nommé par son auteur (principe 15).
        let mut racine = div()
            .id(ElementId::from(SharedString::from(format!(
                "chat-message-{}",
                self.id
            ))))
            .role(gpui_kit::Role::Article)
            .aria_label(self.auteur.name.clone())
            .group("chat-message")
            .flex()
            .flex_col()
            .gap(u(cx, 4.0))
            .w_full()
            .min_w(px(0.0))
            .font_family(FAMILLE_INTERFACE)
            .text_size(px(self.deps.taille_texte))
            .line_height(px(self.deps.taille_texte * INTERLIGNE_MESSAGE))
            .text_color(couleur(p.text_normal));
        if utilisateur {
            racine = racine.items_end();
        }

        // L'auteur AU-DESSUS de la réponse : le texte occupe toute la colonne.
        if !utilisateur {
            racine = racine.child(
                div()
                    .flex()
                    .items_center()
                    .gap(u(cx, 8.0))
                    .text_size(px(theme.typo.font_ui_medium))
                    .line_height(px(theme.typo.font_ui_medium * 1.2))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(avatar(&self.auteur, false, &theme, cx))
                    .child(self.auteur.name.clone()),
            );
        }

        // Le contenu : les parts dans l'ordre, puis l'attente et l'erreur.
        let mut contenu = div().flex().flex_col().min_w(px(0.0)).w_full();
        if utilisateur {
            contenu = contenu
                .px(u(cx, 12.0))
                .py(px(7.0))
                .rounded(u(cx, theme.forme.radius_ml))
                .bg(couleur(p.background_secondary))
                .w_auto();
        }
        // ★ Les pièces jointes consécutives forment UN flux en ligne qui passe à la ligne
        // (`.chat-part[data-part='attachment']` est `inline-flex` dans Fragment, les autres parts
        // sont des blocs) : une rangée `flex-wrap` par série de pièces.
        let mut rangee: Option<gpui_kit::Div> = None;
        for r in &self.rendus {
            if let Rendu::Part {
                kind,
                vivant: Some(v),
                ..
            } = r
                && kind == "attachment"
            {
                let r = rangee
                    .take()
                    .unwrap_or_else(|| div().flex().flex_wrap().items_start());
                rangee = Some(r.child(v.vue()));
                continue;
            }
            if let Some(r) = rangee.take() {
                contenu = contenu.child(r);
            }
            match r {
                Rendu::Texte { hote, .. } => contenu = contenu.child(hote.clone()),
                Rendu::TexteTiers { vue, .. } => contenu = contenu.child(vue.clone()),
                Rendu::Part {
                    vivant: Some(v), ..
                } => contenu = contenu.child(v.vue()),
                Rendu::Part { vivant: None, .. } => {}
            }
        }
        if let Some(r) = rangee.take() {
            contenu = contenu.child(r);
        }
        if self.attend {
            // « ••• » : trois points de 5 px, écart 3 px (`.chat-typing`) ; l'animation est coupée
            // dans les scènes, on les montre fixes.
            let mut points = div()
                .id("chat-attente")
                .role(gpui_kit::Role::Status)
                .aria_label(LIBELLE_ATTENTE)
                .flex()
                .gap(px(3.0))
                .py(u(cx, 6.0));
            for _ in 0..3 {
                points = points.child(
                    div()
                        .size(px(5.0))
                        .rounded(px(999.0))
                        .bg(couleur(p.text_muted))
                        .opacity(0.6),
                );
            }
            contenu = contenu.child(points);
        }
        if let Some(e) = self.erreur.clone() {
            contenu = contenu.child(
                div()
                    .id("chat-erreur")
                    .role(gpui_kit::Role::Alert)
                    .mt(u(cx, 4.0))
                    .text_color(couleur(p.text_error))
                    .text_size(px(12.5))
                    .child(e),
            );
        }
        let mut corps = div().flex().flex_col().gap(u(cx, 4.0)).min_w(px(0.0));
        corps = if utilisateur {
            corps.max_w(relative(0.85))
        } else {
            corps.w_full()
        };
        corps = corps.child(contenu);

        // Les actions : visibles au survol (ou au clavier), comme shadcn.
        if self.actions {
            let ctx = self.deps.contexte(&self.message);
            let copie = self.texte_copie.clone();
            let mut barre = div()
                .flex()
                .gap(u(cx, 2.0))
                .ml(u(cx, -6.0))
                .opacity(0.0)
                .group_hover("chat-message", |s| s.opacity(1.0))
                .child(
                    bouton_icone(
                        format!("chat-copier-{}", self.id),
                        "copy",
                        "Copier",
                        Variante::Discret,
                        true,
                        &theme,
                        cx,
                    )
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copie.clone()));
                    }),
                );
            let slots = slots_applicables(&self.deps.registre.borrow().actions, &ctx);
            for s in slots {
                barre = barre.child((s.rendre)(&ctx, window, cx));
            }
            corps = corps.child(barre);
        }
        racine.child(corps)
    }
}
