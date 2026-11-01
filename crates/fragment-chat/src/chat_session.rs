// Port de app/src/ui/views/chat/ChatSession.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Le PILOTE d'une conversation affichée — hors de toute vue.
//
//  Il charge les messages chez le fournisseur À LA DEMANDE (`ensure_loaded` : une
//  vue en pile d'avatars ne charge rien), envoie un tour et y accumule le flux.
//  Chaque message porte une RÉVISION : la liste ne redessine que ceux qui ont
//  changé (le message en cours, à chaque morceau), pas tout le fil.
//  Appelle les abonnés (`on_change`) à chaque mutation.
//
//  ★ Écarts assumés (voir la PR) : le TS est asynchrone (`Promise`) ; ici le fournisseur
//    est synchrone et pousse le flux par rappel. Le tour se décompose en trois temps
//    publics — `begin_turn`, `apply_event`, `finish_turn` — que `send` enchaîne ; une
//    vue pilotée par des événements (gpui) les appelle elle-même. « Les appels
//    concurrents attendent le même chargement » n'a plus de sens sans concurrence :
//    `ensure_loaded` est idempotent.
// ═══════════════════════════════════════════════════════════════════════════

use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::{ChatEvent, ChatMessage, ChatPart, ChatProvider, Role, SendOpts};

/// Un message vivant : sa révision change à chaque mutation, `streaming` tant que sa réponse s'écrit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveMessage {
    pub message: ChatMessage,
    pub rev: u32,
    pub streaming: bool,
}

impl Deref for LiveMessage {
    type Target = ChatMessage;
    fn deref(&self) -> &ChatMessage {
        &self.message
    }
}

impl DerefMut for LiveMessage {
    fn deref_mut(&mut self) -> &mut ChatMessage {
        &mut self.message
    }
}

static COMPTEUR: AtomicU64 = AtomicU64::new(0);

fn nouvel_id(prefixe: &str) -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    format!(
        "{prefixe}-{ms:x}-{:x}",
        COMPTEUR.fetch_add(1, Ordering::Relaxed)
    )
}

pub struct ChatSession {
    provider: Rc<dyn ChatProvider>,
    conversation: String,
    pub messages: Vec<LiveMessage>,
    pub loaded: bool,
    pub loading: bool,
    pub running: bool,
    pub error: Option<String>,
    listeners: Vec<Box<dyn FnMut()>>,
}

impl ChatSession {
    pub fn new(provider: Rc<dyn ChatProvider>, conversation: impl Into<String>) -> Self {
        Self {
            provider,
            conversation: conversation.into(),
            messages: Vec::new(),
            loaded: false,
            loading: false,
            running: false,
            error: None,
            listeners: Vec::new(),
        }
    }

    pub fn provider(&self) -> &Rc<dyn ChatProvider> {
        &self.provider
    }

    pub fn conversation(&self) -> &str {
        &self.conversation
    }

    /// S'abonne à l'événement `change` (émis à chaque mutation).
    pub fn on_change(&mut self, f: impl FnMut() + 'static) {
        self.listeners.push(Box::new(f));
    }

    fn emettre(&mut self) {
        for f in &mut self.listeners {
            f();
        }
    }

