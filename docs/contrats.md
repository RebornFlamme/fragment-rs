# Contrats gelés : API publique TS -> Rust

**But.** Pour chaque nom public de Fragment (TS), sa forme Rust et sa crate. Ce document est le contrat gelé de la phase 0 : les tâches de port le lisent, elles ne le réinterprètent pas.

**Règle : un changement = une ADR.** Ajouter, renommer, déplacer de crate ou changer la nature (trait/struct/enum) d'une ligne exige `decisions/ADR-NNN-*.md`, puis la mise à jour de ce fichier dans la même PR.

**Date** : 01/11/2026.

**Sources** (Fragment est en lecture seule) :
- `Fragment\app\src\ui\core\public-api.ts` (miroir de `core\api.d.ts` et `core\api.ts`) ; la nature de chaque export est lue dans le fichier qui le déclare.
- Registres `*@1` : chaînes `'…@1'` de `Fragment\app\src\ui\**\api.ts` et `contract.ts`.
- Crates : `program.md` §4 (lignes 113-129). `fragment-core`, `-text`, `-layers`, `-artefacts` ne dépendent pas de gpui-kit ; la présentation va dans `fragment-ui`.

Conventions : snake_case pour les fonctions, CamelCase pour les types, vocabulaire Obsidian/Fragment conservé. « Présentation, pas de contrat » = pas d'équivalent public stable (le rendu GPUI remplace le DOM).

## Table 1 : exports de public-api.ts

