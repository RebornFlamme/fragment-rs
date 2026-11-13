// Port de app/src/ui/views/chat/chat-view.css (la colonne de la note, la grille du cowork) et du
// seuil `SEUIL_COWORK` de ChatView.ts.
//! La géométrie PURE de la vue chat : où tombent la colonne des messages, le titre et le champ, pour
//! une largeur de pane, une barre des conversations et une réserve de marge de lecture donnés.
//!
//! ★ Une seule définition de « la colonne de lecture », partagée avec la note : le texte d'une
//!   réponse tombe EXACTEMENT où tomberait celui d'une note (même x, même largeur). La barre des
//!   conversations prend sur la marge gauche (la colonne centrée sur l'ONGLET entier, jamais sous la
//!   barre : elle glisse juste ce qu'il faut) ; la marge 1/4 – 3/4 réserve un padding à droite. Les
//!   deux seules grandeurs qui bougent — la largeur de la barre et la réserve — sont animées par
//!   l'appelant ([`Transition`]) : tout le reste en dérive à chaque image, rien ne saute.

use fragment_chat::api::ChatMode;

use crate::editeur::constantes::{
    GOUTTIERE_DOC, LARGEUR_BARRE, LARGEUR_DOC, LARGEUR_REPLI, MARGE_LIGNE_DROITE,
    MARGE_LIGNE_GAUCHE,
};
use crate::mode_marge::etat::courbe_bezier;

/// À partir de cette largeur, la vue en page devient un cowork (barre latérale).
pub const SEUIL_COWORK: f32 = 720.0;

/// La largeur de la barre ouverte (`--chat-sidebar-w`, `.chat-sidebar-zone > .chat-sidebar`).
pub const LARGEUR_BARRE_OUVERTE: f32 = 260.0;

/// La place de la barre de défilement, toujours réservée (`scrollbar-gutter: stable`).
pub const BARRE_DEFILEMENT: f32 = LARGEUR_BARRE;

/// `--doc-text-start` : où le texte commence dans la colonne (gouttière + repli + marge de ligne).
pub const DEBUT_TEXTE: f32 = GOUTTIERE_DOC + LARGEUR_REPLI + MARGE_LIGNE_GAUCHE;

/// `--doc-text-end` : où il finit (gouttière + marge droite de ligne).
pub const FIN_TEXTE: f32 = GOUTTIERE_DOC + MARGE_LIGNE_DROITE;

/// La disposition de la vue (`ChatLayout`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// La barre des conversations à côté de la conversation.
    Cowork,
    /// La conversation seule.
    Colonne,
}

impl Disposition {
    /// Le texte de `data-layout`.
    #[must_use]
    pub fn nom(self) -> &'static str {
        match self {
            Disposition::Cowork => "cowork",
            Disposition::Colonne => "column",
        }
    }
}

/// Cowork en PAGE quand la largeur le permet ; sinon la colonne.
#[must_use]
pub fn disposition(mode: ChatMode, largeur_contenu: f32) -> Disposition {
    if mode == ChatMode::Page && largeur_contenu >= SEUIL_COWORK {
        Disposition::Cowork
    } else {
        Disposition::Colonne
    }
}

/// Ce dont dépend la colonne.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entrees {
    /// La largeur du pane (la zone du fil, sans la barre).
    pub pane: f32,
    /// La largeur COURANTE de la barre (0 à 260, animée).
    pub barre: f32,
    /// La réserve de la marge de lecture, en fraction de l'onglet (0 à 0,25, animée).
    pub reserve: f32,
    /// `--doc-width` ; `None` : « Readable line length » coupé (`100 %`).
    pub largeur_doc: Option<f32>,
}

impl Entrees {
    /// Les entrées d'un pane au repos, la note à 720 px.
    #[must_use]
    pub fn au_repos(pane: f32) -> Self {
        Self {
            pane,
            barre: 0.0,
            reserve: 0.0,
            largeur_doc: Some(LARGEUR_DOC),
        }
    }
}

