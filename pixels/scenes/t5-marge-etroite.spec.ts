// Scène T5 « t5-marge-etroite » : fenêtre de 640 px, trois widgets posés : la marge est trop étroite, tout est masqué.
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-marge-etroite', async ({ act, capturer, p, pos }) => {
    await act.poser({ widgets: [{ kind: 'flex', from: pos(3), height: 110 }, { kind: 'flex', from: pos(4), height: 110 }, { kind: 'flex', from: pos(5), height: 110 }] });
    await expect(p.locator('.e2e-room:visible')).toHaveCount(0);
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
}, { fenetre: { largeur: 640, hauteur: 800 } });
