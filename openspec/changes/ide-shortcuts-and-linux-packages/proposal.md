# Proposal

## Why

Quem vem do DBeaver espera os mesmos atalhos e menus: Ctrl+T para um script novo, F4 para abrir o objeto sob o cursor, F3 para escolher um script, menu de contexto no editor e na parte vazia do navegador. Hoje nada disso existe, a rolagem com a roda do mouse é lenta, o editor não reage a Page Up/Page Down e o navegador não pode ser recolhido para dar espaço ao editor. Além disso, no Linux o Sqlion só é distribuído como `.tar.gz`, sem instalação pelo gerenciador de pacotes das distribuições mais usadas.

## What Changes

- **Ctrl+T** abre uma nova aba de script vazia (mesmo comportamento de Ctrl+]: usa a conexão atual e o lugar padrão de novos scripts).
- **F4** abre o objeto sob o cursor ou selecionado no editor (ou o selecionado no navegador): tabela abre a aba da tabela; view, procedure, função, trigger e demais objetos com código abrem a definição.
- **F3** abre uma lista dos scripts do projeto da aba atual, com filtro por nome, para abrir um deles.
- **Botão direito na parte vazia do navegador** abre um menu com Nova conexão, Novo projeto, Abrir projeto, Novo script, Nova pasta de conexões (no projeto padrão) e Atualizar.
- **Botão direito dentro do editor** abre um menu de edição (recortar, copiar, colar, selecionar tudo, executar, formatar, comentar); se o cursor ou a seleção estiver sobre uma tabela, view ou outro objeto conhecido da conexão, o menu ganha uma seção com as ações daquele objeto (as mesmas do navegador).
- **Recolher o navegador** por um botão no cabeçalho do painel e no menu Janela (atalho Ctrl+B); recolhido, resta uma faixa estreita com o botão de expandir. O estado é lembrado entre sessões.
- **Rolagem mais rápida** com a roda do mouse em todo o app, com velocidade ajustável em Preferências.
- **Page Up / Page Down** no editor SQL movem o cursor uma página visível (com Shift, estendendo a seleção) e rolam junto.
- **Pacotes Linux no release**: `.deb` (Debian, Ubuntu, Mint, Pop!_OS), `.rpm` (Fedora, RHEL/Alma/Rocky 10+, openSUSE), AppImage (qualquer distro), bundle Flatpak e um `PKGBUILD` para Arch e derivadas, com atalho no menu de aplicativos e ícone.

## Capabilities

### New Capabilities
- `keyboard-shortcuts`: atalhos de IDE Ctrl+T (novo script), F3 (escolher script do projeto) e F4 (abrir o objeto sob o cursor).
- `editor-context-menu`: menu de contexto do editor SQL, com ações de edição e ações do objeto sob o cursor ou seleção.
- `navigator-panel`: menu de contexto da área vazia do navegador e recolher/expandir o painel lateral.
- `scrolling-and-paging`: velocidade de rolagem da roda do mouse e Page Up/Page Down no editor.

### Modified Capabilities
- `release-pipeline`: o release passa a publicar também pacotes nativos para Linux (deb, rpm, AppImage, Flatpak) e a manter um PKGBUILD para Arch.

## Impact

- `src/app.rs`: atalhos (Ctrl+T, Ctrl+B, F3, F4), menu da área vazia do navegador, painel recolhível, menu de objeto reaproveitado no editor, aplicação da velocidade de rolagem.
- `src/tabs.rs`: menu de contexto do editor, identificação do objeto sob o cursor, Page Up/Page Down.
- `src/dialogs.rs`: janela de escolha de script (F3) e preferência de velocidade de rolagem.
- `src/store.rs`: preferências `scroll_speed` e `nav_collapsed` (com valores padrão para arquivos antigos).
- `src/sqltext.rs`: extração do identificador (com schema e aspas) na posição do cursor.
- `.github/workflows/release.yml`: novos passos de empacotamento Linux; novos arquivos em `packaging/` (arquivo `.desktop`, ícone, metainfo AppStream, manifesto Flatpak, PKGBUILD).
- `Cargo.toml`: metadados `[package.metadata.deb]` e `[package.metadata.generate-rpm]` (ferramentas usadas só no CI, sem novas dependências do app).
- README: instruções de instalação por distribuição.
