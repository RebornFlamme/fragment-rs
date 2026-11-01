// Port de app/src/ui/core/plugin.ts
//! La classe mère des plugins (§11.3) — et la PLUS SIMPLE du projet. C'est le but.
//!
//! Chaque `register_x()` fait EXACTEMENT deux choses, jamais trois :
//!
//! ```text
//!     let retrait = self.host().<registre>.register(…)   // 1. j'écris dans la table
//!     self.register(retrait)                             // 2. je programme mon retrait
//! ```
//!
//! registry.rs ignore l'existence de component.rs, et inversement. Ce trait
//! est le SEUL endroit où ils se rencontrent — le mariage, répété autant de fois
//! qu'il y a de points d'extension, sans aucune logique propre. C'est ce qui
//! rend « le coût marginal d'un point d'extension quasi nul » (§4) : chaque
//! nouveau `register_x()` hérite gratuitement de la sémantique de démontage.
//!
//! La désactivation d'un plugin, c'est `unload()` : le rejeu de la pile en sens
//! inverse. L'auteur n'écrit jamais de `on_unload()` — il n'a jamais eu la
//! responsabilité de nettoyer, donc il ne peut pas l'oublier.
//!
//! Noter le paramètre `owner` passé aux registres : les registres deviennent
//! auditables — chaque entrée dit quel plugin l'a posée.
//!
//! Première moitié du cœur : les mariages génériques (`RegistryHub`) et les
//! commandes. Les autres (`register_view`, `register_extensions`, `add_icon`,
//! `register_layer`, `add_setting_tab`, `register_cli_handler`, `load_data`/
//! `save_data`…) arrivent avec les registres qu'ils épousent, dans leurs crates.

use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::command::{Command, CommandManager};
use crate::component::{Component, ComponentState};
use crate::registry::{OrderedRegistry, Registry};
use crate::registry_hub::{Keyed, Ordered, RegistryDefinition, RegistryError, RegistryHub};

/// §11.1 — le manifest.json d'un plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

impl PluginManifest {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            version: version.to_owned(),
            min_app_version: None,
            description: None,
            author: None,
        }
    }
}

/// Ce qu'un plugin demande à l'App dans cette moitié du cœur : les commandes
/// et l'annuaire. L'App l'implémente ; les autres registres s'ajouteront ici.
pub trait PluginHost {
    fn commands(&self) -> Rc<CommandManager>;
    fn registry(&self) -> RegistryHub;
}

/// L'état commun à tout plugin : à embarquer dans la struct du plugin, rendu
/// par `Plugin::plugin()`. (`Component::state` rend `&base.state`.)
pub struct PluginBase {
    pub state: ComponentState,
    pub manifest: PluginManifest,
    pub host: Rc<dyn PluginHost>,
}

impl PluginBase {
    pub fn new(host: Rc<dyn PluginHost>, manifest: PluginManifest) -> Self {
        Self {
            state: ComponentState::new(),
            manifest,
            host,
        }
    }
}

impl std::fmt::Debug for PluginBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginBase")
            .field("manifest", &self.manifest)
            .field("state", &self.state)
            .finish()
    }
}

pub trait Plugin: Component {
    fn plugin(&self) -> &PluginBase;

    fn manifest(&self) -> &PluginManifest {
        &self.plugin().manifest
    }

    fn host(&self) -> Rc<dyn PluginHost> {
        self.plugin().host.clone()
    }

    // ── Les mariages ─────────────────────────────────────────────────────

