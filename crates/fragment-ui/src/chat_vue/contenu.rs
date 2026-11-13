// Port de app/src/ui/views/chat/ChatView.ts (render, renderComplet, rafraichir, majDefilement) et de
// chat-view.css (la mise en page selon `data-mode` et `data-layout`).
//! Le CONTENU de la vue chat (une entité GPUI) : en-tête, barre des conversations, fil, accueil,
//! champ, pastille. Il assemble les briques de T7 — il n'en réécrit aucune — et applique la
//! géométrie de [`super::geometrie`] : le texte d'une réponse tombe où tomberait celui d'une note.
//!
//! ★ Le contenu se met à jour par [`ContenuChat::pomper`], appelé au début de chaque rendu (et par le
//!   shell à chaque image : la session et les gestes du cœur — menu ⋯, touches — arrivent hors de
//!   GPUI). Il ne reconstruit la structure que si la RÉVISION de l'état change (mode, conversation,
//!   fournisseur) ; ouvrir ou replier la barre ne reconstruit rien : la largeur GLISSE.
//!
//! ★ Écarts assumés (voir `parite.d/fr-t7-vue.toml`) : la surface de document (calques, marge,
//!   surlignages) est à `fr-t7-surface` / `fr-t7-montage` ; le rail et Ctrl+F à
//!   `fr-t7-rail-recherche` ; l'embed du document d'origine à T9.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use fragment_chat::api::{ChatMode, ChatPart, Role};
use fragment_chat::chat_session::LiveMessage;
use fragment_chat::follow_model::Sens;
use gpui_kit::{
    AnyView, App, AppContext as _, ClipboardItem, Context, ElementId, Entity,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role as AriaRole,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, canvas, div,
    px, svg,
};
use serde_json::json;

use super::geometrie::{
    Colonne, DEBUT_TEXTE, Disposition, Entrees, FIN_TEXTE, LARGEUR_BARRE_OUVERTE, colonne,
};
use super::modele::{
    ActionChat, CarteOrigine, EtatChat, Presentation, nouvelle_conversation, ouvrir_conversation,
    ouvrir_origine, supprimer_conversation,
};
use super::vue::{LiensChat, ToucheChat};
use crate::chat_barre_laterale::{BarreLaterale, EvenementBarre, accueil};
use crate::chat_defilement::{Annonceur, Defileur, SuiviLus, pastille};
use crate::chat_fil::primitives::{Variante, bouton_icone};
use crate::chat_fil::{
    Champ, DepsChamp, DepsFil, EvenementChamp, ListeMessages, PartVivant, RenduDePart,
};
use crate::chat_post::{DepsPost, Origine, PostVue, Rappels, post_model};
use crate::echelle::u;
use crate::icones;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Le type du part qui porte la carte d'origine (posé par la vue sur le 1er message).
pub const PART_ORIGINE: &str = "chat-origine";

thread_local! {
    /// Les gestes « ouvrir la source » des cartes, par adresse de conversation : le part n'a pas
    /// accès à la feuille de la vue qui l'affiche.
    static OUVREURS: RefCell<HashMap<String, Rc<dyn Fn()>>> = RefCell::new(HashMap::new());
}

/// La carte d'origine : l'en-tête (icône, titre, flèche) du document d'où vient la conversation.
/// ★ L'embed du document (son passage surligné) est à T9 : ici l'en-tête, et la légende est la
///   question qui la suit.
struct CarteVue {
    carte: CarteOrigine,
    cle: String,
}

impl Render for CarteVue {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        let (p, f) = (&theme.palette, &theme.forme);
        let cle = self.cle.clone();
        let manquant = self.carte.manquant;
        let survol = p.background_secondary;
        let mut tete = div()
            .id(ElementId::from(SharedString::from(format!(
                "chat-attachment-header-{}",
                self.cle
            ))))
            .role(AriaRole::Button)
            .aria_label(format!("Ouvrir « {} »", self.carte.titre))
            .flex()
            .items_center()
            .gap(u(cx, f.size_4_2))
            .px(u(cx, f.size_2_5))
            .py(u(cx, 6.0))
            .rounded(u(cx, f.radius_sm))
            .font_family(FAMILLE_INTERFACE)
            .text_size(u(cx, theme.typo.font_ui_medium))
            .text_color(couleur(p.text_normal))
            .child(
                svg()
                    .path(icones::chemin(&self.carte.icone))
                    .size(u(cx, 15.0))
                    .flex_none(),
            )
            .child(div().flex_1().min_w_0().child(self.carte.titre.clone()))
            .child(
                svg()
                    .path(icones::chemin("arrow-up-right"))
                    .size(u(cx, 15.0))
                    .flex_none(),
            );
        tete = if manquant {
            tete.opacity(0.5)
        } else {
            tete.cursor_pointer()
                .hover(move |s| s.bg(couleur(survol)))
                .on_click(move |_, _, _| {
                    let ouvreur = OUVREURS.with(|o| o.borrow().get(&cle).cloned());
                    if let Some(o) = ouvreur {
                        o();
                    }
                })
        };
        div().w_full().child(tete)
    }
}

struct CarteVivante(Entity<CarteVue>);

impl PartVivant for CarteVivante {
    fn vue(&self) -> AnyView {
        self.0.clone().into()
    }
}

