// Spike S3 : le bootstrap JS de l'hôte de plugins fragment-rs (★ généré dans le dossier de
// staging à côté du `main.js` du plugin, renommé `plugin-main.js`).
//
// Rôle : gpui-shell veut qu'un `main.js` exporte par défaut une sous-classe de `View`. Un plugin
// Fragment exporte une classe `Plugin` (onload). Ce fichier fait le pont :
//   - il construit l'objet `fragment` (l'API publique du plugin) par-dessus le SEUL module hôte
//     `fragment-host`, dont les fonctions ne portent que des DONNÉES (null/bool/nombre/chaîne/
//     tableau/objet : le pont de gpui-shell ne passe jamais un handle de fonction) ;
//   - il garde en JS les callbacks (commandes, rendu des vues) et ne publie à Rust que des
//     descriptions JSON ;
//   - Rust -> JS passe par une BOÎTE AUX LETTRES (`take_messages`) lue au rendu : l'hôte ne peut pas
//     appeler le script de façon synchrone, il dépose un message puis demande un rendu.
//
// MODE (substitué par Rust) : "load" = onload au chargement du module, sans fenêtre ; "init" =
// onload dans `View.init` (le contexte `cx` permet `fragment.spawn`) ; "leaf" = une feuille
// supplémentaire d'un plugin déjà chargé : onload rejoué, mais AUCUNE inscription n'est envoyée.
import { View, div } from "gpui-kit";
import { h_flex, v_flex } from "gpui-base";
import { Text, Label, Button, Progress } from "gpui-component";
import * as host from "fragment-host";
import Plugin from "./plugin-main.js";

const MODE = "__MODE__";
const REGISTER = MODE !== "leaf";

const views = new Map(); // type -> { render(state) }
const commands = new Map(); // id -> { callback }
const widgets = new Map(); // id -> { render() }
const layers = new Map(); // id -> { create(ctx) }
const instances = new Map(); // instance -> cleanup
let spawnCx = null;
let autoId = 0;

function safe(what, f) {
  try {
    return f();
  } catch (e) {
    host.log("erreur-plugin|" + what + "|" + (e && e.message ? e.message : String(e)));
    return undefined;
  }
}

// ── L'arbre déclaratif -> composants gpui-kit ───────────────────────────────────────────────
const STYLE_ARG = {
  w: "w", h: "h", p: "p", px: "px", py: "py", pt: "pt", pb: "pb", pl: "pl", pr: "pr",
  m: "m", mx: "mx", my: "my", gap: "gap", rounded: "rounded", border: "border",
  bg: "bg", color: "text_color", border_color: "border_color", opacity: "opacity",
  text_size: "text_size", grow: "flex_grow",
};
const STYLE_FLAG = new Set([
  "size_full", "flex_1", "items_center", "justify_center", "font_semibold", "overflow_hidden",
]);

function styled(el, style) {
  if (!style) return el;
  for (const [key, value] of Object.entries(style)) {
    if (STYLE_ARG[key]) el = el[STYLE_ARG[key]](value);
    else if (STYLE_FLAG.has(key) && value === true) el = el[key]();
    else throw new Error("style inconnu : " + key);
  }
  return el;
}

function fire(action) {
  if (action && action.command) host.run_command(String(action.command));
  else if (action && action.event) host.event(String(action.event));
}

function build(node) {
  if (node === null || node === undefined) return "";
  if (typeof node === "string") return node;
  if (typeof node === "number") return String(node);
  switch (node.t) {
    case "div":
    case "h_flex":
    case "v_flex": {
      let el = node.t === "div" ? div() : node.t === "h_flex" ? h_flex() : v_flex();
      el = styled(el, node.style);
      for (const child of node.children || []) el = el.child(build(child));
      return el;
    }
    case "text":
      return styled(new Text(String(node.value)), node.style);
    case "label":
      return styled(new Label(String(node.text)), node.style);
    case "progress":
      return styled(new Progress(String(node.id)).value(Number(node.value || 0)), node.style);
    case "button": {
      // ★ L'ordre compte : `.on_click` n'existe plus après un `.w()`/`.h()` sur un composant.
      let button = new Button(String(node.id)).label(String(node.label));
      if (node.onClick) {
        const action = node.onClick;
        button = button.on_click((_event, cx) => {
          fire(action);
          cx.notify();
        });
      }
      return styled(button, node.style);
    }
    default:
      throw new Error("nœud inconnu : " + node.t);
  }
}

