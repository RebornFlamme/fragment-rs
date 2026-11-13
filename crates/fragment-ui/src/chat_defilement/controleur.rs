// Port de app/src/ui/views/chat/scroll/ScrollController.ts (la logique, sans le DOM).
//! Le contrôleur du défilement d'une conversation, PUR : il reçoit des [`Mesures`] (positions des
//! messages en px du contenu défilant) et rend les `scroll_top` à écrire. Le [`super::Defileur`]
//! fait les mesures dans GPUI et applique.
//!
//! Il n'écrit la position que pour trois raisons : l'utilisateur vient d'ENVOYER (placer sa
//! question), il a CHOISI le bord vivant (`live`), ou il DEMANDE un endroit (rail, lien,
//! recherche, Alt+↑/↓, position rouverte). Jamais pendant une lecture libre pour « suivre », jamais
//! à l'arrêt ni à l'erreur d'un tour.
//!
//! ★ GPUI n'a pas l'ancrage natif de Chromium (`overflow-anchor`) : quand un contenu grandit
//! AU-DESSUS de ce qu'on lit (image chargée, message reconstruit, historique préfixé), rien ne
//! garde la place. La passe en mode `free` rejoue donc l'ancre de lecture à chaque image : l'ancre
//! se calcule sur les boîtes de la passe PRÉCÉDENTE et la position COURANTE (un défilement de
//! l'utilisateur entre deux passes est pris en compte), puis se rend sur les boîtes nouvelles. Comme
//! Chromium, rien n'est ancré quand on est tout en haut.

use fragment_chat::follow_model::{
    FollowEvent, FollowMode, MessageBox, ReadingAnchor, Sens, anchor_target, neighbour_turn,
    next_mode, reading_anchor, reserve_height, turn_target,
};

/// Le délai pendant lequel un défilement qui n'est pas le nôtre est attribué au dernier geste (ms).
pub const FENETRE_INTENTION_MS: f64 = 1000.0;

/// Ce que `jump` laisse sous le bas du contenu (px) : le bas ne colle pas au bord.
pub const MARGE_BAS: f64 = 16.0;

/// Le contexte gardé au-dessus d'une question placée : ≈ 2 lignes du tour précédent
/// (`taille × 1,55 × 2`, arrondi comme `Math.round`).
#[must_use]
pub fn contexte_px(taille_police: f64) -> f64 {
    let fs = if taille_police > 0.0 {
        taille_police
    } else {
        16.0
    };
    (fs * 1.55 * 2.0 + 0.5).floor()
}

/// Un message tel que mesuré : son id, s'il est une question (`data-role="user"`), son haut et son
/// bas en px du contenu défilant.
#[derive(Clone, Debug, PartialEq)]
pub struct Boite {
    pub id: String,
    pub question: bool,
    pub top: f64,
    pub bottom: f64,
}

/// Une passe de mesure : la position, la hauteur visible, le bas du contenu (la colonne, SANS la
/// réserve) et les messages, en px du contenu défilant.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesures {
    pub scroll_top: f64,
    pub visible: f64,
    pub bas_contenu: f64,
    pub boites: Vec<Boite>,
}

impl Mesures {
    fn boites_modele(&self) -> Vec<MessageBox> {
        boites_modele(&self.boites)
    }

    fn questions(&self) -> impl Iterator<Item = &Boite> {
        self.boites.iter().filter(|b| b.question)
    }
}

fn boites_modele(b: &[Boite]) -> Vec<MessageBox> {
    b.iter()
        .map(|x| MessageBox {
            id: x.id.clone(),
            top: x.top,
            bottom: x.bottom,
        })
        .collect()
}

/// L'état de l'hôte à une passe : une réponse s'écrit-elle, la réserve est-elle permise (défileur
/// de hauteur FIXE : onglet, panneau).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hote {
    pub streaming: bool,
    pub reserve: bool,
}

/// La position de la vue (`lirePosition`) : le RANG du premier message visible et l'écart de son
/// haut au haut de l'écran — indépendante de la largeur et d'un historique relu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub message: usize,
    pub dy: f64,
}

/// La question en cours de placement après un envoi.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Tour {
    Aucun,
    /// Envoyée, pas encore affichée.
    Attendu,
    Place(String),
}

/// Ce qu'une passe demande : la hauteur de la réserve et, peut-être, une position à écrire.
#[derive(Clone, Debug, PartialEq)]
pub struct Passe {
    pub reserve: f64,
    pub ecrire: Option<f64>,
}

/// Le contrôleur (un par vue chat).
#[derive(Clone, Debug)]
pub struct ControleurDefilement {
    mode: FollowMode,
    tour: Tour,
    /// Le nombre de questions au moment de l'envoi : la nouvelle est celle qui le dépasse.
    avant_envoi: usize,
    /// La réserve suit la dernière question (après un envoi, ou une ouverture sur elle).
    avec_reserve: bool,
    reserve: f64,
    derniere_intention: f64,
    contexte: f64,
    /// La position et les boîtes à la fin de la passe précédente (ou après notre écriture).
    precedent: Option<(f64, Vec<Boite>)>,
    /// Le mode a changé depuis la dernière lecture (la pastille, le rail le relisent).
    change: bool,
}