/// Le rendu du part [`PART_ORIGINE`] (`chat:part@1`).
pub struct RenduOrigine;

impl RenduDePart for RenduOrigine {
    fn type_part(&self) -> &str {
        PART_ORIGINE
    }

    fn creer(
        &self,
        part: &ChatPart,
        _ctx: &fragment_chat::api::ChatSlotContext,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<Box<dyn PartVivant>> {
        let d = part.data.as_ref()?;
        let carte = CarteOrigine {
            doc: d.get("doc")?.as_str()?.to_owned(),
            titre: d.get("titre")?.as_str()?.to_owned(),
            icone: d.get("icone")?.as_str()?.to_owned(),
            manquant: d.get("manquant")?.as_bool()?,
            lien_seul: d.get("lien_seul")?.as_bool()?,
            passage: None,
        };
        let cle = d.get("cle")?.as_str()?.to_owned();
        Some(Box::new(CarteVivante(cx.new(|_| CarteVue { carte, cle }))))
    }
}

/// Où le plan pose la colonne des messages : `(marge gauche, largeur, padding gauche, padding droit)`.
type PoseColonne = Rc<Cell<Option<(f32, f32, f32, f32)>>>;

/// Le plan du document (`.chat-doc`) : la liste des messages, dans la colonne de la note en page.
/// C'est LUI que le défileur fait défiler.
struct PlanChat {
    liste: Entity<ListeMessages>,
    pose: PoseColonne,
}

impl Render for PlanChat {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.pose.get() {
            // Page : `width: D`, décalée de `marge_gauche`, avec les paddings du texte de la note.
            // La liste porte déjà 14 px de chaque côté : l'enveloppe donne le complément.
            Some((ml, w, pl, pr)) => div()
                .ml(px(ml))
                .w(px(w))
                .pl(px((pl - 14.0).max(0.0)))
                .pr(px((pr - 14.0).max(0.0)))
                .child(self.liste.clone()),
            None => div().w_full().child(self.liste.clone()),
        }
    }
}

/// Le contenu de la vue chat.
pub struct ContenuChat {
    plan: Entity<PlanChat>,
    pose: PoseColonne,
    derniere_pose: Option<(f32, f32, f32, f32)>,
    liens: LiensChat,
    liste: Entity<ListeMessages>,
    defileur: Entity<Defileur>,
    barre: Entity<BarreLaterale>,
    annonceur: Entity<Annonceur>,
    champ: Option<Entity<Champ>>,
    post: Option<(String, Entity<PostVue>)>,
    /// La largeur mesurée de la vue à la dernière image.
    largeur: Rc<Cell<f32>>,
    /// Celle que ce rendu a utilisée : un écart déclenche un nouveau rendu.
    largeur_utilisee: f32,
    revision_vue: u64,
    en_cours: bool,
    lus: SuiviLus,
    nouveaux: usize,
    position_posee_pour: String,
    mode_vu: Option<ChatMode>,
    _abonnements: Vec<Subscription>,
}

impl ContenuChat {
    /// Un contenu pour la vue `liens`.
    pub fn new(liens: LiensChat, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (horloge, taille) = {
            let e = liens.etat.borrow();
            (e.ctx.horloge.borrow().clone(), e.ctx.taille_texte)
        };
        let deps = deps_fil(&liens.etat.borrow());
        let liste = cx.new(|_| ListeMessages::new(deps));
        let pose: PoseColonne = Rc::default();
        let plan = cx.new(|_| PlanChat {
            liste: liste.clone(),
            pose: pose.clone(),
        });
        let defileur = cx.new(|cx| Defileur::new(plan.clone().into(), f64::from(taille), cx));
        let mesure = defileur.read(cx).mesure_colonne();
        liste.update(cx, |l, cx| l.poser_mesure(mesure, cx));
        let barre = cx.new(|cx| BarreLaterale::new(horloge, window, cx));
        let annonceur = cx.new(|cx| Annonceur::new(cx));
        let abonnements = vec![cx.subscribe(&barre, Self::sur_barre)];
        // Le rendu du part de la carte d'origine, une fois pour toutes.
        {
            let e = liens.etat.borrow();
            let mut r = e.ctx.contributions.borrow_mut();
            if !r.parts.iter().any(|p| p.type_part() == PART_ORIGINE) {
                r.parts.push(Rc::new(RenduOrigine));
            }
        }
        Self {
            plan,
            pose,
            derniere_pose: None,
            liens,
            liste,
            defileur,
            barre,
            annonceur,
            champ: None,
            post: None,
            largeur: Rc::new(Cell::new(0.0)),
            largeur_utilisee: -1.0,
            revision_vue: u64::MAX,
            en_cours: false,
            lus: SuiviLus::default(),
            nouveaux: 0,
            position_posee_pour: String::new(),
            mode_vu: None,
            _abonnements: abonnements,
        }
    }

    fn sur_barre(&mut self, _: Entity<BarreLaterale>, ev: &EvenementBarre, cx: &mut Context<Self>) {
        let etat = self.liens.etat.clone();
        match ev {
            EvenementBarre::Selectionner(id) => ouvrir_conversation(&etat, id),
            EvenementBarre::Nouvelle => {
                nouvelle_conversation(&etat);
            }
            EvenementBarre::Replier => etat.borrow_mut().basculer_barre(),
        }
        cx.notify();
    }

