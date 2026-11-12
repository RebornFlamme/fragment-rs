//! Le schéma des scènes `pixels/scenes/*.json` et l'analyse de leurs actions.
//!
//! Aucune dépendance à GPUI ici : c'est du JSON typé, testable partout.
//! ★ Une action inconnue ou mal formée est une ERREUR franche (nom de scène, rang de l'action,
//! cause), jamais un « ignoré » : une scène à moitié rejouée produirait une capture trompeuse.

use serde::Deserialize;
use serde_json::Value;

/// Erreurs de lecture d'une scène.
#[derive(Debug, thiserror::Error)]
pub enum ErreurScene {
    #[error("scène « {scene} » : JSON illisible : {source}")]
    Json {
        scene: String,
        source: serde_json::Error,
    },
    #[error("scène « {scene} » : action n°{rang} invalide : {source}")]
    Action {
        scene: String,
        /// Rang à partir de 1, comme on le compte dans le fichier.
        rang: usize,
        source: serde_json::Error,
    },
    #[error("scène « {scene} » : action n°{rang} : touche invalide : {message}")]
    Touche {
        scene: String,
        rang: usize,
        message: String,
    },
    #[error("scène « {scene} » : tranche « {valeur} » inconnue (attendu T1, T2, …)")]
    Tranche { scene: String, valeur: String },
    #[error("lecture de {chemin} : {source}")]
    Lecture {
        chemin: String,
        source: std::io::Error,
    },
    #[error("tranches actives ({chemin}) : {message}")]
    Tranches { chemin: String, message: String },
}

/// Une tranche du programme (`T1`, `T3`…), ordonnée par son numéro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tranche(pub u32);

impl Tranche {
    /// `T3` → `Tranche(3)`.
    pub fn parser(texte: &str) -> Option<Self> {
        let chiffres = texte.strip_prefix('T')?;
        if chiffres.is_empty() || !chiffres.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        chiffres.parse().ok().map(Self)
    }
}

/// Lit les tranches actives dans `pixels/tranches.json` (`{"actives": ["T1", "T4"]}`).
/// ★ Une seule source, lue aussi par `cargo xtask pixels` : une scène marquée `"depuis": "T4"` est
/// rejouée et comparée dès que T4 est active, quel que soit l'état de T3. Fichier absent,
/// illisible ou mal formé : ERREUR, jamais une liste vide silencieuse.
pub fn lire_tranches(chemin: &std::path::Path) -> Result<Vec<Tranche>, ErreurScene> {
    let erreur = |message: String| ErreurScene::Tranches {
        chemin: chemin.display().to_string(),
        message,
    };
    let texte = std::fs::read_to_string(chemin).map_err(|e| erreur(format!("lecture : {e}")))?;
    let json: Value =
        serde_json::from_str(&texte).map_err(|e| erreur(format!("JSON illisible : {e}")))?;
    let liste = json["actives"]
        .as_array()
        .ok_or_else(|| erreur("le champ « actives » (liste) manque".to_owned()))?;
    liste
        .iter()
        .map(|t| {
            t.as_str()
                .and_then(Tranche::parser)
                .ok_or_else(|| erreur(format!("tranche {t} inconnue (attendu T1, T2, …)")))
        })
        .collect()
}

/// Taille d'une fenêtre, en pixels logiques, et facteur d'échelle.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Taille {
    pub width: f64,
    pub height: f64,
    #[serde(default = "un_f64")]
    pub dpr: f64,
}

impl Taille {
    /// Taille de l'image attendue, en pixels physiques.
    pub fn physique(self) -> (u32, u32) {
        (
            (self.width * self.dpr) as u32,
            (self.height * self.dpr) as u32,
        )
    }
}

/// Le thème d'une capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeScene {
    Dark,
    Light,
}

