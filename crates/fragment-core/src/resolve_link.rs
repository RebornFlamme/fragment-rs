// Port de app/src/ui/core/links/resolveLink.ts
//! La résolution d'un lien interne — le rôle qu'aura le MetadataCache (§7.4)
//! quand il existera. En attendant, une passe directe sur le coffre.
//!
//! L'API (`parse_linktext` + `resolve_link`) est pensée pour que le jour où
//! `getFirstLinkpathDest` arrive, on remplace le CORPS de `resolve_link` sans
//! toucher un seul appelant.
//!
//! Le coffre n'est pas encore porté (groupe `core-b`) : `resolve_link` ne lit
//! que `path`/`name`/`basename`/`extension` d'un fichier et deux méthodes du
//! coffre — exactement ce que le test TS simulait. Ces deux traits en sont la
//! mesure ; `TFile`/`Vault` les implémenteront.

/// Ce que `resolve_link` lit d'un fichier du coffre (le sous-ensemble de `TFile`).
pub trait LinkFile: Clone {
    /// Chemin depuis la racine du coffre, séparateur `/`.
    fn path(&self) -> &str;
    /// Nom avec extension.
    fn name(&self) -> &str;
    /// Nom sans extension.
    fn basename(&self) -> &str;
}

/// Ce que `resolve_link` lit du coffre (`getFileByPath` et `getFiles`).
pub trait LinkVault {
    type File: LinkFile;
    fn get_file_by_path(&self, path: &str) -> Option<Self::File>;
    fn get_files(&self) -> Vec<Self::File>;
}

/// Le contenu d'un `[[…]]` / `![[…]]`, découpé en ses parties.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedLink {
    /// La cible seule : sans alias, sans sous-chemin, sans taille.
    pub path: String,
    /// `#titre` ou `^bloc`, vide sinon.
    pub subpath: String,
    /// Le texte après `|` — alias d'affichage OU taille pour un embed image.
    pub alias: Option<String>,
    /// Largeur, si l'alias est une taille (`200` ou `200x100`).
    pub width: Option<u32>,
    /// Hauteur, si l'alias est `200x100`.
    pub height: Option<u32>,
}

/// L'alias est-il une taille `200` ou `200x100` ? (le `/^(\d+)(?:x(\d+))?$/` du TS)
fn taille(alias: &str) -> Option<(u32, Option<u32>)> {
    let (l, h) = match alias.split_once('x') {
        Some((l, h)) => (l, Some(h)),
        None => (alias, None),
    };
    let chiffres = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !chiffres(l) || h.is_some_and(|h| !chiffres(h)) {
        return None;
    }
    let largeur = l.parse().ok()?;
    let hauteur = match h {
        Some(h) => Some(h.parse().ok()?),
        None => None,
    };
    Some((largeur, hauteur))
}

/// Découpe `cible#sous^bloc|alias-ou-taille` en ses parties.
///
/// L'ordre de découpe compte : le `|` d'abord (il termine tout), puis le
/// sous-chemin `#`/`^` sur ce qui reste.
pub fn parse_linktext(inner: &str) -> ParsedLink {
    let mut rest = inner.trim();

    // 1. l'alias / la taille (après le premier '|')
    let mut alias: Option<String> = None;
    if let Some((avant, apres)) = rest.split_once('|') {
        alias = Some(apres.trim().to_owned());
        rest = avant.trim();
    }

    // 2. le sous-chemin (#titre ou ^bloc)
    let mut subpath = String::new();
    if let Some(sub) = rest.find(['#', '^']) {
        subpath = rest[sub..].to_owned();
        rest = rest[..sub].trim();
    }

    // 3. l'alias d'un embed image porte la taille : "200" ou "200x100"
    let mut width = None;
    let mut height = None;
    if let Some((l, h)) = alias.as_deref().filter(|a| !a.is_empty()).and_then(taille) {
        width = Some(l);
        height = h;
    }

    ParsedLink {
        path: rest.to_owned(),
        subpath,
        alias,
        width,
        height,
    }
}

/// Le `/\.[^/.]+$/` du TS : une extension dans le dernier segment.
fn a_une_extension(target: &str) -> bool {
    let dernier = target.rsplit('/').next().unwrap_or(target);
    match dernier.rfind('.') {
        Some(i) => i + 1 < dernier.len(),
        None => false,
    }
}

