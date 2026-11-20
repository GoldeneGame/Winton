# ⚔ La Taverne — table de JDR en ligne

Application de jeu de rôle (style Donjons & Dragons) hébergée sur le PC du MJ.
Les joueurs n'installent rien : ils ouvrent un lien dans leur navigateur (PC, tablette ou téléphone).

## Lancer une partie

1. Double-clique sur **`LaTaverne.exe`**. Le navigateur s'ouvre tout seul, et la fenêtre noire
   affiche les adresses, le mot de passe du MJ et le lien Internet.
   (Depuis le code source : `demarrer.bat`.)
2. Onglet **Maître du jeu**, puis saisis le mot de passe (`dragon` par défaut).
3. Les joueurs ouvrent le lien qu'on leur envoie et entrent **le nom de leur personnage**.
   - Nom nouveau : une fiche et un pion sont créés.
   - Nom existant : le joueur retrouve sa fiche (majuscules et accents ignorés).

Laisse la fenêtre noire ouverte pendant toute la partie. La fermer arrête le serveur.
Tout est sauvegardé automatiquement dans `%APPDATA%\LaTaverne\data` : fiches, cartes, pions, journal.
Ce dossier est séparé du programme : une mise à jour par le launcher ne l'efface jamais.

## Jouer à distance

**Option recommandée : tunnel Cloudflare** (gratuit, sans compte, sans toucher à la box)

1. Rien à faire : au lancement, l'application utilise `cloudflared.exe` (rangé dans `%APPDATA%\LaTaverne`).
   S'il manque, elle le télécharge automatiquement une seule fois (~55 Mo).
2. Un lien `https://xxxx.trycloudflare.com` s'affiche dans la fenêtre
   et dans l'onglet **MJ → Inviter les joueurs** (avec un bouton Copier).
3. Envoie ce lien aux joueurs (Discord, SMS…). ⚠ Le lien change à chaque lancement.

**Même maison / même Wi-Fi** : utilise l'adresse « Réseau local » (ex. `http://192.168.1.147:3000`).
Windows peut demander d'autoriser Node.js dans le pare-feu : accepte pour les réseaux privés.

**Autres solutions** : Tailscale (VPN gratuit, chaque joueur l'installe) ou une redirection
du port 3000 sur ta box. Le tunnel Cloudflare reste le plus simple.

## Fonctionnalités

| | Joueur | MJ |
|---|---|---|
| Fiche de personnage (caracs, PV, CA, inventaire, sorts, notes…) | la sienne | toutes, modifiables |
| Lancer de dés : on choisit ses dés (d4 à d100), le bonus, l’avantage, puis **Lancer** ; les dés roulent à l’écran pour tous | ✔ | ✔ |
| Jets secrets (visibles du MJ seulement) | ✔ | ✔ (visibles de lui seul) |
| Chat, murmures privés | ✔ | ✔ + messages de narration |
| Carte avec grille et pions déplaçables | son pion | tous les pions |
| Pion = petit personnage à créer (peau, coiffure, barbe, tenue, couvre-chef, arme, bouclier, cape, modèles prêts) — ou emblème / portrait, avec cadre (or, feu, arcane…) | le sien | tous, y compris les monstres |
| Pions cachés (monstres en embuscade) | — | ✔ |
| Initiative : ordre, tour actif, rounds | lance la sienne | gère tout |
| Cartes / scènes (images) | — | import et changement de scène |
| Diaporamas (slides) projetés en plein écran chez tous les joueurs, générés par l'IA ou faits à la main | les voit | crée, génère, projette |

Sur les boutons de dés : clic = ajouter un dé, clic droit = en retirer un. Animation et son se coupent sous le bouton Lancer.
Raccourcis du chat : `/r 1d20+5 Attaque` (jet public), `/sr 1d20` (jet secret).
Sur la fiche, clique sur un modificateur (+2, −1…) pour lancer d20 + modificateur.
Onglet **Pion** (ou double-clic sur ton pion) : personnalise son apparence.

## Configuration (`config.json`)

Créé automatiquement dans `%APPDATA%\LaTaverne` au premier lancement
(tape `%APPDATA%\LaTaverne` dans la barre d'adresse de l'explorateur pour l'ouvrir) :

```json
{ "port": 3000, "motDePasseMJ": "dragon", "tunnelCloudflare": true, "ouvrirNavigateur": true, "cleApiAnthropic": "" }
```

Change le mot de passe du MJ avant de partager le lien sur Internet. Relance l'application après une modification.

## Slides générées par l'IA

Onglet **Slides** (MJ) : crée un diaporama, puis décris la scène (« les héros arrivent de nuit au village de Brumeval… »),
choisis le nombre de slides et le ton, et clique sur **✨ Générer**. Claude rédige le titre, le texte d'ambiance
à lire aux joueurs et choisit le thème visuel ; tout reste modifiable, et on peut ajouter une image (importée ou une carte).
**▶ Projeter** affiche les slides en grand chez tous les joueurs ; le MJ avance avec les flèches du clavier ou les boutons.

Pour activer la génération :
1. Crée une clé API sur https://console.anthropic.com (« API Keys »). L'usage est payant : quelques centimes par génération.
2. Colle-la dans `config.json` : `"cleApiAnthropic": "sk-ant-…"`, puis relance l'application.

Sans clé, tout fonctionne quand même : les slides se créent à la main. La clé reste sur ton PC (elle n'est jamais envoyée aux joueurs).

## Déplacer ou copier l'application

`LaTaverne.exe` se suffit à lui-même : copie-le où tu veux, ou installe-le depuis le launcher Winton.
Les données et `config.json` sont dans `%APPDATA%\LaTaverne` (ou dans le dossier indiqué par la
variable d'environnement `LATAVERNE_DATA_DIR`). Pour changer de PC, copie ce dossier.
Les anciennes versions rangeaient `data/` et `config.json` à côté de l'exe : ils sont recopiés
automatiquement au premier lancement (l'original n'est pas supprimé).

Si Windows affiche « Windows a protégé votre ordinateur » au premier lancement :
clique sur **Informations complémentaires**, puis **Exécuter quand même**.
L'avertissement apparaît parce que l'exe n'est pas signé.

## Reconstruire l'exe (développeur)

`npm install`, puis `npm run build` : le résultat est dans `dist/LaTaverne.exe`.

## Repartir à zéro

Arrête le serveur, puis supprime le dossier `%APPDATA%\LaTaverne\data`.
