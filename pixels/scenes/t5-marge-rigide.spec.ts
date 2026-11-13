// Scène T5 « t5-marge-rigide » : un widget RIGIDE (le globe, d'un autre calque) en haut de la marge réduit la place du
// widget « seuil » suivant, qui se compacte ; le dernier (flex) vient dessous.
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-marge-rigide', async ({ act, capturer, p, pos }) => {
    await act.poser({
        rigide: { from: 0, height: 330 },
        widgets: [{ kind: 'seuil', from: pos(3), height: 200 }, { kind: 'flex', from: pos(5), height: 20 }],
    });
    // La place réduite par le rigide a compacté le « seuil ».
    await expect(p.locator('.e2e-room.is-compact')).toHaveCount(1);
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
