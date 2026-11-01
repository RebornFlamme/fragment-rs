// Port de app/src/ui/core/vault/attachments.ts
//
// Les pièces jointes — la partie PURE : où va un fichier collé, et sous quel nom. Aucun
// Vault ni DOM. Les noms et le comportement sont ceux d'Obsidian (« Pasted image … »,
// réglage « Default location for new attachments »).

use serde::{Deserialize, Serialize};

/// `AttachmentLocation` (settings/config.ts) : où écrire une pièce jointe (image collée) :
/// racine, dossier de la note, sous-dossier `attachmentFolder` de la note, ou dossier
/// `attachmentFolder`. Sérialisé en minuscules comme dans `app.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentLocation {
    #[default]
    Root,
    Current,
    Subfolder,
    Folder,
}

/// Retire les `/` en trop et les `./` de tête : `/a//b/` → `a/b`.
fn normaliser(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .map(str::trim)
        .filter(|p| !p.is_empty() && *p != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// Le dossier de `note_path` (`""` = racine).
fn dossier_de(note_path: Option<&str>) -> &str {
    match note_path {
        None | Some("") => "",
        Some(p) => p.rfind('/').map_or("", |i| &p[..i]),
    }
}

/// Le dossier (chemin depuis la racine, `""` = racine) où écrire une pièce jointe collée
/// dans la note `note_path`.
///
/// Un nom de dossier vide retombe sur l'emplacement le plus proche qui a un sens : la
/// racine pour `Folder`, le dossier de la note pour `Subfolder`.
pub fn attachment_folder_for(
    location: AttachmentLocation,
    folder_setting: &str,
    note_path: Option<&str>,
) -> String {
    let nom = normaliser(folder_setting);
    let ici = dossier_de(note_path);
    match location {
        AttachmentLocation::Current => ici.to_owned(),
        AttachmentLocation::Subfolder => {
            if nom.is_empty() {
                ici.to_owned()
            } else {
                normaliser(&format!("{ici}/{nom}"))
            }
        }
        AttachmentLocation::Folder => nom,
        AttachmentLocation::Root => String::new(),
    }
}

/// Une date locale à la seconde. ★ Pas de `chrono` dans le cœur : l'hôte lit l'horloge
/// locale et la passe ici (le TS utilise `getFullYear()`… de l'heure LOCALE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    pub year: i32,
    /// 1 à 12 (le `getMonth() + 1` du TS).
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// `Pasted image 20261004213512.png` — horodaté à la seconde, comme Obsidian.
pub fn pasted_image_name(date: LocalDateTime, ext: &str) -> String {
    format!(
        "Pasted image {}{:02}{:02}{:02}{:02}{:02}.{ext}",
        date.year, date.month, date.day, date.hour, date.minute, date.second
    )
}

/// L'extension d'un type MIME d'image ; png par défaut (le cas d'une capture d'écran).
pub fn extension_for_mime(mime: &str) -> &'static str {
    match mime.to_lowercase().as_str() {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        // image/png et tout type inconnu.
        _ => "png",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AttachmentLocation::*;

    const NOTE: Option<&str> = Some("cours/maths/algebre.md");

    #[test]
    fn root_la_racine() {
        assert_eq!(attachment_folder_for(Root, "attachments", NOTE), "");
    }

    #[test]
    fn current_le_dossier_de_la_note() {
        assert_eq!(
            attachment_folder_for(Current, "attachments", NOTE),
            "cours/maths"
        );
        assert_eq!(
            attachment_folder_for(Current, "attachments", Some("note.md")),
            ""
        );
    }

    #[test]
    fn subfolder_sous_dossier_de_la_note() {
        assert_eq!(
            attachment_folder_for(Subfolder, "attachments", NOTE),
            "cours/maths/attachments"
        );
        assert_eq!(
            attachment_folder_for(Subfolder, "attachments", Some("note.md")),
            "attachments"
        );
    }

    #[test]
    fn folder_le_dossier_indique_normalise() {
        assert_eq!(
            attachment_folder_for(Folder, "/media//images/", NOTE),
            "media/images"
        );
    }

    #[test]
    fn nom_vide_racine_pour_folder_dossier_de_la_note_pour_subfolder() {
        assert_eq!(attachment_folder_for(Folder, "  ", NOTE), "");
        assert_eq!(attachment_folder_for(Subfolder, "", NOTE), "cours/maths");
    }

    #[test]
    fn sans_note_none_relatif_a_la_racine() {
        assert_eq!(
            attachment_folder_for(Subfolder, "attachments", None),
            "attachments"
        );
    }

    #[test]
    fn horodate_a_la_seconde() {
        let d = LocalDateTime {
            year: 2026,
            month: 10,
            day: 4,
            hour: 21,
            minute: 5,
            second: 9,
        };
        assert_eq!(
            pasted_image_name(d, "png"),
            "Pasted image 20261004210509.png"
        );
    }

    #[test]
    fn connus_et_defaut() {
        assert_eq!(extension_for_mime("image/jpeg"), "jpg");
        assert_eq!(extension_for_mime("image/svg+xml"), "svg");
        assert_eq!(extension_for_mime("image/x-inconnu"), "png");
    }

    // ─── Ajoutés ───
    #[test]
    fn le_mime_est_insensible_a_la_casse() {
        assert_eq!(extension_for_mime("IMAGE/GIF"), "gif");
    }

    #[test]
    fn la_localisation_se_lit_comme_dans_app_json() {
        let l: AttachmentLocation = serde_json::from_str("\"subfolder\"").unwrap();
        assert_eq!(l, Subfolder);
    }
}
