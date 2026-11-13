// Scène T11 « t11-reglages-themes » : la fenêtre de réglages (1000 × 700), onglet Appearance, une fois
// « Rose » choisi (la ligne « Themes » le montre). ★ La liste est un <select> natif : son popup est une
// fenêtre du système, hors de toute capture de page ; la scène montre donc la liste FERMÉE sur « Rose ».
import { sceneT11, ouvrirReglages } from './_t11';

sceneT11('t11-reglages-themes', async (c) => {
    await c.choisirTheme('Rose');
    const r = await ouvrirReglages(c, 'Appearance');
    await c.sonde('liste des thèmes', r, '.setting-item:has-text("Themes") select');
    await c.capturer(r);
    await r.close();
});
