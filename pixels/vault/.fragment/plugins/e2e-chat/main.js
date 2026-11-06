// Fixture des scènes pixels, écrite pour ce dépôt : un fournisseur de chat sans réseau.
// Les scènes n'ont besoin que de l'état final : une question, sa réponse, les boutons.
const { Plugin } = require('obsidian');

const AUTEURS = [
    { id: 'julien', name: 'Julien', role: 'user' },
    { id: 'claude', name: 'Claude', role: 'assistant' },
];

const texteDe = (fil) => fil?.[0]?.parts[0].text ?? '';

function creerFournisseur() {
    const fils = new Map();
    const abonnes = new Set();
    let compteur = 0;

    return {
        id: 'e2e',
        label: 'Fournisseur de test',
        icon: 'bot',
        participants: () => AUTEURS,
        title: (id) => texteDe(fils.get(id)).slice(0, 40),
        load: async (id) => structuredClone(fils.get(id) ?? []),
        origin: () => null,
        create: () => 'conv-' + (compteur += 1),
        list: () => Array.from(fils, ([id, fil]) => ({ id, title: texteDe(fil), updatedAt: 0 })).reverse(),
        onChange(rappel) {
            abonnes.add(rappel);
            return () => abonnes.delete(rappel);
        },
        async send(id, question, emettre) {
            const reponse = 'Réponse à « ' + question + ' ». Voici **du gras** et du `code`.';
            const fin = Promise.resolve().then(() => {
                emettre({ type: 'text', text: reponse });
                const fil = fils.get(id) ?? [];
                const ajout = (prefixe, author, text) =>
                    fil.push({ id: prefixe + fil.length, author, parts: [{ type: 'text', text }] });
                ajout('u', 'julien', question);
                ajout('a', 'claude', reponse);
                fils.set(id, fil);
                abonnes.forEach((rappel) => rappel());
                emettre({ type: 'done' });
            });
            return { stop() {}, done: fin };
        },
    };
}

// Un bouton inerte : seule son apparence compte pour la capture.
const bouton = (classe, libelle) => (parent) => {
    const el = document.createElement('button');
    el.className = classe;
    el.textContent = libelle;
    parent.append(el);
};

module.exports = class extends Plugin {
    onload() {
        const registres = this.app.registry;
        const ordonne = (id) => registres.get({ id, kind: 'ordered', label: id });
        const chez = { owner: 'e2e-chat' };
        const id = 'chat:provider@1';
        this.register(registres.get({ id, kind: 'keyed', label: id }).register('e2e', creerFournisseur(), 'e2e-chat'));
        this.register(ordonne('chat:message-actions@1').register({ id: 'e2e-epingler', render: bouton('e2e-epingler', 'Épingler') }, chez));
        this.register(ordonne('chat:composer-tools@1').register({ id: 'e2e-outil-composer', render: bouton('e2e-quick', 'Quick') }, chez));
    }
};
