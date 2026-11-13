// Banc du relevé DOM des glyphes (fr-p3.4, pixels/scenes/_glyphes.ts) : les quatre trous que la
// comparaison réelle a montrés, sur une page de test. Chaque cas vérifie le caractère relevé ET
// sa place contre l'encre réellement peinte dans la capture.
import { test as base, expect, _electron, type Page } from '../../../../Fragment/app/node_modules/@playwright/test';
import { mkdtempSync } from 'fs';
import { tmpdir } from 'os';
import path from 'path';
import { releverDom } from '../../scenes/_glyphes';

// Le moteur des références : l'Electron de Fragment, sur une fenêtre vide (main.cjs).
const test = base.extend<{ page: Page }>({
    page: async ({}, use) => {
        const donnees = mkdtempSync(path.join(tmpdir(), 'glyphes-banc-'));
        const appli = await _electron.launch({ args: [path.join(__dirname, 'main.cjs'), '--force-device-scale-factor=1', `--user-data-dir=${donnees}`] });
        const page = await appli.firstWindow();
        await page.waitForLoadState();
        await use(page);
        await appli.close();
    },
});

type Glyphe = [string, number, number, number, number];

const PAGE = `<!doctype html><html><head><style>
  body { margin: 0; background: #fff; font: 16px/20px Arial, sans-serif; color: #000; }
  .abs { position: absolute; }
  input, select { font: 16px Arial, sans-serif; color: #000; background: #fff; border: 1px solid #fff;
                  padding: 2px 6px; outline: none; appearance: none; }
  input::placeholder { color: #000; opacity: 1; }
</style></head><body></body></html>`;

async function charger(page: Page, corps: string) {
    await page.setContent(PAGE);
    await page.evaluate((html) => { document.body.innerHTML = html; }, corps);
}

/** Les colonnes et lignes d'encre (pixel non blanc) de la capture, dans un rectangle. */
async function encre(page: Page, x0: number, y0: number, x1: number, y1: number) {
    const png = await page.screenshot({ clip: { x: x0, y: y0, width: x1 - x0, height: y1 - y0 } });
    return page.evaluate(async ({ b64, x0, y0 }) => {
        const img = new Image();
        img.src = 'data:image/png;base64,' + b64;
        await img.decode();
        const c = document.createElement('canvas');
        c.width = img.width; c.height = img.height;
        const g = c.getContext('2d')!;
        g.drawImage(img, 0, 0);
        const d = g.getImageData(0, 0, img.width, img.height).data;
        const points: [number, number][] = [];
        for (let y = 0; y < img.height; y++) for (let x = 0; x < img.width; x++) {
            const i = (y * img.width + x) * 4;
            if (d[i] < 128) points.push([x0 + x + 0.5, y0 + y + 0.5]);
        }
        return points;
    }, { b64: png.toString('base64'), x0, y0 });
}

/** Chaque point d'encre sombre tombe dans un rectangle relevé (élargi d'un pixel). */
function encreCouverte(points: [number, number][], glyphes: Glyphe[]) {
    return points.filter(([x, y]) => !glyphes.some(([, gx, gy, l, h]) =>
        x >= gx - 1 && x <= gx + l + 1 && y >= gy - 1 && y <= gy + h + 1));
}

const texte = (g: Glyphe[]) => g.map((x) => x[0]).join('');

test('la valeur et le texte indicatif d un champ sont releves', async ({ page }) => {
    await charger(page, `
      <input class="abs" style="left:10px;top:10px;width:150px" value="panneau">
      <input class="abs" style="left:10px;top:60px;width:150px" placeholder="Search settings...">`);
    const r = await releverDom(page);
    expect(texte(r.glyphes)).toBe('panneauSearchsettings...');
    expect(encreCouverte(await encre(page, 0, 0, 200, 100), r.glyphes)).toEqual([]);
    // Ordre de lecture : x croissant dans chaque champ.
    const xs = r.glyphes.slice(0, 7).map((g) => g[1]);
    expect([...xs].sort((a, b) => a - b)).toEqual(xs);
});

