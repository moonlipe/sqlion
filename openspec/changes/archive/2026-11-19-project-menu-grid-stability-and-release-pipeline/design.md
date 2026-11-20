# Design

## Context

Veja proposal.md (Why). Estado observado no código:

- `src/app.rs`: a árvore do navegador usa `icons::tree_row` + `r.context_menu(...)` (ex.: `scripts_node`). Ainda não há nó de projeto; ele nasce nas tarefas 4.4/4.5 de `add-projects-and-workspace-polish`, que também criam o modelo `Project` em `src/store.rs` (`create_project`, `open_project`, `close_project`, `write_project`).
- Botão Commit (`src/app.rs`): `tx_info` devolve `(modo, aberta, n_instruções, registros)` de um snapshot sem lock; o rótulo só mostra `Commit (registros)` e o número de instruções fica no tooltip.
- `src/grid.rs` `show`: com o painel de valor, `ui.set_min_size(vec2(panel_rect.width() - 22.0, panel_rect.height() - 22.0))` e `value_panel_ui(.., panel_rect.width() - 24.0)` usam larguras que ficam negativas em áreas pequenas; `grid_rect` também pode sair com `min > max`. Esses valores negativos são o suspeito principal do crash (confirmar reproduzindo).
- Memória: `g.dirty` dispara `reindex()` (que faz `to_lowercase` por célula quando há filtro e ordenação com `to_lowercase` por comparação); `g.qr.clone()` com cópia truncada de todas as linhas aparece no modo de texto/estatísticas (`grid.rs` ~747-749); `GridState` guarda `cell_rects` e `row_state` por linha; `on_item` faz `rows.extend`. Nenhuma dessas hipóteses está medida.
- Cargo: `eframe` 0.36 com `glow`, `wayland`, `x11`; `rusqlite` com `bundled`; TLS por `rustls` (sem OpenSSL). Perfil release com `lto = "thin"` e `strip`. Remoto: `github.com/moonlipe/sqlion`.
- `xdg-open` é usado para "Abrir a pasta" (`app.rs`, `dialogs.rs`) e só existe no Linux.

## Goals / Non-Goals

**Goals:**
- Menu de projeto completo e seguro (nada destrutivo sem confirmação).
- Corrigir o crash e a causa do crescimento de memória por medição, com teste de regressão.
- Pipeline que gera os 4 alvos e publica o release a partir de uma tag, sem passos manuais além de criar a tag.

**Non-Goals:**
- Instaladores (.msi, .dmg, AppImage, .deb), assinatura de código e notarização do macOS (ver Open Questions).
- Virtualização total de dados do grid (paginação em disco) ou mudança do tamanho de página.
- Auto-atualização do aplicativo.

## Decisions

1. **Menu de projeto como função própria (`project_menu`)**, ao lado de `table_menu`, chamada pelo nó do projeto. Ações destrutivas abrem `Dialog::RenameProject` e `Dialog::DeleteProject`. Alternativa: reutilizar um diálogo genérico de confirmação; descartada porque a exclusão precisa listar caminho, contagens e abas afetadas.
2. **Remover ≠ Excluir.** Remover da lista só chama `close_project` (mantém a pasta); Excluir apaga a pasta com `remove_dir_all` depois da confirmação. Para reduzir risco, a exclusão se limita à pasta que contém `sqlion-project.json` e recusa caminhos de raiz, `$HOME` ou sem o arquivo do projeto. Alternativa: mandar para a lixeira do sistema; descartada por exigir dependência por plataforma.
3. **Rótulo do Commit** `Commit · N instr · M reg`, desabilitado quando não há transação aberta. Mantém o tooltip. As gravações do grid passam a incrementar o contador do snapshot na mesma rotina das demais escritas; se hoje não incrementam, corrigir na origem em vez de somar na interface.
4. **Crash do grid:** centralizar o cálculo em um helper que devolve retângulos com `min <= max` e tamanhos `>= 0` (`.max(0.0)` e `Rect::from_min_max` validado); se `full.width()` for menor que o mínimo (grid 120 + painel 180), esconder o painel. Teste unitário do helper com larguras 0, negativas e muito pequenas, mais um teste do `show` com `egui::Context` e `RawInput` de tamanhos mínimos.
5. **Memória: medir antes de corrigir.** Primeiro uma tarefa de medição (contagem de alocações/RSS ao carregar 200 e 400 linhas, com e sem painel de valor, filtro e ordenação) para localizar a causa; depois correções direcionadas: não clonar `qr` por quadro, `reindex` só quando `dirty` e com chave de ordenação pré-calculada (cache por coluna, invalidada na ordenação/extensão), limpar `cell_rects` por quadro, e `row_state`/`undo` sem cópias das linhas. Alternativa: limitar linhas em memória; descartada por mudar o comportamento.
6. **Release por tag com matriz** em `.github/workflows/release.yml`: jobs `build` (matriz) → `release`. Alvos: `x86_64-unknown-linux-gnu` (ubuntu-22.04, para compatibilidade de glibc), `aarch64-unknown-linux-gnu` (runner nativo `ubuntu-22.04-arm`; alternativa: cross-compilação com `cross`), `x86_64-pc-windows-msvc` (windows-latest), `aarch64-apple-darwin` e `x86_64-apple-darwin` (ambos em `macos-latest`, arm64; o x86_64 é cross-compilado, pois os runners macos-13 e o rótulo macos-14 não estavam disponíveis). Passos: checkout, `dtolnay/rust-toolchain@stable` com o target, `Swatinem/rust-cache`, verificação tag = versão do `Cargo.toml`, `cargo build --release --target`, empacotamento (tar.gz/zip com binário, LICENSE e README), upload de artefato. O job `release` baixa os artefatos, gera `SHA256SUMS` e cria o release com `gh release create --generate-notes` (permissão `contents: write`). Alternativa: `cargo-dist`; descartada por agora para manter o fluxo explícito e fácil de ajustar, mas registrada como evolução.
7. **Dependências de sistema no Linux:** instalar no runner os pacotes necessários ao `eframe` com `glow`/x11/wayland (`libxkbcommon-dev`, `libwayland-dev`, `libx11-dev`, `libxcursor-dev`, `libxi-dev`, `libgl1-mesa-dev`) — a lista exata é validada no primeiro run.
8. **Código-fonte no release:** gerado no job `release` com `git archive --format=tar.gz|zip --prefix=sqlion-<versão>/ <tag>`, que respeita `.gitattributes` (`export-ignore` para `openspec/`, `docs/` internos e afins) e exclui `target/` e arquivos não versionados por construção. Entra no `SHA256SUMS`. Alternativa: usar apenas o "Source code" automático do GitHub; descartada porque não controla o conteúdo nem o nome, e não gera checksum.
9. **CI separado** em `.github/workflows/ci.yml` (`cargo test`, `cargo build`, `cargo clippy` informativo) em PR e em `main`, só no Linux, para ser rápido.
10. **Portabilidade de "Abrir a pasta":** trocar `xdg-open` por um helper com `xdg-open`/`open`/`explorer` conforme `cfg!(target_os)`, já que agora distribuímos para Windows e macOS.
11. **Versão e tag:** o fluxo de lançamento é atualizar `version` em `Cargo.toml`, commitar e `git tag vX.Y.Z && git push --tags`. Um script/README documenta isso. "Configurar o release automaticamente" = a tag dispara tudo; não se usa bot de versionamento (release-please) por ora.