impl Default for ControleurDefilement {
    fn default() -> Self {
        Self::new(contexte_px(16.0))
    }
}

impl ControleurDefilement {
    /// Un contrôleur `free`, sans réserve ; `contexte` : [`contexte_px`] de la colonne.
    #[must_use]
    pub fn new(contexte: f64) -> Self {
        ControleurDefilement {
            mode: FollowMode::Free,
            tour: Tour::Aucun,
            avant_envoi: 0,
            avec_reserve: false,
            reserve: 0.0,
            derniere_intention: f64::NEG_INFINITY,
            contexte,
            precedent: None,
            change: false,
        }
    }

    #[must_use]
    pub fn mode(&self) -> FollowMode {
        self.mode
    }

    #[must_use]
    pub fn contexte(&self) -> f64 {
        self.contexte
    }

    /// La taille du texte de la colonne a changé.
    pub fn poser_contexte(&mut self, contexte: f64) {
        self.contexte = contexte;
    }

    /// La hauteur courante de la réserve (`.chat-tail`).
    #[must_use]
    pub fn reserve(&self) -> f64 {
        self.reserve
    }

    /// Vrai une fois après chaque changement de mode.
    pub fn prendre_changement(&mut self) -> bool {
        std::mem::take(&mut self.change)
    }

    fn emettre(&mut self, e: FollowEvent) {
        let m = next_mode(self.mode, e);
        if m == self.mode {
            return;
        }
        self.mode = m;
        if m != FollowMode::Turn {
            self.tour = Tour::Aucun;
        }
        self.change = true;
    }

    /// Un geste de l'utilisateur (molette, touches, barre, sélection, lien, focus, Ctrl+F) : la
    /// lecture est à lui. `maintenant` en ms.
    pub fn intention(&mut self, maintenant: f64) {
        self.derniere_intention = maintenant;
        self.emettre(FollowEvent::Intent);
    }

    /// Le bas du contenu (la colonne, sans la réserve) est-il à l'écran ?
    #[must_use]
    pub fn bas_visible(m: &Mesures) -> bool {
        m.bas_contenu <= m.scroll_top + m.visible + 4.0
    }

    /// La position la plus basse possible, réserve comprise.
    fn max(&self, m: &Mesures) -> f64 {
        (m.bas_contenu + self.reserve - m.visible).max(0.0)
    }

    /// Borne et arrondit une cible ; `None` si on y est déjà (à moins d'un pixel).
    fn poser(&mut self, cible: f64, m: &Mesures) -> Option<f64> {
        let t = (cible + 0.5)
            .floor()
            .clamp(0.0, (self.max(m) + 0.5).floor());
        if (m.scroll_top - t).abs() < 1.0 {
            return None;
        }
        self.precedent = Some((t, m.boites.clone()));
        Some(t)
    }

    /// L'utilisateur vient d'envoyer : sa question (la prochaine question affichée) montera en haut.
    pub fn envoye(&mut self, m: &Mesures) {
        self.avant_envoi = m.questions().count();
        self.emettre(FollowEvent::Send);
        self.tour = Tour::Attendu;
        self.change = true;
    }

    /// Le tour est fini (réponse, arrêt, erreur) : AUCUN déplacement.
    pub fn tour_fini(&mut self) {
        self.emettre(FollowEvent::TurnEnd);
    }

    /// Aller au plus récent (pastille, Fin) : la cible, puis on suit.
    pub fn aller_au_plus_recent(&mut self, m: &Mesures) -> Option<f64> {
        self.emettre(FollowEvent::Jump);
        self.poser(m.bas_contenu - m.visible + MARGE_BAS, m)
    }

    /// La réserve que demande le dernier tour (sans rien écrire).
    fn calculer_reserve(&mut self, m: &Mesures, hote: Hote) -> Option<Boite> {
        let derniere = m.questions().last().cloned();
        if self.mode == FollowMode::Turn
            && self.tour == Tour::Attendu
            && m.questions().count() > self.avant_envoi
            && let Some(d) = &derniere
        {
            self.tour = Tour::Place(d.id.clone());
        }
        let tour_box = match &self.tour {
            Tour::Place(id) => m.boites.iter().find(|b| &b.id == id).cloned(),
            _ => None,
        };
        if tour_box.is_some() {
            self.avec_reserve = true;
        }
        let base = tour_box
            .clone()
            .or(if self.avec_reserve { derniere } else { None });
        if hote.reserve {
            if let Some(b) = &base {
                let h = reserve_height(m.visible, self.contexte, m.bas_contenu - b.top);
                if (self.reserve - h).abs() > 1.0 {
                    self.reserve = h;
                }
            }
        } else {
            self.reserve = 0.0;
        }
        tour_box
    }

