# ⚡ ClaudeMeter

> [English](README.md) · [Español](README.es.md) · [Português](README.pt-BR.md) · **Deutsch** · [Français](README.fr.md)

**Echtzeit-Monitor für die Claude-AI-Nutzung unter Windows und macOS.** Behalte deine Abonnementlimits direkt über die Taskleiste oder Menüleiste im Blick.

ClaudeMeter ist eine extrem schlanke Rust-Anwendung. Sie zeigt die 5-Stunden-Sitzung, Wochenlimits sowie Sonnet- und Opus-Kontingente an – ohne einen Browser zu öffnen.

[Projekt-Website](https://klivak.github.io/claude-meter/) · [Downloads](https://github.com/klivak/claudemeter/releases/latest) · [Quellcode](https://github.com/klivak/claudemeter)

![Helles ClaudeMeter-Dashboard](screenshots/dashboard-light-v5.1.png)

## Schnellstart

1. Installiere [Claude Code](https://claude.ai/download) und melde dich einmal über `claude` an.
2. Lade ClaudeMeter von der [Release-Seite](https://github.com/klivak/claudemeter/releases/latest) herunter.
3. Unter Windows startest du `claudemeter.exe`; unter macOS entpackst du `ClaudeMeter-macos-arm64.app.zip` und verschiebst die App nach `/Applications`.
4. Suche das Symbol im Windows-Infobereich oder in der macOS-Menüleiste.

Eine Konfiguration ist nicht nötig: Der Tarif wird automatisch erkannt und die Überwachung startet sofort.

## Funktionen

- Claude-Limits für 5-Stunden-Sitzung, Woche, Sonnet, Opus und künftig verfügbare Werte.
- Optionales Codex-Panel, das lokale Protokolle aus `~/.codex` ausliest.
- Dynamisches Tray-Symbol: Prozentzahl, Ring, Balken oder Kreisdiagramm, farbcodiert nach Auslastung.
- Minimal-, Standard- und Detailansicht mit Verlauf für 24 Stunden, 7 Tage und 30 Tage.
- Die Themen Auto, Hell, Dunkel, Midnight und Sunset.
- Konfigurierbare Benachrichtigungen, Autostart, CSV-/JSON-Export und schwebendes Mini-Widget.
- Oberfläche in 40 Sprachen.

## Datenschutz und Anmeldung

ClaudeMeter fragt weder Passwort noch API-Schlüssel ab. Es verwendet das OAuth-Token, das Claude Code bereits lokal gespeichert hat, und verbindet sich ausschließlich mit `api.anthropic.com`, um deine Nutzungsdaten abzurufen. Es gibt keine Telemetrie.

Das Token wird in `~/.claude/.credentials.json`, im Windows-Anmeldeinformationsmanager oder im macOS-Schlüsselbund gesucht. Diese Zugangsdaten werden niemals verändert.

## Plattformen

| Plattform | Empfohlene Distribution |
|---|---|
| Windows 10/11 | Tragbare `claudemeter.exe` |
| macOS 12+ (Apple Silicon) | `ClaudeMeter-macos-arm64.app.zip` |

Die App ist nativ und enthält weder Electron noch .NET, Java, Python oder Node.js. Der typische Speicherbedarf liegt bei 3–8 MB.

## Downloads prüfen

Nicht signierte Rust-Binärdateien können heuristische Fehlalarme im Antivirus auslösen. Lade nur über [GitHub Releases](https://github.com/klivak/claudemeter/releases/latest) herunter und vergleiche den SHA-256-Wert mit der neben der Binärdatei veröffentlichten `.sha256`-Datei:

```powershell
Get-FileHash .\claudemeter.exe -Algorithm SHA256
Get-Content .\claudemeter.exe.sha256
```

## Aus dem Quellcode bauen

```bash
git clone https://github.com/klivak/claudemeter.git
cd claudemeter
cargo build --release
```

Unter macOS erstellt `sh scripts/build-macos-app.sh` das App-Paket. Vollständige technische Dokumentation, Konfiguration und FAQ findest du im [englischen README](README.md).

## Lizenz

[MIT](LICENSE) – frei für private und kommerzielle Nutzung.

Claude ist eine Marke von Anthropic. ChatGPT ist eine Marke von OpenAI. ClaudeMeter ist ein unabhängiges Open-Source-Projekt ohne offizielle Zugehörigkeit.
