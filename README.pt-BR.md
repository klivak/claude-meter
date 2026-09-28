# ⚡ ClaudeMeter

> [English](README.md) · [Español](README.es.md) · **Português** · [Deutsch](README.de.md) · [Français](README.fr.md)

**Monitor de uso do Claude AI em tempo real para Windows e macOS.** Acompanhe os limites da assinatura pela bandeja do sistema ou barra de menus.

O ClaudeMeter é um aplicativo ultraleve em Rust. Ele mostra a sessão de 5 horas, os limites semanais e as cotas de Sonnet e Opus, sem precisar abrir o navegador.

[Site do projeto](https://klivak.github.io/claude-meter/) · [Downloads](https://github.com/klivak/claudemeter/releases/latest) · [Código-fonte](https://github.com/klivak/claudemeter)

![Painel claro do ClaudeMeter](screenshots/dashboard-light-v5.1.png)

## Início rápido

1. Instale o [Claude Code](https://claude.ai/download) e faça login executando `claude` uma vez.
2. Baixe o ClaudeMeter na página de [releases](https://github.com/klivak/claudemeter/releases/latest).
3. No Windows, execute `claudemeter.exe`; no macOS, descompacte `ClaudeMeter-macos-arm64.app.zip` e mova o app para `/Applications`.
4. Procure o ícone na bandeja do Windows ou na barra de menus do macOS.

Não há configuração obrigatória: o plano é detectado automaticamente e o monitoramento começa em seguida.

## Recursos

- Limites do Claude: sessão de 5 horas, limite semanal, Sonnet, Opus e novos campos disponíveis.
- Painel opcional do Codex, lido localmente dos logs em `~/.codex`.
- Ícone dinâmico: percentual, anel, barra ou gráfico de pizza, colorido conforme o uso.
- Painel minimalista, padrão ou detalhado, com histórico de 24 horas, 7 dias e 30 dias.
- Temas Auto, Claro, Escuro, Midnight e Sunset.
- Alertas configuráveis, inicialização automática, exportação CSV/JSON e miniwidget flutuante.
- Interface em 40 idiomas.

## Privacidade e autenticação

O ClaudeMeter não pede senha ou chave de API. Ele reutiliza o token OAuth já armazenado localmente pelo Claude Code e se comunica apenas com `api.anthropic.com` para consultar os seus dados de uso. Não há telemetria.

O token é procurado em `~/.claude/.credentials.json`, no Gerenciador de Credenciais do Windows ou no Chaves do macOS. Essas credenciais nunca são modificadas.

## Plataformas

| Plataforma | Distribuição recomendada |
|---|---|
| Windows 10/11 | `claudemeter.exe` portátil |
| macOS 12+ (Apple Silicon) | `ClaudeMeter-macos-arm64.app.zip` |

O app é nativo: não inclui Electron, .NET, Java, Python nem Node.js. O uso habitual de memória é de 3–8 MB.

## Segurança dos downloads

Binários Rust sem assinatura podem acionar falsos positivos heurísticos do antivírus. Baixe somente em [GitHub Releases](https://github.com/klivak/claudemeter/releases/latest) e confira o SHA-256 com o arquivo `.sha256` publicado junto ao binário:

```powershell
Get-FileHash .\claudemeter.exe -Algorithm SHA256
Get-Content .\claudemeter.exe.sha256
```

## Compilar do código-fonte

```bash
git clone https://github.com/klivak/claudemeter.git
cd claudemeter
cargo build --release
```

No macOS, execute `sh scripts/build-macos-app.sh` para criar o pacote do aplicativo. Consulte o [README em inglês](README.md) para documentação técnica completa, configuração e FAQ.

## Licença

[MIT](LICENSE): livre para uso pessoal e comercial.

Claude é uma marca da Anthropic. ChatGPT é uma marca da OpenAI. ClaudeMeter é um projeto independente, sem afiliação oficial com elas.
