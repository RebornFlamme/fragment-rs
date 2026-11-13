// Garde du mode invisible (fr-9x4), côté Playwright : le SEUL chemin par lequel une scène lance Electron.
//
// `activerModeInvisible()` (appelée par playwright.config.ts, donc dans chaque processus de test)
// enveloppe `_electron.launch` du Playwright de Fragment — l'instance unique que le harnais e2e de
// Fragment (lecture seule) et nos décors _*.ts appellent tous deux. Chaque lancement :
//   1. refuse de partir si une preuve a déjà échoué (drapeau), ou, sur le PC, sans le verrou de capture ;
//   2. démarre au besoin la sentinelle Win32 (sentinelle.ps1, fenêtre cachée) et lui fait relever le
//      premier plan AVANT ;
//   3. ajoute `-r crochet.cjs` DEVANT les arguments (avant le « . » du harnais) et passe la position hors
//      écran calculée par la sentinelle depuis la liste des moniteurs ;
//   4. vérifie que chaque fenêtre est bien une FenetreInvisible, active l'émulation du focus (la page se
//      croit au premier plan sans jamais l'être) et neutralise page.bringToFront ;
//   5. à la fermeture, fait faire le relevé APRÈS (règle du 13/11 : échec seulement si Electron a pris le
//      premier plan, touché un moniteur ou gagné un bouton de barre des tâches ; un geste de l'utilisateur
//      entre SES fenêtres est compté, pas sanctionné).
// Un ECHEC de la sentinelle tue Electron (elle le fait elle-même), écrit le drapeau et fait échouer tous
// les lancements suivants : le run s'arrête.