## Risks / Trade-offs

- [Excluir pasta apaga dados do usuário] → confirmação com caminho e contagens, recusa de caminhos sem `sqlion-project.json` e raízes, e padrão do foco no botão Cancelar.
- [A causa real da memória pode ser outra] → tarefa de medição primeiro; o teste de regressão mede alocação, não depende de hipótese.
- [Binários macOS sem assinatura são bloqueados pelo Gatekeeper] → documentar `xattr -d com.apple.quarantine`; assinatura fica fora do escopo.
- [Windows sem console] → adicionar `#![windows_subsystem = "windows"]` no release para não abrir terminal; verificar que logs não dependem de stdout.
- [Runners macOS-13 podem ser descontinuados] → cross-compilar `x86_64-apple-darwin` no runner arm64 como alternativa.
- [Build lento (LTO) em 4 alvos] → `rust-cache` e matriz em paralelo.
- [Dependência da mudança em andamento] → aplicar o grupo 1 depois das tarefas 4.4/4.5 de `add-projects-and-workspace-polish`.

## Migration Plan

Sem migração de dados. Primeiro release: mesclar os workflows, criar `v0.1.0` (ou a versão atual do `Cargo.toml`) e conferir o release; em falha, apagar a tag e o release rascunho, corrigir e repetir. Rollback do código é reverter os commits.

## Open Questions

- Assinatura/notarização no macOS e Windows (SmartScreen) e instaladores: podem vir depois sem mudar este plano.
- Nome final dos pacotes para distribuições Linux (AppImage/deb): deferível.

## Resultado da medição (tarefa 4.1)

Benchmark `grid::tests::measure_memory_while_paging` (12 colunas, 200→400 linhas, uma coluna com ~24 kB, rolagem por roda do mouse). Achados:
- Cada célula era copiada (`replace`), hasheada e disposta em layout **por inteiro** a cada quadro, mesmo truncada na tela. Com valores grandes o quadro ficava lento e o uso de memória subia. Correção: o texto da célula é cortado em 300 caracteres antes do layout (`cell_display_text`); o valor completo continua nos dados e no painel de valor. No benchmark o tempo caiu de >100 s para ~10 s e o RSS ficou estável após a página carregar (≈83 MB para 400 linhas).
- A ordenação local calculava `to_lowercase`/`parse` por comparação; passou a calcular as chaves uma vez por linha.
- O modo texto clonava o resultado inteiro por quadro; passou a copiar só as colunas.
- Rolar sem parar via `scroll_to` a cada quadro (navegação por teclado) cresce ≈100 KB por quadro dentro do egui_extras; não aparece com rolagem normal e não foi alterado.
- **Não reproduzido:** o salto de ≈140 MB para ≈1 GB no cenário real. Hipótese a verificar com os dados do usuário: o fallback de paginação em `db.rs` (`run_inner`) executa a consulta inteira e descarta linhas quando a consulta paginada falha, o que carrega a tabela toda na memória.