    /// Crée ou retrouve un canal partagé. Les définitions vivent dans le contrat du plugin.
    fn create_registry<T: 'static>(
        &self,
        definition: &RegistryDefinition<T, Keyed>,
    ) -> Result<Registry<T>, RegistryError>
    where
        Self: Sized,
    {
        self.host().registry().get(definition)
    }

    fn create_ordered_registry<T: 'static>(
        &self,
        definition: &RegistryDefinition<T, Ordered>,
    ) -> Result<OrderedRegistry<T>, RegistryError>
    where
        Self: Sized,
    {
        self.host().registry().get_ordered(definition)
    }

    fn register_registry_entry<T: 'static>(
        &self,
        definition: &RegistryDefinition<T, Keyed>,
        key: &str,
        value: T,
    ) -> Result<(), RegistryError>
    where
        Self: Sized,
    {
        let retrait =
            self.host()
                .registry()
                .get(definition)?
                .register(key, value, &self.manifest().id);
        self.register(retrait);
        Ok(())
    }

    fn register_ordered_entry<T: 'static>(
        &self,
        definition: &RegistryDefinition<T, Ordered>,
        value: T,
        priority: i32,
    ) -> Result<(), RegistryError>
    where
        Self: Sized,
    {
        let retrait = self.host().registry().get_ordered(definition)?.register(
            value,
            priority,
            &self.manifest().id,
        );
        self.register(retrait);
        Ok(())
    }

    /// Contribue à la liste contenue dans une entrée ; ne remplace jamais l'entrée.
    fn register_registry_contribution<T: 'static>(
        &self,
        definition: &RegistryDefinition<OrderedRegistry<T>, Keyed>,
        key: &str,
        value: T,
        priority: i32,
    ) -> Result<(), RegistryError>
    where
        Self: Sized,
    {
        let retrait = self
            .host()
            .registry()
            .collection(definition, key)?
            .register(value, priority, &self.manifest().id);
        self.register(retrait);
        Ok(())
    }

    /// §11.5 — l'id est préfixé par celui du plugin : deux plugins peuvent avoir 'open'.
    ///
    /// ★ L'objet reçu n'est pas muté (A26) : on construit une commande PRÉFIXÉE à
    ///   part. L'id est préfixé par l'ID du plugin ; le nom par le NOM du plugin
    ///   avec « : », que la palette re-découpe pour griser le préfixe. Deux
    ///   sources (id technique / nom affichable), c'est voulu.
    ///
    /// Rend l'entrée VIVE du manager (sa copie désucrée), pas l'intermédiaire :
    /// c'est elle qui s'exécute. (Une commande `mobile_only` n'est jamais
    /// enregistrée : on rend alors le préfixe, faute de mieux.)
    fn add_command(&self, command: Command) -> Rc<Command> {
        let manifest = self.manifest();
        let prefixe = Command {
            id: format!("{}:{}", manifest.id, command.id),
            name: format!("{}: {}", manifest.name, command.name),
            ..command
        };
        let id = prefixe.id.clone();
        let commands = self.host().commands();
        let copie = prefixe.clone();
        self.register(commands.add_command(prefixe, &manifest.id));
        commands.find_command(&id).unwrap_or_else(|| Rc::new(copie))
    }

    /// Retrait explicite. Prend l'id NON préfixé — le préfixe est ajouté ici.
    fn remove_command(&self, id: &str) {
        self.host()
            .commands()
            .remove_command(&format!("{}:{id}", self.manifest().id));
    }

    /// Hook du contrat Obsidian : appelé quand `data.json` du plugin change SUR LE
    /// DISQUE sous ses pieds (sync, édition externe, fenêtre de réglages). Un
    /// plugin le surcharge pour relire ses réglages et rafraîchir son UI.
    fn on_external_settings_change(&self) {}
}

#[cfg(test)]
pub(crate) mod tests {
    //! Le bloc « Plugin.addCommand » de CommandManager.test.ts et le sixième cas
    //! de RegistryHub.test.ts (« un vrai bundle tiers »).
    use super::*;
    use crate::command::CommandHost;
    use crate::command::tests::faire_app;
    use crate::component::BasicComponent;
    use crate::registry_hub::define_registry;

    /// L'App vue par un plugin : les commandes et l'annuaire.
    pub(crate) struct HoteBidon {
        pub commands: Rc<CommandManager>,
        pub registry: RegistryHub,
    }

    impl PluginHost for HoteBidon {
        fn commands(&self) -> Rc<CommandManager> {
            self.commands.clone()
        }
        fn registry(&self) -> RegistryHub {
            self.registry.clone()
        }
    }

    pub(crate) struct PluginBidon {
        base: PluginBase,
    }

    impl Component for PluginBidon {
        fn state(&self) -> &ComponentState {
            &self.base.state
        }
    }

    impl Plugin for PluginBidon {
        fn plugin(&self) -> &PluginBase {
            &self.base
        }
    }

    fn faire_hote() -> (Rc<dyn CommandHost>, Rc<HoteBidon>) {
        let (app, commands) = faire_app(None);
        (
            app,
            Rc::new(HoteBidon {
                commands,
                registry: RegistryHub::new(),
            }),
        )
    }

    fn faire_plugin(hote: Rc<HoteBidon>) -> PluginBidon {
        PluginBidon {
            base: PluginBase::new(
                hote,
                PluginManifest::new("mon-plugin", "Mon Plugin", "1.0.0"),
            ),
        }
    }