impl ThemeScene {
    /// Le suffixe des PNG : `<scène>-dark.png`.
    pub fn suffixe(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// La vue montée dans la fenêtre principale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VueScene {
    /// L'application (défaut).
    #[default]
    App,
    /// La mire de contrôle du rasterizeur.
    Mire,
    /// La vue jouet qui réagit au clic, à la touche et au glisser (preuve du lecteur).
    Jouet,
    /// Un fond de thème et les surfaces flottantes (notice, menu, infobulle, barre d'outils).
    Flottantes,
}

/// Où un geste est envoyé.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cible {
    /// La fenêtre principale.
    #[default]
    Main,
    /// La fenêtre de réglages (2e fenêtre, `settings_window`).
    Settings,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bouton {
    #[default]
    Left,
    Right,
    Middle,
}

/// Un pixel que la capture doit porter : la scène s'auto-contrôle (preuve par le rendu, sans
/// référence Fragment). Utile aux scènes de preuve du lecteur (`jouet-*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PixelAttendu {
    pub x: u32,
    pub y: u32,
    pub rgb: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Une action de scène. Le nom JSON est `type` (kebab-case).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Action {
    /// Exécute une commande du `CommandManager` ; `args` est publié avant, sur l'événement
    /// `pixels:args` (une `serde_json::Value`).
    Command {
        id: String,
        #[serde(default)]
        args: Option<Value>,
    },
    /// Ouvre un fichier du coffre (événement `pixels:open`) : décor T3 et suivants.
    Open {
        path: String,
        #[serde(default)]
        split: bool,
    },
    Click {
        x: f32,
        y: f32,
        #[serde(default)]
        button: Bouton,
        #[serde(default = "un_u32")]
        count: u32,
        #[serde(default)]
        target: Cible,
    },
    Hover {
        x: f32,
        y: f32,
        #[serde(default)]
        target: Cible,
    },
    /// Une combinaison, à la manière de Playwright : `Enter`, `Control+P`, `Shift+Tab`.
    Key {
        keys: String,
        #[serde(default)]
        target: Cible,
    },
    /// Du texte, caractère par caractère.
    Type {
        text: String,
        #[serde(default)]
        target: Cible,
    },
    Wait {
        ms: u64,
    },
    /// Polices prêtes, 2 images, 150 ms, SANS bouger le pointeur.
    WaitStable,
    /// Charge un arbre d'onglets de vues vides (événement `pixels:layout`).
    Layout {
        tree: Value,
    },
    /// Un geste continu : appui en `from`, déplacements intermédiaires bouton enfoncé, puis
    /// relâchement en `to` si `release` (sinon le pointeur reste maintenu pour la capture).
    Drag {
        from: Point,
        to: Point,
        #[serde(default = "vrai")]
        release: bool,
        #[serde(default = "dix_u32")]
        steps: u32,
        #[serde(default)]
        button: Bouton,
        #[serde(default)]
        target: Cible,
    },
}

/// Les commandes de scène du thème (T11) : l'id et les `args` que les scènes `t11-*` publient sur
/// `pixels:args` avant la commande. Le module `fragment-ui/themes` les branche sur `ThemesCoffre`.
pub const COMMANDE_THEME: &str = "pixels:theme-set";
/// Voir [`COMMANDE_THEME`] : choisit l'accent des réglages.
pub const COMMANDE_ACCENT: &str = "pixels:accent-set";

/// `args` de `pixels:theme-set` : le nom du thème de coffre (`""` = la palette de Fragment).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArgsTheme {
    pub name: String,
}

/// `args` de `pixels:accent-set` : la couleur choisie (`null` = l'accent du thème).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArgsAccent {
    pub couleur: Option<String>,
}

fn un_f64() -> f64 {
    1.0
}
fn un_u32() -> u32 {
    1
}
fn dix_u32() -> u32 {
    10
}
fn vrai() -> bool {
    true
}

/// Une combinaison de touches décomposée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Touche {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    /// `Meta` / `Cmd` (la touche « plateforme » de GPUI).
    pub meta: bool,
    /// Le nom de la touche pour GPUI : `enter`, `escape`, `a`…
    pub cle: String,
    /// Le texte que la touche produirait seule (`None` pour les touches nommées).
    pub caractere: Option<String>,
}

