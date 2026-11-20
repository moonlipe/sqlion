# Design

## Context

- Os atalhos globais ficam em `App::frame_logic` (`src/app.rs`), consumidos com `consume_key` antes dos editores; os comandos do editor chegam ao `SqlTab` por `SqlCmd` (`s.cmd`) e são aplicados em `SqlTab::apply_cmd`. Ctrl+] já chama `new_script_here()`; Ctrl+T está livre.
- O editor é um `egui::TextEdit::multiline` dentro de um `ScrollArea` (`SqlTab::show_editor`, `src/tabs.rs`). O `TextEdit` do egui 0.36 não trata Page Up/Page Down e não tem menu de contexto.
- O navegador é um `egui::Panel::left("navigator")` redimensionável (`nav_w`), com um `ScrollArea` em que cada linha da árvore tem seu `context_menu`. As ações de tabela/view estão em `App::table_menu`; as de objetos com código, inline em `App::object_folder`; código-fonte é aberto por `Req::OpenSource` e tabelas por `Req::OpenTable` (que já ativa a aba existente).
- Metadados: `Shared::tables` / `Shared::objects` (caches) e `ensure_tables` / `ensure_objects`, que consultam o banco de forma síncrona quando o cache falta. O realce usa `HlNames`, carregado em segundo plano.
- Rolagem: o egui multiplica cada passo de linha da roda por `InputOptions::line_scroll_speed` (40 px no nativo); passos em pixels (touchpad de precisão) não passam por esse fator.
- Release: `.github/workflows/release.yml` compila 5 alvos e publica `.tar.gz`/`.zip`, código-fonte e `SHA256SUMS`. Não há ícone do app em arquivo nem `.desktop`; o `ViewportBuilder` não define `app_id` nem ícone.

## Goals / Non-Goals

**Goals:**
- Uma única função de identificação de objeto usada por F4 e pelo menu do editor.
- Reaproveitar os menus do navegador no editor, sem duplicar as ações.
- Pacotes Linux gerados do mesmo binário de release já compilado, sem recompilar por formato.

**Non-Goals:**
- Publicar no Flathub, no AUR ou em repositórios apt/dnf próprios (exigem contas e revisão externas); o release entrega os arquivos e o PKGBUILD fica pronto para quem quiser publicar.
- Snap, Nix, Gentoo e pacotes para Windows/macOS (MSI, dmg).
- Atalhos configuráveis pelo usuário.
- Page Up/Page Down fora do editor (a grade já trata; listas e árvore usam a rolagem padrão).

## Decisions

### Identificação do objeto sob o cursor
Nova função pura em `src/sqltext.rs`: `ident_at(text, char_pos) -> Option<(Option<String>, String)>`, que acha o identificador que contém a posição, juntando `schema.nome` e tirando aspas `"…"`, colchetes `[…]` e crases `` `…` ``. Com seleção, usa o texto selecionado (aparado) pelas mesmas regras. Fica separada para ser testada sem banco.

A resolução vai em `Shared::find_object(conn, schema?, name, allow_query) -> Option<ObjRef>` (`src/shared.rs`), com `ObjRef { schema, name, kind: Table | View | Code { key, detail } }`:
1. schemas candidatos: o informado; sem schema, o padrão da conexão e depois os de `Shared::schemas`;
2. em cada schema, tabelas/views (`tables`), depois as pastas de objetos com código do tipo de banco (as mesmas `FolderDef` que o navegador mostra), comparando o nome sem diferenciar maiúsculas;
3. `allow_query = false` olha só os caches (menu de contexto: nunca espera o banco); `true` usa `ensure_*` (F4: ação explícita, como o clique duplo no navegador).

*Alternativa:* usar só `HlNames`. Descartada: não tem procedures/funções nem o schema de cada nome.

F4 vira `SqlCmd::OpenObject`; o `SqlTab` resolve e empilha `Req::OpenTable` ou `Req::OpenSource` (view usa a chave `"view"`, já suportada por `object_source` e `editable_source`). Em `frame_logic`, F4 vai para o editor quando a aba ativa é SQL e o editor tem o foco (`memory.has_focus(id do editor)`); senão, usa `nav_sel` (`NavSel::Table`).

