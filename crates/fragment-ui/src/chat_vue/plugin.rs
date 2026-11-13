// Port de app/src/ui/views/chat/ChatPlugin.ts (la vue, le schéma `chat:`, les commandes
// `chat:open`, `chat:new`, `chat:new-tab`) et de chatKey.ts.
//! Le plugin cœur « chat » : la vue, l'adresse `chat:<fournisseur>/<id>` d'une conversation et ses
//! trois commandes.
//!
//! Non porté ici (autres tâches) : les pièces jointes et Répondre (`fr-t7-pieces-citation`), le
//! lien d'un message (`copy-link`, une action de message qui connaît la vue : `fr-t7-montage`), la
//! barre de l'agent (`fr-t7-barre-agent`).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use fragment_chat::api::{CHAT_VIEW_TYPE, ChatProvider};
use fragment_chat::chat_key::{CHAT_SCHEME, chat_key, parse_chat_key};
use fragment_core::{
    App as _, AppCoeur, Command, Disposer, DocumentKey, DocumentProvider, ItemId, NouvelleFeuille,
    OWNER_CORE, OpenViewState, Registry, ViewState, Workspace,
};
use serde_json::{Map, Value, json};

use super::modele::ContexteChat;
use super::vue::fabrique;
use crate::chat_barre_laterale::Horloge;
use crate::chat_fil::registre_vide;

/// Le propriétaire des enregistrements (l'id du plugin cœur).
pub const PLUGIN_CHAT: &str = "chat";

/// `chat:open`.
pub const COMMANDE_OUVRIR: &str = "chat:open";
/// `chat:new`.
pub const COMMANDE_NOUVELLE: &str = "chat:new";
/// `chat:new-tab`.
pub const COMMANDE_NOUVELLE_ONGLET: &str = "chat:new-tab";

/// Le registre des fournisseurs (`chat:provider@1`).
#[must_use]
pub fn registre_fournisseurs() -> Registry<Rc<dyn ChatProvider>> {
    Registry::new("chat:provider@1", "Fournisseurs de chat")
}

/// Le fournisseur par défaut : le dernier inscrit (un double de test s'y substitue).
fn fournisseur_par_defaut(
    fournisseurs: &Registry<Rc<dyn ChatProvider>>,
) -> Option<Rc<dyn ChatProvider>> {
    fournisseurs.entries().pop().map(|e| e.value)
}

impl ContexteChat {
    /// Le contexte d'une app du cœur : ouvrir un document passe par le Workspace, le dock se lit
    /// dans son arbre. Le reste (marge, horloge, téléchargement) est à la charge de l'appelant.
    #[must_use]
    pub fn pour_app(
        app: &AppCoeur,
        fournisseurs: Registry<Rc<dyn ChatProvider>>,
        horloge: Horloge,
        telecharger: super::modele::Telechargeur,
    ) -> Self {
        let ws = Rc::downgrade(app.workspace());
        let ws_dock = ws.clone();
        Self {
            fournisseurs,
            contributions: registre_vide(),
            en_tete: Rc::default(),
            pied: Rc::default(),
            horloge: Rc::new(RefCell::new(horloge)),
            marge: None,
            documents: app.documents().clone(),
            ms: Rc::new(temps_ms),
            est_dans_dock: Rc::new(move |leaf| {
                ws_dock.upgrade().is_some_and(|w| {
                    w.try_borrow().is_ok_and(|w| {
                        let racine = w.arbre().get_root(leaf);
                        racine == w.left_split() || racine == w.right_split()
                    })
                })
            }),
            ouvrir: Rc::new(move |leaf, cle, e_state| ouvrir_dans(&ws, leaf, cle, e_state)),
            telecharger,
            taille_texte: 16.0,
            ligne_lisible: Rc::new(Cell::new(true)),
        }
    }
}

/// Le temps réel, en ms depuis le premier appel.
fn temps_ms() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static DEBUT: OnceLock<Instant> = OnceLock::new();
    DEBUT.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Ouvre un document dans la feuille : une navigation (l'historique ← → s'en souvient).
