// Spike S5 — capture de référence des formules telles que Fragment les rend (MathJax SVG dans
// l'éditeur Markdown), par thème. Pour chaque formule : un PNG rogné autour du <svg> et une
// ligne de mesures (rectangle exact, couleurs, corps, ex, conteneur MathJax).
import { test } from '../../../../Fragment/app/node_modules/@playwright/test';
import { mkdirSync, writeFileSync } from 'fs';
import path from 'path';
import { lancerLecteur } from '../../../../Fragment/app/e2e/readerHarness';
import { FORMULES } from './formules';

const SORTIE = path.resolve(__dirname, '..', 'reference');

function note(): string {
    const lignes = ['Formules du spike S5', ''];
    for (const f of FORMULES) {
        if (f.bloc) lignes.push('Bloc suivant.', '', '$$', f.tex, '$$', '');
        else lignes.push(`Texte $${f.tex}$ fin.`, '');
    }
    lignes.push('Fin du document.');
    return lignes.join('\n');
}

test('capture des formules', async () => {
    test.setTimeout(240_000);
    mkdirSync(SORTIE, { recursive: true });
    const h = await lancerLecteur({ 'maths.md': note() }, 'maths.md', { args: ['--force-device-scale-factor=1', '--high-dpi-support=1'] });
    try {
        await h.electronApp.evaluate(({ BrowserWindow }) => {
            for (const w of BrowserWindow.getAllWindows()) w.setContentSize(1280, 1600);
        });
        const p = h.page;
        await p.addStyleTag({ content: '*{caret-color:transparent!important;animation:none!important;transition:none!important}' });
        const mesures: unknown[] = [];
        const tous = '.cm-math-inline svg, .cm-math-block svg';
        for (const theme of ['dark', 'light'] as const) {
            await p.evaluate((v) => (window as any).app.appearance.set('baseColorScheme', v), theme);
            await p.waitForFunction((v) => document.documentElement.dataset.theme === v, theme);
            await p.mouse.move(2, 2);
            await p.evaluate(() => document.fonts.ready);
            await p.waitForTimeout(400);
            const n = await p.locator(tous).count();
            if (n !== FORMULES.length) throw new Error(`${n} formules rendues, ${FORMULES.length} attendues`);
            for (let i = 0; i < n; i++) {
                const svg = p.locator(tous).nth(i);
                await svg.scrollIntoViewIfNeeded();
                await p.mouse.move(2, 2);
                await p.waitForTimeout(150);
                const m = await svg.evaluate((el) => {
                    const r = el.getBoundingClientRect();
                    const conteneur = el.closest('mjx-container') as HTMLElement;
                    const sonde = document.createElement('span');
                    sonde.style.cssText = 'display:inline-block;width:100ex;height:100em;position:absolute;visibility:hidden';
                    conteneur.appendChild(sonde);
                    const s = sonde.getBoundingClientRect();
                    sonde.remove();
                    const cs = getComputedStyle(conteneur);
                    let fond = 'rgba(0, 0, 0, 0)';
                    for (let e: HTMLElement | null = conteneur; e; e = e.parentElement) {
                        const b = getComputedStyle(e).backgroundColor;
                        if (b !== 'rgba(0, 0, 0, 0)' && b !== 'transparent') { fond = b; break; }
                    }
                    const parent = getComputedStyle(conteneur.parentElement as HTMLElement);
                    return {
                        x: r.x, y: r.y, largeur: r.width, hauteur: r.height,
                        ex_px: s.width / 100, em_px: s.height / 100,
                        corps_conteneur: cs.fontSize, corps_parent: parent.fontSize, police_parent: parent.fontFamily,
                        couleur: cs.color, fond,
                        style_conteneur: conteneur.getAttribute('style'),
                        svg: el.outerHTML,
                    };
                });
                const clip = { x: Math.floor(m.x) - 2, y: Math.floor(m.y) - 2, width: 0, height: 0 };
                clip.width = Math.ceil(m.x + m.largeur) + 2 - clip.x;
                clip.height = Math.ceil(m.y + m.hauteur) + 2 - clip.y;
                const png = await p.screenshot({ clip, animations: 'disabled', caret: 'hide' });
                const nom = `${FORMULES[i].nom}-${theme}`;
                writeFileSync(path.join(SORTIE, `${nom}.png`), png);
                mesures.push({ nom, formule: FORMULES[i].nom, theme, bloc: FORMULES[i].bloc, tex: FORMULES[i].tex, clip, ...m });
            }
        }
        writeFileSync(path.join(SORTIE, 'mesures.json'), JSON.stringify(mesures, null, 1));
    } finally {
        await h.electronApp.close();
    }
});
