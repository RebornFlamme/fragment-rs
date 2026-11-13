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
export async function releverDom(page: Page): Promise<ReleveDom> {
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

        // ── Couverture ───────────────────────────────────────────────────────────────────────
        // ★ Même règle que le candidat (capture.rs) : une lettre n'est écartée que si un aplat
        // OPAQUE peint au-dessus la couvre EN ENTIER. Tester le seul centre écartait le « New Tab »
        // à moitié sous un menu, dont le centre tombait pile sur le bord du menu.
        // elementsFromPoint rend la pile de peinture au point, du dessus vers le dessous ; un
        // élément `pointer-events: none` n'y figure pas (fantôme de glisser, voile) : sa lettre,
        // posée par-dessus le reste, n'est jamais tenue pour couverte.
        const opacites = new Map<Element, number>();
        const opacite = (e: Element | null): number => {
            if (!e) return 1;
            const connue = opacites.get(e);
            if (connue !== undefined) return connue;
            const o = Number(getComputedStyle(e).opacity) * opacite(e.parentElement);
            opacites.set(e, o);
            return o;
        };
        const opaque = (e: Element) => {
            const m = /^rgba?\(([^)]*)\)$/.exec(getComputedStyle(e).backgroundColor);
            if (!m) return false;
            const parts = m[1].split(',');
            const alpha = parts.length === 4 ? Number(parts[3]) : 1;
            return alpha >= 1 && opacite(e) >= 1;
        };
        const couvertEn = (el: Element, x: number, y: number) => {
            for (const e of document.elementsFromPoint(x, y)) {
                if (e === el || el.contains(e) || e.contains(el)) return false;
                if (opaque(e)) return true;
            }
            return false;
        };
        const couvert = (el: Element, r: Rect) => {
            if (getComputedStyle(el).pointerEvents === 'none') return false;
            const m = 0.5;
            const points: [number, number][] = [
                [(r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2],
                [r.x0 + m, r.y0 + m], [r.x1 - m, r.y0 + m],
                [r.x0 + m, r.y1 - m], [r.x1 - m, r.y1 - m],
            ];
            return points.every(([x, y]) => couvertEn(el, x, y));
        };

        // ── Points de suspension (text-overflow: ellipsis) ───────────────────────────────────
        // ★ Le DOM garde le texte entier ; Chromium peint « … » à la place des lettres qui ne
        // tiennent pas. Le conteneur tronqué est le bloc qui porte text-overflow et un overflow
        // non visible ; ses lettres dont le bord droit passe la limite (bord du contenu moins la
        // largeur de « … ») ne sont pas peintes, et « … » se pose juste après la dernière qui tient.
        type Tronque = { limite: number; largeur: number; dernier: Rect | null; gauche: number; el: Element };
        const tronques = new Map<Element, Tronque | null>();
        const tronqueDe = (el: Element): Tronque | null => {
            for (let e: Element | null = el; e; e = e.parentElement) {
                if (tronques.has(e)) return tronques.get(e) ?? null;
                const s = getComputedStyle(e);
                if (s.display === 'inline' || s.display === 'contents') continue;
                let t: Tronque | null = null;
                const html = e as HTMLElement;
                if (s.textOverflow === 'ellipsis' && s.overflowX !== 'visible' && html.scrollWidth > html.clientWidth) {
                    const mesure = document.createElement('span');
                    mesure.textContent = '…';
                    mesure.style.cssText = 'position:fixed;visibility:hidden;white-space:pre;left:0;top:0';
                    e.appendChild(mesure);
                    const largeur = mesure.getBoundingClientRect().width;
                    mesure.remove();
                    const b = e.getBoundingClientRect();
                    const gauche = b.left + html.clientLeft + parseFloat(s.paddingLeft);
                    const droite = b.left + html.clientLeft + html.clientWidth - parseFloat(s.paddingRight);
                    t = { limite: droite - largeur, largeur, dernier: null, gauche, el: e };
                }
                tronques.set(e, t);
                return t;
            }
            return null;
        };

        // ── Une lettre mesurée ───────────────────────────────────────────────────────────────
        const poser = (c: string, entier: Rect, clip: Rect, el: Element) => {
            const visible = inter(entier, clip);
            if (vide(visible)) {
                releve.ecartes.decoupes += 1;
                return;
            }
            if (couvert(el, visible)) {
                releve.ecartes.couverts += 1;
                return;
            }
            const rogne = visible.x0 !== entier.x0 || visible.y0 !== entier.y0 || visible.x1 !== entier.x1 || visible.y1 !== entier.y1;
            if (rogne) releve.rognes.push(releve.glyphes.length);
            releve.glyphes.push([c, visible.x0, visible.y0, visible.x1 - visible.x0, visible.y1 - visible.y0]);
        };
        const plage = document.createRange();
        const lettres = (texte: Text, el: Element, s: CSSStyleDeclaration, clip: Rect, tronque: Tronque | null) => {
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
                if (tronque) {
                    if (entier.x1 > tronque.limite + 0.01) {
                        releve.ecartes.decoupes += 1;
                        continue;
                    }
                    if (!tronque.dernier || entier.x1 >= tronque.dernier.x1) tronque.dernier = entier;
                }
                poser(casse(c, s.textTransform, debutDeMot), entier, clip, el);
            }
        };

        // ── Le texte des nœuds ───────────────────────────────────────────────────────────────
        const marcheur = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        for (let n = marcheur.nextNode(); n; n = marcheur.nextNode()) {
            const texte = n as Text;
            const el = texte.parentElement;
            if (!el || !texte.data.trim()) continue;
            // Le texte d'un <textarea> ou des <option> d'un <select> n'est pas peint tel quel :
            // il passe par les champs, plus bas.
            if (el.closest('textarea, select')) continue;
            if (!el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) {
                releve.ecartes.invisibles += 1;
                continue;
            }
            const s = getComputedStyle(el);
            if (transparent(s.color) || transparent(s.getPropertyValue('-webkit-text-fill-color'))) {
                releve.ecartes.invisibles += 1;
                continue;
            }
            lettres(texte, el, s, decoupe(el), tronqueDe(el));
        }
        for (const t of new Set(tronques.values())) {
            if (!t) continue;
            const ref = t.dernier;
            const x0 = ref ? ref.x1 : t.gauche;
            const b = t.el.getBoundingClientRect();
            const y0 = ref ? ref.y0 : b.top;
            const y1 = ref ? ref.y1 : b.bottom;
            poser('…', { x0, y0, x1: x0 + t.largeur, y1 }, decoupe(t.el), t.el);
        }

        // ── Les champs : <input>, <textarea>, <select> ───────────────────────────────────────
        // ★ Leur texte (valeur, texte indicatif, option choisie) n'est pas un nœud du DOM. On le
        // recopie dans un MIROIR invisible posé sur la boîte du champ, même police, même
        // remplissage, même défilement, et on le mesure comme le reste. La couverture se juge sur
        // le champ lui-même ; le miroir (`visibility: hidden`, rien de peint) est retiré aussitôt.
        const sansTexte = new Set(['button', 'checkbox', 'color', 'file', 'hidden', 'image', 'radio', 'range', 'reset', 'submit']);
        const champs = Array.from(document.querySelectorAll('input, textarea, select')) as HTMLElement[];
        for (const champ of champs) {
            if (champ instanceof HTMLInputElement && sansTexte.has(champ.type)) continue;
            if (!champ.checkVisibility({ opacityProperty: true, visibilityProperty: true })) continue;
            let contenu = '';
            let style = getComputedStyle(champ);
            if (champ instanceof HTMLSelectElement) {
                contenu = champ.selectedIndex >= 0 ? champ.options[champ.selectedIndex].text : '';
            } else {
                const saisie = champ as HTMLInputElement | HTMLTextAreaElement;
                contenu = saisie.value || saisie.placeholder;
                if (!saisie.value && saisie.placeholder) style = getComputedStyle(champ, '::placeholder');
                if (champ instanceof HTMLInputElement && champ.type === 'password') contenu = '';
            }
            if (!contenu.trim()) continue;
            if (transparent(style.color) || transparent(style.getPropertyValue('-webkit-text-fill-color'))) {
                releve.ecartes.invisibles += 1;
                continue;
            }
            const sc = getComputedStyle(champ);
            const b = champ.getBoundingClientRect();
            const zone = champ instanceof HTMLTextAreaElement;
            const miroir = document.createElement('div');
            const interieur = document.createElement('div');
            miroir.style.cssText = [
                'position:fixed', 'visibility:hidden', 'pointer-events:none', 'overflow:hidden', 'margin:0',
                `left:${b.left}px`, `top:${b.top}px`, `width:${b.width}px`, `height:${b.height}px`,
                'box-sizing:border-box', 'border-style:solid', 'border-color:transparent',
                `border-width:${sc.borderTopWidth} ${sc.borderRightWidth} ${sc.borderBottomWidth} ${sc.borderLeftWidth}`,
                `padding:${sc.paddingTop} ${sc.paddingRight} ${sc.paddingBottom} ${sc.paddingLeft}`,
                // Un champ d'une ligne centre son texte verticalement dans sa boîte de contenu.
                ...(zone ? [] : ['display:flex', 'flex-direction:column', 'justify-content:center']),
            ].join(';');
            interieur.style.cssText = [
                `font:${sc.font}`, `letter-spacing:${sc.letterSpacing}`, `word-spacing:${sc.wordSpacing}`,
                `text-transform:${style.textTransform}`, `text-indent:${sc.textIndent}`, `text-align:${sc.textAlign}`,
                `line-height:${sc.lineHeight}`, `white-space:${zone ? 'pre-wrap' : 'pre'}`,
                `font-feature-settings:${sc.fontFeatureSettings}`, `font-kerning:${sc.fontKerning}`,
                `direction:${sc.direction}`, `transform:translate(${-champ.scrollLeft}px, ${-champ.scrollTop}px)`,
            ].join(';');
            interieur.textContent = contenu;
            miroir.appendChild(interieur);
            document.body.appendChild(miroir);
            // Découpe : la boîte de remplissage du champ, dans la découpe de ses conteneurs.
            const x0 = b.left + champ.clientLeft;
            const y0 = b.top + champ.clientTop;
            const clip = inter(decoupe(champ), { x0, y0, x1: x0 + champ.clientWidth, y1: y0 + champ.clientHeight });
            lettres(interieur.firstChild as Text, champ, style, clip, null);
            miroir.remove();
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