// L'arbre doit survivre à JSON.stringify/parse : c'est ce qui ferait aussi passer la frontière
// d'un hôte WASM. Le JSON reçu est aussi remis à Rust (`record_tree`) pour les assertions.
function renderTree(tree) {
  const json = JSON.stringify(tree);
  host.record_tree(json);
  return build(JSON.parse(json));
}

// ── L'API `fragment` ────────────────────────────────────────────────────────────────────────
function send(fn, ...args) {
  if (REGISTER) host[fn](...args);
}

const api = {
  registerView(type, def) {
    views.set(type, def);
    send("register_view", String(type));
  },
  addCommand(cmd) {
    commands.set(cmd.id, cmd);
    send("add_command", JSON.stringify({ id: cmd.id, name: cmd.name, icon: cmd.icon || null }));
  },
  // ★ Pas de prédicat ni de callback ici : `DocumentProvider` est SYNCHRONE côté Rust (titre d'un
  //   onglet restauré, historique) et l'hôte ne peut pas appeler le script de façon synchrone.
  //   Le fournisseur est donc déclaratif : schéma, type de vue, gabarit de titre.
  registerDocumentProvider(p) {
    send(
      "register_provider",
      JSON.stringify({ scheme: p.scheme, view: p.view, title: p.title || "{rest}", icon: p.icon || null }),
    );
  },
  addWidget(w) {
    const id = w.id || "widget-" + ++autoId;
    widgets.set(id, w);
    send(
      "add_widget",
      JSON.stringify({ id, mode: w.mode, side: w.side || "right", pos: w.pos || 0, dy: w.dy || 0 }),
    );
    return {
      id,
      remove() {
        widgets.delete(id);
        send("remove_widget", id);
      },
    };
  },
  // ★ `appliesTo` : un prédicat chez Fragment (`(view) => …`), une DONNÉE ici (`{ viewType }`).
  registerLayer(layer) {
    layers.set(layer.id, layer);
    send(
      "register_layer",
      JSON.stringify({
        id: layer.id,
        name: layer.name,
        icon: layer.icon || null,
        defaultEnabled: layer.defaultEnabled !== false,
        appliesTo: layer.appliesTo || null,
      }),
    );
  },
  requestRender() {
    host.request_render();
  },
  runCommand(id) {
    host.run_command(String(id));
  },
  log(...parts) {
    host.log(parts.map(String).join(" "));
  },
  config() {
    return JSON.parse(host.config());
  },
  spawn(fn) {
    if (!spawnCx) throw new Error("fragment.spawn n'est disponible que pendant onload (mode init)");
    spawnCx.spawn(async (cx) => {
      try {
        await fn();
      } catch (e) {
        host.log("erreur-plugin|spawn|" + (e && e.message ? e.message : String(e)));
      }
      cx.notify();
    });
  },
};

let plugin = null;
function boot(cx) {
  if (plugin) return;
  spawnCx = cx;
  plugin = new Plugin();
  safe("onload", () => plugin.onload(api));
  spawnCx = null;
}

if (MODE === "load") boot(null);

// ── Rust -> JS : la boîte aux lettres, lue à chaque rendu ───────────────────────────────────
function pump() {
  const messages = JSON.parse(host.take_messages());
  for (const m of messages) {
    if (m.kind === "command") {
      const c = commands.get(m.id);
      if (c) safe("command:" + m.id, () => c.callback());
    } else if (m.kind === "layer-create") {
      const layer = layers.get(m.layer);
      if (layer) {
        const cleanup = safe("layer:" + m.layer, () => layer.create(m.ctx));
        if (typeof cleanup === "function") instances.set(m.instance, cleanup);
      }
    } else if (m.kind === "layer-destroy") {
      const cleanup = instances.get(m.instance);
      instances.delete(m.instance);
      if (cleanup) safe("layer-destroy", cleanup);
    }
  }
}

export default class PluginHostView extends View {
  init(_props, cx) {
    if (MODE !== "load") boot(cx);
  }

  render(_cx) {
    pump();
    const target = host.active();
    if (!target) return div();
    const sep = target.indexOf(":");
    const kind = target.slice(0, sep);
    const name = target.slice(sep + 1);
    const state = JSON.parse(host.active_state());
    const def = kind === "view" ? views.get(name) : widgets.get(name);
    if (!def) return div().child("introuvable : " + target);
    const tree = safe("render:" + target, () => def.render(state));
    if (tree === undefined) return div().child("erreur de rendu : " + target);
    return renderTree(tree);
  }
}