    /// La disposition courante, d'après la largeur mesurée.
    #[must_use]
    pub fn disposition(&self) -> Disposition {
        self.liens.etat.borrow().disposition(self.largeur.get())
    }

    /// La largeur mesurée de la vue.
    #[must_use]
    pub fn largeur(&self) -> f32 {
        self.largeur.get()
    }

    /// Le champ, s'il existe.
    #[must_use]
    pub fn champ(&self) -> Option<Entity<Champ>> {
        self.champ.clone()
    }

    /// Le défileur du fil.
    #[must_use]
    pub fn defileur(&self) -> Entity<Defileur> {
        self.defileur.clone()
    }

    /// La liste des messages.
    #[must_use]
    pub fn liste(&self) -> Entity<ListeMessages> {
        self.liste.clone()
    }

    /// La barre des conversations.
    #[must_use]
    pub fn barre_laterale(&self) -> Entity<BarreLaterale> {
        self.barre.clone()
    }

    /// Les entrées de la géométrie de la colonne, à cet instant.
    #[must_use]
    pub fn entrees(&self) -> Entrees {
        let e = self.liens.etat.borrow();
        let dispo_page = e.mode() == ChatMode::Page;
        let largeur = self.largeur.get();
        let barre = if dispo_page && e.disposition(largeur) == Disposition::Cowork {
            e.barre.valeur((e.ctx.ms)())
        } else {
            0.0
        };
        Entrees {
            pane: (largeur - barre).max(0.0),
            barre,
            reserve: e.reserve().0,
            largeur_doc: e
                .ctx
                .ligne_lisible
                .get()
                .then_some(crate::editeur::constantes::LARGEUR_DOC),
        }
    }

    /// La colonne à cet instant.
    #[must_use]
    pub fn colonne(&self) -> Colonne {
        colonne(self.entrees())
    }

    // ── La mise à jour ─────────────────────────────────────────────────────────

    /// Rattrape l'état : actions du menu, gestes du clavier, session, structure. Appelé à chaque
    /// rendu et par le shell à chaque image.
    pub fn pomper(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.jouer_actions(cx);
        self.jouer_touches(cx);
        // Le mode ou la conversation ont changé : la structure se reconstruit.
        let (revision, mode) = {
            let e = self.liens.etat.borrow();
            (e.revision, e.mode())
        };
        if revision != self.revision_vue || self.mode_vu != Some(mode) {
            self.revision_vue = revision;
            self.mode_vu = Some(mode);
            self.reconstruire(window, cx);
        }
        // La marge de lecture replie (ou rouvre) la barre.
        {
            let mut e = self.liens.etat.borrow_mut();
            let marge = e.marge_ouverte();
            e.suivre_marge(marge);
        }
        self.rafraichir(window, cx);
        self.demandes(window, cx);
    }

    fn jouer_actions(&mut self, cx: &mut Context<Self>) {
        let actions: Vec<ActionChat> = self
            .liens
            .etat
            .borrow()
            .actions
            .borrow_mut()
            .drain(..)
            .collect();
        for a in actions {
            match a {
                ActionChat::Copier(t) => cx.write_to_clipboard(ClipboardItem::new_string(t)),
                ActionChat::Telecharger { nom, contenu } => {
                    let tel = self.liens.etat.borrow().ctx.telecharger.clone();
                    tel(&nom, &contenu);
                }
                ActionChat::Supprimer => {
                    supprimer_conversation(&self.liens.etat);
                }
            }
        }
    }

    fn jouer_touches(&mut self, cx: &mut Context<Self>) {
        let touches: Vec<ToucheChat> = self.liens.touches.borrow_mut().drain(..).collect();
        for t in touches {
            self.defileur.update(cx, |d, cx| match t {
                ToucheChat::TourPrecedent => {
                    d.tour_voisin(Sens::Precedent, cx);
                }
                ToucheChat::TourSuivant => {
                    d.tour_voisin(Sens::Suivant, cx);
                }
                ToucheChat::PlusRecent => d.aller_au_plus_recent(cx),
            });
        }
    }

    /// (Re)construit ce qui dépend du mode et de la conversation : le champ, les dépendances du fil.
    fn reconstruire(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Une reconstruction ne perd ni la saisie ni les états des extensions du champ.
        if let Some(champ) = self.champ.take() {
            let (texte, etats) = {
                let c = champ.read(cx);
                (c.valeur(cx), c.sauver())
            };
            let mut e = self.liens.etat.borrow_mut();
            if !texte.is_empty() && e.brouillon.is_none() {
                e.brouillon = Some(texte);
            }
            for (k, v) in etats {
                e.etats_champ.insert(k, v);
            }
        }
        self.post = None;
        let deps = deps_fil(&self.liens.etat.borrow());
        self.liste.update(cx, |l, cx| l.poser_deps(deps, cx));
        let (mode, avec_champ) = {
            let e = self.liens.etat.borrow();
            let m = e.mode();
            (m, m != ChatMode::Embedded && e.session.is_some())
        };
        if avec_champ {
            self.creer_champ(mode, window, cx);
        }
        // Les ouvreurs des cartes d'origine de cette conversation.
        let cle = self.liens.etat.borrow().document();
        if let Some(cle) = cle {
            let etat = self.liens.etat.clone();
            let ouvreur: Rc<dyn Fn()> = Rc::new(move || {
                let carte = etat.borrow().carte_origine();
                if let Some(c) = carte {
                    ouvrir_origine(&etat, &c);
                }
            });
            OUVREURS.with(|o| o.borrow_mut().insert(cle, ouvreur));
        }
        if let Some(s) = self.liens.etat.borrow().session.clone() {
            s.borrow_mut().ensure_loaded();
        }
        self.liens.etat.borrow().sale.set(true);
        cx.notify();
    }

