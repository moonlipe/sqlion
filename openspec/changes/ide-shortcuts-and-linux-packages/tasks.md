# Tasks

## 1. Identificação e resolução de objetos

- [x] 1.1 Implementar `sqltext::ident_at(text, char_pos)` e a variante para texto selecionado (`schema.nome`, aspas duplas, colchetes, crases, cursor no início/fim/meio, dentro de string e comentário → `None`); verificar com testes unitários em `src/sqltext.rs`
- [x] 1.2 Implementar `Shared::find_object(conn, schema, name, allow_query) -> Option<ObjRef>` (schema padrão primeiro, depois os demais; tabelas, views e pastas de objetos com código do tipo de banco; sem diferenciar maiúsculas; `allow_query = false` só olha caches); verificar com teste em SQLite de exemplo cobrindo tabela, view, nome inexistente e cache vazio sem consulta

## 2. Atalhos Ctrl+T, F4 e F3

- [x] 2.1 Ligar Ctrl+T em `frame_logic` a `new_script_here()` com foco no editor novo; verificar com teste de frame (evento Ctrl+T abre aba SQL vazia ligada à conexão atual) e que fechá-la vazia não deixa arquivo
- [x] 2.2 Adicionar `SqlCmd::OpenObject` e F4: no editor focado resolve com `ident_at` + `find_object(.., true)` e empilha `Req::OpenTable`/`Req::OpenSource`; fora do editor usa `nav_sel`; objeto não encontrado ou conexão fechada vai para a barra de status; verificar com testes de frame em SQLite (tabela abre/ativa aba da tabela, view abre a definição, nome desconhecido não abre aba)
- [x] 2.3 Fazer `open_script_file` ativar a aba existente do mesmo arquivo; verificar com teste (abrir duas vezes mantém uma aba)
- [x] 2.4 Criar `Dialog::PickScript` (pasta do projeto da aba ativa → projeto da conexão atual → scripts soltos; ordem por modificação; filtro; setas, Enter, clique duplo, Esc; estado vazio com "Novo script") e ligar F3; verificar com teste do filtro/ordem e percorrendo o diálogo no app
- [x] 2.5 Atualizar a lista de atalhos da tela Sobre e do README com Ctrl+T, F3, F4 e Ctrl+B; verificar lendo a tela Sobre no app

## 3. Menus compartilhados e menu de contexto do editor

- [x] 3.1 Extrair `table_menu` e o menu de objetos com código para funções que recebem `&mut Shared` e o `ObjRef` e só emitem `Req` (trocar `open_sql` por `Req::OpenSql`); verificar que o menu do navegador continua com as mesmas ações (`cargo test` e conferência no app)
- [x] 3.2 Adicionar o menu de contexto do `TextEdit`: posicionamento do cursor no clique direito fora da seleção, Recortar/Copiar (desabilitados sem seleção)/Colar via `ViewportCommand::Request*`, Selecionar tudo, Executar instrução, Executar script, Formatar, Comentar, Abrir objeto (F4), com atalhos ao lado; verificar com teste de frame (clique direito dentro da seleção a mantém; fora move o cursor) e no app
- [x] 3.3 Incluir a seção do objeto (resolvido uma vez ao abrir, `find_object(.., false)`, guardado em `ctx_obj`) com as ações compartilhadas; verificar com teste em SQLite (tabela no clique mostra a seção; palavra-chave ou cache vazio não mostra) e no app

## 4. Navegador: área vazia e recolher

- [x] 4.1 Adicionar `Prefs::nav_collapsed` e `Prefs::nav_width` com `#[serde(default)]`; verificar com teste de leitura de preferências antigas sem os campos
- [x] 4.2 Desenhar a faixa recolhida e o botão de recolher/expandir, o item "Mostrar navegador" no menu Janela e Ctrl+B (sem alterar o texto do editor focado); verificar com teste de frame (Ctrl+B alterna, largura anterior volta) e no app
- [x] 4.3 Menu da área vazia (Nova conexão, Novo projeto, Abrir projeto, Novo script, Nova pasta de conexões no projeto padrão, Atualizar) com a área clicável depois da árvore, sem tomar o clique dos itens; verificar com teste de frame (clique direito abaixo da árvore abre o menu; sobre uma conexão abre o da conexão) e no app sem conexões

## 5. Rolagem e Page Up/Page Down

- [x] 5.1 Adicionar `Prefs::scroll_speed` (padrão 2.0, faixa 0,5–5) no diálogo de Preferências e aplicá-la em `frame_logic` via `input_options.line_scroll_speed = 40 × fator` quando mudar; verificar com teste (padrão resulta em 80; mudar a preferência reaplica) e girando a roda no editor, grade e navegador
- [x] 5.2 Implementar Page Up/Page Down no editor (página visível em linhas, coluna mantida, limites do texto, Shift estende a seleção, rolagem acompanha) e na lista do autocompletar; verificar com testes de frame em texto de 200 linhas cobrindo os quatro cenários da spec

## 6. Pacotes Linux

- [x] 6.1 Criar o ícone do app (SVG de origem e PNG 64/128/256) em `packaging/linux/icons/`, o `.desktop` e o metainfo AppStream; verificar com `desktop-file-validate` e `appstreamcli validate --no-net`
- [x] 6.2 Definir `with_app_id("io.github.moonlipe.Sqlion")` e `with_icon` (PNG embutido) no `ViewportBuilder`; verificar que `cargo build` passa e que a janela aparece com o ícone no painel do desktop
- [x] 6.3 Adicionar `[package.metadata.deb]` e `[package.metadata.generate-rpm]` ao `Cargo.toml`; verificar localmente com `cargo deb --no-build` e `cargo generate-rpm` sobre `target/release/sqlion` e inspecionando os pacotes (`dpkg -c`, `rpm -qlp`, dependências com `dpkg -I`/`rpm -qRp`)
- [ ] 6.4 Criar o manifesto Flatpak (binário pré-compilado, runtime `org.freedesktop.Platform//24.08`, permissões de rede, Wayland/X11, DRI e pasta pessoal); verificar com `flatpak-builder --user --install` local e `flatpak run io.github.moonlipe.Sqlion`
- [ ] 6.5 Montar o AppDir e gerar o AppImage com `linuxdeploy`; verificar executando o `.AppImage` gerado localmente
- [x] 6.6 Criar `packaging/linux/arch/PKGBUILD` (fonte do release, checksum, instala binário, licença, `.desktop`, ícones, metainfo); verificar com `namcap PKGBUILD` e `makepkg -si` a partir de um `git archive` local
- [ ] 6.7 Adicionar o job `linux-packages` (x86_64 e aarch64; Flatpak só x86_64) ao `release.yml`, fazer o `release` depender dele, incluir os pacotes no `SHA256SUMS` e imprimir o checksum do código-fonte no resumo; verificar disparando o workflow manualmente e conferindo os artefatos `.deb`, `.rpm`, `.AppImage` e `.flatpak` com os nomes da spec
- [x] 6.8 Documentar no README a instalação por família (Debian/Ubuntu, Fedora/RHEL/openSUSE, Arch, Flatpak, AppImage) e o requisito de glibc do AppImage; verificar que os comandos do README batem com os nomes dos artefatos da execução manual

## 7. Integração

- [ ] 7.1 Rodar `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings` e `cargo build --release`, e percorrer no app os cenários de todas as specs da mudança; anotar o que ficou de fora
