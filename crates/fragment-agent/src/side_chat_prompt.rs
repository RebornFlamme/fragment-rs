// Port de app/src/ui/views/agent/side-chat/sideChatPrompt.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Ce qu'« Ask » envoie à l'agent — pur (testé via cardModel.test.ts en TS).
//
//  Le premier tour porte le DOCUMENT et le passage cité (préambule, envoyé avec
//  la question écrite) ; les suivants ne portent que la question : la session
//  reprise (même sessionId) a déjà le contexte. Un Ask dans une conversation de
//  l'agent est une BRANCHE : l'agent remplace ce préambule.
//
//  ★ Consignes en ANGLAIS, langue neutre pour le modèle : rédigées en français,
//    elles tiraient toute réponse vers le français (constaté avec le vrai agent).
//    La langue de la réponse est celle de la question.
// ═══════════════════════════════════════════════════════════════════════════

pub const SIDE_CHAT_SYSTEM: &str = "The user selected a passage — in the document below, or in the conversation above when this one branches from it — and asks you about it.\nAnswer from that context; say so when it does not let you answer.\nBe concise. Answer in the language of the user’s question.";

/// Le document tel que l'agent le reçoit au premier tour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentSource {
    Text { path: String, text: String },
    /// Le texte n'est pas entièrement disponible (PDF non peint) : l'agent lit le fichier.
    File { path: String, absolute_path: String },
}

fn document(doc: &DocumentSource) -> String {
    match doc {
        DocumentSource::Text { path, text } => format!("<document path=\"{path}\">\n{text}\n</document>"),
        DocumentSource::File { path, absolute_path } => {
            format!("<document path=\"{path}\">\nThe document is the file {absolute_path}: read it with Read.\n</document>")
        }
    }
}

/// Le préambule du premier tour : le document, puis le passage — la question suit.
pub fn side_chat_preamble(doc: &DocumentSource, quote: &str) -> String {
    let q = quote.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}\n\nAbout \"{q}\":", document(doc))
}

/// Les outils d'un tour : aucun quand le texte est fourni, la lecture du fichier sinon.
pub fn tools_for(doc: &DocumentSource) -> Vec<String> {
    match doc {
        DocumentSource::Text { .. } => Vec::new(),
        DocumentSource::File { .. } => vec!["Read".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> DocumentSource {
        DocumentSource::Text { path: "a.md".into(), text: "Bonjour".into() }
    }

    // describe('askPrompt') de cardModel.test.ts

    #[test]
    fn preambule_document_passage_cite_sans_numeros_de_paragraphe() {
        let p = side_chat_preamble(&doc(), "Nat\nSec");
        assert_eq!(p, "<document path=\"a.md\">\nBonjour\n</document>\n\nAbout \"Nat Sec\":");
        assert!(!p.contains('¶'));
    }

    #[test]
    fn pdf_sans_texte_complet_lagent_lit_le_fichier_read_sinon_aucun_outil() {
        let pdf = DocumentSource::File { path: "p.pdf".into(), absolute_path: "C:/v/p.pdf".into() };
        assert!(side_chat_preamble(&pdf, "x").contains("read it with Read"));
        assert_eq!(tools_for(&pdf), vec!["Read".to_string()]);
        assert_eq!(tools_for(&doc()), Vec::<String>::new());
    }
}