    fn creer_champ(&mut self, mode: ChatMode, window: &mut Window, cx: &mut Context<Self>) {
        let deps = {
            let e = self.liens.etat.borrow();
            let Some(p) = e.fournisseur_actif() else {
                return;
            };
            DepsChamp {
                placeholder: match mode {
                    ChatMode::Anchored => "Question de suivi",
                    ChatMode::Bar => "Demander à l’agent…",
                    _ => "Comment puis-je t’aider ?",
                }
                .to_owned(),
                contexte: fragment_chat::api::ChatSlotContext {
                    provider: e.fournisseur.clone(),
                    conversation: e.conversation.clone(),
                    mode,
                    message: None,
                    streaming: false,
                },
                registre: e.ctx.contributions.clone(),
                sans: p.without().to_vec(),
                etat_initial: e.etats_champ.clone(),
            }
        };
        let champ = cx.new(|cx| Champ::new(deps, window, cx));
        self._abonnements
            .push(cx.subscribe_in(&champ, window, Self::sur_champ));
        self.champ = Some(champ);
    }

    fn sur_champ(
        &mut self,
        _: &Entity<Champ>,
        ev: &EvenementChamp,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ev {
            EvenementChamp::Envoyer(envoi) => {
                // Une INTENTION : la question va monter en haut, la réponse grandir dessous.
                if self.liens.etat.borrow().mode() != ChatMode::Bar {
                    self.defileur.update(cx, |d, cx| d.envoye(cx));
                }
                self.liens.etat.borrow_mut().envoyer(
                    &envoi.texte,
                    envoi.opts.clone(),
                    envoi.parts.clone(),
                );
                self.liens.etat.borrow().sale.set(true);
            }
            EvenementChamp::Arreter => {
                if let Some(s) = self.liens.etat.borrow().session.clone() {
                    s.borrow().stop();
                }
            }
            EvenementChamp::Echap => self.liens.etat.borrow_mut().echappe = true,
        }
        cx.notify();
    }

