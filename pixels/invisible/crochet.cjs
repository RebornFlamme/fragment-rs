'use strict';
// Crochet du mode invisible (fr-9x4), chargé dans le processus PRINCIPAL d'Electron par
// `electron -r <ce fichier> .` — donc AVANT le main de Fragment, qu'on ne modifie pas (lecture seule).
//
// Il enveloppe BrowserWindow pour TOUTES les fenêtres (principale, réglages, plugins, gestion des
// coffres) : créées `show: false`, posées HORS de tous les moniteurs, `skipTaskbar`, non
// activables, puis montrées par `showInactive()`. `show()`, `focus()`, `moveTop()` et tout ce qui
// ramènerait une fenêtre sur un écran (maximize, plein écran, center, setPosition…) sont neutralisés.
// Les fenêtres natives qui naîtraient ailleurs (DevTools, dialogues, menus contextuels, notifications,
// ouverture d'une appli externe) sont refusées. Chaque refus est journalisé.
//
// ★ Electron lit les `-r` seulement AVANT le chemin de l'appli (son default_app arrête l'analyse au
//   premier argument positionnel) : le garde place `-r` devant le `.` du harnais.
// ★ `require('electron')` expose des accesseurs NON configurables : on ne peut pas remplacer
//   BrowserWindow dessus. On intercepte donc le CHARGEMENT du module « electron » (Module._load,
//   que l'import ESM de Fragment emprunte aussi — vérifié par le garde : « classe » au journal).
// ★ L'option `show` est lue par le constructeur NATIF après `_init` : patcher le prototype ne suffit
//   pas, la fenêtre serait déjà montrée (et activée). Il faut la sous-classe.

const fs = require('fs');
const Module = require('module');
const electron = require('electron');

const JOURNAL = process.env.FRAGMENT_INVISIBLE_JOURNAL || '';
const X0 = Number(process.env.FRAGMENT_INVISIBLE_X);
const Y0 = Number(process.env.FRAGMENT_INVISIBLE_Y);

function noter(o) {
    const ligne = JSON.stringify({ t: Date.now(), source: 'crochet', pid: process.pid, ...o });
    if (JOURNAL) {
        try { fs.appendFileSync(JOURNAL, ligne + '\n'); } catch { /* journal indisponible : stderr suffit */ }
    }
    process.stderr.write(`[invisible] ${ligne}\n`);
}

function arreter(raison, detail) {
    noter({ type: 'ECHEC', raison, detail });
    // ★ exit et non quit : quit attend les fenêtres (beforeunload…), exit coupe tout de suite.
    electron.app.exit(97);
}

if (process.type !== 'browser') {
    // Chargé ailleurs que dans le processus principal (ne devrait pas arriver) : rien à faire.
    module.exports = {};
    return;
}
if (!Number.isFinite(X0) || !Number.isFinite(Y0)) {
    // Sans position hors écran calculée par la sentinelle, on ne montre RIEN : refus de démarrer.
    process.stderr.write('[invisible] FRAGMENT_INVISIBLE_X/Y absents : lancement refusé (mode invisible seul).\n');
    process.exit(97);
}

const { app } = electron;
for (const s of ['disable-backgrounding-occluded-windows', 'disable-renderer-backgrounding', 'disable-background-timer-throttling']) {
    app.commandLine.appendSwitch(s);
}

const Orig = electron.BrowserWindow;
const proto = Orig.prototype;
const vrai = {
    showInactive: proto.showInactive,
    setBounds: proto.setBounds,
    setContentBounds: proto.setContentBounds,
    getBounds: proto.getBounds,
};

/** La position hors écran : celle de la sentinelle (pixels physiques), repoussée au besoin au-delà
 *  des écrans que voit Electron (DIP) — le plus à droite des deux gagne. */
function horsEcran() {
    let x = X0;
    const y = Y0;
    try {
        for (const d of electron.screen.getAllDisplays()) x = Math.max(x, d.bounds.x + d.bounds.width + 2000);
    } catch { /* écran pas encore disponible (avant ready) : la position de la sentinelle suffit */ }
    return { x, y };
}

function intersecte(a, b) {
    return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
}

