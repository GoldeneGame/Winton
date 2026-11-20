# Winton

Un launcher de jeux pour Windows, et les jeux qu'il installe.
Le launcher et chaque jeu se mettent à jour en un clic : quand une nouvelle version est publiée ici,
un bouton **Mise à jour** apparaît dans le launcher.

## Installer

1. Télécharge `launcher-windows.zip` dans la dernière [Release du launcher](https://github.com/GoldeneGame/Winton/releases).
2. Dézippe-le dans un dossier à toi (ex. `Documents\Winton`), puis lance `Launcher.exe`.
3. Les jeux disponibles apparaissent dans la section **À installer** : clique pour les installer.

Windows peut afficher « Windows a protégé votre ordinateur » : **Informations complémentaires**, puis **Exécuter quand même**
(les exe ne sont pas signés par un certificat payant ; les mises à jour, elles, sont signées, voir plus bas).

À côté de `Launcher.exe` :

```
Launcher.exe
LauncherData\   ta bibliothèque, tes temps de jeu, tes jaquettes (jamais touché par les mises à jour)
Jeux\           les jeux installés depuis le launcher (remplacés à chaque mise à jour)
```

Les sauvegardes des jeux sont rangées ailleurs (La Taverne : `%APPDATA%\LaTaverne`), donc une mise à jour ne les efface jamais.

## Organisation du dépôt

```
catalogue.json            liste des jeux proposés par le launcher (id, nom, exe à lancer)
launcher/                 le launcher (Rust + egui)
jeux/la-taverne/          La Taverne (Node.js, compilée en un seul .exe)
scripts/release.ps1       publier une nouvelle version
.github/workflows/        construction, signature et publication automatiques
```

## Publier une mise à jour

1. Fais tes modifications et commite-les.
2. Écris les nouveautés dans le `CHANGELOG.md` du composant, sous un titre `## X.Y.Z`.
3. Lance :
   ```powershell
   .\scripts\release.ps1 la-taverne 1.1.0     # ou : .\scripts\release.ps1 launcher 1.1.0
   ```
   Le script met le numéro de version, crée le commit et le tag `la-taverne-v1.1.0`, puis demande
   avant d'envoyer sur GitHub.
4. La GitHub Action **Release** construit l'exe, le zippe, le signe et crée la Release.
   Les launchers vérifient au démarrage puis toutes les heures, et proposent alors la mise à jour
   (ou tout de suite avec « Vérifier les mises à jour »).

### Ajouter un nouveau jeu

1. Mets ses sources dans `jeux/<id>/` avec un `package.json` (champ `version`) dont `npm run build`
   produit le jeu dans `dist/`, et un `CHANGELOG.md`.
2. Ajoute-le dans `catalogue.json` : `{ "id": "<id>", "name": "Nom affiché", "exe": "Jeu.exe" }`.
3. Publie-le : `.\scripts\release.ps1 <id> 1.0.0`.

Le jeu doit ranger ses sauvegardes **en dehors** de son dossier (ex. `%APPDATA%\<Jeu>`),
car ce dossier est entièrement remplacé à chaque mise à jour.

## Sécurité des mises à jour

Même principe que [Ronnie](https://github.com/Jellfedora/ronnie) :

- Chaque Release contient `<composant>-windows.zip`, son empreinte `.sha256` et sa signature `.sig` (Ed25519).
- La signature est faite par la GitHub Action avec une clé privée stockée dans le secret `WINTON_UPDATE_KEY`.
  La clé publique correspondante est intégrée au launcher (`launcher/src/update.rs`).
- Le launcher refuse d'installer une archive dont l'empreinte ou la signature ne correspond pas.
  Même quelqu'un qui pourrait publier une Release sans la clé privée ne pourrait pas faire installer quoi que ce soit.

La clé privée n'est **jamais** dans le dépôt. Si elle est perdue, il faut en générer une nouvelle,
mettre la nouvelle clé publique dans `update.rs` et republier le launcher (que chacun devra re-télécharger une fois).

## Développement

- Launcher : `launcher\build.ps1` (Rust, toolchain `stable-x86_64-pc-windows-gnu` + MinGW, ou MSVC).
  `WINTON_NO_UPDATE=1` désactive la vérification des mises à jour.
- La Taverne : `cd jeux\la-taverne`, `npm install`, `npm start` (ou `npm run build` pour l'exe).