    /// Les demandes de l'ouverture : brouillon, envoi, états des extensions, focus, message d'un lien.
    fn demandes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(champ) = self.champ.clone() else {
            return;
        };
        let (brouillon, envoi, etats, focus) = {
            let mut e = self.liens.etat.borrow_mut();
            (
                e.brouillon.take(),
                e.envoi_demande.take(),
                std::mem::take(&mut e.etats_champ),
                std::mem::take(&mut e.focus_demande),
            )
        };
        if let Some((texte, opts, parts)) = envoi {
            self.defileur.update(cx, |d, cx| d.envoye(cx));
            self.liens.etat.borrow_mut().envoyer(&texte, opts, parts);
            self.liens.etat.borrow().sale.set(true);
        }
        if let Some(b) = brouillon {
            champ.update(cx, |c, cx| c.definir_valeur(&b, window, cx));
        }
        if !etats.is_empty() {
            champ.update(cx, |c, _| c.restaurer(&etats));
        }
        if focus {
            champ.update(cx, |c, cx| c.focus(window, cx));
        }
    }

    /// Met à jour le fil, le titre, l'accueil, la pastille ; place la conversation à l'ouverture.
    fn rafraichir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (session, sale) = {
            let e = self.liens.etat.borrow();
            (e.session.clone(), e.sale.replace(false))
        };
        let Some(session) = session else { return };
        if let Some(champ) = &self.champ {
            let en_cours = session.borrow().running;
            champ.update(cx, |c, cx| c.set_running(en_cours, cx));
        }
        if sale {
            // La barre des conversations suit l'historique du fournisseur (par signature).
            let (p, conv) = {
                let e = self.liens.etat.borrow();
                (e.fournisseur_actif(), e.conversation.clone())
            };
            if let Some(p) = p {
                self.barre
                    .update(cx, |b, cx| b.suivre(p.as_ref(), &conv, cx));
            }
            let (messages, streaming, ids_questions, mode) = {
                let e = self.liens.etat.borrow();
                let s = session.borrow();
                let moi = e
                    .fournisseur_actif()
                    .and_then(|p| {
                        p.participants(&e.conversation)
                            .into_iter()
                            .find(|x| x.role == Role::User)
                            .map(|x| x.id)
                    })
                    .unwrap_or_default();
                let carte = e.carte_origine();
                let premier = e.premier_message_utilisateur();
                let messages: Vec<LiveMessage> = s
                    .messages
                    .iter()
                    .map(|m| avec_carte(m, carte.as_ref(), premier.as_deref(), &e))
                    .collect();
                let ids: Vec<(String, bool)> = s
                    .messages
                    .iter()
                    .map(|m| (m.id.clone(), m.author == moi))
                    .collect();
                (messages, s.running, ids, e.mode())
            };
            if mode != ChatMode::Embedded
                || self.liens.etat.borrow().presentation == Presentation::Fil
            {
                self.liste
                    .update(cx, |l, cx| l.mettre_a_jour(&messages, window, cx));
                let reserve = matches!(mode, ChatMode::Page | ChatMode::Panel);
                self.defileur.update(cx, |d, cx| {
                    d.poser_fil(ids_questions, streaming, reserve, cx)
                });
            } else {
                self.majorer_post(window, cx);
            }
            // Début et fin d'un tour : l'annonceur, le défileur.
            let en_cours = session.borrow().running;
            if en_cours && !self.en_cours {
                let qui = self.nom_assistant();
                self.annonceur
                    .update(cx, |a, cx| a.dire(&format!("{qui} répond"), cx));
            } else if !en_cours && self.en_cours {
                self.defileur.update(cx, |d, cx| d.tour_fini(cx));
                let erreur = session
                    .borrow()
                    .messages
                    .last()
                    .and_then(|m| m.error.clone());
                let texte = erreur.map_or_else(
                    || "Réponse terminée".to_owned(),
                    |e| format!("Erreur : {e}"),
                );
                self.annonceur.update(cx, |a, cx| a.dire(&texte, cx));
            }
            self.en_cours = en_cours;
            self.position_initiale(&session, cx);
            self.majorer_lus(&session, cx);
            cx.notify();
        } else {
            self.majorer_lus(&session, cx);
        }
    }

    fn nom_assistant(&self) -> String {
        let e = self.liens.etat.borrow();
        e.fournisseur_actif()
            .and_then(|p| {
                p.participants(&e.conversation)
                    .into_iter()
                    .find(|x| x.role == Role::Assistant)
                    .map(|x| x.name)
            })
            .unwrap_or_else(|| "L’assistant".to_owned())
    }

    /// La vue s'ouvre sur la DERNIÈRE QUESTION en haut (pas le bas absolu) — sauf position demandée.
    fn position_initiale(
        &mut self,
        session: &Rc<RefCell<fragment_chat::chat_session::ChatSession>>,
        cx: &mut Context<Self>,
    ) {
        let (mode, cle, demande, posee) = {
            let e = self.liens.etat.borrow();
            (
                e.mode(),
                e.document().unwrap_or_default(),
                e.message_demande,
                e.position_posee,
            )
        };
        if matches!(mode, ChatMode::Embedded | ChatMode::Anchored) || !session.borrow().loaded {
            return;
        }
        let ids: Vec<String> = session
            .borrow()
            .messages
            .iter()
            .map(|m| m.id.clone())
            .collect();
        // Un lien de message (`#m12`) : ce message en haut.
        if let Some(rang) = demande {
            if let Some(id) = ids.get(rang) {
                self.defileur.update(cx, |d, cx| d.reveler(id, cx));
                self.liens.etat.borrow_mut().message_demande = None;
            }
            return;
        }
        if posee || self.position_posee_pour == cle {
            return;
        }
        self.position_posee_pour = cle;
        let moi = self
            .liens
            .etat
            .borrow()
            .fournisseur_actif()
            .and_then(|p| {
                let e = self.liens.etat.borrow();
                p.participants(&e.conversation)
                    .into_iter()
                    .find(|x| x.role == Role::User)
                    .map(|x| x.id)
            })
            .unwrap_or_default();
        let derniere = session
            .borrow()
            .messages
            .iter()
            .rev()
            .find(|m| m.author == moi)
            .map(|m| m.id.clone());
        if let Some(id) = derniere {
            self.defileur
                .update(cx, |d, cx| d.reveler_derniere(&id, cx));
        }
    }

    /// Les non-lus de la pastille : lus quand le bas est resté visible, ou suivis.
    fn majorer_lus(
        &mut self,
        session: &Rc<RefCell<fragment_chat::chat_session::ChatSession>>,
        cx: &mut Context<Self>,
    ) {
        let maintenant = (self.liens.etat.borrow().ctx.ms)();
        let (suivi, bas) = {
            let d = self.defileur.read(cx);
            (
                d.mode() != fragment_chat::follow_model::FollowMode::Free,
                d.bas_visible(),
            )
        };
        let (n, moi_flux): (usize, Vec<(bool, bool)>) = {
            let e = self.liens.etat.borrow();
            let s = session.borrow();
            let moi = e
                .fournisseur_actif()
                .and_then(|p| {
                    p.participants(&e.conversation)
                        .into_iter()
                        .find(|x| x.role == Role::User)
                        .map(|x| x.id)
                })
                .unwrap_or_default();
            (
                s.messages.len(),
                s.messages
                    .iter()
                    .map(|m| (m.author == moi, m.streaming))
                    .collect(),
            )
        };
        let lus = self.lus.observer(n, suivi, bas, true, maintenant);
        self.nouveaux = crate::chat_defilement::pastille::nouveaux(&moi_flux, lus);
    }

    /// Le POST (présentation `post`, vue embarquée) : redessiné seulement si ce qu'il montre change.
    fn majorer_post(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (modele, origine, signature, deps) = {
            let e = self.liens.etat.borrow();
            let (Some(p), Some(s)) = (e.fournisseur_actif(), e.session.clone()) else {
                return;
            };
            let s = s.borrow();
            let msgs: Vec<_> = s.messages.iter().map(|m| m.message.clone()).collect();
            let modele = post_model(&msgs, &p.participants(&e.conversation));
            let origine = e.origine().map(|o| Origine {
                titre: e.ctx.documents.title(&o.doc),
                icone: e
                    .ctx
                    .documents
                    .icon(&o.doc)
                    .unwrap_or_else(|| "file-text".to_owned()),
                citation: o.quote,
            });
            let sig = format!("{modele:?}|{origine:?}|{}", s.loaded);
            let h = e.ctx.horloge.borrow();
            let deps = DepsPost {
                id: SharedString::from(format!("chat-post-{}", e.conversation)),
                origine: origine.clone(),
                maintenant: h.maintenant(),
                fuseau: Rc::new(fragment_chat::calendrier::DecalageFixe(0)),
                fond: theme_ou_defaut(cx).palette.workspace_bg,
            };
            (modele, origine, sig, deps)
        };
        let _ = origine;
        match &mut self.post {
            Some((sig, vue)) if *sig == signature => {
                let _ = vue;
            }
            Some((sig, vue)) => {
                *sig = signature;
                vue.update(cx, |v, cx| v.poser_modele(modele, window, cx));
            }
            None => {
                let vue = cx.new(|cx| PostVue::new(modele, deps, Rappels::default(), window, cx));
                self.post = Some((signature, vue));
            }
        }
    }
}