fn ouvrir_dans(ws: &Weak<RefCell<Workspace>>, leaf: ItemId, cle: &str, e_state: Option<Value>) {
    let Some(ws) = ws.upgrade() else { return };
    let Ok(mut w) = ws.try_borrow_mut() else {
        return;
    };
    let open = OpenViewState {
        e_state: e_state.and_then(|v| v.as_object().cloned()),
        ..OpenViewState::default()
    };
    w.open_document_in(leaf, cle, Some(open));
}

/// Le schéma `chat:` : une conversation est un DOCUMENT (ouverte, naviguée ← →, annotée).
pub struct DocumentsChat {
    fournisseurs: Registry<Rc<dyn ChatProvider>>,
}

impl DocumentsChat {
    fn fournisseur_de(&self, cle: &str) -> Option<(Rc<dyn ChatProvider>, String)> {
        let k = parse_chat_key(cle)?;
        Some((self.fournisseurs.get(&k.provider)?, k.conversation))
    }
}

impl DocumentProvider for DocumentsChat {
    fn scheme(&self) -> &str {
        CHAT_SCHEME
    }

    fn open(&self, key: &str) -> Option<ViewState> {
        let k = parse_chat_key(key)?;
        let mut etat = Map::new();
        etat.insert("provider".into(), Value::String(k.provider));
        etat.insert("conversation".into(), Value::String(k.conversation));
        Some(ViewState {
            type_: CHAT_VIEW_TYPE.to_owned(),
            state: Some(etat),
            ..ViewState::default()
        })
    }

    fn key_of(&self, state: &ViewState) -> Option<DocumentKey> {
        if state.type_ != CHAT_VIEW_TYPE {
            return None;
        }
        let s = state.state.as_ref()?;
        let p = s.get("provider")?.as_str().filter(|p| !p.is_empty())?;
        let c = s.get("conversation")?.as_str().filter(|c| !c.is_empty())?;
        Some(DocumentKey::new(chat_key(p, c)))
    }

    fn title(&self, key: &str) -> String {
        self.fournisseur_de(key)
            .map(|(p, c)| p.title(&c))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "Nouvelle conversation".to_owned())
    }

    fn icon(&self, key: &str) -> Option<String> {
        Some(
            self.fournisseur_de(key)
                .and_then(|(p, _)| p.icon().map(str::to_owned))
                .unwrap_or_else(|| "message-square".to_owned()),
        )
    }

    /// Fournisseur pas (encore) chargé : on ne jette pas l'historique. Une conversation neuve n'est
    /// pas encore listée : elle n'existe pas.
    fn exists(&self, key: &str) -> bool {
        let Some(k) = parse_chat_key(key) else {
            return false;
        };
        match self.fournisseurs.get(&k.provider) {
            None => true,
            Some(p) => p.list().iter().any(|c| c.id == k.conversation),
        }
    }
}

/// Ouvre une conversation dans un nouvel onglet du centre — le cowork quand il a la place.
fn ouvrir_au_centre(ws: &Weak<RefCell<Workspace>>, fournisseur: &str, conversation: &str) {
    let Some(ws) = ws.upgrade() else { return };
    let Ok(mut w) = ws.try_borrow_mut() else {
        return;
    };
    if let Ok(leaf) = w.get_leaf(NouvelleFeuille::Onglet) {
        w.set_view_state(leaf, etat_de_vue(fournisseur, conversation), None);
    }
}

fn etat_de_vue(fournisseur: &str, conversation: &str) -> ViewState {
    let mut etat = Map::new();
    etat.insert("provider".into(), json!(fournisseur));
    etat.insert("conversation".into(), json!(conversation));
    ViewState {
        type_: CHAT_VIEW_TYPE.to_owned(),
        state: Some(etat),
        active: Some(true),
        ..ViewState::default()
    }
}

