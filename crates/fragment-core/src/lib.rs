//! Component/register*, registres, événements, portées clavier, commandes, plugins, Vault, documents, modèle du workspace, utilitaires purs.
//!
//! Phase 1, première moitié (`fr-p1-core-a`) : commandes, registres, clavier,
//! utilitaires purs. La seconde moitié (`fr-p1-core-b`) apporte le workspace, le
//! Vault, les documents.
//!
//! ★ Cette crate ne dépend JAMAIS de `gpui-kit` et ne connaît aucune vue par son
//!   nom (program.md §9) : ce qui touche au DOM dans Fragment est soit abstrait
//!   derrière un trait (stockage, chargeur), soit laissé à `fragment-ui`.

pub mod diff_minimal;
pub mod frontmatter;
pub mod lru;
pub mod semver;
pub mod swipe;

pub use diff_minimal::{Replacement, diff_minimal};
pub use frontmatter::parse_front_matter_string_array;
pub use lru::Lru;
pub use semver::{compare_versions, is_newer_version};
pub use swipe::{SWIPE_LEFT, SWIPE_RIGHT, SWIPE_SEUIL, SwipeDetector, SwipeDirection};
pub mod fuzzy_search;
pub mod resolve_link;
pub use fuzzy_search::{
    FuzzyMatch, FuzzySearch, SearchMatches, SearchResult, prepare_fuzzy_search,
};
pub use resolve_link::{LinkFile, LinkVault, ParsedLink, parse_linktext, resolve_link};
pub mod component;
pub mod events;
mod ordered_map;
pub mod registry;
pub mod registry_hub;
pub use component::{BasicComponent, Cleanup, Component, ComponentState};
pub use events::{EventRef, Events};
pub use registry::{
    Disposer, InspectableRegistry, OrderedRegistry, Registry, RegistryEntry, WeakOrderedRegistry,
    WeakRegistry,
};
pub use registry_hub::{
    Keyed, Kind, Ordered, RegistryDefinition, RegistryError, RegistryHub, RegistryKind,
    define_ordered_registry, define_registry,
};
pub mod capture_hotkey;
pub mod command;
pub mod keymap;
pub mod plugin;
pub use capture_hotkey::{capture_hotkey, find_hotkey_conflict};
pub use command::{CheckResult, Command, CommandHost, CommandManager, EditorContext};
pub use keymap::{
    Hotkey, HotkeyManager, HotkeyStore, InputEvent, KeyEvent, Keymap, KeymapContext,
    KeymapEventHandler, KeymapResult, Modifier, Modifiers, PaneType, Plateforme, PointerEvent,
    Scope, format_hotkey,
};
pub use plugin::{Plugin, PluginBase, PluginHost, PluginManifest};
pub mod plugins;
pub use plugins::{
    CatalogEntry, CommunityLoader, Listing, PluginAssets, PluginCtor, PluginError,
    PluginRegistrySource, PluginStorage, PluginUpdate, Plugins, SourceId, id_plugin_valide,
};

// ─── Seconde moitié (fr-p1-core-b) : documents, workspace (données), divers ───
pub mod breadcrumb;
pub mod document_key;
pub mod documents;
pub mod leaf_history;
pub mod workspace;
pub use breadcrumb::{
    BREADCRUMB_MAX_DEPTH, BreadcrumbFolder, BreadcrumbSegment, ancestor_folders,
    breadcrumb_segments,
};
pub use document_key::{
    DocumentKey, ParsedDocumentKey, format_document_key, is_under, is_vault_key, move_key,
    parse_document_key,
};
pub use documents::{
    DocumentError, DocumentEvent, DocumentEventType, DocumentManager, DocumentProvider,
    EventOrigin, OWNER_CORE,
};
pub use leaf_history::{HISTORY_MAX, HistoryEntry, LeafHistory};
pub use workspace::layout::{
    LayoutNode, LeafLayout, SplitLayout, TabsLayout, WorkspaceLayout, is_workspace_layout,
};
pub use workspace::{OpenViewState, SplitDirection, ViewState, ViewStateResult};
pub mod attachments;
pub mod debounce;
pub mod dispatch;
pub mod drag;
pub mod embed;
pub mod external;
pub mod platform;
pub mod secret_storage;
pub mod settings;
pub mod theme_setting;
pub mod window_host;
pub mod zoom;
pub use attachments::{
    AttachmentLocation, LocalDateTime, attachment_folder_for, extension_for_mime, pasted_image_name,
};
pub use debounce::{Debouncer, debounce};
pub use dispatch::{ExternalCommands, dispatch_external, executer_requete_cli, handle_protocol};
pub use drag::{DragData, DragHost, DragManager, DropExternal, FileMove, GHOST_OFFSET};
pub use embed::{
    EMPTY_VIEW_TYPE, EmbedHost, EmbedKind, MAX_EMBED_DEPTH, WheelDelta, embed_kind, wheel_pixels,
};
pub use external::{
    EXTERNAL_HANDLER_ERROR, EXTERNAL_UNKNOWN, ExternalCommandSpec, ExternalError, ExternalHandler,
    ExternalParams, ExternalResult, HandlerError,
};
pub use platform::{PLATFORM, PlatformFlags};
pub use secret_storage::{SecretError, SecretStorage};
pub use settings::{
    SettingColorControl, SettingControl, SettingControlBase, SettingDefinition,
    SettingDefinitionAction, SettingDefinitionAddItem, SettingDefinitionBase,
    SettingDefinitionControl, SettingDefinitionEmpty, SettingDefinitionGroup,
    SettingDefinitionItem, SettingDefinitionList, SettingDefinitionPage, SettingDefinitionTree,
    SettingDropdownControl, SettingExtraButtonControl, SettingFileControl, SettingFolderControl,
    SettingGroupItem, SettingNumberControl, SettingSecretControl, SettingSliderControl,
    SettingTextAreaControl, SettingTextControl, SettingToggleControl, SettingTreeNode,
};
pub use theme_setting::{
    ANCIEN_ACCENT_PAR_DEFAUT, SchemaCouleurs, SchemaEffectif, accent_choisi, schema_effectif,
};
pub use window_host::{CaptureRect, WindowHost, WindowHostError};
pub use zoom::{
    SensZoom, ZOOM_DEFAUT, ZOOM_MAX, ZOOM_MIN, ZOOM_PALIERS, format_zoom, normaliser_zoom,
    normaliser_zoom_valeur, zoom_suivant,
};