    /// Charge une fois.
    pub fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loading = true;
        self.emettre();
        match self.provider.load(&self.conversation) {
            Ok(msgs) => {
                // Un tour lancé pendant le chargement reste après l'historique.
                let mut tous: Vec<LiveMessage> = msgs
                    .into_iter()
                    .map(|message| LiveMessage {
                        message,
                        rev: 0,
                        streaming: false,
                    })
                    .collect();
                tous.append(&mut self.messages);
                self.messages = tous;
                self.error = None;
            }
            Err(e) => self.error = Some(e.0),
        }
        self.loaded = true;
        self.loading = false;
        self.emettre();
    }

    fn id_du_role(&self, role: Role, defaut: &str) -> String {
        self.provider
            .participants(&self.conversation)
            .into_iter()
            .find(|p| p.role == role)
            .map_or_else(|| defaut.to_string(), |p| p.id)
    }

    pub fn user_id(&self) -> String {
        self.id_du_role(Role::User, "user")
    }

    pub fn assistant_id(&self) -> String {
        self.id_du_role(Role::Assistant, "assistant")
    }

    /// Premier temps d'un tour : charge l'historique, pose le message de l'utilisateur et la
    /// réponse (vide, en flux). Rend le texte rogné à envoyer au fournisseur, ou `None` si le
    /// tour est ignoré (vide sans parts, ou déjà en cours).
    pub fn begin_turn(&mut self, text: &str, extra: Vec<ChatPart>) -> Option<String> {
        let t = text.trim().to_string();
        if (t.is_empty() && extra.is_empty()) || self.running {
            return None;
        }
        // L'historique d'abord : relu APRÈS le tour, le transcript contiendrait déjà ce tour (doublon).
        if !self.loaded {
            self.ensure_loaded();
        }
        // ★ Le texte d'ABORD, les parts des extensions ensuite (comme à la relecture) : l'index
        //   du part de texte reste 0, les positions de la surface ne bougent pas.
        let mut parts: Vec<ChatPart> = Vec::new();
        if !t.is_empty() {
            parts.push(ChatPart::text(t.clone()));
        }
        parts.extend(extra);
        let author = self.user_id();
        self.messages.push(LiveMessage {
            message: ChatMessage {
                id: nouvel_id("u"),
                author,
                parts,
                ..ChatMessage::default()
            },
            rev: 0,
            streaming: false,
        });
        let author = self.assistant_id();
        self.messages.push(LiveMessage {
            message: ChatMessage {
                id: nouvel_id("a"),
                author,
                ..ChatMessage::default()
            },
            rev: 0,
            streaming: true,
        });
        self.running = true;
        self.emettre();
        Some(t)
    }

    fn toucher(&mut self) {
        if let Some(m) = self.messages.last_mut() {
            m.rev += 1;
        }
        self.emettre();
    }

    /// Deuxième temps : un événement du flux, accumulé dans la réponse en cours (le dernier message).
    pub fn apply_event(&mut self, e: ChatEvent) {
        let Some(reponse) = self.messages.last_mut() else {
            return;
        };
        match e {
            ChatEvent::Text(texte) => match reponse.parts.last_mut() {
                Some(dernier) if dernier.kind == "text" => {
                    dernier
                        .text
                        .get_or_insert_with(String::new)
                        .push_str(&texte);
                }
                _ => reponse.parts.push(ChatPart::text(texte)),
            },
            ChatEvent::Part(part) => {
                // Un id vide est « absent » (`e.part.id ? … : -1`).
                let i = part
                    .id
                    .as_deref()
                    .filter(|id| !id.is_empty())
                    .and_then(|id| {
                        reponse
                            .parts
                            .iter()
                            .position(|p| p.id.as_deref() == Some(id))
                    });
                match i {
                    Some(i) => reponse.parts[i] = part,
                    None => reponse.parts.push(part),
                }
            }
            ChatEvent::Remove(id) => reponse
                .parts
                .retain(|p| p.id.as_deref() != Some(id.as_str())),
            ChatEvent::Done { error } => {
                if let Some(e) = error.filter(|e| !e.is_empty()) {
                    reponse.error = Some(e);
                }
                reponse.streaming = false;
                self.running = false;
            }
        }
        self.toucher();
    }

    /// Troisième temps : le fournisseur a rendu la main (avec une erreur, ou pas). Rien n'est
    /// à faire si le flux s'est terminé proprement ; sinon la réponse cesse d'être « en flux ».
    pub fn finish_turn(&mut self, error: Option<String>) {
        if let (Some(e), Some(reponse)) = (error, self.messages.last_mut()) {
            reponse.error = Some(e);
        }
        let en_cours = self.messages.last().is_some_and(|m| m.streaming) || self.running;
        if en_cours {
            if let Some(reponse) = self.messages.last_mut() {
                reponse.streaming = false;
            }
            self.running = false;
            self.toucher();
        }
    }

    /// Envoie un tour. `extra` : ce que les extensions du champ ajoutent au message de
    /// l'utilisateur (des pièces jointes…) ; avec eux, le texte peut être vide.
    pub fn send(&mut self, text: &str, opts: Option<SendOpts>, extra: Vec<ChatPart>) {
        let Some(t) = self.begin_turn(text, extra) else {
            return;
        };
        let provider = Rc::clone(&self.provider);
        let conversation = self.conversation.clone();
        let resultat = provider.send(
            &conversation,
            &t,
            &mut |e| self.apply_event(e),
            opts.as_ref(),
        );
        self.finish_turn(resultat.err().map(|e| e.0));
    }

    pub fn stop(&self) {
        self.provider.stop(&self.conversation);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use serde_json::json;

    use super::*;
    use crate::api::{ChatError, Participant};

    #[derive(Default)]
    struct Fournisseur {
        script: Vec<ChatEvent>,
        loads: Cell<usize>,
        echec: bool,
        recu: RefCell<Vec<(String, Option<SendOpts>)>>,
    }

    impl Fournisseur {
        fn avec(script: Vec<ChatEvent>) -> Self {
            Self {
                script,
                ..Self::default()
            }
        }

        fn standard() -> Self {
            Self::avec(vec![
                ChatEvent::Text("Bon".into()),
                ChatEvent::Text("jour.".into()),
                ChatEvent::Done { error: None },
            ])
        }
    }

    impl ChatProvider for Fournisseur {
        fn id(&self) -> &str {
            "t"
        }
        fn label(&self) -> &str {
            "Test"
        }
        fn participants(&self, _c: &str) -> Vec<Participant> {
            vec![
                Participant {
                    id: "j".into(),
                    name: "Julien".into(),
                    initials: None,
                    role: Role::User,
                },
                Participant {
                    id: "c".into(),
                    name: "Claude".into(),
                    initials: None,
                    role: Role::Assistant,
                },
            ]
        }
        fn title(&self, _c: &str) -> String {
            "Test".into()
        }
        fn load(&self, _c: &str) -> Result<Vec<ChatMessage>, ChatError> {
            self.loads.set(self.loads.get() + 1);
            Ok(vec![ChatMessage {
                id: "m0".into(),
                author: "j".into(),
                parts: vec![ChatPart::text("avant")],
                ..ChatMessage::default()
            }])
        }
        fn send(
            &self,
            _c: &str,
            text: &str,
            on_event: &mut dyn FnMut(ChatEvent),
            opts: Option<&SendOpts>,
        ) -> Result<(), ChatError> {
            self.recu
                .borrow_mut()
                .push((text.to_string(), opts.cloned()));
            if self.echec {
                return Err(ChatError::new("hors ligne"));
            }
            for e in &self.script {
                on_event(e.clone());
            }
            Ok(())
        }
    }

    fn session(p: &Rc<Fournisseur>) -> ChatSession {
        let provider: Rc<dyn ChatProvider> = Rc::clone(p) as Rc<dyn ChatProvider>;
        ChatSession::new(provider, "x")
    }

    fn part(kind: &str, id: Option<&str>, data: Option<serde_json::Value>) -> ChatPart {
        ChatPart {
            kind: kind.into(),
            id: id.map(Into::into),
            text: None,
            data,
        }
    }

    fn texte_part(id: &str, texte: &str) -> ChatPart {
        ChatPart {
            id: Some(id.into()),
            ..ChatPart::text(texte)
        }
    }

    #[test]
    fn charge_une_seule_fois_meme_appele_en_parallele() {
        let p = Rc::new(Fournisseur::standard());
        let mut s = session(&p);
        // Pas de concurrence ici : deux appels de suite (le second ne recharge pas).
        s.ensure_loaded();
        s.ensure_loaded();
        assert_eq!(p.loads.get(), 1);
        let textes: Vec<_> = s.messages.iter().map(|m| m.parts[0].text.clone()).collect();
        assert_eq!(textes, vec![Some("avant".to_string())]);
    }

    #[test]
    fn un_tour_message_utilisateur_reponse_qui_accumule_le_texte_revision_incrementee_fin_de_flux()
    {
        let p = Rc::new(Fournisseur::standard());
        let mut s = session(&p);
        s.send("  salut ", None, vec![]);
        // L'historique (« avant ») est chargé d'abord.
        let (u, a) = (&s.messages[1], &s.messages[2]);
        assert_eq!(u.author, "j");
        assert_eq!(u.parts, vec![ChatPart::text("salut")]);
        assert_eq!(a.author, "c");
        assert_eq!(a.parts, vec![ChatPart::text("Bonjour.")]);
        assert!(!a.streaming);
        assert_eq!(a.rev, 3);
        assert!(!s.running);
    }

    #[test]
    fn parts_non_texte_et_erreur() {
        let p = Rc::new(Fournisseur::avec(vec![
            ChatEvent::Part(part("outil", None, Some(json!(1)))),
            ChatEvent::Text("ok".into()),
            ChatEvent::Done {
                error: Some("quota".into()),
            },
        ]));
        let mut s = session(&p);
        s.send("q", None, vec![]);
        let derniere = s
            .messages
            .last()
            .map(|m| m.message.clone())
            .unwrap_or_default();
        let types: Vec<_> = derniere.parts.iter().map(|p| p.kind.as_str()).collect();
        assert_eq!(types, vec!["outil", "text"]);
        assert_eq!(derniere.error.as_deref(), Some("quota"));
    }

    #[test]
    fn un_part_reemis_avec_le_meme_id_remplace_l_ancien_resultat_d_outil() {
        let p = Rc::new(Fournisseur::avec(vec![
            ChatEvent::Part(part("outil", Some("t1"), Some(json!("en cours")))),
            ChatEvent::Part(part("outil", Some("t1"), Some(json!("fini")))),
            ChatEvent::Done { error: None },
        ]));
        let mut s = session(&p);
        s.send("q", None, vec![]);
        let derniere = s
            .messages
            .last()
            .map(|m| m.parts.clone())
            .unwrap_or_default();
        assert_eq!(
            derniere,
            vec![part("outil", Some("t1"), Some(json!("fini")))]
        );
    }

    #[test]
    fn vide_ou_deja_en_cours_ignore_un_fournisseur_qui_leve_erreur_sur_la_reponse_plus_en_cours() {
        let p = Rc::new(Fournisseur {
            echec: true,
            ..Fournisseur::standard()
        });
        let mut s = session(&p);
        s.send("   ", None, vec![]);
        // Pas même chargé : rien n'a été envoyé.
        assert_eq!(s.messages.len(), 0);
        s.send("q", None, vec![]);
        assert_eq!(
            s.messages.last().and_then(|m| m.error.clone()).as_deref(),
            Some("hors ligne")
        );
        assert!(!s.running);
    }

    #[test]
    fn les_parts_des_extensions_du_champ_pieces_jointes_suivent_le_texte_avec_eux_le_texte_peut_etre_vide()
     {
        let p = Rc::new(Fournisseur::standard());
        let mut s = session(&p);
        let pj = ChatPart {
            kind: "attachment".into(),
            data: Some(json!({ "name": "a.png", "mime": "image/png" })),
            ..ChatPart::default()
        };
        let opts: SendOpts = json!({ "attachments": ["A"] })
            .as_object()
            .cloned()
            .unwrap_or_default();
        s.send("regarde", Some(opts.clone()), vec![pj.clone()]);
        let avant_derniere = |s: &ChatSession| s.messages[s.messages.len() - 2].parts.clone();
        assert_eq!(
            avant_derniere(&s),
            vec![ChatPart::text("regarde"), pj.clone()]
        );
        s.send("   ", None, vec![pj.clone()]);
        assert_eq!(avant_derniere(&s), vec![pj]);
        s.send("   ", None, vec![]);
        assert_eq!(
            *p.recu.borrow(),
            vec![("regarde".to_string(), Some(opts)), (String::new(), None)]
        );
    }

    #[test]
    fn remove_un_part_du_message_en_flux_le_quitte_par_son_id() {
        let p = Rc::new(Fournisseur::avec(vec![
            ChatEvent::Part(texte_part("answer", "Je regarde.")),
            ChatEvent::Part(part("agent-trace", Some("trace"), Some(json!({})))),
            ChatEvent::Remove("answer".into()),
            ChatEvent::Part(texte_part("answer", "Réponse.")),
            ChatEvent::Done { error: None },
        ]));
        let mut s = session(&p);
        s.send("q", None, vec![]);
        let derniere = s
            .messages
            .last()
            .map(|m| m.parts.clone())
            .unwrap_or_default();
        let forme: Vec<_> = derniere
            .iter()
            .map(|p| (p.kind.as_str(), p.text.as_deref()))
            .collect();
        assert_eq!(
            forme,
            vec![("agent-trace", None), ("text", Some("Réponse."))]
        );
    }
}