    #[test]
    fn prefixe_l_id_par_l_id_du_plugin_et_le_nom_par_son_nom() {
        let (_app, hote) = faire_hote();
        let plugin = faire_plugin(hote.clone());
        plugin.load();

        plugin.add_command(Command::new("ouvrir", "Ouvrir le panneau").with_callback(|| {}));

        let cmd = hote.commands.find_command("mon-plugin:ouvrir").unwrap();
        // Le « : » est le séparateur que la palette re-découpe pour le préfixe grisé.
        assert_eq!(cmd.name, "Mon Plugin: Ouvrir le panneau");
    }

    #[test]
    fn rend_l_entree_vive_du_manager_et_laisse_l_objet_recu_intact_a26() {
        let (_app, hote) = faire_hote();
        let plugin = faire_plugin(hote.clone());
        plugin.load();

        let entree = Command::new("x", "X").with_callback(|| {});
        let sortie = plugin.add_command(entree.clone());

        // L'objet du plugin n'est pas muté : ni son id, ni son nom ne changent.
        assert_eq!(entree.id, "x");
        assert_eq!(entree.name, "X");
        // La sortie est l'entrée préfixée du registre, pas l'objet reçu.
        assert_eq!(sortie.id, "mon-plugin:x");
        assert!(Rc::ptr_eq(
            &sortie,
            &hote.commands.find_command("mon-plugin:x").unwrap()
        ));
    }

    #[test]
    fn retire_ses_commandes_au_unload_sans_que_le_plugin_ecrive_onunload() {
        let (_app, hote) = faire_hote();
        let plugin = faire_plugin(hote.clone());
        plugin.load();

        plugin.add_command(Command::new("a", "A").with_callback(|| {}));
        plugin.add_command(Command::new("b", "B").with_callback(|| {}));
        assert_eq!(hote.commands.size(), 2);

        plugin.unload();
        assert_eq!(hote.commands.size(), 0);
    }

    #[test]
    fn remove_command_prend_l_id_non_prefixe() {
        let (_app, hote) = faire_hote();
        let plugin = faire_plugin(hote.clone());
        plugin.load();

        plugin.add_command(Command::new("a", "A").with_callback(|| {}));
        plugin.remove_command("a");

        assert_eq!(hote.commands.size(), 0);
    }

    // RegistryHub.test.ts, 6e cas. Le bundle TS était évalué par `new Function`
    // avec `require('fragment')` ; ici c'est une struct du crate, le même contrat.

    type Transformer = Rc<dyn Fn(i32) -> i32>;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Outil {
        id: &'static str,
    }

    struct PluginTiers {
        base: PluginBase,
    }

    impl Component for PluginTiers {
        fn state(&self) -> &ComponentState {
            &self.base.state
        }
        fn on_load(&self) {
            let feature = self.add_child(Rc::new(BasicComponent::new()));
            let point = define_registry::<Transformer>("third-party:transformers@1", None).unwrap();
            let registry = self.create_registry(&point).unwrap();
            feature.register(registry.register("double", Rc::new(|x| x * 2), &self.manifest().id));
            self.register_registry_contribution(
                &define_registry::<OrderedRegistry<Outil>>("annotation:tools@1", None).unwrap(),
                "custom",
                Outil { id: "custom" },
                0,
            )
            .unwrap();
        }
    }

    impl Plugin for PluginTiers {
        fn plugin(&self) -> &PluginBase {
            &self.base
        }
    }

    #[test]
    fn un_vrai_bundle_tiers_cree_son_registre_et_publie_des_contributions_nettoyees_par_component()
    {
        let (_app, hote) = faire_hote();
        let registry = hote.registry.clone();
        let plugin = PluginTiers {
            base: PluginBase::new(hote, PluginManifest::new("third-party", "Third party", "1")),
        };
        plugin.load();
        let channel = registry
            .get(&define_registry::<Transformer>("third-party:transformers@1", None).unwrap())
            .unwrap();
        assert_eq!(channel.get("double").map(|f| f(21)), Some(42));
        plugin.unload();
        assert_eq!(channel.size(), 0);
        assert_eq!(
            registry
                .collection(
                    &define_registry::<OrderedRegistry<Outil>>("annotation:tools@1", None).unwrap(),
                    "custom"
                )
                .unwrap()
                .size(),
            0
        );
        plugin.load();
        assert_eq!(channel.get("double").map(|f| f(2)), Some(4));
        plugin.unload();
    }

    #[test]
    fn le_manifest_se_lit_en_camel_case() {
        // Ajouté au port : `minAppVersion` ↔ `min_app_version`.
        let m: PluginManifest = serde_json::from_str(
            r#"{"id":"demo","name":"Demo","version":"1.0.0","minAppVersion":"1.5.0"}"#,
        )
        .unwrap();
        assert_eq!(m.min_app_version.as_deref(), Some("1.5.0"));
    }
}
