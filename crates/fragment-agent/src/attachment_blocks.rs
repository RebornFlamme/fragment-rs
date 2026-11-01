// Port de app/src/ui/views/agent/attachmentBlocks.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  Les PIÈCES JOINTES d'un message ↔ les blocs de contenu du SDK. Pur (testé).
//
//    image png / jpeg / gif / webp → bloc `image` NATIF (le modèle la voit, sans Read) ;
//    PDF                           → bloc `document` (base64) ;
//    texte (text/*, json, code…)   → bloc `document` texte, titré par le nom ;
//    tout le reste                 → copié sur le disque (dossier des pièces jointes
//                                    de l'agent), un bloc texte `<attachment …/>` en
//                                    donne le chemin : l'agent l'ouvre (Read, shell).
//  Le transcript du SDK garde ces blocs : la relecture (`attachment_from_block`)
//  rend les pièces jointes de chaque message.
// ═══════════════════════════════════════════════════════════════════════════

use std::sync::LazyLock;

use serde_json::{Value, json};

use crate::contrat::{ChatBlock, Entree};
use crate::motif::Motif;

/// Un bloc de contenu du message utilisateur (forme de l'API Messages).
pub type ContentBlock = Entree;

/// Une pièce jointe d'un message.
// TODO(contrat) : remplacer par fragment_chat::ChatAttachment
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatAttachment {
    pub name: String,
    pub mime: String,
    pub data: Vec<u8>,
}

pub const IMAGES_NATIVES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

static EXT_TEXTE: LazyLock<Motif> = LazyLock::new(|| {
    Motif::new(
        r"(?i)\.(md|markdown|txt|csv|tsv|json|jsonl|ya?ml|toml|xml|html?|css|scss|js|mjs|cjs|ts|tsx|jsx|py|rb|go|rs|java|kt|c|h|cpp|hpp|cs|sh|ps1|bat|sql|tex|bib|ini|cfg|log|svg)$",
    )
});
static MIME_TEXTE: LazyLock<Motif> =
    LazyLock::new(|| Motif::new(r"json|xml|javascript|typescript|yaml|svg"));