/// Résout une cible de lien vers un fichier, ou `None` si rien ne correspond.
///
/// MVP sans MetadataCache, en trois passes du plus précis au plus large :
///   1. chemin exact depuis la racine du coffre ;
///   2. chemin exact + `.md` (un `[[Note]]` désigne `Note.md`) ;
///   3. par nom de fichier n'importe où — le CHEMIN LE PLUS COURT gagne, ce qui
///      approxime la règle du « chemin le plus court » d'Obsidian.
///
/// `source_path` est ignoré pour l'instant (pas de résolution relative) mais reste
/// dans la signature : c'est le point d'entrée du futur index.
pub fn resolve_link<V: LinkVault>(
    vault: &V,
    linkpath: &str,
    source_path: Option<&str>,
) -> Option<V::File> {
    let _ = source_path;
    let target = linkpath.trim().replace('\\', "/");
    let target = target.strip_prefix("./").unwrap_or(&target);
    if target.is_empty() {
        return None;
    }

    // 1. chemin exact
    if let Some(exact) = vault.get_file_by_path(target) {
        return Some(exact);
    }

    // 2. chemin exact + .md (extension implicite d'un lien interne)
    let has_ext = a_une_extension(target);
    if !has_ext && let Some(md) = vault.get_file_by_path(&format!("{target}.md")) {
        return Some(md);
    }

    // 3. par nom de fichier (basename si sans extension, name sinon)
    let wanted = target.to_lowercase();
    let mut candidates: Vec<V::File> = vault
        .get_files()
        .into_iter()
        .filter(|f| {
            if has_ext {
                f.name().to_lowercase() == wanted
                    || f.path().to_lowercase().ends_with(&format!("/{wanted}"))
            } else {
                f.basename().to_lowercase() == wanted
            }
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }

    // Tri stable : à longueur égale, l'ordre du coffre départage, comme en TS.
    candidates.sort_by_key(|f| f.path().len());
    candidates.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Fakes minimaux : resolve_link ne lit que path/name/basename et
    //    get_file_by_path / get_files. On n'a pas besoin d'un vrai Vault. ──

    #[derive(Clone, Debug, PartialEq)]
    struct FakeFile {
        path: String,
        name: String,
        basename: String,
    }

    impl LinkFile for FakeFile {
        fn path(&self) -> &str {
            &self.path
        }
        fn name(&self) -> &str {
            &self.name
        }
        fn basename(&self) -> &str {
            &self.basename
        }
    }

    fn fake_file(path: &str) -> FakeFile {
        let name = path.rsplit('/').next().unwrap().to_owned();
        let basename = match name.rfind('.') {
            Some(dot) if dot > 0 => name[..dot].to_owned(),
            _ => name.clone(),
        };
        FakeFile {
            path: path.to_owned(),
            name,
            basename,
        }
    }

    struct FakeApp {
        files: Vec<FakeFile>,
    }

    impl LinkVault for FakeApp {
        type File = FakeFile;
        fn get_file_by_path(&self, p: &str) -> Option<FakeFile> {
            self.files.iter().find(|f| f.path == p).cloned()
        }
        fn get_files(&self) -> Vec<FakeFile> {
            self.files.clone()
        }
    }

    fn fake_app(paths: &[&str]) -> FakeApp {
        FakeApp {
            files: paths.iter().map(|p| fake_file(p)).collect(),
        }
    }

    fn chemin(f: Option<FakeFile>) -> Option<String> {
        f.map(|f| f.path)
    }

    // parse_linktext

    #[test]
    fn separe_la_cible_seule() {
        let p = parse_linktext("Ma note");
        assert_eq!(
            (p.path.as_str(), p.subpath.as_str(), p.alias),
            ("Ma note", "", None)
        );
    }

    #[test]
    fn lit_un_alias_texte() {
        let p = parse_linktext("Note|Un alias");
        assert_eq!(
            (p.path.as_str(), p.alias.as_deref()),
            ("Note", Some("Un alias"))
        );
    }

    #[test]
    fn lit_une_taille_largeur_seule() {
        let p = parse_linktext("img.png|200");
        assert_eq!(
            (p.path.as_str(), p.width, p.height),
            ("img.png", Some(200), None)
        );
    }

    #[test]
    fn lit_une_taille_largeur_x_hauteur() {
        let p = parse_linktext("img.png|200x100");
        assert_eq!(
            (p.path.as_str(), p.width, p.height),
            ("img.png", Some(200), Some(100))
        );
    }

    #[test]
    fn separe_le_sous_chemin_titre() {
        let p = parse_linktext("Note#Section");
        assert_eq!((p.path.as_str(), p.subpath.as_str()), ("Note", "#Section"));
    }

    #[test]
    fn separe_le_sous_chemin_bloc() {
        let p = parse_linktext("Note^abc123");
        assert_eq!((p.path.as_str(), p.subpath.as_str()), ("Note", "^abc123"));
    }

    #[test]
    fn combine_sous_chemin_et_alias() {
        let p = parse_linktext("Note#Section|Alias");
        assert_eq!(
            (p.path.as_str(), p.subpath.as_str(), p.alias.as_deref()),
            ("Note", "#Section", Some("Alias"))
        );
    }

    // resolve_link

    #[test]
    fn resout_un_chemin_exact() {
        let app = fake_app(&["dossier/photo.png", "note.md"]);
        assert_eq!(
            chemin(resolve_link(&app, "dossier/photo.png", None)).as_deref(),
            Some("dossier/photo.png")
        );
    }

    #[test]
    fn ajoute_md_implicitement_pour_un_lien_de_note() {
        let app = fake_app(&["Idées.md"]);
        assert_eq!(
            chemin(resolve_link(&app, "Idées", None)).as_deref(),
            Some("Idées.md")
        );
    }

    #[test]
    fn resout_par_basename_quand_le_chemin_nu_echoue() {
        let app = fake_app(&["a/b/Cible.md"]);
        assert_eq!(
            chemin(resolve_link(&app, "Cible", None)).as_deref(),
            Some("a/b/Cible.md")
        );
    }

    #[test]
    fn resout_une_image_par_nom_de_fichier_n_importe_ou() {
        let app = fake_app(&["assets/logo.png"]);
        assert_eq!(
            chemin(resolve_link(&app, "logo.png", None)).as_deref(),
            Some("assets/logo.png")
        );
    }

    #[test]
    fn prefere_le_chemin_le_plus_court_en_cas_d_homonymes() {
        let app = fake_app(&["a/b/c/Note.md", "Note.md"]);
        assert_eq!(
            chemin(resolve_link(&app, "Note", None)).as_deref(),
            Some("Note.md")
        );
    }

    #[test]
    fn rend_null_quand_rien_ne_correspond() {
        let app = fake_app(&["autre.md"]);
        assert_eq!(resolve_link(&app, "Inexistant", None), None);
    }

    #[test]
    fn ignore_une_cible_vide() {
        let app = fake_app(&["note.md"]);
        assert_eq!(resolve_link(&app, "   ", None), None);
    }
}