/// Où tombent la colonne, le titre et le champ (coordonnées du pane, sauf mention).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colonne {
    /// `W` : la largeur de l'onglet, pane + barre.
    pub onglet: f32,
    /// `A` : la place de la note dans le défileur (pane − barre de défilement − réserve).
    pub dispo: f32,
    /// `D` : la colonne, jamais plus large que la place.
    pub largeur: f32,
    /// Le bord gauche de la colonne dans le plan du défileur.
    pub marge_gauche: f32,
    /// Le padding droit du défileur (la réserve en px).
    pub reserve_px: f32,
    /// Le x du TEXTE (celui du titre et du champ aussi).
    pub texte_x: f32,
    /// La largeur du texte (celle du champ).
    pub texte_largeur: f32,
}

impl Colonne {
    /// Le bord droit du texte.
    #[must_use]
    pub fn texte_droite(&self) -> f32 {
        self.texte_x + self.texte_largeur
    }

    /// Le bord gauche du texte dans l'ONGLET (barre comprise) : ce que la note aurait.
    #[must_use]
    pub fn texte_x_onglet(&self, barre: f32) -> f32 {
        self.texte_x + barre
    }
}

/// La colonne pour ces entrées. Les `%` du CSS se résolvent ici : dans le défileur 100 % = `A`, dans
/// le pane le même `A` se calcule — un seul calcul.
#[must_use]
pub fn colonne(e: Entrees) -> Colonne {
    let onglet = e.pane + e.barre;
    let reserve_px = onglet * e.reserve;
    let dispo = (e.pane - BARRE_DEFILEMENT - reserve_px).max(0.0);
    // `--doc-width: 100 %` : la place entière (dans le pane `min(100 %, A)` vaut aussi `A`).
    let largeur = e.largeur_doc.map_or(dispo, |d| d.min(dispo));
    let marge_gauche = ((dispo + e.barre - largeur) / 2.0 - e.barre).max(0.0);
    let texte_x = marge_gauche + DEBUT_TEXTE;
    Colonne {
        onglet,
        dispo,
        largeur,
        marge_gauche,
        reserve_px,
        texte_x,
        texte_largeur: (largeur - DEBUT_TEXTE - FIN_TEXTE).max(0.0),
    }
}

/// Une grandeur qui glisse d'une valeur à l'autre (`transition` d'une propriété enregistrée) : la
/// barre des conversations (`--anim-duration-moderate`) et, côté marge, la réserve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    depuis: f32,
    vers: f32,
    debut_ms: f64,
    duree_ms: f64,
    courbe: [f32; 4],
}

impl Transition {
    /// Une grandeur au repos sur `valeur`.
    #[must_use]
    pub fn repos(valeur: f32, courbe: [f32; 4]) -> Self {
        Self {
            depuis: valeur,
            vers: valeur,
            debut_ms: 0.0,
            duree_ms: 0.0,
            courbe,
        }
    }

    /// Part d'où elle est vers `vers`. Même cible : rien ne change (un second geste ne relance pas).
    pub fn aller_a(&mut self, vers: f32, maintenant_ms: f64, duree_ms: f64) {
        if (vers - self.vers).abs() < f32::EPSILON {
            return;
        }
        self.depuis = self.valeur(maintenant_ms);
        self.vers = vers;
        self.debut_ms = maintenant_ms;
        self.duree_ms = duree_ms;
    }

    /// Saute à `valeur`, sans transition (le premier rendu : l'apparition de la vue n'anime rien).
    pub fn poser(&mut self, valeur: f32) {
        self.depuis = valeur;
        self.vers = valeur;
        self.duree_ms = 0.0;
    }

    /// La valeur à `maintenant_ms`.
    #[must_use]
    pub fn valeur(&self, maintenant_ms: f64) -> f32 {
        if self.duree_ms <= 0.0 {
            return self.vers;
        }
        let x = ((maintenant_ms - self.debut_ms) / self.duree_ms).clamp(0.0, 1.0) as f32;
        if x >= 1.0 {
            return self.vers;
        }
        self.depuis + (self.vers - self.depuis) * courbe_bezier(self.courbe, x)
    }

    /// La cible.
    #[must_use]
    pub fn cible(&self) -> f32 {
        self.vers
    }

    /// Court-elle encore à `maintenant_ms` ? (la vue redemande une image)
    #[must_use]
    pub fn en_cours(&self, maintenant_ms: f64) -> bool {
        self.duree_ms > 0.0 && maintenant_ms - self.debut_ms < self.duree_ms
    }
}
