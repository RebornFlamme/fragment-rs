// Port de app/src/ui/views/agent/lasso/lassoPrompt.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Ce qu'« Entourer ▸ Ask » envoie à l'agent — pur.
//
//  La zone n'est connue de l'agent QUE par sa capture (décision : des screens,
//  ce qui marche sur toute vue — texte, figure d'un PDF, image, page HTML) : une
//  PIÈCE JOINTE du message (bloc image natif). Le préambule porte le document,
//  comme Ask ; la question suit.
//  Consignes en ANGLAIS ; réponse dans la langue de la question.
// ═══════════════════════════════════════════════════════════════════════════

use crate::side_chat_prompt::DocumentSource;

pub const LASSO_SYSTEM: &str = "The user circled a region of the document with a pen and asks you about it.\nThe attached image is that region as they see it on screen (circle included): look at it first. The document follows.\nAnswer from that context; say so when it does not let you answer.\nBe concise. Answer in the language of the user’s question.";

fn document(doc: &DocumentSource) -> String {
    match doc {
        DocumentSource::Text { path, text } => format!("<document path=\"{path}\">\n{text}\n</document>"),
        DocumentSource::File { path, absolute_path } => {
            format!("<document path=\"{path}\">\nThe document is the file {absolute_path}: read it with Read if you need more than the image.\n</document>")
        }
    }
}

/// Le préambule du premier tour : le document, puis la zone (vue par l'image) — la question suit.
pub fn lasso_preamble(doc: &DocumentSource) -> String {
    format!("{}\n\nThe user circled a region of this document (the attached image). About it:", document(doc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_preambule_le_document_puis_la_zone_vue_par_limage_et_about_it() {
        let p = lasso_preamble(&DocumentSource::Text { path: "cours.md".into(), text: "Le photon passe.".into() });
        assert!(p.contains("<document path=\"cours.md\">\nLe photon passe.\n</document>"));
        assert!(p.contains("the attached image"));
        assert!(p.ends_with("About it:"));
    }

    #[test]
    fn un_document_sans_texte_image_son_fichier_a_lire_avec_read_au_besoin() {
        assert!(lasso_preamble(&DocumentSource::File { path: "fig.png".into(), absolute_path: "C:/v/fig.png".into() }).contains("C:/v/fig.png"));
    }
}
