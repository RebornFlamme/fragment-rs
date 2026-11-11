// Scène T1 « t1-split-proportions » : trois colonnes en 25 / 50 / 25 (flex-grow du layout), celle
// du milieu coupée en deux piles empilées 25 / 75. Les arêtes fractionnaires sont arrondies par
// Chromium à la peinture : c'est ce que le candidat doit reproduire.
import { sceneT1 } from './_t1';

sceneT1('t1-split-proportions', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    for (let i = 0; i < 4; i++) await sonde(`pile ${i + 1}`, p, '.workspace-tabs', i);
    for (let i = 0; i < 3; i++) await sonde(`poignée ${i + 1}`, p, '.workspace-resize-handle', i);
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
}, {
    main: {
        type: 'split', direction: 'vertical', children: [
            { type: 'tabs', tabs: 1, dimension: 25 },
            { type: 'split', direction: 'horizontal', dimension: 50, children: [
                { type: 'tabs', tabs: 2, current: 1, dimension: 25 },
                { type: 'tabs', tabs: 1, dimension: 75 },
            ] },
            { type: 'tabs', tabs: 1, dimension: 25 },
        ],
    },
});
