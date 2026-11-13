// ═══════════════════════════════════════════════════════════════════════════
//  Relevé de l'IDENTITÉ des glyphes côté référence (fr-p3.4, ADR-009).
//
//  Juste avant une capture, on parcourt le DOM de la fenêtre et on note, pour
//  chaque caractère visible, le caractère et le rectangle de sa lettre
//  (Range.getClientRects, en pixels de la capture). Le sas compare ensuite
//  caractère contre caractère, à la même place, avec le glyphe peint que le
//  candidat lit dans sa scène GPUI. Fragment n'est pas touché : tout passe par
//  le DOM de la page pilotée par Playwright.
//
//  Fichier écrit : reference/<scène>-<thème>.glyphes.json, voisin du PNG.
//  Format (version 1), un glyphe par ligne pour que les diffs restent lisibles :
//    { "version": 1, "source": "dom", "largeur": L, "hauteur": H,
//      "glyphes": [ ["a", x, y, largeur, hauteur], … ],
//      "rognes": [ indices des glyphes coupés par un rectangle de découpe ],
//      "ecartes": { "couverts": n, "decoupes": n, "invisibles": n } }
//  Rectangles en pixels de la capture (px CSS × échelle de la capture), au
//  centième. Le candidat écrit le même format avec "source": "scene".
//
//  Tant que le relevé n'est pas branché sur le sas, il est OPTIONNEL : il ne
//  s'écrit que si la variable d'environnement FRAGMENT_GLYPHES vaut 1.
//
//  Ce fichier n'a aucun test (pas de suffixe .spec.ts).
// ═══════════════════════════════════════════════════════════════════════════
import type { Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { writeFileSync } from 'fs';

export const VERSION_GLYPHES = 1;

/** Ce que la page renvoie : glyphes en px CSS, écartés comptés par motif. */
interface ReleveDom {
    largeurCss: number;
    hauteurCss: number;
    glyphes: [string, number, number, number, number][];
    rognes: number[];
    ecartes: { couverts: number; decoupes: number; invisibles: number };
}

/** Le relevé est-il demandé ? (FRAGMENT_GLYPHES=1) */
export function releveActif(): boolean {
    return process.env.FRAGMENT_GLYPHES === '1';
}

/** Largeur et hauteur d'un PNG, lues dans son en-tête IHDR. */
function taillePng(png: Buffer): [number, number] {
    return [png.readUInt32BE(16), png.readUInt32BE(20)];
}

/**
 * Relève les glyphes visibles de `page`. À appeler AVANT la capture, sur le même état (rien ne
 * bouge entre les deux : animations coupées, souris immobile).
 */
async function releverDom(page: Page): Promise<ReleveDom> {
    return page.evaluate(() => {
        type Rect = { x0: number; y0: number; x1: number; y1: number };
        const vide = (r: Rect) => r.x1 <= r.x0 || r.y1 <= r.y0;
        const inter = (a: Rect, b: Rect): Rect => ({
            x0: Math.max(a.x0, b.x0), y0: Math.max(a.y0, b.y0),
            x1: Math.min(a.x1, b.x1), y1: Math.min(a.y1, b.y1),
        });
        const fenetre: Rect = { x0: 0, y0: 0, x1: window.innerWidth, y1: window.innerHeight };

        // ★ Le rectangle de découpe d'un élément suit la chaîne de ses BLOCS CONTENEURS, pas celle
        // de ses parents : un `position: absolute` échappe au `overflow: hidden` d'un parent non
        // positionné, un `position: fixed` à tous (sauf ancêtre transformé).
        const conteneur = (e: Element): Element | null => {
            const pos = getComputedStyle(e).position;
            let a = e.parentElement;
            if (pos === 'fixed') {
                while (a) {
                    const s = getComputedStyle(a);
                    if (s.transform !== 'none' || s.filter !== 'none' || s.contain.includes('paint')) return a;
                    a = a.parentElement;
                }
                return null;
            }
            if (pos === 'absolute') {
                while (a) {
                    const s = getComputedStyle(a);
                    if (s.position !== 'static' || s.transform !== 'none') return a;
                    a = a.parentElement;
                }
                return null;
            }
            return a;
        };
        const decoupes = new Map<Element, Rect>();
        const decoupe = (e: Element | null): Rect => {
            if (!e) return fenetre;
            const connu = decoupes.get(e);
            if (connu) return connu;
            let r = decoupe(conteneur(e));
            const s = getComputedStyle(e);
            const coupe = (v: string) => v !== 'visible';
            if (coupe(s.overflowX) || coupe(s.overflowY) || s.contain.includes('paint')) {
                // Boîte de remplissage : bordures exclues, barres de défilement exclues.
                const b = e.getBoundingClientRect();
                const html = e as HTMLElement;
                const x0 = b.left + html.clientLeft;
                const y0 = b.top + html.clientTop;
                const boite: Rect = {
                    x0: coupe(s.overflowX) ? x0 : -Infinity,
                    y0: coupe(s.overflowY) ? y0 : -Infinity,
                    x1: coupe(s.overflowX) ? x0 + html.clientWidth : Infinity,
                    y1: coupe(s.overflowY) ? y0 + html.clientHeight : Infinity,
                };
                r = inter(r, boite);
            }
            decoupes.set(e, r);
            return r;
        };

        const transparent = (couleur: string) => /^rgba\(.*,\s*0\)$/.test(couleur) || couleur === 'transparent';
        const casse = (c: string, transformation: string, debutDeMot: boolean) => {
            if (transformation === 'uppercase') return c.toLocaleUpperCase();
            if (transformation === 'lowercase') return c.toLocaleLowerCase();
            if (transformation === 'capitalize' && debutDeMot) return c.toLocaleUpperCase();
            return c;
        };
        // Blancs et caractères sans encre (espaces, séparateurs invisibles).
        const sansEncre = /^[\s​-‏⁠﻿­]$/u;

        const releve = {
            largeurCss: window.innerWidth,
            hauteurCss: window.innerHeight,
            glyphes: [] as [string, number, number, number, number][],
            rognes: [] as number[],
            ecartes: { couverts: 0, decoupes: 0, invisibles: 0 },
        };
        const marcheur = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        const plage = document.createRange();
        for (let n = marcheur.nextNode(); n; n = marcheur.nextNode()) {
            const texte = n as Text;
            const el = texte.parentElement;
            if (!el || !texte.data.trim()) continue;
            if (!el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) {
                releve.ecartes.invisibles += 1;
                continue;
            }
            const s = getComputedStyle(el);
            if (transparent(s.color) || transparent(s.getPropertyValue('-webkit-text-fill-color'))) {
                releve.ecartes.invisibles += 1;
                continue;
            }
            const clip = decoupe(el);
            let i = 0;
            let precedent = ' ';
            for (const c of texte.data) {
                const debut = i;
                i += c.length;
                const debutDeMot = /\s/u.test(precedent);
                precedent = c;
                if (sansEncre.test(c)) continue;
                plage.setStart(texte, debut);
                plage.setEnd(texte, i);
                const r = Array.from(plage.getClientRects()).find((b) => b.width > 0 && b.height > 0);
                if (!r) continue;
                const entier: Rect = { x0: r.left, y0: r.top, x1: r.right, y1: r.bottom };
                const visible = inter(entier, clip);
                if (vide(visible)) {
                    releve.ecartes.decoupes += 1;
                    continue;
                }
                // Couvert par un autre élément (modale, menu, voile) : le pixel montre l'autre.
                const touche = document.elementFromPoint((visible.x0 + visible.x1) / 2, (visible.y0 + visible.y1) / 2);
                if (!touche || !(el === touche || el.contains(touche) || touche.contains(el))) {
                    releve.ecartes.couverts += 1;
                    continue;
                }
                const rogne = visible.x0 !== entier.x0 || visible.y0 !== entier.y0 || visible.x1 !== entier.x1 || visible.y1 !== entier.y1;
                if (rogne) releve.rognes.push(releve.glyphes.length);
                releve.glyphes.push([
                    casse(c, s.textTransform, debutDeMot),
                    visible.x0, visible.y0, visible.x1 - visible.x0, visible.y1 - visible.y0,
                ]);
            }
        }
        plage.detach();
        return releve;
    });
}

/** Un nombre au centième, sans `-0` (le fichier doit être identique d'une passe à l'autre). */
function centieme(v: number): number {
    const r = Math.round(v * 100) / 100;
    return Object.is(r, -0) ? 0 : r;
}

/** Le texte du fichier `.glyphes.json` : un glyphe par ligne. */
export function serialiserGlyphes(releve: ReleveDom, largeur: number, hauteur: number): string {
    const echelle = largeur / releve.largeurCss;
    const lignes = releve.glyphes.map(([c, x, y, l, h]) =>
        '    ' + JSON.stringify([c, centieme(x * echelle), centieme(y * echelle), centieme(l * echelle), centieme(h * echelle)]));
    return [
        '{',
        `  "version": ${VERSION_GLYPHES}, "source": "dom", "largeur": ${largeur}, "hauteur": ${hauteur},`,
        `  "ecartes": ${JSON.stringify(releve.ecartes)},`,
        `  "rognes": ${JSON.stringify(releve.rognes)},`,
        '  "glyphes": [',
        lignes.join(',\n'),
        '  ]',
        '}',
        '',
    ].join('\n');
}

/**
 * Relève les glyphes de `page` puis prend la capture : `capture()` rend le PNG. Le relevé est écrit
 * à côté du PNG (`<png sans .png>.glyphes.json`) quand FRAGMENT_GLYPHES=1 ; sinon, seule la capture
 * est faite. Le PNG n'est PAS écrit ici : l'appelant garde la main.
 */
export async function avecGlyphes(page: Page, fichierPng: string, capture: () => Promise<Buffer>): Promise<Buffer> {
    if (!releveActif()) return capture();
    const releve = await releverDom(page);
    const png = await capture();
    const [largeur, hauteur] = taillePng(png);
    writeFileSync(fichierPng.replace(/\.png$/, '.glyphes.json'), serialiserGlyphes(releve, largeur, hauteur));
    return png;
}