| Nom TS | Nature TS | Forme Rust | Crate | Note |
|---|---|---|---|---|
| Component | classe | trait Component | fragment-core | cycle de vie, `register*` |
| Registry | classe | struct Registry | fragment-core | générique `Registry<T>` |
| OrderedRegistry | classe | struct OrderedRegistry | fragment-core | priorité, à id égal la plus haute gagne |
| RegistryHub | classe | struct RegistryHub | fragment-core | |
| defineRegistry | fonction | fn define_registry | fragment-core | |
| defineOrderedRegistry | fonction | fn define_ordered_registry | fragment-core | |
| RegistryDefinition | interface | struct RegistryDefinition | fragment-core | |
| LayerContext | interface | struct LayerContext | fragment-layers | |
| LayerMenuSection | interface | struct LayerMenuSection | fragment-layers | rendu du menu Calques dans fragment-ui |
| LayerSpec | interface | struct LayerSpec | fragment-layers | fabrique = `Box<dyn Fn>` |
| LayerInstanceRef | interface | struct LayerInstanceRef | fragment-layers | |
| LayerPreset | interface | struct LayerPreset | fragment-layers | |
| OverlayHost | interface | trait OverlayHost | fragment-layers | impl dans fragment-ui |
| ActivationEntry | interface | struct ActivationEntry | fragment-layers | WidgetActivation |
| CompactInfo | interface | struct CompactInfo | fragment-layers | |
| GutterCluster | interface | struct GutterCluster | fragment-layers | clusterGutter |
| GutterMember | interface | struct GutterMember | fragment-layers | |
| DocumentSurface | interface | trait DocumentSurface | fragment-text | |
| Marker | interface | struct Marker | fragment-text | |
| TextSurface | interface | trait TextSurface | fragment-text | `coords_at_pos`, `pos_at_coords` |
| SurfaceText | interface | struct SurfaceText | fragment-text | |
| EditorRange | interface | struct EditorRange | fragment-text | |
| EditorPosition | interface | struct EditorPosition | fragment-text | |
| EditorChange | interface | struct EditorChange | fragment-text | |
| MappedPos | interface | struct MappedPos | fragment-text | |
| Rect | interface | struct Rect | fragment-text | partagé avec layers (dépend de text) |
| DocumentKey | alias de type | struct DocumentKey | fragment-core | newtype de String |
| DocumentProvider | interface | trait DocumentProvider | fragment-core | |
| DocumentEvent | alias (union) | enum DocumentEvent | fragment-core | |
| DocumentManager | classe | struct DocumentManager | fragment-core | |
| ToolbarOption | interface | struct ToolbarOption | fragment-ui | |
| Events | classe | struct Events | fragment-core | |
| EventRef | classe | struct EventRef | fragment-core | jeton de désabonnement |
| Plugin | classe abstraite | trait Plugin | fragment-core | |
| Menu | classe | struct Menu | fragment-ui | surface plugin, rendu gpui-kit |
| MenuItem | classe | struct MenuItem | fragment-ui | `set_section` |
| Toolbar | classe | struct Toolbar | fragment-ui | |
| ToolbarItem | classe | struct ToolbarItem | fragment-ui | |
| Modal | classe | struct Modal | fragment-ui | |
| Notice | classe | struct Notice | fragment-ui | |
| Platform | constante (objet) | const PLATFORM | fragment-core | drapeaux OS, pas de navigateur |
| SuggestModal | classe abstraite | trait SuggestModal | fragment-ui | |
| FuzzySuggestModal | classe abstraite | trait FuzzySuggestModal | fragment-ui | |
| Scope | classe | struct Scope | fragment-core | |
| Keymap | classe | struct Keymap | fragment-core | |
| prepareFuzzySearch | fonction | fn prepare_fuzzy_search | fragment-core | |
| renderMatches | fonction | présentation, pas de contrat | fragment-ui | écrit des spans DOM ; Rust : runs de texte côté ui |
| CliError | classe (erreur) | struct CliError | fragment-cli | |
| EXIT | constante | const EXIT | fragment-cli | codes de sortie, `shared/cli/protocol.ts` |
| debounce | fonction | fn debounce | fragment-core | |
| setIcon | fonction | présentation, pas de contrat | fragment-ui | prend un HTMLElement ; remplacé par l'icône gpui-kit |
| setTooltip | fonction | présentation, pas de contrat | fragment-ui | idem, infobulle gpui-kit |
| displayTooltip | fonction | présentation, pas de contrat | fragment-ui | idem |
| TooltipOptions | interface | présentation, pas de contrat | fragment-ui | options de l'infobulle DOM |
| TooltipPlacement | alias (union) | présentation, pas de contrat | fragment-ui | |
| TAbstractFile | classe abstraite | enum TAbstractFile | fragment-core | File ou Folder |
| TFile | classe | struct TFile | fragment-core | |
| TFolder | classe | struct TFolder | fragment-core | |
| Vault | classe | struct Vault | fragment-core | événements origin user ou disk |
| View | classe abstraite | trait View | fragment-core | rendu hors du trait (ui) |
| ItemView | classe abstraite | trait ItemView | fragment-core | |
| FileView | classe abstraite | trait FileView | fragment-core | |
| EditableFileView | classe abstraite | trait EditableFileView | fragment-core | |
| TextFileView | classe abstraite | trait TextFileView | fragment-core | |
| WorkspaceLeaf | classe | struct WorkspaceLeaf | fragment-core | |
| Workspace | classe | struct Workspace | fragment-core | |
| WorkspaceItem | classe abstraite | trait WorkspaceItem | fragment-core | |
| WorkspaceParent | classe abstraite | trait WorkspaceParent | fragment-core | |
| WorkspaceSplit | classe | struct WorkspaceSplit | fragment-core | |
| WorkspaceRoot | classe | struct WorkspaceRoot | fragment-core | |
| WorkspaceSidedock | classe | struct WorkspaceSidedock | fragment-core | |
| SplitDirection | alias (union) | enum SplitDirection | fragment-core | |
| WorkspaceTabs | classe | struct WorkspaceTabs | fragment-core | |
| WidgetLayer | classe | struct WidgetLayer | fragment-layers | modèle ; rendu dans fragment-ui |
| mapAnchor | fonction | fn map_anchor | fragment-text | mapRange/mapAnchor |
| renderViewIcon | fonction | présentation, pas de contrat | fragment-ui | avatar de vue, DOM |
| renderViewIcons | fonction | présentation, pas de contrat | fragment-ui | idem |
| embedDocument | fonction | fn embed_document | fragment-ui | héberge une vue par clé |
| HostedView | classe | struct HostedView | fragment-ui | |
| revealPassage | fonction | fn reveal_passage | fragment-text | modèle ; défilement dans ui |
| hasText | fonction (garde) | fn has_text | fragment-text | |
| isEditable | fonction (garde) | fn is_editable | fragment-text | |
| Setting | classe | struct Setting | fragment-ui | builder |
| SettingTab | classe abstraite | trait SettingTab | fragment-ui | |
| PluginSettingTab | classe abstraite | trait PluginSettingTab | fragment-ui | |
| SettingGroup | classe | struct SettingGroup | fragment-ui | |
| SettingPage | classe | struct SettingPage | fragment-ui | |
| BaseComponent | classe abstraite | trait BaseComponent | fragment-ui | |
| ValueComponent | classe abstraite | trait ValueComponent | fragment-ui | |
| ToggleComponent | classe | struct ToggleComponent | fragment-ui | |
| TextComponent | classe | struct TextComponent | fragment-ui | |
| TextAreaComponent | classe | struct TextAreaComponent | fragment-ui | |
| DropdownComponent | classe | struct DropdownComponent | fragment-ui | |
| SliderComponent | classe | struct SliderComponent | fragment-ui | |
| ColorComponent | classe | struct ColorComponent | fragment-ui | |
| SearchComponent | classe | struct SearchComponent | fragment-ui | |
| ButtonComponent | classe | struct ButtonComponent | fragment-ui | |
| ExtraButtonComponent | classe | struct ExtraButtonComponent | fragment-ui | |
| SecretComponent | classe | struct SecretComponent | fragment-ui | |
| FileFolderComponent | classe | struct FileFolderComponent | fragment-ui | |
| addSetting | fonction | fn add_setting | fragment-ui | API déclarative |
| requestUrl | fonction async | fn request_url | fragment-plugins | client HTTP ; voir cas douteux |
| parseFrontMatterStringArray | fonction | fn parse_front_matter_string_array | fragment-core | frontmatter |
| App | classe | trait App | fragment-core | « App (traits) » |
| Command | interface | struct Command | fragment-core | |
| InstanceInfo | interface | struct InstanceInfo | fragment-cli | index d'instances |
| PluginManifest | interface | struct PluginManifest | fragment-core | |
| Instruction | interface | struct Instruction | fragment-ui | légende de SuggestModal |
| Hotkey | interface | struct Hotkey | fragment-core | |
| Modifier | alias (union) | enum Modifier | fragment-core | |
| SearchResult | interface | struct SearchResult | fragment-core | |
| SearchMatches | alias (tuples) | type SearchMatches | fragment-core | `Vec<(usize, usize)>` |
| FuzzyMatch | interface | struct FuzzyMatch | fragment-core | |
| CliData | alias | type CliData | fragment-cli | |
| CliFlags | alias | type CliFlags | fragment-cli | |
| CliHandler | alias (fonction) | type CliHandler | fragment-cli | |
| CliHandlerSpec | alias | struct CliHandlerSpec | fragment-cli | |
| CliHost | interface | trait CliHost | fragment-cli | |
| CliInstallResult | interface | struct CliInstallResult | fragment-cli | |
| WindowHost | interface | trait WindowHost | fragment-core | impl dans fragment-app ; voir cas douteux |
| CaptureRect | interface | struct CaptureRect | fragment-core | |
| SettingTabSection | alias (union) | enum SettingTabSection | fragment-ui | |
| SecretStorage | classe | trait SecretStorage | fragment-core | impl trousseau OS dans fragment-app |
| RequestUrlOptions | interface | struct RequestUrlOptions | fragment-plugins | |
| RequestUrlResponse | interface | struct RequestUrlResponse | fragment-plugins | |
| Editor | interface | trait Editor | fragment-text | |
| ViewState | interface | struct ViewState | fragment-core | |
| OpenViewState | interface | struct OpenViewState | fragment-core | |
| ViewStateResult | interface | struct ViewStateResult | fragment-core | |
| PaneType | alias (union) | enum PaneType | fragment-core | |
| ArtefactKind | interface | struct ArtefactKind | fragment-artefacts | |
| ArtefactContext | interface | struct ArtefactContext | fragment-artefacts | |
| ArtefactInstance | interface | trait ArtefactInstance | fragment-artefacts | |
| ArtefactScope | alias (union) | enum ArtefactScope | fragment-artefacts | |
| DocScope | alias (union) | enum DocScope | fragment-artefacts | |
| DurableAnchor | alias | struct DurableAnchor | fragment-artefacts | citation + cascade |
| Rec | interface | struct Rec | fragment-artefacts | record du journal |
| Fields | interface (map) | type Fields | fragment-artefacts | `BTreeMap<String, Value>` |
| Op | alias (union) | enum Op | fragment-artefacts | |
| Tx | interface | struct Tx | fragment-artefacts | |
| Filter | interface | struct Filter | fragment-artefacts | |
| Change | interface | struct Change | fragment-artefacts | |
| Origin | alias (union) | enum Origin | fragment-artefacts | |
| ArtefactManager | classe | struct ArtefactManager | fragment-artefacts | |
| WidgetHandle | interface | trait WidgetHandle | fragment-layers | |
| WidgetAnchor | alias (union) | enum WidgetAnchor | fragment-layers | |
| WidgetBox | interface | struct WidgetBox | fragment-layers | |
| LazyWidget | interface | trait LazyWidget | fragment-layers | |
| ViewportCorner | alias (union) | enum ViewportCorner | fragment-layers | |
| WidgetOptions | interface | struct WidgetOptions | fragment-layers | |
| GutterSide | alias (union) | enum GutterSide | fragment-layers | |
| OverlayPlane | alias (union) | enum OverlayPlane | fragment-layers | |
| GutterSlot | interface | struct GutterSlot | fragment-layers | GutterArbiter |
| ViewCreator | alias (fonction) | type ViewCreator | fragment-core | `Box<dyn Fn(WorkspaceLeaf) -> Box<dyn View>>` |
| EmbedCreator | alias (fonction) | type EmbedCreator | fragment-ui | |
| EmbedContext | interface | struct EmbedContext | fragment-ui | |
| EmbedRenderer | interface | trait EmbedRenderer | fragment-ui | |
| EmbedDocumentOptions | interface | struct EmbedDocumentOptions | fragment-ui | |
| EmbeddedDocument | interface | trait EmbeddedDocument | fragment-ui | |
| PassageRef | interface | struct PassageRef | fragment-text | |
| ToolbarOrientation | alias (union) | enum ToolbarOrientation | fragment-ui | |
| VaultEventInfo | interface | struct VaultEventInfo | fragment-core | |
| SettingControl | alias (union) | enum SettingControl | fragment-ui | |
| SettingControlBase | interface | struct SettingControlBase | fragment-ui | |
| SettingToggleControl | interface | struct SettingToggleControl | fragment-ui | |
| SettingTextControl | interface | struct SettingTextControl | fragment-ui | |
| SettingTextAreaControl | interface | struct SettingTextAreaControl | fragment-ui | |
| SettingNumberControl | interface | struct SettingNumberControl | fragment-ui | |
| SettingSliderControl | interface | struct SettingSliderControl | fragment-ui | |
| SettingDropdownControl | interface | struct SettingDropdownControl | fragment-ui | |
| SettingColorControl | interface | struct SettingColorControl | fragment-ui | |
| SettingFileControl | interface | struct SettingFileControl | fragment-ui | |
| SettingFolderControl | interface | struct SettingFolderControl | fragment-ui | |
| SettingSecretControl | interface | struct SettingSecretControl | fragment-ui | |
| SettingDefinition | alias (union) | enum SettingDefinition | fragment-ui | |
| SettingDefinitionBase | interface | struct SettingDefinitionBase | fragment-ui | |
| SettingDefinitionControl | interface | struct SettingDefinitionControl | fragment-ui | |
| SettingDefinitionItem | interface | struct SettingDefinitionItem | fragment-ui | |
| SettingDefinitionGroup | interface | struct SettingDefinitionGroup | fragment-ui | |
| SettingDefinitionPage | interface | struct SettingDefinitionPage | fragment-ui | |
| SettingDefinitionList | interface | struct SettingDefinitionList | fragment-ui | |
| SettingDefinitionEmpty | interface | struct SettingDefinitionEmpty | fragment-ui | |
| SettingDefinitionAction | interface | struct SettingDefinitionAction | fragment-ui | |
| SettingDefinitionRender | interface | struct SettingDefinitionRender | fragment-ui | |
| SettingDefinitionAddItem | interface | struct SettingDefinitionAddItem | fragment-ui | |
| SettingDefinitionTree | interface | struct SettingDefinitionTree | fragment-ui | |
| SettingTreeNode | interface | struct SettingTreeNode | fragment-ui | |
| SettingGroupItem | alias (union) | enum SettingGroupItem | fragment-ui | |

