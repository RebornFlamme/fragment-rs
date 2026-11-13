// Sonde temporaire (non commitée) : réactiver la feuille (null puis PDF) pour empiler la portée de la vue.
import { sceneT4 } from './_t4';

sceneT4('zz-sonde-recherche', async ({ p }) => {
    const etat = (label: string) => p.evaluate((l) => {
        const a = document.activeElement as HTMLElement | null;
        const bar = document.querySelector('.pdf-find-bar') as HTMLElement | null;
        const inp = document.querySelector('.pdf-find-input') as HTMLInputElement | null;
        return JSON.stringify({ l, actif: a ? a.tagName + '.' + a.className : null, bar: bar?.className, val: inp?.value, count: document.querySelector('.pdf-find-count')?.textContent, hl: document.querySelectorAll('.pdf-find-highlight').length });
    }, label);
    const log = async (l: string) => console.log('SONDE ' + await etat(l));
    await p.evaluate(() => { const w = (window as any).app.workspace; const f = w.getLeavesOfType('pdf')[0]; w.setActiveLeaf(null); w.setActiveLeaf(f); });
    await p.keyboard.press('Control+f');
    await log('ctrl+f immediat');
    await p.locator('.pdf-find-bar.is-open').waitFor({ timeout: 5000 }).catch(() => {});
    await p.waitForTimeout(300);
    await log('ctrl+f +300');
    await p.keyboard.type('balise');
    await p.waitForTimeout(1500);
    await log('apres saisie');
});