impl Touche {
    /// Analyse `Control+Shift+P`, `Enter`, `a`. Le dernier élément est la touche, les autres des
    /// modificateurs.
    pub fn parser(texte: &str) -> Result<Self, String> {
        // `+` seul est la touche plus ; `Control++` est Ctrl et plus.
        let (modifs, touche) = match texte.rsplit_once('+') {
            Some((m, t)) if !t.is_empty() => (m, t),
            Some((m, _)) => (m.trim_end_matches('+'), "+"),
            None => ("", texte),
        };
        let mut t = Self {
            control: false,
            alt: false,
            shift: false,
            meta: false,
            cle: String::new(),
            caractere: None,
        };
        for m in modifs.split('+').filter(|m| !m.is_empty()) {
            match m.to_ascii_lowercase().as_str() {
                "control" | "ctrl" => t.control = true,
                "alt" | "option" => t.alt = true,
                "shift" => t.shift = true,
                "meta" | "cmd" | "command" | "super" => t.meta = true,
                autre => return Err(format!("modificateur inconnu « {autre} » dans « {texte} »")),
            }
        }
        if touche.is_empty() {
            return Err("touche vide".to_owned());
        }
        if let Some(nom) = nom_de_touche(touche) {
            t.cle = nom.to_owned();
            if nom == "space" {
                t.caractere = Some(" ".to_owned());
            }
            return Ok(t);
        }
        let mut caracteres = touche.chars();
        match (caracteres.next(), caracteres.next()) {
            (Some(c), None) => {
                t.caractere = Some(c.to_string());
                t.cle = c.to_lowercase().collect();
                if c.is_uppercase() {
                    t.shift = true;
                }
                // Un raccourci n'insère pas de texte.
                if t.control || t.alt || t.meta {
                    t.caractere = None;
                }
                Ok(t)
            }
            _ => Err(format!("touche inconnue « {touche} »")),
        }
    }

    /// Une touche produisant le caractère `c` (pour `type`).
    pub fn pour_caractere(c: char) -> Self {
        let nom = match c {
            ' ' => Some("space"),
            '\n' => Some("enter"),
            '\t' => Some("tab"),
            _ => None,
        };
        Self {
            control: false,
            alt: false,
            shift: c.is_uppercase(),
            meta: false,
            cle: nom.map_or_else(|| c.to_lowercase().collect(), str::to_owned),
            caractere: Some(c.to_string()),
        }
    }
}

