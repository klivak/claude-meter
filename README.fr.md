# ⚡ ClaudeMeter

> [English](README.md) · [Español](README.es.md) · [Português](README.pt-BR.md) · [Deutsch](README.de.md) · **Français**

**Moniteur d’utilisation de Claude AI en temps réel pour Windows et macOS.** Suivez les limites de votre abonnement depuis la zone de notification ou la barre des menus.

ClaudeMeter est une application Rust très légère. Elle affiche votre session de 5 heures, vos limites hebdomadaires ainsi que les quotas Sonnet et Opus, sans ouvrir de navigateur.

[Site du projet](https://klivak.github.io/claude-meter/) · [Téléchargements](https://github.com/klivak/claudemeter/releases/latest) · [Code source](https://github.com/klivak/claudemeter)

![Tableau de bord clair ClaudeMeter](screenshots/dashboard-light-v5.1.png)

## Démarrage rapide

1. Installez [Claude Code](https://claude.ai/download), puis connectez-vous une fois en exécutant `claude`.
2. Téléchargez ClaudeMeter depuis la page des [releases](https://github.com/klivak/claudemeter/releases/latest).
3. Sous Windows, lancez `claudemeter.exe`. Sous macOS, décompressez `ClaudeMeter-macos-arm64.app.zip` et placez l’application dans `/Applications`.
4. Repérez l’icône dans la zone de notification Windows ou dans la barre des menus macOS.

Aucun réglage n’est obligatoire : votre forfait est détecté automatiquement et le suivi démarre immédiatement.

## Fonctionnalités

- Limites Claude : session de 5 heures, limite hebdomadaire, Sonnet, Opus et futurs champs disponibles.
- Panneau Codex facultatif, lu localement depuis les journaux `~/.codex`.
- Icône dynamique : pourcentage, anneau, barre ou diagramme circulaire, coloré selon l’utilisation.
- Tableaux de bord minimal, standard et détaillé, avec historique de 24 heures, 7 jours et 30 jours.
- Thèmes Auto, Clair, Sombre, Midnight et Sunset.
- Alertes configurables, démarrage automatique, export CSV/JSON et mini-widget flottant.
- Interface disponible en 40 langues.

## Confidentialité et authentification

ClaudeMeter ne demande ni mot de passe ni clé API. Il réutilise le jeton OAuth que Claude Code a déjà enregistré localement et communique uniquement avec `api.anthropic.com` afin de consulter vos propres données d’utilisation. Aucune télémétrie n’est intégrée.

Le jeton est recherché dans `~/.claude/.credentials.json`, le Gestionnaire d’identifiants Windows ou le Trousseau macOS. Ces identifiants ne sont jamais modifiés.

## Plateformes

| Plateforme | Distribution recommandée |
|---|---|
| Windows 10/11 | `claudemeter.exe` portable |
| macOS 12+ (Apple Silicon) | `ClaudeMeter-macos-arm64.app.zip` |

L’application est native : elle n’embarque ni Electron, ni .NET, ni Java, ni Python, ni Node.js. Sa consommation habituelle est de 3 à 8 Mo de mémoire.

## Vérifier les téléchargements

Les binaires Rust non signés peuvent déclencher de faux positifs heuristiques de l’antivirus. Téléchargez uniquement depuis [GitHub Releases](https://github.com/klivak/claudemeter/releases/latest), puis comparez le SHA-256 avec le fichier `.sha256` publié à côté du binaire :

```powershell
Get-FileHash .\claudemeter.exe -Algorithm SHA256
Get-Content .\claudemeter.exe.sha256
```

## Compiler depuis les sources

```bash
git clone https://github.com/klivak/claudemeter.git
cd claudemeter
cargo build --release
```

Sous macOS, exécutez `sh scripts/build-macos-app.sh` pour produire le paquet de l’application. Consultez le [README anglais](README.md) pour la documentation technique complète, la configuration et la FAQ.

## Licence

[MIT](LICENSE) : utilisation personnelle et commerciale autorisée.

Claude est une marque d’Anthropic. ChatGPT est une marque d’OpenAI. ClaudeMeter est un projet open source indépendant, sans affiliation officielle.