    /// Après chaque mise à jour du fil ou défilement (une fois par image au plus).
    pub fn passe(&mut self, m: &Mesures, hote: Hote, maintenant: f64) -> Passe {
        // Un défilement qui n'est pas le nôtre : DESCENDU lui-même au bas pendant un flux, il le
        // choisit.
        let precedent = self.precedent.take();
        if let Some((avant, _)) = &precedent
            && (m.scroll_top - avant).abs() >= 2.0
        {
            let descend = m.scroll_top > avant + 1.0;
            if descend
                && maintenant - self.derniere_intention < FENETRE_INTENTION_MS
                && Self::bas_visible(m)
            {
                self.emettre(FollowEvent::ReachedBottom {
                    streaming: hote.streaming,
                });
            }
        }
        let tour_box = self.calculer_reserve(m, hote);
        let cible = match (self.mode, tour_box) {
            (FollowMode::Turn, Some(b)) => Some(turn_target(b.top, self.contexte)),
            (FollowMode::Live, _) => Some(m.bas_contenu - m.visible + MARGE_BAS),
            (FollowMode::Free, _) => precedent.as_ref().and_then(|(_, boites)| {
                // Garder la place (l'`overflow-anchor` de Chromium, absent de GPUI).
                if m.scroll_top <= 0.0 {
                    return None;
                }
                let a = reading_anchor(&boites_modele(boites), m.scroll_top)?;
                let t = anchor_target(&a, &m.boites_modele())?;
                ((t - m.scroll_top).abs() >= 1.0).then_some(t)
            }),
            _ => None,
        };
        let ecrire = cible.and_then(|c| self.poser(c, m));
        if ecrire.is_none() {
            self.precedent = Some((m.scroll_top, m.boites.clone()));
        }
        Passe {
            reserve: self.reserve,
            ecrire,
        }
    }

    /// Placer un message en haut (rail, Alt+↑/↓, lien, ouverture) — une demande : mode `free`.
    pub fn reveler(&mut self, id: &str, m: &Mesures) -> Option<f64> {
        let b = m.boites.iter().find(|x| x.id == id)?.top;
        self.emettre(FollowEvent::Intent);
        let t = turn_target(b, self.contexte);
        // Déjà en place : rien à écrire, mais la demande est servie.
        Some(self.poser(t, m).unwrap_or(m.scroll_top))
    }

    /// Ouvrir sur la DERNIÈRE question en haut : la réserve (comme après un envoi) lui donne la
    /// place de monter même si le dernier tour est court.
    pub fn reveler_derniere(&mut self, id: &str, m: &Mesures, hote: Hote) -> Option<f64> {
        self.avec_reserve = true;
        self.calculer_reserve(m, hote);
        self.reveler(id, m)
    }

    /// Une autre conversation : plus de réserve.
    pub fn oublier_reserve(&mut self) {
        self.avec_reserve = false;
        self.tour = Tour::Aucun;
        self.reserve = 0.0;
        self.precedent = None;
    }

    /// L'ancre de lecture (avant une reconstruction, pour la position de la vue).
    #[must_use]
    pub fn ancre(m: &Mesures) -> Option<ReadingAnchor> {
        reading_anchor(&m.boites_modele(), m.scroll_top)
    }

    /// Rend une ancre de lecture : la cible, ou `None` si son message n'est pas (encore) là.
    pub fn restaurer(&mut self, a: &ReadingAnchor, m: &Mesures) -> Option<f64> {
        let t = anchor_target(a, &m.boites_modele())?;
        Some(self.poser(t, m).unwrap_or(m.scroll_top))
    }

    /// La position de la vue : `{message: rang, dy}`.
    #[must_use]
    pub fn position(m: &Mesures) -> Option<Position> {
        let a = Self::ancre(m)?;
        let rang = m.boites.iter().position(|b| b.id == a.message)?;
        Some(Position {
            message: rang,
            dy: a.dy,
        })
    }

    /// Rend une position de la vue : la cible, ou `None` si le message de ce rang n'est pas là.
    pub fn aller_a_position(&mut self, p: Position, m: &Mesures) -> Option<f64> {
        let b = m.boites.get(p.message)?;
        let a = ReadingAnchor {
            message: b.id.clone(),
            dy: p.dy,
        };
        self.restaurer(&a, m)
    }

    /// La question suivante ou précédente de la lecture (Alt+↓ / Alt+↑).
    #[must_use]
    pub fn tour_voisin(&self, m: &Mesures, sens: Sens) -> Option<String> {
        let qs: Vec<&Boite> = m.questions().collect();
        let tops: Vec<f64> = qs.iter().map(|b| b.top).collect();
        let i = neighbour_turn(&tops, m.scroll_top, self.contexte, sens)?;
        qs.get(i).map(|b| b.id.clone())
    }
}