static EXT_PDF: LazyLock<Motif> = LazyLock::new(|| Motif::new(r"(?i)\.pdf$"));
static MARQUEUR: LazyLock<Motif> = LazyLock::new(|| {
    Motif::new(r#"^<attachment name="([^"]*)" path="([^"]*)">[^<]*</attachment>$"#)
});

pub use crate::contrat::AttachmentKind;

/// Comment une pièce part au modèle.
pub fn attachment_kind(name: &str, mime: &str) -> AttachmentKind {
    if IMAGES_NATIVES.contains(&mime) {
        return AttachmentKind::Image;
    }
    if mime == "application/pdf" || EXT_PDF.is_match(name) {
        return AttachmentKind::Pdf;
    }
    if mime.starts_with("text/") || MIME_TEXTE.is_match(mime) || EXT_TEXTE.is_match(name) {
        return AttachmentKind::Text;
    }
    AttachmentKind::File
}

/// Base64 standard (RFC 4648, avec remplissage) — pas de dépendance pour 15 lignes.
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn echapper(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn desechapper(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

/// Le marqueur d'un fichier posé sur le disque (bloc texte).
pub fn file_marker(name: &str, path: &str) -> String {
    format!(
        "<attachment name=\"{}\" path=\"{}\">The user attached this file: open it from that path (Read, or the shell).</attachment>",
        echapper(name),
        echapper(path)
    )
}

fn bloc(v: Value) -> ContentBlock {
    match v {
        Value::Object(m) => m,
        _ => Entree::new(),
    }
}

/// Les pièces → les blocs (dans l'ordre). `save_file` range un fichier « autre » et
/// rend son chemin absolu.
pub fn to_content_blocks(
    atts: &[ChatAttachment],
    mut save_file: impl FnMut(&ChatAttachment) -> String,
) -> Vec<ContentBlock> {
    let mut out = Vec::new();
    for a in atts {
        out.push(bloc(match attachment_kind(&a.name, &a.mime) {
            AttachmentKind::Image => json!({ "type": "image", "source": { "type": "base64", "media_type": a.mime, "data": base64(&a.data) } }),
            AttachmentKind::Pdf => {
                json!({ "type": "document", "title": a.name, "source": { "type": "base64", "media_type": "application/pdf", "data": base64(&a.data) } })
            }
            AttachmentKind::Text => json!({
                "type": "document", "title": a.name,
                "source": { "type": "text", "media_type": "text/plain", "data": String::from_utf8_lossy(&a.data) }
            }),
            AttachmentKind::File => json!({ "type": "text", "text": file_marker(&a.name, &save_file(a)) }),
        }));
    }
    out
}

fn chaine<'a>(b: &'a Entree, k: &str) -> Option<&'a str> {
    b.get(k).and_then(Value::as_str)
}

/// Un bloc d'un message utilisateur du transcript → une pièce jointe, ou `None` (texte, résultat…).
pub fn attachment_from_block(b: &Entree) -> Option<ChatBlock> {
    let src = b.get("source").and_then(Value::as_object);
    let champ = |k: &str| {
        src.and_then(|s| s.get(k))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
    };
    let genre = chaine(b, "type");
    if genre == Some("image")
        && champ("type") == Some("base64")
        && let (Some(media), Some(data)) = (champ("media_type"), champ("data"))
    {
        return Some(ChatBlock::Attachment {
            name: "image".into(),
            mime: media.into(),
            url: Some(format!("data:{media};base64,{data}")),
            path: None,
        });
    }
    if genre == Some("document") {
        let pdf = champ("media_type") == Some("application/pdf");
        let name = chaine(b, "title").map(str::to_string).unwrap_or_else(|| {
            if pdf {
                "document.pdf".into()
            } else {
                "document.txt".into()
            }
        });
        return Some(ChatBlock::Attachment {
            name,
            mime: if pdf { "application/pdf" } else { "text/plain" }.into(),
            url: None,
            path: None,
        });
    }
    if genre == Some("text")
        && let Some(m) = chaine(b, "text").and_then(|t| MARQUEUR.captures(t))
    {
        return Some(ChatBlock::Attachment {
            name: desechapper(&m[1]),
            mime: "application/octet-stream".into(),
            url: None,
            path: Some(desechapper(&m[2])),
        });
    }
    None
}

/// Le titre d'une conversation qui commence sans texte : ses pièces.
pub fn titre_des_pieces(noms: &[&str]) -> String {
    let titre: String = noms.join(", ").chars().take(80).collect();
    if titre.is_empty() {
        "Pièce jointe".into()
    } else {
        titre
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn octets(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    fn att(name: &str, mime: &str, data: &str) -> ChatAttachment {
        ChatAttachment {
            name: name.into(),
            mime: mime.into(),
            data: octets(data),
        }
    }

    #[test]
    fn le_type_image_native_pdf_texte_mime_ou_extension_autre() {
        assert_eq!(attachment_kind("a.png", "image/png"), AttachmentKind::Image);
        assert_eq!(attachment_kind("a.bmp", "image/bmp"), AttachmentKind::File);
        assert_eq!(attachment_kind("c.pdf", ""), AttachmentKind::Pdf);
        assert_eq!(
            attachment_kind("n.md", "application/octet-stream"),
            AttachmentKind::Text
        );
        assert_eq!(
            attachment_kind("d.json", "application/json"),
            AttachmentKind::Text
        );
        assert_eq!(
            attachment_kind("x.zip", "application/zip"),
            AttachmentKind::File
        );
    }

    #[test]
    fn aller_image_bloc_image_base64_pdf_document_texte_document_texte_autre_marqueur_avec_le_chemin()
     {
        let mut ranges = Vec::new();
        let blocs = to_content_blocks(
            &[
                att("p.png", "image/png", "PNG"),
                att("c.pdf", "application/pdf", "%PDF"),
                att("n.md", "text/markdown", "# Titre é"),
                att("x \"1\".zip", "application/zip", "PK"),
            ],
            |a| {
                ranges.push(a.name.clone());
                "C:/pj/x.zip".to_string()
            },
        );
        assert_eq!(
            Value::Object(blocs[0].clone()),
            json!({ "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": base64(b"PNG") } })
        );
        assert_eq!(blocs[0]["source"]["data"], "UE5H");
        assert_eq!(blocs[1]["type"], "document");
        assert_eq!(blocs[1]["title"], "c.pdf");
        assert_eq!(blocs[1]["source"]["type"], "base64");
        assert_eq!(blocs[1]["source"]["media_type"], "application/pdf");
        assert_eq!(
            Value::Object(blocs[2].clone()),
            json!({ "type": "document", "title": "n.md", "source": { "type": "text", "media_type": "text/plain", "data": "# Titre é" } })
        );
        assert_eq!(
            Value::Object(blocs[3].clone()),
            json!({ "type": "text", "text": file_marker("x \"1\".zip", "C:/pj/x.zip") })
        );
        assert_eq!(ranges, vec!["x \"1\".zip".to_string()]);
    }

    #[test]
    fn retour_chaque_bloc_redevient_une_piece_nom_type_vignette_dune_image_le_texte_ordinaire_reste_du_texte()
     {
        let blocs = to_content_blocks(
            &[
                att("p.png", "image/png", "PNG"),
                att("c.pdf", "application/pdf", "%PDF"),
                att("n.md", "text/plain", "x"),
                att("x \"1\".zip", "application/zip", "PK"),
            ],
            |_| "C:/pj/a&b.zip".to_string(),
        );
        assert_eq!(
            attachment_from_block(&blocs[0]),
            Some(ChatBlock::Attachment {
                name: "image".into(),
                mime: "image/png".into(),
                url: Some(format!("data:image/png;base64,{}", base64(b"PNG"))),
                path: None
            })
        );
        assert_eq!(
            attachment_from_block(&blocs[1]),
            Some(ChatBlock::Attachment {
                name: "c.pdf".into(),
                mime: "application/pdf".into(),
                url: None,
                path: None
            })
        );
        assert_eq!(
            attachment_from_block(&blocs[2]),
            Some(ChatBlock::Attachment {
                name: "n.md".into(),
                mime: "text/plain".into(),
                url: None,
                path: None
            })
        );
        assert_eq!(
            attachment_from_block(&blocs[3]),
            Some(ChatBlock::Attachment {
                name: "x \"1\".zip".into(),
                mime: "application/octet-stream".into(),
                url: None,
                path: Some("C:/pj/a&b.zip".into())
            })
        );
        assert_eq!(
            attachment_from_block(&bloc(json!({ "type": "text", "text": "une question" }))),
            None
        );
        assert_eq!(
            attachment_from_block(&bloc(json!({ "type": "tool_result", "content": "x" }))),
            None
        );
    }

    #[test]
    fn le_titre_dune_conversation_sans_texte() {
        assert_eq!(titre_des_pieces(&["a.png", "b.pdf"]), "a.png, b.pdf");
        assert_eq!(titre_des_pieces(&[]), "Pièce jointe");
    }

    #[test]
    fn base64_remplissage() {
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"Man"), "TWFu");
    }
}