/// Les dépendances du fil pour l'état courant.
fn deps_fil(e: &EtatChat) -> DepsFil {
    DepsFil {
        participants: e
            .fournisseur_actif()
            .map(|p| p.participants(&e.conversation))
            .unwrap_or_default(),
        registre: e.ctx.contributions.clone(),
        fournisseur: e.fournisseur.clone(),
        conversation: e.conversation.clone(),
        mode: e.mode(),
        taille_texte: e.ctx.taille_texte,
    }
}

/// Le message, avec la carte d'origine devant son premier part s'il porte la source.
fn avec_carte(
    m: &LiveMessage,
    carte: Option<&CarteOrigine>,
    premier: Option<&str>,
    e: &EtatChat,
) -> LiveMessage {
    let (Some(c), Some(id)) = (carte, premier) else {
        return m.clone();
    };
    if m.id != id {
        return m.clone();
    }
    let mut m = m.clone();
    m.message.parts.insert(
        0,
        ChatPart {
            kind: PART_ORIGINE.to_owned(),
            id: Some("origine".to_owned()),
            text: None,
            data: Some(json!({
                "doc": c.doc, "titre": c.titre, "icone": c.icone, "manquant": c.manquant,
                "lien_seul": c.lien_seul,
                "cle": e.document().unwrap_or_default(),
            })),
        },
    );
    m
}