import { _electron, type ElectronApplication, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { spawn, type ChildProcessWithoutNullStreams } from 'child_process';
import { appendFileSync, existsSync, mkdirSync, writeFileSync } from 'fs';
import path from 'path';

const ICI = __dirname;
export const CROCHET = path.join(ICI, 'crochet.cjs');
export const SENTINELLE = path.join(ICI, 'sentinelle.ps1');
const SORTIE = path.join(ICI, '..', '.out', 'invisible');
export const DRAPEAU_ECHEC = path.join(SORTIE, 'ECHEC.txt');
const JOURNAL_SENTINELLE = path.join(SORTIE, 'sentinelle.jsonl');
const JOURNAL_CROCHET = path.join(SORTIE, 'crochet.jsonl');
const JOURNAL_GARDE = path.join(SORTIE, 'garde.jsonl');
export const VERROU_CAPTURE = 'C:/dev/fragment-capture.lock';
const MARQUE = Symbol.for('fragment-rs.mode-invisible');

type Ligne = { type: string; id?: string; [k: string]: unknown };

function noter(o: Record<string, unknown>): void {
    mkdirSync(SORTIE, { recursive: true });
    appendFileSync(JOURNAL_GARDE, JSON.stringify({ t: Date.now(), source: 'garde', pid: process.pid, ...o }) + '\n');
}

let echecGlobal: string | null = null;
const actifs = new Set<ElectronApplication>();

function declarerEchec(raison: string): void {
    if (!echecGlobal) {
        echecGlobal = raison;
        noter({ type: 'ECHEC', raison });
        mkdirSync(SORTIE, { recursive: true });
        if (!existsSync(DRAPEAU_ECHEC)) writeFileSync(DRAPEAU_ECHEC, `${new Date().toISOString()} ${raison}\n`);
        console.error(`[invisible] ÉCHEC : ${raison} — Electron fermé, run arrêté.`);
    }
    for (const app of actifs) {
        try { app.process().kill(); } catch { /* déjà mort */ }
    }
}

function refuserSiEchec(): void {
    if (echecGlobal) throw new Error(`mode invisible : run arrêté après l'échec d'une preuve (${echecGlobal}).`);
    if (existsSync(DRAPEAU_ECHEC)) {
        throw new Error(`mode invisible : une preuve a échoué lors d'un run précédent (${DRAPEAU_ECHEC}). Analyser le journal, puis supprimer ce fichier.`);
    }
}

// ---- Sentinelle ------------------------------------------------------------------------------
class Sentinelle {
    private attentes: { type: string; id: string; ok: (l: Ligne) => void; ko: (e: Error) => void }[] = [];
    private reste = '';
    horsEcran: [number, number] = [0, 0];
    private constructor(private proc: ChildProcessWithoutNullStreams) {}

    static demarrer(): Promise<Sentinelle> {
        mkdirSync(SORTIE, { recursive: true });
        const proc = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', SENTINELLE,
            '-Racine', String(process.pid), '-Journal', JOURNAL_SENTINELLE, '-Drapeau', DRAPEAU_ECHEC], { windowsHide: true });
        const s = new Sentinelle(proc);
        proc.stdout.setEncoding('utf8');
        proc.stdout.on('data', (d: string) => s.recevoir(d));
        proc.stderr.setEncoding('utf8');
        proc.stderr.on('data', (d: string) => noter({ type: 'sentinelle-stderr', texte: d.slice(0, 2000) }));
        proc.on('exit', (code) => {
            noter({ type: 'sentinelle-sortie', code });
            if (code !== 0 || actifs.size > 0) declarerEchec(`sentinelle arrêtée (code ${code}) : plus de preuve`);
            for (const a of s.attentes) a.ko(new Error(`sentinelle arrêtée (code ${code})`));
            s.attentes = [];
            if (sentinelle === s) sentinelle = null;
        });
        process.once('exit', () => { try { proc.stdin.end('QUITTER\n'); } catch { /* déjà fermée */ } });
        return s.attendre('pret', '', 30_000).then((l) => {
            s.horsEcran = l.horsEcran as [number, number];
            if (!l.crochets) throw new Error('sentinelle : SetWinEventHook a échoué');
            noter({ type: 'sentinelle-prete', moniteurs: l.moniteurs, horsEcran: l.horsEcran });
            return s;
        });
    }

    private recevoir(d: string): void {
        this.reste += d;
        let i: number;
        while ((i = this.reste.indexOf('\n')) >= 0) {
            const brut = this.reste.slice(0, i).trim();
            this.reste = this.reste.slice(i + 1);
            if (!brut.startsWith('{')) continue;
            let l: Ligne;
            try { l = JSON.parse(brut); } catch { continue; }
            if (l.type === 'ECHEC') {
                declarerEchec(`preuve « ${String(l.preuve)} » : ${JSON.stringify(l.detail)}`);
                for (const a of this.attentes) a.ko(new Error(`mode invisible : preuve « ${String(l.preuve)} » en échec`));
                this.attentes = [];
                continue;
            }
            const k = this.attentes.findIndex((a) => a.type === l.type && (a.id === '' || a.id === l.id));
            if (k >= 0) this.attentes.splice(k, 1)[0].ok(l);
        }
    }

    private attendre(type: string, id: string, delai: number): Promise<Ligne> {
        return new Promise((ok, ko) => {
            const minuteur = setTimeout(() => ko(new Error(`sentinelle : pas de réponse « ${type} » en ${delai} ms`)), delai);
            this.attentes.push({ type, id, ok: (l) => { clearTimeout(minuteur); ok(l); }, ko: (e) => { clearTimeout(minuteur); ko(e); } });
        });
    }

    commande(verbe: 'AVANT' | 'APRES', id: string): Promise<Ligne> {
        const p = this.attendre(verbe === 'AVANT' ? 'avant' : 'apres', id, 15_000);
        this.proc.stdin.write(`${verbe} ${id}\n`);
        return p;
    }
}

let sentinelle: Sentinelle | null = null;
let demarrage: Promise<Sentinelle> | null = null;
function laSentinelle(): Promise<Sentinelle> {
    if (sentinelle) return Promise.resolve(sentinelle);
    demarrage ??= Sentinelle.demarrer().then((s) => { sentinelle = s; demarrage = null; return s; }, (e) => { demarrage = null; throw e; });
    return demarrage;
}

// ---- Lancement -------------------------------------------------------------------------------
let compteur = 0;

async function brancherPage(app: ElectronApplication, page: Page, id: string): Promise<void> {
    // ★ L'émulation du focus (la page se croit au premier plan : caret, sélection active, :focus) —
    //   Playwright ne la pose pas pour Electron, et la fenêtre n'est jamais activée en mode invisible.
    const proto = Object.getPrototypeOf(page);
    if (!proto[MARQUE]) {
        proto[MARQUE] = true;
        proto.bringToFront = async function () { noter({ type: 'neutralise', methode: 'page.bringToFront' }); };
    }
    try {
        const cdp = await app.context().newCDPSession(page);
        await cdp.send('Emulation.setFocusEmulationEnabled', { enabled: true });
    } catch (e) {
        noter({ type: 'focus-emulation-impossible', id, erreur: String(e).slice(0, 300) });
    }
    // Chaque fenêtre doit porter la marque du crochet (BrowserWindow bien intercepté). On compte les
    // fenêtres par BaseWindow : BrowserWindow.getAllWindows() filtre sur le nom de classe.
    try {
        let marques: boolean[] = [];
        for (let essai = 0; essai < 10 && marques.length === 0; essai++) {
            if (essai) await new Promise((f) => setTimeout(f, 300));
            marques = await app.evaluate(({ BaseWindow }) => BaseWindow.getAllWindows().map((w) => (w as any).fragmentInvisible === true));
        }
        noter({ type: 'fenetres', id, marques });
        if (marques.some((m) => !m)) declarerEchec(`fenêtre hors crochet (${marques.filter((m) => !m).length} sur ${marques.length})`);
    } catch (e) {
        if (!String(e).includes('closed')) noter({ type: 'controle-classes-impossible', id, erreur: String(e).slice(0, 300) });
    }
}