/** Contrôle côté Electron (la sentinelle fait le sien, en pixels physiques, indépendamment). */
function verifier(w, quand) {
    if (w.isDestroyed()) return;
    const b = vrai.getBounds.call(w);
    try {
        for (const d of electron.screen.getAllDisplays()) {
            if (intersecte(b, d.bounds)) { arreter('fenetre-sur-ecran', { quand, id: w.id, bounds: b, ecran: d.bounds }); return; }
        }
    } catch { /* screen indisponible : la sentinelle reste juge */ }
}

function ramener(w, quand) {
    if (w.isDestroyed()) return;
    const b = vrai.getBounds.call(w);
    const h = horsEcran();
    if (b.x !== h.x || b.y !== h.y) vrai.setBounds.call(w, { ...b, x: h.x, y: h.y });
    verifier(w, quand);
}

class FenetreInvisible extends Orig {
    constructor(options = {}) {
        const voulueVisible = options.show !== false;
        const h = horsEcran();
        super({
            ...options,
            show: false,
            x: h.x,
            y: h.y,
            center: false,
            fullscreen: false,
            kiosk: false,
            alwaysOnTop: false,
            skipTaskbar: true,
            focusable: false,
        });
        noter({ type: 'fenetre', id: this.id, titre: options.title ?? '', voulueVisible, bounds: vrai.getBounds.call(this) });
        for (const ev of ['move', 'resize', 'show', 'restore', 'maximize', 'enter-full-screen']) {
            this.on(ev, () => ramener(this, ev));
        }
        neutraliserWebContents(this.webContents);
        if (voulueVisible) {
            vrai.showInactive.call(this);
            verifier(this, 'construction');
        }
    }
}

// ---- Méthodes neutralisées (toutes les fenêtres, même celles d'une autre provenance) -------------
const refus = (nom, faire) => function (...args) {
    noter({ type: 'neutralise', methode: nom, id: this?.id });
    return faire ? faire.apply(this, args) : undefined;
};
proto.show = refus('show', function () { vrai.showInactive.call(this); verifier(this, 'show'); });
proto.showInactive = function () { vrai.showInactive.call(this); verifier(this, 'showInactive'); };
for (const nom of ['focus', 'moveTop', 'maximize', 'center', 'setPosition', 'flashFrame', 'setAlwaysOnTop', 'setVisibleOnAllWorkspaces']) {
    proto[nom] = refus(nom);
}
proto.setFullScreen = refus('setFullScreen');
proto.setSimpleFullScreen = refus('setSimpleFullScreen');
proto.setKiosk = refus('setKiosk');
proto.setSkipTaskbar = refus('setSkipTaskbar');
proto.setFocusable = refus('setFocusable');
proto.setBounds = function (b, ...reste) {
    const h = horsEcran();
    const r = vrai.setBounds.call(this, { ...vrai.getBounds.call(this), ...b, x: h.x, y: h.y }, ...reste);
    verifier(this, 'setBounds');
    return r;
};
proto.setContentBounds = function (b, ...reste) {
    const r = vrai.setContentBounds.call(this, b, ...reste);
    ramener(this, 'setContentBounds');
    return r;
};

let wcPatche = false;
function neutraliserWebContents(wc) {
    if (wcPatche || !wc) return;
    wcPatche = true;
    const p = Object.getPrototypeOf(wc);
    // ★ DevTools détachés = une fenêtre native centrée sur l'écran : jamais (le harnais les ferme de toute façon).
    p.openDevTools = refus('webContents.openDevTools');
}

