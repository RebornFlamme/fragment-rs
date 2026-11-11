# Parité des tests

Généré par `cargo xtask parite` à partir des `crates/*/parite.toml`. Ne pas éditer à la main : modifier le `parite.toml` du crate, puis relancer la commande.

Règle de phase 1 : au moins 90 % des cas source portés ; chaque cas non porté porte sa raison. « Cas source » = cas du TS d'origine ; « Portés » = `#[test]` de même nom en snake_case ; « Ajoutés » = tests Rust sans équivalent TS (hors du taux).

## Total

| Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|
| 922 | 864 | 414 | 58 | 93.7 % |

**PORTE ATTEINTE** : 93.7 % des cas source portés.

- Modules non portés sans raison : aucun
- Fichiers illisibles : aucun

## Crates sans modules portés

fragment-html, spike-maths (pas de `parite.toml`).

## fragment-agent

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| actions_model | actions/actionsModel.ts | 11 | 11 | 0 | — |
| attachment_blocks | attachmentBlocks.ts | 4 | 4 | 0 | — |
| card_model | side-chat/cardModel.ts | 1 | 1 | 0 | — |
| contrat | contract.ts + types.ts + api.ts | 0 | 0 | 0 | — |
| controls_model | controls/controlsModel.ts | 5 | 5 | 0 | — |
| engine | engine/engine.ts (E/S : disque, processus, réseau — non porté) | 5 | 0 | 0 | 5 — extrait le SEUL binaire, vérifié, et le retrouve ensuite sans réseau : faux registre npm HTTP + gunzip + sha512 + disque : transport et E/S, hors des modèles purs ; appartient au harness `claude` (processus enfant) ; empreinte fausse → refusé, rien posé : idem (téléchargement réel contre un faux serveur) ; archive sans binaire → refusé : idem ; une nouvelle version efface les anciennes : idem (disque) ; registre sans cette version → erreur claire : idem (HTTP 404) |
| engine_model | engine/engineModel.ts (+ parties pures de engine/engine.ts) | 11 | 11 | 0 | — |
| events | events.ts | 13 | 13 | 0 | — |
| harness_registry | harness/registry.ts | 3 | 3 | 0 | — |
| lasso_prompt | lasso/lassoPrompt.ts | 2 | 2 | 0 | — |
| one_shot_options | oneShotOptions.ts | 2 | 2 | 0 | — |
| permission_model | policy/permissionCard.ts (partie pure ; le rendu DOM de la carte reste côté UI) | 2 | 2 | 0 | — |
| policy_model | policy/policyModel.ts + policy/presets.ts | 15 | 15 | 0 | — |
| quick_model | quick/quickModel.ts | 8 | 7 | 0 | 1 — chaque icône de la liste existe dans le jeu Lucide embarqué : dépend du paquet `lucide-static` (jeu d'icônes JS) : la vérification se fera côté fragment-ui, qui embarque le jeu d'icônes ; QUICK_ICONS est ici la liste de référence |
| screen_model | screenModel.ts | 2 | 2 | 0 | — |
| side_chat_prompt | side-chat/sideChatPrompt.ts | 2 | 2 | 0 | — |
| skills_model | skillsModel.ts | 14 | 12 | 0 | 2 — le bundle réel : 7 skills, 22 fichiers, préprompt sans liens .agents/ : lit le bundle Vite (`skillsPlugin.ts`, import `?raw` des skills) : l'embarquement des skills dans le binaire est une tâche du harness / de l'app, pas du modèle ; le prompt livré : maths en LaTeX seulement, non-rendus listés : lit `FRAGMENT_PROMPT` (skillsPlugin.ts, bundle Vite) : même raison |
| text_access | side-chat/textAccess.ts | 2 | 2 | 0 | — |
| trace_model | trace/traceModel.ts | 15 | 15 | 0 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-agent | 117 | 109 | 0 | 8 | 93.2 % |

## fragment-annotation

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| anchor_link | app/src/ui/views/doc-widget/anchorLink.ts | 4 | 4 | 0 | — |
| api | app/src/ui/views/annotation/api.ts | 0 | 0 | 0 | — |
| artefacts_menu | app/src/ui/views/artefacts/menuModel.ts | 5 | 5 | 0 | — |
| cluster_filter | app/src/ui/views/doc-widget/clusterFilter.ts | 5 | 5 | 0 | — |
| geometry | app/src/ui/views/annotation/geometry.ts | 15 | 15 | 0 | — |
| persistence | app/src/ui/views/annotation/AnnotationPersistence.ts | 4 | 1 | 0 | 3 — persiste un record par trait et les calques, recharge sans écriture d’écho : test d'intégration sur un vrai ArtefactManager + App async (microtâches) ; la partie pure (encodage `operation`, décodage) est testée par des cas ajoutés, l'orchestration revient à l'hôte ; une lecture lente ne ressuscite pas un trait supprimé ni ne perd une édition locale : dépend de l'async du manager ; la règle (révision périmée) est portée dans `Revisions` et testée par un cas ajouté ; la suppression distante et le déplacement de scope retirent la projection locale : dépend de `ArtefactManager.subscribe` et du filtrage de scope du manager (déjà testés dans fragment-artefacts) |
| session | app/src/ui/views/annotation/AnnotationSession.ts | 7 | 6 | 0 | 1 — monte et démonte les enfants tiers à chaud sans perdre les données : les extensions (`annotation:extensions@1`) sont des `Component` enfants montés par la session : cycle de vie GPUI, à porter avec le plugin d'annotation de fragment-ui |
| shapes | app/src/ui/views/annotation/shapes.ts | 4 | 4 | 0 | — |
| source | app/src/ui/views/annotation/AnnotationSource.ts | 0 | 0 | 0 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-annotation | 44 | 40 | 0 | 4 | 90.9 % |

## fragment-app

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| crates/fragment-app/src/boot.rs | app/src/ui/seedCore.ts | 0 | 0 | 2 | — |
| crates/fragment-app/src/boot.rs, crates/fragment-app/src/lib.rs | app/src/ui/bootApp.tsx | 0 | 0 | 0 | — |
| crates/fragment-app/src/hote_fenetre.rs | app/src/ui/windowHost.ts | 0 | 0 | 0 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-app | 0 | 0 | 2 | 0 | 100.0 % |

## fragment-artefacts

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| src/types.rs | ui/core/artefacts/types.ts | 0 | 0 | 0 | — |
| src/clock.rs | ui/core/artefacts/clock.ts | 0 | 0 | 0 | — |
| src/fold.rs | ui/core/artefacts/fold.ts | 7 | 7 | 0 | — |
| src/anchor/mod.rs | ui/core/artefacts/anchor.ts | 27 | 27 | 0 | — |
| src/objects.rs | ui/core/artefacts/objects.ts | 0 | 0 | 0 | — |
| src/cache.rs | ui/core/artefacts/cache.ts | 0 | 0 | 0 | — |
| src/journal/reader.rs | ui/core/artefacts/journal/reader.ts | 0 | 0 | 0 | — |
| src/journal/writer.rs | ui/core/artefacts/journal/writer.ts | 0 | 0 | 0 | — |
| src/journal/import.rs | ui/core/artefacts/journal/import.ts | 0 | 0 | 0 | — |
| src/journal/compact.rs | ui/core/artefacts/journal/compact.ts | 0 | 0 | 0 | — |
| src/device.rs + src/fs.rs | ui/core/artefacts/device.ts | 0 | 0 | 0 | — |
| src/artefact_manager.rs | ui/core/artefacts/ArtefactManager.ts | 31 | 30 | 0 | 1 — un document de PLUGIN est renommé ou supprimé › un événement sans fournisseur est ignoré : Teste `DocumentManager.notify` (documents/DocumentManager.ts → fragment-core, tâche fr-p1-core-b) : le filtrage par fournisseur n'est pas dans cette crate, le manager ne reçoit que des événements déjà fournis. |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-artefacts | 65 | 64 | 0 | 1 | 98.5 % |

## fragment-chat

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| api | api.ts | 0 | 0 | 0 | — |
| chat_key | chatKey.ts | 1 | 1 | 0 | — |
| chat_session | ChatSession.ts | 7 | 7 | 0 | — |
| chat_surface_model | chatSurfaceModel.ts | 11 | 11 | 0 | — |
| follow_model | scroll/followModel.ts | 11 | 11 | 0 | — |
| format | format.ts | 5 | 5 | 0 | — |
| history_model | components/historyModel.ts | 7 | 7 | 0 | — |
| post_model | components/postModel.ts | 3 | 3 | 0 | — |
| rail | scroll/rail.ts | 4 | 4 | 0 | — |
| search_model | scroll/searchModel.ts | 3 | 3 | 0 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-chat | 52 | 52 | 0 | 0 | 100.0 % |

## fragment-cli

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| client_pur | app/src/cli/clientPur.ts | 16 | 16 | 0 | — |
| coffres | app/src/electron/vaults.ts + main.ts (canaux vaults:) + shared/vaultManagerRoute.ts + ui/vault-manager/VaultManager.ts + ui/core/VaultSwitcher.ts | 7 | 7 | 0 | — |
| hote | app/src/ui/core/cli/types.ts, CliHost.ts, external/types.ts, ui/core/app.ts (InstanceInfo) | 0 | 0 | 0 | — |
| instance_index | app/src/shared/cli/instanceIndex.ts | 2 | 1 | 0 | 1 — sonderTuyau — vivant quand on écoute, mort sinon : ouvre un vrai tuyau nommé / socket (net) : c'est le transport, hors du modèle pur ; la sonde est injectée (trait SondeInstances) et testée avec une sonde fausse dans le test porté d'instances_vivantes |
| pipe | app/src/shared/cli/pipe.ts | 3 | 3 | 0 | — |
| protocol | app/src/shared/cli/protocol.ts | 10 | 10 | 0 | — |
| serveur | app/src/ui/coreCommands/cliServer.ts | 3 | 0 | 0 | 3 — un second serveur sur le même coffre → ErreurCoffreDejaServi, pas un EADDRINUSE brut : démarre de vrais serveurs sur un tuyau nommé : transport, hors du modèle pur (le type ErreurCoffreDejaServi est porté) ; deux coffres DIFFÉRENTS coexistent sans se percuter : vraies sockets (transport) ; nettoie un socket ORPHELIN puis démarre : vraies sockets et fichier .sock POSIX (transport) |
| shim | app/src/shared/cli/shim.ts | 13 | 13 | 0 | — |
| vault_path | app/src/shared/vaultPath.ts | 9 | 9 | 0 | — |
| vault_registry | app/src/shared/cli/vaultRegistry.ts | 3 | 3 | 0 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-cli | 66 | 62 | 0 | 4 | 93.9 % |

## fragment-core

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| crates/fragment-core/src/lru.rs | app/src/ui/core/lru.ts | 3 | 3 | 0 | — |
| crates/fragment-core/src/diff_minimal.rs | app/src/ui/core/diffMinimal.ts | 8 | 8 | 0 | — |
| crates/fragment-core/src/frontmatter.rs | app/src/ui/core/frontmatter.ts | 4 | 4 | 0 | — |
| crates/fragment-core/src/semver.rs | app/src/ui/core/plugins/registry/semver.ts | 4 | 4 | 0 | — |
| crates/fragment-core/src/swipe.rs | app/src/ui/core/keymap/swipe.ts | 6 | 6 | 0 | — |
| crates/fragment-core/src/fuzzy_search.rs | app/src/ui/core/fuzzySearch.ts | 6 | 6 | 1 | — |
| crates/fragment-core/src/keymap/format_hotkey.rs | app/src/ui/core/fuzzySearch.test.ts (bloc formatHotkey) | 4 | 4 | 0 | — |
| crates/fragment-core/src/resolve_link.rs | app/src/ui/core/links/resolveLink.ts | 14 | 14 | 0 | — |
| crates/fragment-core/src/events.rs | app/src/ui/core/events.ts | 0 | 0 | 4 | — |
| crates/fragment-core/src/component.rs | app/src/ui/core/component.ts | 0 | 0 | 3 | — |
| crates/fragment-core/src/registry.rs | app/src/ui/core/registry.ts | 0 | 0 | 0 | — |
| crates/fragment-core/src/registry_hub.rs | app/src/ui/core/RegistryHub.ts | 6 | 6 | 1 | — |
| crates/fragment-core/src/keymap.rs + keymap/{types,matching,scope}.rs | app/src/ui/core/keymap/{types,matching,Scope,Keymap}.ts | 22 | 22 | 1 | — |
| crates/fragment-core/src/keymap/hotkey_manager.rs | app/src/ui/core/keymap/HotkeyManager.ts | 13 | 13 | 1 | — |
| crates/fragment-core/src/command.rs | app/src/ui/core/CommandManager.ts | 18 | 18 | 0 | — |
| crates/fragment-core/src/plugin.rs | app/src/ui/core/plugin.ts | 4 | 4 | 1 | — |
| crates/fragment-core/src/plugins.rs | app/src/ui/core/plugins.ts | 18 | 18 | 1 | — |
| crates/fragment-core/src/capture_hotkey.rs | app/src/ui/core/keymap/captureHotkey.ts | 6 | 6 | 0 | — |
| crates/fragment-core/src/document_key.rs | app/src/ui/core/documents/documentKey.ts | 4 | 4 | 2 | — |
| crates/fragment-core/src/documents.rs | app/src/ui/core/documents/DocumentManager.ts + documents/types.ts | 5 | 5 | 1 | — |
| crates/fragment-core/src/workspace.rs | app/src/ui/core/workspace/types.ts | 0 | 0 | 3 | — |
| crates/fragment-core/src/workspace/layout.rs | app/src/ui/core/workspace/layout.ts | 14 | 14 | 2 | — |
| crates/fragment-core/src/leaf_history.rs | app/src/ui/core/workspace/LeafHistory.ts | 7 | 7 | 1 | — |
| crates/fragment-core/src/breadcrumb.rs | app/src/ui/core/workspace/breadcrumb.ts | 7 | 7 | 0 | — |
| crates/fragment-core/src/embed.rs | app/src/ui/core/embed/embedDocument.ts + embed/embedFrame.ts (wheelPixels) | 4 | 4 | 1 | — |
| crates/fragment-core/src/attachments.rs | app/src/ui/core/vault/attachments.ts | 8 | 8 | 2 | — |
| crates/fragment-core/src/theme_setting.rs | app/src/ui/core/settings/theme.ts (+ accentChoisi de settings/config.ts) | 2 | 2 | 1 | — |
| crates/fragment-core/src/zoom.rs | app/src/ui/core/window/zoom.ts | 9 | 9 | 0 | — |
| crates/fragment-core/src/drag.rs | app/src/ui/core/DragManager.ts | 2 | 2 | 7 | — |
| crates/fragment-core/src/dispatch.rs + external.rs | app/src/ui/core/external/dispatch.ts + external/types.ts + App.handleProtocol (app.ts) | 6 | 6 | 1 | — |
| crates/fragment-core/src/dispatch.rs | app/src/ui/core/cli/dispatch.ts | 13 | 5 | 0 | 8 — 8 cas de `CliPlugin — les intégrées` (help, version, commands, command id=…) : testent coreCommands/CliPlugin (greffon de fragment-cli / fragment-plugins), pas le dispatcher |
| crates/fragment-core/src/settings.rs | app/src/ui/core/settings/definition.ts (types) | 1 | 0 | 3 | 1 — renderDefinition / renderDefinitions / addSetting / applyControl / renderTreeLevel : rendu DOM : fragment-ui |
| crates/fragment-core/src/window_host.rs | app/src/ui/core/window/WindowHost.ts | 0 | 0 | 1 | — |
| crates/fragment-core/src/secret_storage.rs | app/src/ui/core/settings/SecretStorage.ts | 5 | 0 | 1 | 5 — les 5 cas de SecretStorage.test.ts : Ils testent l'implémentation (secrets.json + chiffrement) qui vit dans fragment-app ; le cœur ne porte que le trait. Portés avec l'implémentation dans fragment-plugins (PR #16, 5/5) : compter ces cas une seule fois. |
| crates/fragment-core/src/platform.rs | app/src/ui/core/Platform.ts | 0 | 0 | 1 | — |
| crates/fragment-core/src/debounce.rs | app/src/ui/core/debounce.ts | 0 | 0 | 5 | — |
| crates/fragment-core/tests/contrats.rs | (contrat) docs/contrats.md table 1 | 0 | 0 | 2 | — |
| crates/fragment-core/tests/workspace_arbre.rs | app/e2e/workspace-structure.spec.ts | 3 | 3 | 0 | — |
| crates/fragment-core/tests/workspace_arbre.rs | app/e2e/resize-gesture.spec.ts | 6 | 5 | 0 | 1 — contre-épreuve : sans le CSS du gel, le côté qui rétrécit reflue aussi : Teste la feuille de style qui applique la boîte figée (workspace.css) ; le cœur ne fait que calculer les boîtes. À porter avec le rendu (fragment-ui, fr-t1-splits). |
| crates/fragment-core/src/workspace/arbre.rs | app/src/ui/core/workspace/WorkspaceItem.ts + WorkspaceSplit.ts | 0 | 0 | 13 | — |
| crates/fragment-core/src/workspace/pile.rs | app/src/ui/core/workspace/WorkspaceTabs.ts | 0 | 0 | 11 | — |
| crates/fragment-core/src/workspace/redimension.rs | app/src/ui/core/workspace/resizeGesture.ts | 0 | 0 | 4 | — |
| crates/fragment-core/tests/workspace_espace.rs | app/e2e/activeleaf-tracking.spec.ts | 4 | 4 | 0 | — |
| crates/fragment-core/tests/workspace_espace.rs | app/e2e/boot-scope.spec.ts | 2 | 2 | 0 | — |
| crates/fragment-core/tests/workspace_espace.rs | app/e2e/workspace-layout.spec.ts | 5 | 5 | 0 | — |
| crates/fragment-core/src/workspace/commandes.rs + tests/workspace_espace.rs | app/src/ui/coreCommands/WorkspaceCommandsPlugin.ts (+ .test.ts) | 5 | 3 | 1 | 2 — 2 cas « next-tab — LE BUG (logique expédiée) » : ils reproduisent l'ANCIENNE logique, pas le code : portés dans fragment-plugins ; dans le cœur, la dérive est impossible (montrer un onglet de la pile de l'active l'active) ; workspace:open (CLI), marge de lecture (marginMode.ts) : validation de workspace:open dans fragment-plugins, attente du défilement = rendu ; la marge est un autre module |
| crates/fragment-core/src/workspace/commandes.rs | app/src/ui/coreCommands/AppCommandsPlugin.ts (hors zoom) | 0 | 0 | 2 | — |
| crates/fragment-core/src/workspace/espace.rs + feuille.rs + restauration.rs | app/src/ui/core/workspace/Workspace.ts + WorkspaceLeaf.ts | 0 | 0 | 5 | — |
| crates/fragment-core/src/workspace/vue.rs + feuille.rs | app/src/ui/core/workspace/View.ts + ItemView.ts (modèle) + EmptyView.ts + DeferredView.ts | 0 | 0 | 6 | — |
| crates/fragment-core/src/app.rs | app/src/ui/core/app.ts | 0 | 0 | 0 | — |
| crates/fragment-core/src/config.rs + apparence.rs | app/src/ui/core/settings/ConfigStore.ts + config.ts (AppearanceConfig, applyAppearance) + sync.ts | 0 | 0 | 4 | — |
| crates/fragment-core/tests/apparence.rs + crates/fragment-core/src/commandes_zoom.rs + crates/fragment-ui/tests/reglages.rs | app/e2e/app-zoom.spec.ts + app/src/ui/coreCommands/AppCommandsPlugin.ts (zoom-in, zoom-out, reset-zoom) | 13 | 8 | 0 | 5 — le menu Electron n'a plus de zoom actif (neutraliserZoomDuMenu) : propre à Electron : GPUI n'a pas de menu d'application à neutraliser ; Ctrl+molette sur une note ne zoome pas : Chromium zoomerait de lui-même ; GPUI n'a aucun zoom par molette. Aucune garde à tester (le geste n'est lié à rien) ; capture : le rectangle en px CSS est converti selon le zoom (devicePixelRatio) : contrat WindowHost::capture, dont l'implémentation GPUI est dans fr-t1-binaire ; pas de devicePixelRatio ici (u() met les longueurs à l'échelle, pas la capture) ; PDF : Ctrl+= reste le zoom du PDF ; PDF : canvas repeints à la nouvelle densité : la vue PDF est de la phase 5 (portée de vue avant la racine : à rejouer avec elle) ; la repeinte des canvas est un souci de Chromium ; boot : `window.outerWidth / window.innerWidth > 1.3` (toute l'interface a rétréci) : mesure du DOM ; la mise à l'échelle de toute l'interface par u() est vérifiée par les scènes pixels t1-zoom-150 et t1-zoom-80 |
| crates/fragment-ui/src/reglages/{mod,fenetre,curseur}.rs | app/src/ui/settings/SettingsWindow.ts + core/settings/{Setting,SettingTab,SettingPage,SettingManager}.ts + settings.css (Appearance) | 0 | 0 | 5 | — |
| crates/fragment-core/src/vault/coffre.rs | app/src/ui/core/vault/vault.ts | 0 | 0 | 95 | — |
| crates/fragment-core/src/vault/fichier.rs | app/src/ui/core/vault/TAbstractFile.ts, TFile.ts, TFolder.ts | 0 | 0 | 4 | — |
| crates/fragment-core/src/vault/adaptateur.rs, memoire.rs | app/src/ui/core/vault/DataAdapter.ts | 0 | 0 | 0 | — |
| crates/fragment-core/src/vault/fournisseur.rs | app/src/ui/core/documents/vaultProvider.ts | 0 | 0 | 23 | — |
| crates/fragment-core/src/vault/coffre.rs | app/src/ui/core/links/resolveLink.ts (sur un vrai Vault) | 0 | 0 | 7 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-core | 261 | 239 | 227 | 22 | 91.6 % |

## fragment-layers

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| src/gutter_stack.rs | ui/core/layers/gutterStack.ts | 21 | 21 | 0 | — |
| src/types.rs | ui/core/layers/types.ts | 0 | 0 | 3 | — |
| src/overlay_host.rs | ui/core/layers/types.ts (OverlayHost, LayerContext, LayerSpec) | 0 | 0 | 3 | — |
| src/widget_activation.rs | ui/core/layers/WidgetActivation.ts | 8 | 8 | 3 | — |
| src/gutter_arbiter.rs | ui/core/layers/GutterArbiter.ts | 3 | 3 | 4 | — |
| src/widget_layer.rs | ui/core/layers/WidgetLayer.ts | 3 | 3 | 23 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-layers | 35 | 35 | 36 | 0 | 100.0 % |

## fragment-markdown

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| editor_transforms | views/markdown/editorTransforms.ts | 43 | 43 | 0 | — |
| frontmatter_model | views/markdown/properties/frontmatterModel.ts | 13 | 13 | 0 | — |
| read_only_markdown | views/markdown/readOnlyMarkdown.ts | 5 | 5 | 0 | — |
| readme_markdown | settings/readmeMarkdown.ts | 4 | 4 | 0 | — |
| sanitize_html | views/markdown/sanitizeHtml.ts | 5 | 5 | 0 | — |
| table_model | views/markdown/table/tableModel.ts | 18 | 18 | 0 | — |
| crates/fragment-markdown/src/commandes/actions.rs | app/src/ui/views/markdown/EditorCommandsPlugin.ts (table ACTIONS, appliquerTransformation, appliquerCommandeEditeur, collerTexteBrut, toutSelectionner) | 0 | 0 | 10 | — |
| crates/fragment-markdown/src/commandes/menu.rs | app/src/ui/views/markdown/MarkdownView.ts (remplirMenuEditeur, wireContextMenu : sections) | 3 | 3 | 11 | — |
| crates/fragment-markdown/src/commandes/insertion.rs | app/src/ui/views/markdown/table/tableField.ts (separerDuTableau, saisieSousTableau) | 1 | 1 | 2 | — |
| crates/fragment-markdown/src/coller_image/mod.rs | app/src/ui/views/markdown/pasteImage.ts ; app/src/ui/core/vault/attachments.ts (usage) ; e2e paste-image.spec.ts, markdown-table.spec.ts | 3 | 3 | 12 | — |
| crates/fragment-markdown/src/syntaxe/analyse_blocs.rs, en_ligne.rs, genre.rs | app/src/ui/views/markdown/language.ts (markdownExtensions : GFM, Subscript, Superscript, WikiLink, Highlight, InlineMath, BlockMath, MathHighlight) sur @lezer/markdown 1.7.2 | 0 | 0 | 28 | — |
| crates/fragment-markdown/src/syntaxe/arbre.rs | (ajout) arbre à plat et positions UTF-16 (ADR-004) | 0 | 0 | 5 | — |
| crates/fragment-markdown/src/syntaxe/modele_rangees.rs | (ajout) types partagés par fr-t3-decorations et fr-t3-blocs-modele (contrat set_row_model, ADR-006) | 0 | 0 | 4 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-markdown | 95 | 95 | 72 | 0 | 100.0 % |

## fragment-pdf

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| pdf_editor | app/src/ui/views/pdf/PdfEditor.ts | 9 | 9 | 0 | — |
| pdf_find_offsets | app/src/ui/views/pdf/pdfFindOffsets.ts | 7 | 7 | 0 | — |
| pdf_geometry | app/src/ui/views/pdf/pdfGeometry.ts | 7 | 7 | 0 | — |
| pdf_outline | app/src/ui/views/pdf/pdfOutline.ts | 3 | 3 | 0 | — |
| pdf_zoom | app/src/ui/views/pdf/pdfZoom.ts | 7 | 7 | 0 | — |
| crates/fragment-pdf/src/laterale/*.rs | app/src/ui/views/pdf/PdfSidebar.ts (modes, vignettes virtualisées, arbre de la table, reveler, poignée), section barre latérale de pdf-view.css | 6 | 6 | 15 | — |
| crates/fragment-pdf/src/document/mod.rs | app/src/ui/views/pdf/PdfView.ts (chargement, pages, couche texte, liens), pdfOutline.ts (resoudreOutline, resoudreDestination) | 0 | 0 | 11 | — |
| crates/fragment-pdf/src/vue_modele/*.rs | app/src/ui/views/pdf/PdfView.ts (appliquerEchelle, modes, ResizeObserver, IntersectionObserver, majPage, champ de page, repeindre, finDeGeste, zoomerMolette, lirePosition, allerAPosition, getEphemeralState), devicePixels.ts | 12 | 12 | 20 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-pdf | 51 | 51 | 46 | 0 | 100.0 % |

## fragment-plugins

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| command_palette | app/src/ui/views/command-palette/CommandPalettePlugin.ts | 2 | 2 | 0 | — |
| github_registry_source | app/src/ui/core/plugins/registry/GithubRegistrySource.ts | 8 | 8 | 0 | — |
| margin_mode | app/src/ui/coreCommands/marginMode.ts | 0 | 0 | 0 | — |
| registry_types | app/src/ui/core/plugins/registry/types.ts | 0 | 0 | 0 | — |
| secret_storage | app/src/ui/core/settings/SecretStorage.ts | 5 | 5 | 0 | — |
| workspace_commands | app/src/ui/coreCommands/WorkspaceCommandsPlugin.ts | 5 | 5 | 0 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-plugins | 20 | 20 | 0 | 0 | 100.0 % |

## fragment-snap

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| snap_model | app/src/ui/snap/snapModel.ts | 20 | 20 | 0 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-snap | 20 | 20 | 0 | 0 | 100.0 % |

## fragment-text

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| src/editor.rs | ui/core/editor/Editor.ts | 2 | 2 | 0 | — |
| src/viewport.rs | ui/core/editor/viewport.ts | 6 | 6 | 0 | — |
| src/find_passage.rs | ui/core/editor/findPassage.ts | 4 | 4 | 1 | — |
| src/reveal_passage.rs | ui/core/editor/revealPassage.ts | 5 | 5 | 1 | — |
| src/range_highlights.rs | ui/core/editor/rangeHighlights.ts | 4 | 4 | 0 | — |
| src/visibility.rs | ui/core/layers/visibility.ts | 0 | 0 | 0 | — |
| src/map_anchor.rs | ui/core/layers/WidgetLayer.ts (mapAnchor) | 3 | 3 | 0 | — |
| src/text_delta.rs | ui/core/editor/textDelta.ts | 0 | 0 | 8 | — |
| src/html_bridge.rs | shared/htmlBridge.ts (partie pure) | 7 | 5 | 6 | 2 — le script est du JS valide : porte sur HTML_BRIDGE_SCRIPT (du JavaScript exécuté par la page) : non porté, le moteur de page de fragment-html n'est pas choisi ; les échappements des regex survivent à la chaîne (antislash w, antislash S) : idem : propriété de la chaîne JS du script |
| src/html_text_model.rs | ui/views/html/htmlTextModel.ts | 9 | 9 | 2 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-text | 40 | 38 | 18 | 2 | 95.0 % |

## fragment-theme

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| manager | app/src/ui/theme/api.ts | 0 | 0 | 0 | — |
| pas_de_litteral | app/src/ui/theme-tokens.test.ts | 5 | 2 | 0 | 3 — aucune forme de l'échelle en dur hors de index.css : scrute des CSS ; côté Rust, `u()` d'echelle.rs et le contrôle de revue ; le détecteur attrape ce qu'il doit (contre-épreuve) [forme] : même raison : porte sur du CSS ; la coloration syntaxique lit les jetons --code-* : CodeMirror, T3 |
| theme | app/src/ui/index.css | 0 | 0 | 0 | — |
| theme_model | app/src/ui/theme/themeModel.ts | 4 | 4 | 0 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-theme | 9 | 6 | 0 | 3 | 66.7 % |

## fragment-ui

| Module | Fichier TS | Cas source | Portés | Ajoutés | Non portés (raison) |
|---|---|---|---|---|---|
| crates/fragment-ui/tests/splits.rs | app/e2e/workspace-structure.spec.ts | 3 | 3 | 0 | — |
| crates/fragment-app/tests/boot.rs, crates/fragment-ui/tests/onglets.rs | app/e2e/workspace-layout.spec.ts | 5 | 4 | 0 | 1 — §8.4 — seules les feuilles visibles sont matérialisées ; un clic charge la suivante : Sans vue réelle (T1 n'a que la vue vide), la matérialisation ne se voit pas ; repris par T2/T3 avec la vue Markdown. Le cœur la teste (restauration.rs). |
| crates/fragment-ui/tests/onglets.rs, crates/fragment-app/tests/boot.rs | app/e2e/activeleaf-tracking.spec.ts | 4 | 4 | 0 | — |
| crates/fragment-app/tests/boot.rs | app/e2e/boot-scope.spec.ts | 2 | 2 | 0 | — |
| crates/fragment-ui/tests/splits.rs | app/e2e/tab-drag-split.spec.ts | 1 | 1 | 0 | — |
| crates/fragment-app/tests/boot.rs, crates/fragment-app/tests/clavier.rs, crates/fragment-ui/tests/reglages.rs | app/e2e/app-zoom.spec.ts | 11 | 6 | 2 | 5 — le menu Electron n’a plus de zoom actif : Pas de menu Electron : GPUI n'a pas de rôle de menu zoomIn/zoomOut (n.a.). ; Ctrl+molette sur une note ne zoome pas : Aucune liaison Ctrl+molette dans GPUI ni dans le pont ; à tester avec la vue Markdown (T3). ; capture : le rectangle en px CSS est converti selon le zoom : WindowHost::capture n'existe que sous la feature de test de GPUI (lecteur de scènes) ; dette notée dans fragment-app. ; PDF : Ctrl+= reste le zoom du PDF, l’app ne bouge pas : Vue PDF : T4 (le mécanisme, une portée de vue qui consomme Ctrl+=, est testé par la_portee_de_la_vue_passe_avant_la_racine). ; PDF : après un zoom de l’app, les canvas sont repeints à la nouvelle densité : Vue PDF : T4. |
| crates/fragment-app/tests/clavier.rs | app/e2e/hotkeys.spec.ts | 5 | 4 | 2 | 1 — ★ éditer hotkeys.json à la main prend effet SANS redémarrage : Exige le watcher du dossier (notify, T2). |
| crates/fragment-ui/tests/palette.rs | app/e2e/command-palette.spec.ts | 6 | 3 | 0 | 3 — Mod+P ouvre, on tape, ↓ ↑ naviguent, ↵ exécute, la palette se ferme : Le parcours au clavier est rejoué par les scènes t1-palette* (fr-t1.1) ; pas de banc headless avec la couche modale et le pont clavier ensemble. ; une commande dont checkCallback lève est masquée, la palette survit : Une fermeture Rust ne « lève » pas : le cas devient une panique absorbée par CommandManager, testé dans le cœur. ; le préfixe du plugin est rendu à part, et le raccourci en <kbd> : Jugé au pixel (t1-palette), pas par le DOM. |
| crates/fragment-ui/tests/splits.rs | app/e2e/resize-gesture.spec.ts | 6 | 4 | 0 | 2 — dock vers l'intérieur (cas du dock) : Repris par la scène t1-docks-ouverts et les tests de poignée de dock ; nom e2e non repris tel quel (fr-t1-splits). ; contre-épreuve : sans le CSS du gel, le côté qui rétrécit reflue aussi : Retire un CSS : sans objet dans GPUI (fr-t1-splits). |
| crates/fragment-ui/tests/splits.rs | app/e2e/tab-drop-frame-space.spec.ts | 2 | 2 | 0 | — |
|  | app/e2e/right-dock.spec.ts | 1 | 0 | 0 | 1 — panneau droit : toggle topbar + reveal + topbar intacte : « reveal » ouvre une vue d'outil (chat, T7) ; le basculement seul est couvert par les tests de splits et la scène t1-docks-ouverts. |
|  | app/e2e/view-header-nav.spec.ts | 1 | 0 | 0 | 1 — ← → naviguent dans l’historique de l’onglet, le titre reste centré : Il faut des documents (T2) ; l'état grisé est la scène t1-entete-vue (T1.md §5). |
| crates/fragment-ui/src/clavier/mod.rs | app/src/ui/core/keymap/Keymap.ts (écoute keydown de la fenêtre) | 0 | 0 | 3 | — |
| crates/fragment-ui/tests/spike_s6.rs, crates/fragment-ui/tests/spike_s6/smil.rs | (ajout) spike S6 : rendu natif de gpui-kit, SVG et SMIL contre Chromium, politique d'assainissement | 0 | 0 | 6 | — |

| Synthèse | Cas source | Portés | Ajoutés | Non portés | Taux |
|---|---|---|---|---|---|
| fragment-ui | 47 | 33 | 13 | 14 | 70.2 % |

