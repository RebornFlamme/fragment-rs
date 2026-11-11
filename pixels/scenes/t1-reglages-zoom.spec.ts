// Scène T1 « t1-reglages-zoom » : le curseur de « Zoom level » saisi et amené à 150 %, bouton MAINTENU.
// Le zoom s'applique au relâché : la capture montre l'étiquette « 150 % » et une fenêtre encore à 100 %.
import { sceneT1, expect } from './_t1';
import { ouvrirReglages, rendreLeZoom, zoomFenetre } from './_t1-reglages';

sceneT1('t1-reglages-zoom', async (c) => {
    const r = await ouvrirReglages(c, 'Appearance');
    try {
        const piste = r.locator('[data-setting-id="zoom"] input.slider');
        const b = (await piste.boundingBox())!;
        // Le pouce parcourt la piste moins son diamètre (16 px) : 100 % au tiers, 150 % aux deux tiers.
        const xDe = (valeur: number) => b.x + 8 + ((valeur - 50) / 150) * (b.width - 16);
        const y = b.y + b.height / 2;
        await c.sonde('curseur du zoom', r, '[data-setting-id="zoom"] input.slider');
        await r.mouse.move(xDe(100), y);
        await r.mouse.down();
        await r.mouse.move(xDe(150), y, { steps: 8 });
        await expect(r.locator('[data-setting-id="zoom"] .slider-value')).toHaveText('150 %');
        expect(await zoomFenetre(c.p)).toBeCloseTo(1, 2);
        await c.capturer(r);
        await r.mouse.up();
    } finally {
        await rendreLeZoom(c);
        await r.close();
    }
});