/** Remplace `cible[nom]` ; un échec (propriété figée) est journalisé — la sentinelle reste juge. */
function remplacer(cible, nom, valeur, etiquette) {
    try {
        cible[nom] = valeur;
        if (cible[nom] !== valeur) noter({ type: 'remplacement-ignore', methode: etiquette });
    } catch (e) {
        noter({ type: 'remplacement-impossible', methode: etiquette, erreur: String(e) });
    }
}
remplacer(app, 'focus', refus('app.focus'), 'app.focus');
for (const nom of ['showOpenDialog', 'showOpenDialogSync', 'showSaveDialog', 'showSaveDialogSync', 'showMessageBox', 'showMessageBoxSync', 'showCertificateTrustDialog']) {
    remplacer(electron.dialog, nom, function () {
        noter({ type: 'neutralise', methode: `dialog.${nom}` });
        throw new Error(`mode invisible : dialog.${nom} refusé (une boîte native s'afficherait à l'écran)`);
    }, `dialog.${nom}`);
}
remplacer(electron.dialog, 'showErrorBox', (titre, contenu) => noter({ type: 'neutralise', methode: 'dialog.showErrorBox', titre, contenu: String(contenu).slice(0, 300) }), 'dialog.showErrorBox');
remplacer(electron.Menu.prototype, 'popup', refus('Menu.popup'), 'Menu.popup');
if (electron.Notification) remplacer(electron.Notification.prototype, 'show', refus('Notification.show'), 'Notification.show');
remplacer(electron.shell, 'openExternal', async (url) => { noter({ type: 'neutralise', methode: 'shell.openExternal', url: String(url).slice(0, 200) }); }, 'shell.openExternal');
remplacer(electron.shell, 'openPath', async (p) => { noter({ type: 'neutralise', methode: 'shell.openPath', chemin: String(p).slice(0, 200) }); return ''; }, 'shell.openPath');
remplacer(electron.shell, 'showItemInFolder', refus('shell.showItemInFolder'), 'shell.showItemInFolder');

// ---- Interception du module « electron » ---------------------------------------------------------
const facade = new Proxy(electron, {
    get(cible, cle, recepteur) {
        if (cle === 'BrowserWindow') return FenetreInvisible;
        return Reflect.get(cible, cle, recepteur);
    },
});
// CommonJS : require('electron').
const chargerVrai = Module._load;
Module._load = function (requete, ...reste) {
    if (requete === 'electron' || requete === 'electron/main') return facade;
    return chargerVrai.call(this, requete, ...reste);
};
// ★ ESM (le main de Fragment est un module ES) : `import { BrowserWindow } from 'electron'` NE passe PAS
//   par Module._load (mesuré sur une appli factice : la vraie classe était servie). On redirige donc la
//   résolution de « electron » vers un module ES synthétique qui réexporte la façade, par les crochets de
//   modules de Node (registerHooks, synchrones, dans ce fil). Ce module ne réimporte pas « electron » :
//   pas de boucle. Sans registerHooks, on refuse de démarrer plutôt que de montrer une vraie fenêtre.
const CLE_FACADE = Symbol.for('fragment-rs.invisible.facade');
globalThis[CLE_FACADE] = facade;
const URL_FACADE = 'file:///fragment-rs-invisible/facade-electron.mjs';
if (typeof Module.registerHooks !== 'function') {
    process.stderr.write('[invisible] module.registerHooks absent : interception ESM impossible, lancement refusé.\n');
    process.exit(97);
}
const nomsExportes = Object.keys(electron).filter((k) => /^[A-Za-z_$][\w$]*$/.test(k) && k !== 'default');
const sourceFacade = [
    `const e = globalThis[Symbol.for('fragment-rs.invisible.facade')];`,
    'export default e;',
    ...nomsExportes.map((k) => `export const ${k} = e.${k};`),
].join('\n');
Module.registerHooks({
    resolve(specifier, context, suivant) {
        const esm = (context.conditions ?? []).includes('import');
        if (esm && (specifier === 'electron' || specifier === 'electron/main')) {
            return { url: URL_FACADE, format: 'module', shortCircuit: true };
        }
        return suivant(specifier, context);
    },
    load(url, context, suivant) {
        if (url === URL_FACADE) return { format: 'module', source: sourceFacade, shortCircuit: true };
        return suivant(url, context);
    },
});

// Preuve pour le garde : la classe effectivement servie au main de Fragment (vérifiée à ready).
app.once('ready', () => {
    noter({ type: 'pret', horsEcran: horsEcran(), ecrans: (() => { try { return electron.screen.getAllDisplays().map((d) => d.bounds); } catch { return []; } })() });
});

noter({ type: 'charge', x: X0, y: Y0, argv: process.argv.slice(1, 6) });
module.exports = { FenetreInvisible };
