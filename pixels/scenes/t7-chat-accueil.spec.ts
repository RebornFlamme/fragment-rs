// Scène T7 « t7-chat-accueil » : conversation vide : la salutation selon l'heure et le champ, centrés.
import { sceneT7a, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-accueil', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-vide');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
