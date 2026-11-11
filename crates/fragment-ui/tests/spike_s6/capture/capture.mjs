// Capture de référence du spike S6 : les fixtures rendues par Chromium (playwright-core du dépôt Fragment,
// utilisé en lecture seule, rien n'y est écrit). Sorties : reference/mesures.json et reference/*.png.
// Usage : node capture.mjs
import { chromium } from 'file:///C:/Users/julie/OneDrive/Documents/Projects/Fragment/app/node_modules/playwright-core/index.mjs';
import { readFileSync, writeFileSync, mkdirSync, readdirSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const ici = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.join(ici, '..', 'fixtures');
const sortie = path.join(ici, '..', 'reference');
mkdirSync(sortie, { recursive: true });

const page_html = (corps) => `<!doctype html><html><head><style>
:root { --color-accent: #7c83ff; color: #dcddde; }
body { margin: 0; background: #1e1e1e; color: #dcddde; font: 16px/1.5 sans-serif; }
#c { width: 640px; }
p { margin: 0; }
</style></head><body><div id="c">${corps}</div></body></html>`;

/** f05/f06 sont du texte avec HTML : chaque paragraphe devient un <p>. */
function corps(nom, src) {
    if (nom.startsWith('f05')) return `<p>${src.trim()}</p>`;
    if (nom.startsWith('f06')) return src.trim().split(/\n\n+/).map((s) => (s.startsWith('<!--') ? s : `<p>${s}</p>`)).join('');
    return src;
}

const navigateur = await chromium.launch({ headless: true, executablePath: process.env.CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe' });
const mesures = {};
for (const f of readdirSync(fixtures).filter((n) => n.endsWith('.md')).sort()) {
    const nom = f.replace(/\.md$/, '');
    const src = readFileSync(path.join(fixtures, f), 'utf8');
    const ctx = await navigateur.newContext({ viewport: { width: 800, height: 600 }, deviceScaleFactor: 1 });
    const page = await ctx.newPage();
    await page.setContent(page_html(corps(nom, src)));
    const r = await page.locator('#c').evaluate((e) => {
        const b = e.getBoundingClientRect();
        const arr = (v) => Math.round(v * 100) / 100;
        return { rect: { x: arr(b.x), y: arr(b.y), width: arr(b.width), height: arr(b.height) }, texte: e.innerText };
    });
    const m = { ...r };
    if (nom.startsWith('f02')) {
        await page.locator('#s').screenshot({ path: path.join(sortie, 'f02-svg-statique.png') });
        m.svg = await page.locator('#s').evaluate((e) => { const b = e.getBoundingClientRect(); return { width: b.width, height: b.height }; });
    }
    if (nom.startsWith('f03')) {
        m.temps = {};
        await page.locator('#s').evaluate((e) => e.pauseAnimations());
        for (const t of [0.5, 1.0, 1.5]) {
            await page.locator('#s').evaluate((e, tt) => e.setCurrentTime(tt), t);
            const cle = String(Math.round(t * 1000)).padStart(4, '0');
            await page.locator('#s').screenshot({ path: path.join(sortie, `f03-t${cle}.png`) });
            m.temps[t] = await page.evaluate(() => {
                const g = document.getElementById('groupe');
                return {
                    cx: document.getElementById('rond').cx.animVal.value,
                    tx: g.transform.animVal.numberOfItems ? g.transform.animVal.getItem(0).matrix.e : null,
                };
            });
        }
    }
    if (nom.startsWith('f04')) {
        const avant = (await page.locator('#c').boundingBox()).height;
        await page.locator('summary').click();
        const apres = (await page.locator('#c').boundingBox()).height;
        m.hauteur_ferme = avant;
        m.hauteur_ouvert = apres;
    }
    if (nom.startsWith('f09')) {
        // Référence NON assainie : le piège s'exécute. Le test Rust vérifie que l'assainissement le coupe.
        await page.locator('#p').click();
        await page.locator('#a').click().catch(() => {});
        m.globales = await page.evaluate(() => [window.x, window.y, window.z, window.w]);
    }
    mesures[nom] = m;
    await ctx.close();
}
await navigateur.close();
writeFileSync(path.join(sortie, 'mesures.json'), JSON.stringify(mesures, null, 2));
console.log(JSON.stringify(mesures, null, 2));