test('un champ defile garde ses lettres visibles a leur place', async ({ page }) => {
    await charger(page, `<input class="abs" style="left:10px;top:10px;width:40px" value="abcdefghijkl">`);
    await page.evaluate(() => { const i = document.querySelector('input')!; i.scrollLeft = 30; });
    const r = await releverDom(page);
    expect(encreCouverte(await encre(page, 0, 0, 80, 40), r.glyphes)).toEqual([]);
    expect(r.glyphes.length).toBeGreaterThan(0);
    expect(texte(r.glyphes).startsWith('a')).toBe(false);
});

test('un select montre le texte de son option choisie', async ({ page }) => {
    await charger(page, `<select class="abs" style="left:10px;top:10px;width:120px">
        <option>Light</option><option selected>Dark</option></select>`);
    const r = await releverDom(page);
    expect(texte(r.glyphes)).toBe('Dark');
    expect(encreCouverte(await encre(page, 0, 0, 140, 50), r.glyphes)).toEqual([]);
});

test('une lettre a moitie sous un aplat opaque est gardee, entierement dessous elle est couverte', async ({ page }) => {
    await charger(page, `
      <div class="abs" style="left:10px;top:10px">New Tab</div>
      <div class="abs" style="left:0;top:20px;width:400px;height:40px;background:#fff"></div>
      <div class="abs" style="left:10px;top:100px">Cache</div>
      <div class="abs" style="left:0;top:90px;width:400px;height:40px;background:#fff"></div>`);
    const r = await releverDom(page);
    expect(texte(r.glyphes)).toBe('NewTab');
    expect(r.ecartes.couverts).toBe(5);
});

test('un fantome pointer-events none et un voile transparent ne couvrent rien', async ({ page }) => {
    await charger(page, `
      <div class="abs" style="left:0;top:0;width:400px;height:100px;background:#fff"></div>
      <div class="abs" style="left:10px;top:10px;pointer-events:none">Ghost</div>
      <div class="abs" style="left:10px;top:50px">Voile</div>
      <div class="abs" style="left:0;top:40px;width:400px;height:40px;background:rgba(0,0,0,0)"></div>`);
    const r = await releverDom(page);
    expect(texte(r.glyphes)).toBe('GhostVoile');
    expect(r.ecartes.couverts).toBe(0);
});

test('un texte tronque releve ses lettres visibles puis les points de suspension', async ({ page }) => {
    await charger(page, `<div class="abs" style="left:10px;top:10px;width:70px;overflow:hidden;
        white-space:nowrap;text-overflow:ellipsis"><span>Tableau de bord</span></div>`);
    const r = await releverDom(page);
    const t = texte(r.glyphes);
    expect(t.endsWith('…')).toBe(true);
    expect(t.length).toBeLessThan('Tableaudebord…'.length);
    expect('Tableaudebord'.startsWith(t.slice(0, -1))).toBe(true);
    expect(encreCouverte(await encre(page, 0, 0, 100, 40), r.glyphes)).toEqual([]);
    // Le « … » est dans la boîte, après la dernière lettre gardée.
    const [, x, , l] = r.glyphes[r.glyphes.length - 1];
    expect(x + l).toBeLessThanOrEqual(80.5);
    expect(x).toBeGreaterThanOrEqual(r.glyphes[r.glyphes.length - 2][1]);
});

test('un texte tronque trop etroit garde sa premiere lettre rognee sans points de suspension', async ({ page }) => {
    // La première lettre d'une ligne tronquée est rognée, jamais remplacée par « … »
    // (onglet trop étroit de t1-boutons-fenetre).
    await charger(page, `<div class="abs" style="left:10px;top:10px;width:8px;overflow:hidden;
        white-space:nowrap;text-overflow:ellipsis"><span>New Tab</span></div>`);
    const r = await releverDom(page);
    expect(texte(r.glyphes)).toBe('N');
    expect(r.rognes).toEqual([0]);
    expect(encreCouverte(await encre(page, 0, 0, 60, 40), r.glyphes)).toEqual([]);
});