/// Ouvre (ou montre) le chat de `fournisseur` dans le dock droit. Sans conversation demandée :
/// celle déjà affichée, sinon la plus récente, sinon une neuve. Une conversation demandée va dans
/// le chat qui l'affiche déjà, sinon dans le premier chat du dock.
fn ouvrir_au_dock(
    ws: &Weak<RefCell<Workspace>>,
    p: &Rc<dyn ChatProvider>,
    conversation: Option<String>,
) {
    let Some(ws) = ws.upgrade() else { return };
    let Ok(mut w) = ws.try_borrow_mut() else {
        return;
    };
    let droite = w.right_split();
    let etat_de = |w: &Workspace, l: ItemId| -> (String, String) {
        let key = w.document_de(l).map(|d| d.to_string()).unwrap_or_default();
        parse_chat_key(&key).map_or_else(Default::default, |k| (k.provider, k.conversation))
    };
    let chats: Vec<(ItemId, String)> = w
        .get_leaves_of_type(CHAT_VIEW_TYPE)
        .into_iter()
        .filter(|&l| w.arbre().get_root(l) == droite)
        .map(|l| (l, etat_de(&w, l)))
        .filter(|(_, (prov, _))| prov == p.id())
        .map(|(l, (_, c))| (l, c))
        .collect();
    let feuille = match &conversation {
        Some(c) => chats.iter().find(|(_, x)| x == c).or(chats.first()),
        None => chats.first(),
    }
    .map(|(l, _)| *l);
    let conv = conversation
        .or_else(|| {
            feuille.and_then(|l| chats.iter().find(|(x, _)| *x == l).map(|(_, c)| c.clone()))
        })
        .filter(|c| !c.is_empty())
        .or_else(|| p.list().first().map(|c| c.id.clone()))
        .or_else(|| p.create())
        .unwrap_or_default();
    let Some(leaf) = feuille.or_else(|| w.get_right_leaf().ok()) else {
        return;
    };
    w.set_view_state(leaf, etat_de_vue(p.id(), &conv), None);
    w.reveal_leaf(leaf);
}

/// Enregistre la vue `chat`, le schéma `chat:` et les commandes. Rend les retraits.
pub fn enregistrer(app: &AppCoeur, ctx: ContexteChat) -> Vec<Disposer> {
    let fournisseurs = ctx.fournisseurs.clone();
    let mut retraits =
        vec![
            app.view_registry()
                .register(CHAT_VIEW_TYPE, fabrique(ctx), PLUGIN_CHAT),
        ];
    match app.documents().register(
        Rc::new(DocumentsChat {
            fournisseurs: fournisseurs.clone(),
        }),
        OWNER_CORE,
    ) {
        Ok(r) => retraits.push(r),
        Err(e) => eprintln!("fragment : chat : le schéma chat: n'a pas pu être inscrit : {e}"),
    }
    let ws = Rc::downgrade(app.workspace());
    let (f1, w1) = (fournisseurs.clone(), ws.clone());
    retraits.push(app.commands().add_command(
        Command::new(COMMANDE_OUVRIR, "Ouvrir le chat").with_check_callback(move |checking| {
            let p = fournisseur_par_defaut(&f1)?;
            if !checking {
                ouvrir_au_dock(&w1, &p, None);
            }
            Some(true)
        }),
        PLUGIN_CHAT,
    ));
    let (f2, w2) = (fournisseurs.clone(), ws.clone());
    retraits.push(app.commands().add_command(
        Command::new(COMMANDE_NOUVELLE, "Nouvelle conversation").with_check_callback(
            move |checking| {
                let p = fournisseur_par_defaut(&f2)?;
                if !checking {
                    ouvrir_au_dock(&w2, &p, Some(p.create()?));
                }
                Some(true)
            },
        ),
        PLUGIN_CHAT,
    ));
    let (f3, w3) = (fournisseurs, ws);
    retraits.push(
        app.commands().add_command(
            Command::new(
                COMMANDE_NOUVELLE_ONGLET,
                "Nouvelle conversation dans un onglet",
            )
            .with_check_callback(move |checking| {
                let p = fournisseur_par_defaut(&f3)?;
                if !checking {
                    ouvrir_au_centre(&w3, p.id(), &p.create()?);
                }
                Some(true)
            }),
            PLUGIN_CHAT,
        ),
    );
    retraits
}