impl Render for ContenuChat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.pomper(window, cx);
        let theme = theme_ou_defaut(cx);
        let (p, f) = (&theme.palette, &theme.forme);

        let (mode, disposition, historique, vide, titre, barre_ouverte, anime, reserve_anime) = {
            let e = self.liens.etat.borrow();
            let mode = e.mode();
            let disposition = e.disposition(self.largeur.get());
            let ms = (e.ctx.ms)();
            let (_, reserve_anime) = e.reserve();
            let session = e.session.clone();
            let vide = session.as_ref().is_some_and(|s| {
                let s = s.borrow();
                s.loaded && s.messages.is_empty() && !s.running
            });
            (
                mode,
                disposition,
                e.historique && disposition == Disposition::Colonne,
                vide,
                e.titre(),
                e.barre_ouverte,
                e.barre.en_cours(ms),
                reserve_anime,
            )
        };
        self.largeur_utilisee = self.largeur.get();
        if anime || reserve_anime {
            window.request_animation_frame();
        }
        let col = self.colonne();
        let pose = (mode == ChatMode::Page
            && !(vide && !matches!(mode, ChatMode::Anchored | ChatMode::Bar)))
        .then_some((col.marge_gauche, col.largeur, DEBUT_TEXTE, FIN_TEXTE));
        if pose != self.derniere_pose {
            self.derniere_pose = pose;
            self.pose.set(pose);
            self.plan.update(cx, |_, cx| cx.notify());
        }
        let compact = matches!(mode, ChatMode::Anchored | ChatMode::Bar);
        let accueil_visible = vide && !compact;
        let page = mode == ChatMode::Page;
        let barre_px = col.onglet - self.entrees().pane;

        // ── L'en-tête (page, panneau) ──
        let en_tete = (!compact && mode != ChatMode::Embedded).then(|| {
            let (a_liste, a_creer) = {
                let e = self.liens.etat.borrow();
                let p = e.fournisseur_actif();
                (p.is_some(), p.is_some())
            };
            let mut actions = div().flex().items_center().gap(u(cx, f.size_2_1));
            {
                let e = self.liens.etat.borrow();
                let slots = crate::chat_fil::registre::slots_applicables(
                    &e.ctx.en_tete.borrow(),
                    &fragment_chat::api::ChatSlotContext {
                        provider: e.fournisseur.clone(),
                        conversation: e.conversation.clone(),
                        mode,
                        message: None,
                        streaming: false,
                    },
                );
                let ctx = fragment_chat::api::ChatSlotContext {
                    provider: e.fournisseur.clone(),
                    conversation: e.conversation.clone(),
                    mode,
                    message: None,
                    streaming: false,
                };
                for s in &slots {
                    actions = actions.child((s.rendre)(&ctx, window, cx));
                }
            }
            if a_liste && disposition == Disposition::Colonne {
                let actif = self.liens.etat.borrow().historique;
                let b = bouton_icone(
                    "chat-historique",
                    "history",
                    "Historique",
                    Variante::Discret,
                    true,
                    &theme,
                    cx,
                );
                let b = if actif {
                    b.bg(couleur(p.background_secondary))
                        .text_color(couleur(p.text_normal))
                } else {
                    b
                };
                actions = actions.child(b.on_click(cx.listener(|this, _, _, cx| {
                    {
                        let mut e = this.liens.etat.borrow_mut();
                        e.historique = !e.historique;
                    }
                    cx.notify();
                })));
            }
            if a_creer {
                actions = actions.child(
                    bouton_icone(
                        "chat-nouvelle",
                        "square-pen",
                        "Nouvelle conversation",
                        Variante::Discret,
                        true,
                        &theme,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        nouvelle_conversation(&this.liens.etat);
                        cx.notify();
                    })),
                );
            }
            let mut tete = div()
                .relative()
                .flex()
                .items_center()
                .gap(u(cx, f.size_4_2))
                .min_h(u(cx, 40.0))
                .py(u(cx, f.size_2_3))
                .pr(u(cx, f.size_4_2))
                .border_b(px(f.border_width))
                .border_color(if disposition == Disposition::Cowork {
                    gpui_kit::transparent_black()
                } else {
                    couleur(p.tab_outline_color).into()
                })
                .font_family(FAMILLE_INTERFACE)
                .text_color(couleur(p.text_normal));
            // Page : le titre est au x du texte de la colonne ; sinon la marge d'une bulle.
            tete = if page {
                tete.pl(px(0.0))
            } else {
                tete.pl(u(cx, f.size_2_7))
            };
            if disposition == Disposition::Cowork {
                // Présent dans les deux états (hors flux, visible barre repliée) : rien ne bouge.
                let visible = !barre_ouverte;
                let b = bouton_icone(
                    "chat-header-toggle",
                    "panel-left",
                    "Afficher la barre latérale",
                    Variante::Discret,
                    visible,
                    &theme,
                    cx,
                );
                let b = if visible {
                    b.on_click(cx.listener(|this, _, _, cx| {
                        this.liens.etat.borrow_mut().basculer_barre();
                        cx.notify();
                    }))
                } else {
                    b.invisible()
                };
                tete = tete.child(div().absolute().left(u(cx, 8.0)).top(u(cx, 5.0)).child(b));
            }
            let mut titre_el = div()
                .id("chat-titre")
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(u(cx, theme.typo.font_ui_medium))
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .child(titre.clone());
            if page {
                titre_el = titre_el.ml(px(col.texte_x));
            }
            if accueil_visible {
                titre_el = titre_el.invisible();
            }
            tete.child(titre_el).child(actions)
        });

        // ── Le corps : l'historique en liste, l'accueil, ou le fil ──
        let corps = if mode == ChatMode::Embedded
            && self.liens.etat.borrow().presentation == Presentation::Post
        {
            div()
                .flex_1()
                .min_h_0()
                .children(self.post.as_ref().map(|(_, v)| v.clone()))
                .into_any_element()
        } else if historique {
            self.historique_liste(&theme, cx).into_any_element()
        } else {
            self.fil(&theme, &col, mode, compact, accueil_visible, vide, cx)
                .into_any_element()
        };

        // ── Le champ ──
        let champ = (mode != ChatMode::Embedded)
            .then(|| self.champ.clone())
            .flatten()
            .map(|c| {
                // ★ Le champ porte ses propres 12 px de marge (thread.css) : en page il se pose aux
                //   bords du TEXTE, donc l'enveloppe les reprend à la colonne (marges négatives).
                if page {
                    div()
                        .flex_none()
                        .ml(px(col.texte_x - 12.0))
                        .w(px(col.texte_largeur + 24.0))
                        .child(c)
                } else {
                    div().flex_none().w_full().child(c)
                }
            });
        let pastille = (!compact && mode != ChatMode::Embedded && !accueil_visible).then(|| {
            let etat = self.defileur.read(cx).etat_pastille(self.nouveaux);
            let defileur = self.defileur.clone();
            pastille(
                etat,
                move |_, cx| defileur.update(cx, |d, cx| d.aller_au_plus_recent(cx)),
                cx,
            )
        });

        let largeur = self.largeur.clone();
        let mut pane = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full();
        if let Some(t) = en_tete {
            pane = pane.child(t);
        }
        pane = pane.child(corps);
        if let Some(pa) = pastille {
            pane = pane.child(pa);
        }
        if let Some(c) = champ {
            pane = pane.child(c);
        }
        pane = pane.child(self.annonceur.clone());

        let mut racine = div()
            .id("chat-vue")
            .role(AriaRole::Group)
            .relative()
            .flex()
            .size_full()
            .min_h_0()
            .font_family(FAMILLE_INTERFACE)
            .bg(couleur(p.workspace_bg))
            .text_color(couleur(p.text_normal))
            .child(
                // La mesure de la vue : sa boîte, relevée avant le dessin.
                canvas(
                    move |b, _, _| largeur.set(f32::from(b.size.width)),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );
        if disposition == Disposition::Cowork {
            // TOUJOURS montée en cowork : repliée, sa cellule a une largeur nulle (animée) et le
            // contenu, qui garde ses 260 px, est rogné.
            racine = racine.child(
                div()
                    .id("chat-sidebar-zone")
                    .flex_none()
                    .overflow_hidden()
                    .h_full()
                    .w(px(barre_px.clamp(0.0, LARGEUR_BARRE_OUVERTE)))
                    .child(
                        div()
                            .w(px(LARGEUR_BARRE_OUVERTE))
                            .h_full()
                            .child(self.barre.clone()),
                    ),
            );
        }
        let fin = cx.entity();
        let mesure = self.largeur.clone();
        let utilisee = self.largeur_utilisee;
        window.on_next_frame(move |_, cx| {
            if (mesure.get() - utilisee).abs() > 0.5 {
                fin.update(cx, |_, cx| cx.notify());
            }
        });
        racine.child(pane)
    }
}