async function lancerInvisible(lancer: (o: any) => Promise<ElectronApplication>, options: any = {}): Promise<ElectronApplication> {
    refuserSiEchec();
    if (!process.env.GITHUB_ACTIONS && !existsSync(VERROU_CAPTURE)) {
        throw new Error(`mode invisible : verrou de capture absent (${VERROU_CAPTURE}) — lancer par scripts/pixels-reference.ps1.`);
    }
    const s = await laSentinelle();
    const id = `${process.pid}-${++compteur}`;
    const avant = await s.commande('AVANT', id);
    noter({ type: 'avant', id, premierPlan: avant.premierPlan });
    const args = ['-r', CROCHET, ...(options.args ?? [])];
    const env = {
        ...(options.env ?? process.env),
        FRAGMENT_INVISIBLE_X: String(s.horsEcran[0]),
        FRAGMENT_INVISIBLE_Y: String(s.horsEcran[1]),
        FRAGMENT_INVISIBLE_JOURNAL: JOURNAL_CROCHET,
    };
    // ★ Une seule demande APRÈS, partagée : l'événement « close » et app.close() attendent LA MÊME
    //   réponse (sinon close() rendait la main tout de suite et le processus de test sortait avant la
    //   réponse de la sentinelle).
    let fin: Promise<void> | null = null;
    const terminer = (): Promise<void> => {
        fin ??= s.commande('APRES', id).then((apres) => {
            noter({ type: 'apres', id, identique: apres.identique, changementsUtilisateur: apres.changementsUtilisateur, balayages: apres.balayages, relevesBarre: apres.relevesBarre, fenetresMax: apres.fenetresMax });
            console.log(`[invisible] ${id} : jamais au premier plan, aucune fenêtre sur un moniteur ni dans la barre des tâches (${String(apres.balayages)} balayages, ${String(apres.relevesBarre)} relevés de barre, ${String(apres.changementsUtilisateur)} geste(s) de l'utilisateur).`);
        });
        return fin;
    };
    let app: ElectronApplication;
    try {
        app = await lancer({ ...options, args, env });
    } catch (e) {
        await terminer().catch(() => {});
        throw e;
    }
    actifs.add(app);
    noter({ type: 'lance', id, pidElectron: app.process().pid });
    for (const p of app.windows()) void brancherPage(app, p, id);
    app.on('window', (p) => void brancherPage(app, p, id));
    app.on('close', () => { actifs.delete(app); void terminer().catch(() => {}); });
    const fermer = app.close.bind(app);
    app.close = async () => {
        try { await fermer(); } finally { actifs.delete(app); }
        await terminer();
        refuserSiEchec();
    };
    return app;
}

/**
 * Active le mode invisible pour ce processus — et refuse de démarrer s'il ne peut pas l'être.
 * Pas d'autre mode : `FRAGMENT_PIXELS_MODE`, s'il est posé, ne peut valoir que « invisible ».
 */
export function activerModeInvisible(): void {
    const mode = process.env.FRAGMENT_PIXELS_MODE;
    if (mode !== undefined && mode !== 'invisible') {
        throw new Error(`mode invisible seul : FRAGMENT_PIXELS_MODE=${mode} refusé (fr-9x4, décision de l'utilisateur).`);
    }
    if (process.platform !== 'win32') throw new Error('mode invisible : les preuves (sentinelle Win32) exigent Windows.');
    for (const f of [CROCHET, SENTINELLE]) {
        if (!existsSync(f)) throw new Error(`mode invisible : ${f} introuvable — démarrage refusé.`);
    }
    const e = _electron as any;
    if (e[MARQUE]) return;
    const lancer = e.launch.bind(e);
    e.launch = (options: unknown) => lancerInvisible(lancer, options);
    e[MARQUE] = true;
}

/** Vrai si `_electron.launch` passe bien par le garde (contrôlé par la config). */
export function modeInvisibleActif(): boolean {
    return !!(_electron as any)[MARQUE];
}