/// Noms Playwright vers noms GPUI.
fn nom_de_touche(nom: &str) -> Option<&'static str> {
    Some(match nom {
        "Enter" | "Return" => "enter",
        "Escape" | "Esc" => "escape",
        "Tab" => "tab",
        "Backspace" => "backspace",
        "Delete" => "delete",
        "Space" | " " => "space",
        "ArrowUp" | "Up" => "up",
        "ArrowDown" | "Down" => "down",
        "ArrowLeft" | "Left" => "left",
        "ArrowRight" | "Right" => "right",
        "Home" => "home",
        "End" => "end",
        "PageUp" => "pageup",
        "PageDown" => "pagedown",
        "F1" => "f1",
        "F2" => "f2",
        "F3" => "f3",
        "F4" => "f4",
        "F5" => "f5",
        "F6" => "f6",
        "F7" => "f7",
        "F8" => "f8",
        "F9" => "f9",
        "F10" => "f10",
        "F11" => "f11",
        "F12" => "f12",
        _ => return None,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneBrute {
    scene: Option<String>,
    window: Option<Taille>,
    settings_window: Option<Taille>,
    themes: Option<Vec<ThemeScene>>,
    /// Le coffre de fixture : lu par les tâches de décor (T3+), pas par le lecteur.
    #[allow(dead_code)]
    vault: Option<String>,
    #[serde(default)]
    vue: VueScene,
    depuis: Option<String>,
    /// Scène de preuve de la CI Rust, sans équivalent Fragment (`mire`, `jouet-*`).
    #[serde(default)]
    sans_reference: bool,
    #[serde(default)]
    pixels_attendus: Vec<PixelAttendu>,
    #[serde(default)]
    actions: Vec<Value>,
}

/// Une scène analysée.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub nom: String,
    pub fenetre: Taille,
    /// Présente si la scène capture la fenêtre de réglages (1000 × 700).
    pub reglages: Option<Taille>,
    pub themes: Vec<ThemeScene>,
    pub vue: VueScene,
    pub depuis: Option<Tranche>,
    pub sans_reference: bool,
    /// Pixels que chaque capture doit porter (voir [`PixelAttendu`]).
    pub pixels_attendus: Vec<PixelAttendu>,
    pub actions: Vec<Action>,
}

impl Scene {
    /// Vrai si la tranche de la scène n'est pas active : ni rejouée ni comparée.
    pub fn en_attente_de_tranche(&self, actives: &[Tranche]) -> bool {
        self.depuis.is_some_and(|d| !actives.contains(&d))
    }

    /// La taille de l'image capturée : celle de la fenêtre de réglages si la scène en a une.
    pub fn taille_capture(&self) -> Taille {
        self.reglages.unwrap_or(self.fenetre)
    }
}

/// Analyse le texte JSON d'une scène. `nom_par_defaut` sert quand le JSON n'a pas de champ `scene`
/// (le nom du fichier sans extension).
pub fn analyser_scene(texte: &str, nom_par_defaut: &str) -> Result<Scene, ErreurScene> {
    let brute: SceneBrute = serde_json::from_str(texte).map_err(|source| ErreurScene::Json {
        scene: nom_par_defaut.to_owned(),
        source,
    })?;
    let nom = brute.scene.unwrap_or_else(|| nom_par_defaut.to_owned());
    let depuis = match brute.depuis {
        None => None,
        Some(v) => Some(Tranche::parser(&v).ok_or(ErreurScene::Tranche {
            scene: nom.clone(),
            valeur: v,
        })?),
    };
    let mut actions = Vec::with_capacity(brute.actions.len());
    for (i, valeur) in brute.actions.into_iter().enumerate() {
        let rang = i + 1;
        let action: Action =
            serde_json::from_value(valeur).map_err(|source| ErreurScene::Action {
                scene: nom.clone(),
                rang,
                source,
            })?;
        if let Action::Key { keys, .. } = &action {
            Touche::parser(keys).map_err(|message| ErreurScene::Touche {
                scene: nom.clone(),
                rang,
                message,
            })?;
        }
        actions.push(action);
    }
    Ok(Scene {
        nom,
        fenetre: brute.window.unwrap_or(Taille {
            width: 1280.0,
            height: 800.0,
            dpr: 1.0,
        }),
        reglages: brute.settings_window,
        themes: brute.themes.unwrap_or_else(|| vec![ThemeScene::Dark]),
        vue: brute.vue,
        depuis,
        sans_reference: brute.sans_reference,
        pixels_attendus: brute.pixels_attendus,
        actions,
    })
}

/// Lit toutes les scènes `*.json` d'un dossier, triées par nom de fichier.
pub fn lire_scenes(dossier: &std::path::Path) -> Result<Vec<Scene>, ErreurScene> {
    let lecture = |chemin: &std::path::Path, source| ErreurScene::Lecture {
        chemin: chemin.display().to_string(),
        source,
    };
    let mut chemins: Vec<std::path::PathBuf> = std::fs::read_dir(dossier)
        .map_err(|e| lecture(dossier, e))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    chemins.sort();
    let mut scenes = Vec::new();
    for chemin in chemins {
        let texte = std::fs::read_to_string(&chemin).map_err(|e| lecture(&chemin, e))?;
        let nom = chemin
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        scenes.push(analyser_scene(&texte, &nom)?);
    }
    Ok(scenes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(actions: &str) -> String {
        format!(r#"{{"scene":"t","actions":{actions}}}"#)
    }

    #[test]
    fn une_scene_sans_champ_prend_les_defauts() {
        let s = analyser_scene("{}", "vide").unwrap();
        assert_eq!(s.nom, "vide");
        assert_eq!(s.fenetre.physique(), (1280, 800));
        assert_eq!(s.themes, vec![ThemeScene::Dark]);
        assert_eq!(s.vue, VueScene::App);
        assert!(s.actions.is_empty() && s.reglages.is_none() && s.depuis.is_none());
    }

    #[test]
    fn toutes_les_actions_du_schema_sont_lues() {
        let s = analyser_scene(
            &scene(
                r#"[
                {"type":"command","id":"a:b","args":{"k":1}},
                {"type":"open","path":"note.md","split":true},
                {"type":"click","x":1,"y":2,"button":"right","count":2,"target":"settings"},
                {"type":"hover","x":3,"y":4},
                {"type":"key","keys":"Control+Shift+P"},
                {"type":"type","text":"Bonjour"},
                {"type":"wait","ms":10},
                {"type":"wait-stable"},
                {"type":"layout","tree":{"type":"tabs"}},
                {"type":"drag","from":{"x":1,"y":2},"to":{"x":30,"y":40},"release":false,"steps":4}
            ]"#,
            ),
            "t",
        )
        .unwrap();
        assert_eq!(s.actions.len(), 10);
        assert!(matches!(
            s.actions[2],
            Action::Click {
                button: Bouton::Right,
                count: 2,
                target: Cible::Settings,
                ..
            }
        ));
        assert!(matches!(
            s.actions[9],
            Action::Drag {
                release: false,
                steps: 4,
                ..
            }
        ));
    }

    #[test]
    fn les_defauts_du_clic_et_du_glisser() {
        let s = analyser_scene(
            &scene(
                r#"[{"type":"click","x":1,"y":2},
                    {"type":"drag","from":{"x":0,"y":0},"to":{"x":5,"y":5}}]"#,
            ),
            "t",
        )
        .unwrap();
        assert!(matches!(
            s.actions[0],
            Action::Click {
                button: Bouton::Left,
                count: 1,
                target: Cible::Main,
                ..
            }
        ));
        assert!(matches!(
            s.actions[1],
            Action::Drag {
                release: true,
                steps: 10,
                ..
            }
        ));
    }

    #[test]
    fn une_action_inconnue_est_une_erreur_claire() {
        let e = analyser_scene(
            &scene(r#"[{"type":"wait","ms":1},{"type":"teleporte"}]"#),
            "t",
        )
        .unwrap_err();
        let m = e.to_string();
        assert!(m.contains("scène « t »") && m.contains("n°2"), "{m}");
        assert!(m.contains("teleporte"), "{m}");
    }

    #[test]
    fn un_champ_manquant_ou_en_trop_est_une_erreur() {
        assert!(analyser_scene(&scene(r#"[{"type":"click","x":1}]"#), "t").is_err());
        assert!(analyser_scene(&scene(r#"[{"type":"wait","ms":1,"vite":true}]"#), "t").is_err());
        assert!(analyser_scene(r#"{"inconnu":1}"#, "t").is_err());
    }

    #[test]
    fn une_touche_invalide_est_une_erreur() {
        let e = analyser_scene(&scene(r#"[{"type":"key","keys":"Hyper+Q"}]"#), "t").unwrap_err();
        assert!(e.to_string().contains("modificateur inconnu"), "{e}");
        assert!(analyser_scene(&scene(r#"[{"type":"key","keys":"Entrée"}]"#), "t").is_err());
    }

    #[test]
    fn les_tranches_se_comparent_par_numero() {
        assert_eq!(Tranche::parser("T3"), Some(Tranche(3)));
        assert!(Tranche::parser("T11") > Tranche::parser("T3"));
        assert_eq!(Tranche::parser("3"), None);
        assert_eq!(Tranche::parser("Tx"), None);
        let s = analyser_scene(r#"{"depuis":"T3"}"#, "t").unwrap();
        assert!(s.en_attente_de_tranche(&[Tranche(1)]));
        assert!(!s.en_attente_de_tranche(&[Tranche(3)]));
        // T4 active sans T3 : une scène « depuis T4 » est jouée, une « depuis T3 » attend.
        let t4 = analyser_scene(r#"{"depuis":"T4"}"#, "t").unwrap();
        let actives = [Tranche(1), Tranche(4)];
        assert!(!t4.en_attente_de_tranche(&actives));
        assert!(s.en_attente_de_tranche(&actives));
        // Pas de « depuis » : toujours jouée.
        assert!(
            !analyser_scene("{}", "t")
                .unwrap()
                .en_attente_de_tranche(&[])
        );
        assert!(analyser_scene(r#"{"depuis":"bientot"}"#, "t").is_err());
    }

    #[test]
    fn les_tranches_actives_sont_lues_et_les_erreurs_sont_claires() {
        let dossier = std::env::temp_dir().join(format!("tranches-app-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let f = dossier.join("tranches.json");
        std::fs::write(&f, r#"{"actives":["T1","T4"]}"#).unwrap();
        assert_eq!(lire_tranches(&f).unwrap(), vec![Tranche(1), Tranche(4)]);
        for mauvais in [r#"{"actives":["quatre"]}"#, "{ pas du json", "{}"] {
            std::fs::write(&f, mauvais).unwrap();
            assert!(lire_tranches(&f).is_err(), "{mauvais}");
        }
        let e = lire_tranches(&dossier.join("absent.json")).unwrap_err();
        assert!(e.to_string().contains("absent.json"), "{e}");
        let _ = std::fs::remove_dir_all(&dossier);
    }

    #[test]
    fn le_fichier_du_depot_active_t1() {
        let f = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../pixels/tranches.json");
        assert!(lire_tranches(&f).unwrap().contains(&Tranche(1)));
    }

    #[test]
    fn la_capture_suit_la_fenetre_de_reglages() {
        let s = analyser_scene(
            r#"{"settings_window":{"width":1000,"height":700,"dpr":1}}"#,
            "t",
        )
        .unwrap();
        assert_eq!(s.taille_capture().physique(), (1000, 700));
    }

    #[test]
    fn les_touches_se_decomposent() {
        let t = Touche::parser("Control+Shift+P").unwrap();
        assert!(t.control && t.shift && !t.alt && !t.meta);
        assert_eq!((t.cle.as_str(), t.caractere), ("p", None));
        let e = Touche::parser("Enter").unwrap();
        assert_eq!((e.cle.as_str(), e.caractere), ("enter", None));
        let a = Touche::parser("a").unwrap();
        assert_eq!((a.cle.as_str(), a.caractere.as_deref()), ("a", Some("a")));
        let plus = Touche::parser("Control++").unwrap();
        assert!(plus.control);
        assert_eq!(plus.cle, "+");
        assert_eq!(
            Touche::parser("Space").unwrap().caractere.as_deref(),
            Some(" ")
        );
    }

    #[test]
    fn le_texte_tape_majuscules_comprises() {
        let b = Touche::pour_caractere('B');
        assert!(b.shift);
        assert_eq!((b.cle.as_str(), b.caractere.as_deref()), ("b", Some("B")));
    }

    #[test]
    fn les_scenes_du_depot_sont_toutes_valides() {
        let dossier = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../pixels/scenes");
        let scenes = lire_scenes(&dossier).unwrap();
        assert!(scenes.iter().any(|s| s.nom == "fenetre-vide"));
        assert!(
            scenes
                .iter()
                .any(|s| s.nom == "mire" && s.vue == VueScene::Mire)
        );
    }

    #[test]
    fn les_scenes_t11_portent_les_commandes_du_theme() {
        let dossier = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../pixels/scenes");
        let scenes = lire_scenes(&dossier).unwrap();
        let mut theme = 0;
        let mut accent = 0;
        for scene in scenes.iter().filter(|s| s.nom.starts_with("t11-")) {
            assert_eq!(scene.depuis, Tranche::parser("T11"), "{}", scene.nom);
            for action in &scene.actions {
                let Action::Command { id, args } = action else {
                    continue;
                };
                let args = args.clone().unwrap_or(Value::Null);
                if id == COMMANDE_THEME {
                    serde_json::from_value::<ArgsTheme>(args).unwrap();
                    theme += 1;
                } else if id == COMMANDE_ACCENT {
                    serde_json::from_value::<ArgsAccent>(args).unwrap();
                    accent += 1;
                }
            }
        }
        assert!(theme >= 4 && accent >= 1);
    }
}