## Table 2 : registres *@1

Tous déclarés par `defineRegistry` / `defineOrderedRegistry` ; en Rust : `RegistryHub::define::<T>("id@1")`, l'id reste identique.

| Id | Déclaré dans (chemin) | Forme de l'entrée (TS résumé) | Forme Rust | Crate |
|---|---|---|---|---|
| theme:manager@1 | `app/src/ui/theme/api.ts` | `ThemeManager` : list, refresh, openFolder, onChange | trait ThemeManager | fragment-theme |
| agent:harness@1 | `app/src/ui/views/agent/contract.ts` | registre ordonné d'`AgentHarness` : id, label, capabilities, status, models, run | trait AgentHarness | fragment-agent |
| agent:service@1 | `app/src/ui/views/agent/api.ts` | `AgentService` : openChat, ask, tour sans chat | trait AgentService | fragment-agent |
| annotation:tools@1 | `app/src/ui/views/annotation/api.ts` | ordonné d'`AnnotationTool` : id, label, icon, section, begin(gesture) | trait AnnotationTool | fragment-annotation |
| annotation:kinds@1 | `app/src/ui/views/annotation/api.ts` | ordonné de `StrokeKind` : above, opacity, widthFactor, hitTest | struct StrokeKind | fragment-annotation |
| annotation:presets@1 | `app/src/ui/views/annotation/api.ts` | ordonné d'`AnnotationPreset` : id, name, tools, filter | struct AnnotationPreset | fragment-annotation |
| annotation:extensions@1 | `app/src/ui/views/annotation/api.ts` | ordonné d'`AnnotationExtension` : create(session) -> Component | trait AnnotationExtension | fragment-annotation |
| annotation:filters@1 | `app/src/ui/views/annotation/api.ts` | ordonné de fonctions `(stroke, session) -> bool` | type AnnotationFilter | fragment-annotation |
| annotation:markers@1 | `app/src/ui/views/annotation/api.ts` | ordonné d'`AnnotationMarkerProvider` : above, markers(session) | trait AnnotationMarkerProvider | fragment-annotation |
| chat:provider@1 | `app/src/ui/views/chat/api.ts` | `ChatProvider` : id, label, icon, without, … | trait ChatProvider | fragment-chat |
| chat:composer@1 | `app/src/ui/views/chat/api.ts` | ordonné de `ChatComposerExtension` : id, appliesTo, create(composer, ctx) | trait ChatComposerExtension | fragment-chat |
| chat:header-actions@1 | `app/src/ui/views/chat/api.ts` | ordonné de `ChatSlot` : id, align, appliesTo, render(el, ctx) | struct ChatSlot (modèle) ; rendu dans fragment-ui | fragment-chat |
| chat:message-actions@1 | `app/src/ui/views/chat/api.ts` | ordonné de `ChatSlot` | struct ChatSlot | fragment-chat |
| chat:composer-tools@1 | `app/src/ui/views/chat/api.ts` | ordonné de `ChatSlot` (align start ou end) | struct ChatSlot | fragment-chat |
| chat:footer@1 | `app/src/ui/views/chat/api.ts` | ordonné de `ChatSlot` | struct ChatSlot | fragment-chat |
| chat:part@1 | `app/src/ui/views/chat/api.ts` | ordonné de `ChatPartRenderer` : type, render(el, part, ctx) -> handle | trait ChatPartRenderer | fragment-chat |
| chat:text@1 | `app/src/ui/views/chat/api.ts` | ordonné de `ChatTextRenderer` : id, appliesTo, render(text, ctx) | trait ChatTextRenderer | fragment-chat |
| markdown:render@1 | `app/src/ui/views/markdown/api.ts` | `MarkdownRenderService` : mount(el, text, opts) | trait MarkdownRenderService | fragment-markdown |
| doc-widget:host@1 | `app/src/ui/views/doc-widget/api.ts` | `DocWidgetHost` : open, close, setOpen, list(file) | trait DocWidgetHost | fragment-ui |

