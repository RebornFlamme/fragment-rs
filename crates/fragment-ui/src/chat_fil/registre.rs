// Port de app/src/ui/views/chat/api.ts (les registres `chat:part@1`, `chat:text@1`,
// `chat:message-actions@1`, `chat:composer-tools@1`, `chat:composer@1`) côté RENDU.
//! Les points de branchement du fil : ce qu'un plugin contribue au rendu d'un message et du champ.
//!
//! `fragment-chat::api` porte la part « modèle » de ces contrats (type, id, `appliesTo`, état d'une
//! extension du champ) ; le `render(el, …)` DOM n'a pas d'équivalent, il est ICI, en GPUI :
//!
//! - [`RenduDePart`] (`chat:part@1`) : dessine un type de part et rend un [`PartVivant`], la
//!   POIGNÉE du TS (`update` / `destroy`). Un part qui vit (un chrono, un état déplié par
//!   l'utilisateur) est mis à jour, pas redessiné. C'est le point où `fr-t7-trace` branche le sien ;
//! - [`RenduDeTexte`] (`chat:text@1`) : un rendu du texte, prioritaire s'il s'applique ;
//! - [`SlotChat`] : une contribution des emplacements `message-actions` et `composer-tools` ;
//! - [`ExtensionChamp`] (`chat:composer@1`) : le comportement du champ, avec une zone au-dessus du
//!   texte si elle en veut une.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_chat::api::{ChatComposerExtension, ChatPart, ChatSlot, ChatSlotContext, SlotAlign};
use gpui_kit::{AnyElement, AnyView, App, Window};

/// Un part non-texte, vivant : la poignée que rend sa contribution.
pub trait PartVivant {
    /// La vue à poser dans le message.
    fn vue(&self) -> AnyView;

    /// Une nouvelle version du même part (même place, même type). `true` : mise à jour faite ;
    /// `false` (défaut) : le message le détruit et le redessine par [`RenduDePart::creer`].
    fn mettre_a_jour(
        &mut self,
        _part: &ChatPart,
        _ctx: &ChatSlotContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> bool {
        false
    }

    /// Le part quitte le message (ou va être redessiné) : libérer ce qu'il tient.
    fn detruire(&mut self, _cx: &mut App) {}
}

/// Le rendu d'un type de part (`chat:part@1`).
pub trait RenduDePart {
    /// Le type de part pris en charge (`ChatPart::kind`).
    fn type_part(&self) -> &str;

    /// Dessine le part, ou `None` s'il ne sait pas (le part reste invisible).
    fn creer(
        &self,
        part: &ChatPart,
        ctx: &ChatSlotContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Box<dyn PartVivant>>;
}

/// Un rendu du TEXTE d'un message (`chat:text@1`) : le premier qui s'applique gagne.
pub trait RenduDeTexte {
    fn id(&self) -> &str;
    fn applique(&self, _ctx: &ChatSlotContext) -> bool {
        true
    }
    fn rendre(
        &self,
        texte: &str,
        ctx: &ChatSlotContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView;
}

/// Ce que rend un slot : un élément, à chaque image.
pub type RenduSlot = Rc<dyn Fn(&ChatSlotContext, &mut Window, &mut App) -> AnyElement>;

/// Une contribution d'emplacement (`message-actions`, `composer-tools`) : le côté modèle
/// (`ChatSlot`) et le rendu.
#[derive(Clone)]
pub struct SlotChat {
    pub slot: Rc<ChatSlot>,
    pub rendre: RenduSlot,
}

impl SlotChat {
    #[must_use]
    pub fn new(slot: ChatSlot, rendre: RenduSlot) -> Self {
        Self {
            slot: Rc::new(slot),
            rendre,
        }
    }

    /// Où il se place dans la barre du champ (défaut : au début).
    #[must_use]
    pub fn align(&self) -> SlotAlign {
        self.slot.align.unwrap_or_default()
    }
}

/// Une bande AU-DESSUS du texte du champ (des puces…), rendue à chaque image.
pub type ZoneChamp = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// Une extension du champ (`chat:composer@1`) : son comportement et, facultative, sa zone.
#[derive(Clone)]
pub struct ExtensionChamp {
    pub extension: Rc<dyn ChatComposerExtension>,
    /// À id égal, la plus haute priorité gagne.
    pub priorite: i32,
    pub zone: Option<ZoneChamp>,
}

impl ExtensionChamp {
    #[must_use]
    pub fn new(extension: Rc<dyn ChatComposerExtension>) -> Self {
        Self {
            extension,
            priorite: 0,
            zone: None,
        }
    }

    #[must_use]
    pub fn avec_priorite(mut self, priorite: i32) -> Self {
        self.priorite = priorite;
        self
    }

    #[must_use]
    pub fn avec_zone(mut self, zone: ZoneChamp) -> Self {
        self.zone = Some(zone);
        self
    }
}

/// Tout ce que le fil lit des contributions, « relu à l'usage » : le registre vit derrière un
/// `Rc<RefCell<…>>` ([`RegistreFil`]) et un plugin peut y ajouter après coup.
#[derive(Default)]
pub struct ContributionsFil {
    pub parts: Vec<Rc<dyn RenduDePart>>,
    pub textes: Vec<Rc<dyn RenduDeTexte>>,
    pub actions: Vec<SlotChat>,
    pub outils: Vec<SlotChat>,
    pub extensions: Vec<ExtensionChamp>,
}

/// Le registre partagé entre le fil et le champ.
pub type RegistreFil = Rc<RefCell<ContributionsFil>>;

/// Un registre vide.
#[must_use]
pub fn registre_vide() -> RegistreFil {
    Rc::new(RefCell::new(ContributionsFil::default()))
}

/// Le rendu du premier part de ce type, s'il y en a un.
#[must_use]
pub fn rendu_de_part(registre: &RegistreFil, kind: &str) -> Option<Rc<dyn RenduDePart>> {
    registre
        .borrow()
        .parts
        .iter()
        .find(|r| r.type_part() == kind)
        .cloned()
}

/// Le rendu du texte qui s'applique (le premier), s'il y en a un.
#[must_use]
pub fn rendu_de_texte(
    registre: &RegistreFil,
    ctx: &ChatSlotContext,
) -> Option<Rc<dyn RenduDeTexte>> {
    registre
        .borrow()
        .textes
        .iter()
        .find(|t| t.applique(ctx))
        .cloned()
}

/// Les slots d'un emplacement qui s'appliquent au contexte, dans l'ordre.
#[must_use]
pub fn slots_applicables(liste: &[SlotChat], ctx: &ChatSlotContext) -> Vec<SlotChat> {
    liste
        .iter()
        .filter(|s| s.slot.applies(ctx))
        .cloned()
        .collect()
}

/// Les extensions du champ retenues : celles que le fournisseur ne refuse pas (`without`), qui
/// s'appliquent au contexte, UNE par id (la plus haute priorité ; à priorité égale, la première
/// ajoutée), dans l'ordre de première apparition.
#[must_use]
pub fn extensions_retenues(
    toutes: &[ExtensionChamp],
    ctx: &ChatSlotContext,
    sans: &[String],
) -> Vec<ExtensionChamp> {
    let mut retenues: Vec<ExtensionChamp> = Vec::new();
    for e in toutes {
        let id = e.extension.id();
        if sans.iter().any(|s| s == id) || !e.extension.applies_to(ctx) {
            continue;
        }
        match retenues.iter_mut().find(|r| r.extension.id() == id) {
            Some(r) if e.priorite > r.priorite => *r = e.clone(),
            Some(_) => {}
            None => retenues.push(e.clone()),
        }
    }
    retenues
}