### Menus compartilhados
`table_menu` e o menu dos objetos com código passam a ser funções que recebem `&mut Shared` e o `ObjRef` e devolvem pedidos (`Req`) em vez de mexer em `App` diretamente — o que já é quase o caso, pois quase tudo passa por `self.sh.req(...)`. As duas chamadas que usam `App` (`open_sql`) passam a `Req::OpenSql`. Assim o `SqlTab` (que só tem `&mut Shared`) desenha o mesmo menu. *Alternativa:* devolver uma ação do `SqlTab` ao `App` para ele desenhar o menu; descartada por exigir o menu fora do `Response` do editor.

### Menu de contexto do editor
`out.response.context_menu(...)` no `TextEdit`. No `secondary_clicked`, a posição do ponteiro vira índice com `galley.cursor_from_pos(pointer - galley_pos)`; se cair fora da seleção, o cursor vai para lá (estado do `TextEdit` gravado como já é feito em `set_cursor`). O objeto da seção é resolvido uma vez ao abrir o menu e guardado no `SqlTab` (`ctx_obj`) para não refazer a busca a cada frame. Recortar/Copiar/Colar usam `ViewportCommand::RequestCut/RequestCopy/RequestPaste`, que geram os eventos de teclado que o `TextEdit` já entende, com o foco devolvido ao editor. Os demais itens viram `SqlCmd` (mesmo caminho dos atalhos).

### Page Up / Page Down
Consumidos em `show_editor` antes do `TextEdit` (e, com o autocompletar aberto, usados para mover `ac.sel` em blocos de 10). Depois do `show`, com `out.galley`: posição do cursor `pos_from_cursor`, alvo `y ± altura visível` (altura do `ScrollArea`, arredondada para linhas inteiras), `cursor_from_pos(x, alvo)`; acima de 0 vai ao início, abaixo do fim vai ao fim. Shift mantém a âncora (`secondary`). O deslocamento do `ScrollArea` é ajustado pela mesma distância (`set_scroll`), o que mantém o cursor na mesma posição relativa da tela, como nos editores comuns. A coluna "desejada" vem do `x` do cursor, então linhas curtas não a perdem em passos seguidos.

### Velocidade de rolagem
Preferência `scroll_speed: f32` (padrão 2.0, faixa 0.5–5.0, `#[serde(default)]`). Em `frame_logic`, quando muda (mesmo esquema de `applied_theme`): `ctx.options_mut(|o| o.input_options.line_scroll_speed = 40.0 * scroll_speed)`. Um único ponto vale para todas as áreas roláveis; touchpads em pixels não são afetados. *Alternativa:* multiplicar `smooth_scroll_delta` por área; descartada por espalhar o ajuste.

### Navegador recolhível e menu da área vazia
`Prefs::nav_collapsed: bool` (`#[serde(default)]`); `nav_w` também passa a ser gravado (`Prefs::nav_width`, padrão o valor atual). Recolhido, desenha-se `Panel::left("navigator_rail")` não redimensionável com ~28 px e o botão de expandir; expandido, o painel atual com um botão de recolher no cabeçalho. Ctrl+B é consumido em `frame_logic` (o `TextEdit` não usa Ctrl+B). Item "Mostrar navegador" (checkbox) no menu Janela.

Área vazia: depois da árvore no `ScrollArea`, um `ui.allocate_response(ui.available_size(), Sense::click())` ocupa o resto; o `context_menu` dele mostra o menu da área vazia. Itens da árvore desenhados antes recebem o clique primeiro, então o menu de cada item continua valendo. Para o espaço entre o filtro e a árvore, o mesmo menu é ligado ao `Response` do painel inteiro apenas quando nenhum item tratou o clique (`ctx.is_context_menu_open()` no frame).

### Escolha de script (F3)
`Dialog::PickScript { dir, filter, sel }` em `src/dialogs.rs`. Pasta: `scripts_dir` do projeto da aba ativa (pelo caminho do arquivo do script ou pela conexão via `new_script_dir`), senão o do projeto da conexão atual, senão os scripts soltos. Lista lida do disco ao abrir (`*.sql`, ordem por `modified` decrescente), filtro por substring sem diferenciar maiúsculas, setas/Enter/Esc. Abrir usa `open_script_file`, que passa a ativar a aba existente do mesmo arquivo em vez de duplicar.