## Contrôle

Recomptés par lecture des `export` de public-api.ts (blocs `export { a, b }`, `export type { … }` et blocs multi-lignes inclus) :

| Mesure | Valeur |
|---|---|
| Exports de public-api.ts (comptés) | 188 |
| Lignes de la table 1 | 188 |
| Registres `*@1` trouvés (hors tests) | 19 |
| Lignes de la table 2 | 19 |

Égalités vérifiées. Cas douteux :
1. `requestUrl` et ses types : placés dans fragment-plugins (hôte), à confirmer contre fragment-core.
2. `WindowHost`, `CaptureRect` : trait dans fragment-core, implémenté par fragment-app ; la capture est liée à la fenêtre.
3. `SettingControl*` et `SettingDefinition*` : données pures, mais non listées dans §4 ; mis dans fragment-ui faute de mieux.
4. `Menu`, `Toolbar`, `Modal`, `Notice`, `Setting*` : classes DOM exposées aux plugins ; struct dans fragment-ui, l'API JS reste dans fragment-plugins.
5. `Rect` : dans fragment-text, réutilisé par fragment-layers.
6. `OverlayHost` : trait dans fragment-layers, l'hôte réel est dans fragment-ui.
7. `ViewCreator`, `View*` : dans fragment-core sans rendu ; la vue GPUI est construite côté ui.
8. `chat:*` ChatSlot : le `render(el)` DOM n'a pas d'équivalent ; le contrat Rust garde id/align/appliesTo, le rendu est fragment-ui.
9. `EXIT` vient de `shared/cli/protocol.ts`, pas de `ui/core`.
