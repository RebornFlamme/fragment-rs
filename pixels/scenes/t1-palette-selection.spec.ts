// Scène T1 « t1-palette-selection » : la palette de commandes, requête « panneau », puis ↓, sur le décor des surfaces flottantes (fond de scène, rien d'autre).
import { writeFileSync, mkdirSync } from 'fs';
import path from 'path';
import { sceneFlottante } from './_t1-flottantes';
import { expect } from './_t1';

sceneFlottante('t1-palette-selection', async ({ p, act, capturer }) => {
    await act.command('command-palette:open');
    await expect(p.locator('.prompt').first()).toBeVisible();
    await act.type('panneau');
    await act.key('ArrowDown');
    await act.hover(2, 2);
    await act.waitStable();
    // Relevé des lignes rendues (aide à écrire le décor Rust : mêmes commandes, mêmes raccourcis).
    const lignes = await p.locator('.prompt .suggestion-item').allInnerTexts();
    const tous = await p.evaluate(() => (window as any).app.commands.listCommands().map((c: any) => c.name));
    mkdirSync(path.resolve(__dirname, '..', '.out'), { recursive: true });
    writeFileSync(path.resolve(__dirname, '..', '.out', 't1-palette-selection.lignes.json'), JSON.stringify({ lignes, tous }, null, 1));
    await capturer();
    await act.key('Escape');
});