impl ContenuChat {
    /// L'historique en liste (colonne) : un bouton par conversation.
    fn historique_liste(
        &self,
        theme: &fragment_theme::Theme,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        let (p, f) = (&theme.palette, &theme.forme);
        let (items, actif) = {
            let e = self.liens.etat.borrow();
            (
                e.fournisseur_actif().map(|p| p.list()).unwrap_or_default(),
                e.conversation.clone(),
            )
        };
        let mut liste = div()
            .id("chat-history")
            .role(AriaRole::List)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(u(cx, f.size_2_3))
            .text_size(u(cx, theme.typo.font_ui_medium));
        if items.is_empty() {
            liste = liste.child("Aucune conversation.");
        }
        for c in items {
            let id = c.id.clone();
            let est_active = c.id == actif;
            let titre = if c.title.is_empty() {
                "Sans titre".to_owned()
            } else {
                c.title.clone()
            };
            let survol = p.background_secondary;
            let mut ligne = div()
                .id(ElementId::from(SharedString::from(format!(
                    "chat-history-{}",
                    c.id
                ))))
                .role(AriaRole::ListItem)
                .w_full()
                .py(px(7.0))
                .px(u(cx, f.size_2_5))
                .rounded(u(cx, f.radius_sm))
                .cursor_pointer()
                .child(titre)
                .on_click(cx.listener(move |this, _, _, cx| {
                    {
                        let mut e = this.liens.etat.borrow_mut();
                        e.historique = false;
                    }
                    if id == this.liens.etat.borrow().conversation {
                        cx.notify();
                    } else {
                        ouvrir_conversation(&this.liens.etat, &id);
                        cx.notify();
                    }
                }));
            ligne = if est_active {
                ligne.bg(couleur(survol))
            } else {
                ligne.hover(move |s| s.bg(couleur(survol)))
            };
            liste = liste.child(ligne);
        }
        liste
    }

    /// Le fil (ou l'accueil) : le plan défilant, avec sa colonne aux bords de la note en page.
    #[allow(clippy::too_many_arguments)]
    fn fil(
        &self,
        theme: &fragment_theme::Theme,
        col: &Colonne,
        mode: ChatMode,
        compact: bool,
        accueil_visible: bool,
        vide: bool,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Div {
        let page = mode == ChatMode::Page;
        if accueil_visible {
            // L'accueil : la salutation et le champ restent au milieu de la hauteur (le champ est
            // posé après, marges automatiques de part et d'autre du groupe).
            let (nom, icone) = {
                let e = self.liens.etat.borrow();
                let nom = e
                    .fournisseur_actif()
                    .and_then(|p| {
                        p.participants(&e.conversation)
                            .into_iter()
                            .find(|x| x.role == Role::User)
                            .map(|x| x.name)
                    })
                    .unwrap_or_default();
                (
                    nom,
                    e.fournisseur_actif()
                        .and_then(|p| p.icon().map(str::to_owned)),
                )
            };
            let horloge = self.liens.etat.borrow().ctx.horloge.borrow().clone();
            return div().flex_none().mt_auto().child(accueil(
                &nom,
                &horloge,
                icone.as_deref().or(Some("sparkles")),
                page,
                theme,
                cx,
            ));
        }
        if compact && vide {
            return div()
                .flex_none()
                .py(u(cx, theme.forme.size_4_6))
                .px(u(cx, theme.forme.size_4_4))
                .text_color(couleur(theme.palette.text_muted))
                .text_size(u(cx, theme.typo.font_ui_medium))
                .child("Pose ta question.");
        }
        // La colonne (marges, largeur, paddings de la note) est posée par le plan, DANS le défileur.
        div()
            .flex_1()
            .min_h_0()
            .pr(px(if page { col.reserve_px } else { 0.0 }))
            .child(self.defileur.clone())
    }
}
