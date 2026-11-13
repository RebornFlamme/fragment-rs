// Port de app/src/ui/views/chat/components/attachmentData.ts et de `fromFile` (attachments.ts).
//! Les données d'une pièce jointe : ce qu'on MONTRE (`AttachmentPartData`) et ce qu'on ENVOIE
//! (`ChatAttachment`), sans rien de graphique.

use std::path::{Path, PathBuf};

use fragment_agent::attachment_blocks::base64;
use fragment_agent::fournisseur::pieces::decoder_base64;
use fragment_chat::api::{AttachmentPartData, ChatAttachment, ChatPart};
use serde_json::{Value, json};

/// Une image (montrée en vignette) ?
#[must_use]
pub fn est_image(mime: &str) -> bool {
    mime.starts_with("image/")
}

/// L'URL `data:` d'un contenu : la forme que prend l'image d'un part (celle que la relecture d'une
/// conversation produit aussi).
#[must_use]
pub fn url_donnees(mime: &str, octets: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(octets))
}

/// Le mime et les octets d'une URL `data:…;base64,…`, ou `None`.
#[must_use]
pub fn octets_de_url(url: &str) -> Option<(String, Vec<u8>)> {
    let reste = url.strip_prefix("data:")?;
    let (entete, corps) = reste.split_once(',')?;
    let mime = entete.strip_suffix(";base64")?;
    Some((mime.to_owned(), decoder_base64(corps)?))
}

/// Les données MONTRÉES d'une pièce (part `attachment`) : une image reçoit son URL.
#[must_use]
pub fn part_data(a: &ChatAttachment) -> AttachmentPartData {
    AttachmentPartData {
        name: a.name.clone(),
        mime: a.mime.clone(),
        url: est_image(&a.mime).then(|| url_donnees(&a.mime, &a.data)),
    }
}

/// Le part `attachment` d'une pièce, tel que le message le garde.
#[must_use]
pub fn part_attachment(a: &ChatAttachment) -> ChatPart {
    ChatPart {
        kind: "attachment".into(),
        data: serde_json::to_value(part_data(a)).ok(),
        ..ChatPart::default()
    }
}

/// Le nom d'une pièce sans nom : `image.<extension>` (`jpeg` devient `jpg`, hors image `bin`).
#[must_use]
pub fn nom_par_defaut(mime: &str) -> String {
    let ext = mime
        .strip_prefix("image/")
        .map_or("bin".to_owned(), |e| e.replace("jpeg", "jpg"));
    format!("image.{ext}")
}

/// Le mime d'un fichier d'après son extension (le dépôt d'un fichier ne dit pas son type).
#[must_use]
pub fn mime_de_nom(nom: &str) -> &'static str {
    let ext = nom.rsplit_once('.').map_or("", |(_, e)| e).to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// Une pièce depuis un contenu : le nom manquant devient `image.<ext>`, le mime manquant
/// `application/octet-stream` (`fromFile`).
#[must_use]
pub fn piece_depuis_octets(
    nom: Option<&str>,
    mime: Option<&str>,
    octets: Vec<u8>,
) -> ChatAttachment {
    let mime = mime
        .filter(|m| !m.is_empty())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let name = nom
        .filter(|n| !n.is_empty())
        .map_or_else(|| nom_par_defaut(&mime), str::to_owned);
    ChatAttachment {
        name,
        mime,
        data: octets,
    }
}

/// Les fichiers d'un dépôt : un fichier illisible, ou un dossier, est ignoré.
#[must_use]
pub fn pieces_de_chemins(chemins: &[PathBuf]) -> Vec<ChatAttachment> {
    chemins.iter().filter_map(|c| piece_de_chemin(c)).collect()
}

fn piece_de_chemin(c: &Path) -> Option<ChatAttachment> {
    if !c.is_file() {
        return None;
    }
    let nom = c.file_name()?.to_str()?;
    let octets = std::fs::read(c).ok()?;
    Some(piece_depuis_octets(
        Some(nom),
        Some(mime_de_nom(nom)),
        octets,
    ))
}

/// Une pièce en JSON, pour `SendOpts` et `save` : `{name, mime, data}` (octets en base64, le format
/// que lit `fragment-agent::fournisseur`).
#[must_use]
pub fn piece_json(a: &ChatAttachment) -> Value {
    json!({ "name": a.name, "mime": a.mime, "data": base64(&a.data) })
}

/// L'inverse de [`piece_json`] ; `None` si la forme n'est pas celle-là.
#[must_use]
pub fn piece_depuis_json(v: &Value) -> Option<ChatAttachment> {
    Some(ChatAttachment {
        name: v.get("name")?.as_str()?.to_owned(),
        mime: v.get("mime")?.as_str()?.to_owned(),
        data: decoder_base64(v.get("data")?.as_str()?)?,
    })
}