### Pacotes Linux
Arquivos novos em `packaging/linux/`: `io.github.moonlipe.Sqlion.desktop`, `io.github.moonlipe.Sqlion.metainfo.xml`, ícones `icons/{64,128,256}.png` (e o SVG de origem), `flatpak/io.github.moonlipe.Sqlion.yml`, `arch/PKGBUILD`. O app passa a usar `with_app_id("io.github.moonlipe.Sqlion")` e `with_icon` (PNG 256 embutido com `include_bytes!`) no `ViewportBuilder`, para o desktop associar a janela ao lançador.

No workflow, um novo job `linux-packages` (matriz x86_64 em `ubuntu-22.04` e aarch64 em `ubuntu-22.04-arm`) baixa o `.tar.gz` do job `build` e gera:
- `.deb` com `cargo-deb --no-build` (metadados em `[package.metadata.deb]`: dependências `libc6, libgl1, libxkbcommon0, libwayland-client0, libx11-6, libxcursor1, libxi6, libxrandr2`; assets binário, `.desktop`, ícones, metainfo);
- `.rpm` com `cargo-generate-rpm` (`[package.metadata.generate-rpm]`, requisitos equivalentes com nomes Fedora/openSUSE via `auto-req` desligado e lista explícita: `mesa-libGL`, `libxkbcommon`, `libwayland-client`, `libX11`, `libXcursor`, `libXi`, `libXrandr`);
- AppImage com `linuxdeploy` (AppDir montado com o binário, `.desktop` e ícone; as bibliotecas gráficas ficam no sistema, como recomenda o AppImage para OpenGL);
- só no x86_64: Flatpak com `flatpak-builder` no runtime `org.freedesktop.Platform//24.08`, instalando o binário pré-compilado (módulo `simple` com o `.tar.gz` local), permissões `--share=network --socket=wayland --socket=fallback-x11 --device=dri --filesystem=home`, e `flatpak build-bundle` para gerar o `.flatpak`.

O job `release` passa a depender de `build` e `linux-packages` e inclui tudo em `SHA256SUMS`. O `PKGBUILD` usa `source=(…/sqlion-$pkgver-source.tar.gz)` com `sha256sums` atualizados por versão (um passo do workflow imprime o checksum no resumo da execução para facilitar). *Alternativa:* `cargo-bundle` ou `nfpm` para deb/rpm; `cargo-deb`/`cargo-generate-rpm` são os mais usados com Rust e leem os metadados do `Cargo.toml`.

## Risks / Trade-offs

- [F4 síncrono pode travar a UI em banco lento] → só quando o cache falta e é ação explícita, igual ao clique duplo no navegador hoje; a barra de status mostra "procurando…".
- [`RequestPaste` depende do backend (Wayland/X11)] → é o mesmo caminho do Ctrl+V; se falhar, nada é colado e o texto não se altera.
- [AppImage com glibc do Ubuntu 22.04] → roda em distros com glibc ≥ 2.35; documentado no README.
- [Flatpak com `--filesystem=home` é permissão ampla] → necessária para scripts, projetos em pastas arbitrárias e arquivos SQLite; documentado.
- [Nomes de dependências RPM diferem entre Fedora e openSUSE] → usar nomes de biblioteca (`libGL.so.1()(64bit)` etc.) em vez de nomes de pacote no `requires`.
- [Dobrar a rolagem pode ficar rápido demais em mouses de passo fino] → ajuste de 0,5× a 5× nas Preferências.
- [`with_app_id` muda a classe da janela no X11/Wayland] → sem efeito em dados; atalhos de desktop antigos de quem extraiu o `.tar.gz` continuam funcionando pelo caminho do binário.

## Migration Plan

Preferências novas têm `#[serde(default)]`; arquivos antigos carregam com os padrões. O release seguinte (0.3.0) é o primeiro com pacotes Linux; os `.tar.gz` continuam iguais. Para desfazer o empacotamento basta remover o job `linux-packages`.
