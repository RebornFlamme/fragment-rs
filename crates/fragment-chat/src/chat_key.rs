// Port de app/src/ui/views/chat/chatKey.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  L'ADRESSE d'une conversation (documents-de-plugin.md) : `chat:<fournisseur>/<id>`.
//  Un seul schéma pour tous les fournisseurs (l'agent, demain un autre) : c'est
//  le plugin `chat` qui le fournit, et il délègue titres et listes aux
//  fournisseurs. Pur, testé.
// ═══════════════════════════════════════════════════════════════════════════

pub const CHAT_SCHEME: &str = "chat";

/// La clé de document d'une conversation. Un id de fournisseur ne contient pas de `/`.
pub fn chat_key(provider: &str, conversation: &str) -> String {
    format!("{CHAT_SCHEME}:{provider}/{conversation}")
}

/// Le fournisseur et la conversation d'une clé `chat:…`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatKey {
    pub provider: String,
    pub conversation: String,
}

/// `chat:agent/x` → `{provider: "agent", conversation: "x"}` ; autre chose → `None`.
pub fn parse_chat_key(key: &str) -> Option<ChatKey> {
    let reste = key.strip_prefix(CHAT_SCHEME)?.strip_prefix(':')?;
    let i = reste.find('/')?;
    // Fournisseur vide, ou `/` en dernier (conversation vide) : refusé.
    if i == 0 || i == reste.len() - 1 {
        return None;
    }
    Some(ChatKey {
        provider: reste[..i].to_string(),
        conversation: reste[i + 1..].to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_refus_du_reste() {
        assert_eq!(chat_key("agent", "abc/def"), "chat:agent/abc/def");
        assert_eq!(
            parse_chat_key("chat:agent/abc/def"),
            Some(ChatKey {
                provider: "agent".into(),
                conversation: "abc/def".into()
            })
        );
        assert_eq!(parse_chat_key("notes/a.md"), None);
        assert_eq!(parse_chat_key("chat:agent"), None);
        assert_eq!(parse_chat_key("chat:agent/"), None);
    }
}
