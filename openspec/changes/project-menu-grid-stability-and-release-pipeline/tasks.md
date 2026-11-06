## 1. Menu de gerenciamento de projetos

- [x] 1.1 Pré-requisito: confirmar que as tarefas 4.4/4.5 de `add-projects-and-workspace-polish` (modelo `Project` e nó de projeto na árvore) estão aplicadas; se não, aplicá-las antes. Verificar com `cargo build` e o nó de projeto visível na árvore.
- [x] 1.2 Em `src/store.rs`, adicionar `rename_project` (nome vazio/repetido recusado, grava o arquivo do projeto) e `delete_project_dir` (recusa caminhos sem `sqlion-project.json`, raiz e `$HOME`); verificar com testes em pastas temporárias, inclusive as recusas.
- [x] 1.3 Criar `project_menu` em `src/app.rs` com as ações Renomear, Remover da lista, Excluir a pasta, Abrir a pasta, Copiar o caminho, Novo script, Nova conexão e Versionamento, desabilitando destrutivas no projeto padrão; verificar por captura de tela do menu.
- [x] 1.4 Diálogos `RenameProject` e `DeleteProject` em `src/dialogs.rs`, com caminho, contagens e aviso de abas com alterações não gravadas; ao excluir, fechar as abas dependentes; verificar com teste de remover vs. excluir (pasta permanece vs. some) e captura de tela.
- [x] 1.5 Helper multiplataforma para "Abrir a pasta" (`xdg-open`/`open`/`explorer`) usado em `app.rs` e `dialogs.rs`; verificar com `cargo build` e `cargo check --target x86_64-pc-windows-gnu` quando disponível, ou revisão do `cfg`.

## 2. Commit com número de transações

- [x] 2.1 Reproduzir: em modo Manual, executar 2 UPDATEs e gravar uma edição do grid e observar o rótulo; registrar o que o contador mostra hoje. Verificar anotando o resultado no PR.
- [x] 2.2 Mostrar `Commit · N instr · M reg` no botão (`src/app.rs`) e garantir que gravações do grid incrementam o contador na origem; verificar com teste do snapshot (duas instruções com 3 e 2 registros → 2 e 5) e do caso de gravação do grid, e por captura de tela.

## 3. Estabilidade do grid em áreas pequenas

- [x] 3.1 Reproduzir o crash (reduzir o painel dividido e o grid ao mínimo, com e sem painel de valor) e anotar a mensagem do panic. Verificar com o teste que falha antes da correção.
- [x] 3.2 Extrair o cálculo de retângulos de `grid::show` para um helper sem valores negativos e esconder o painel quando faltar espaço; verificar com testes unitários (larguras 0, negativas, 100, 400) e um teste de `show` com tamanhos mínimos que não faz panic.
- [x] 3.3 Revisar os demais `width() - N`/`height() - N` e `allocate_exact_size`/`add_sized` em `grid.rs`, `tabs.rs` e `app.rs` para o mesmo problema; verificar com `grep` e correção de cada ocorrência com `.max(0.0)`.

## 4. Memória ao paginar

- [x] 4.1 Medir RSS/alocações ao carregar 200 e 400 linhas e rolar, com e sem painel de valor, filtro e ordenação (script ou teste que alimenta `GridState` com linhas sintéticas e conta bytes); registrar a causa em `design.md` (Decisão 5). Verificar com os números antes/depois no PR.
- [x] 4.2 Corrigir a causa medida (candidatas: `g.qr.clone()` por quadro, `reindex` repetido com `to_lowercase` por célula, `cell_rects`/`row_state`/undo com cópias); verificar com teste de regressão que a memória de 400 linhas é < 2,5× a de 200 e que rolar sem buscar não aumenta o uso.
- [x] 4.3 Garantir que `reindex` só roda quando necessário na extensão de página (`tabs.rs` `rows.extend`) e que a chave de ordenação é calculada uma vez por coluna; verificar com teste contando chamadas/tempo de `reindex` em 5 páginas.

## 5. CI e release multiplataforma

- [ ] 5.1 Criar `.github/workflows/ci.yml` (`cargo test`, `cargo build` no Linux com dependências do `eframe`, em PR e `main`); verificar com `actionlint` e o primeiro run verde.
- [ ] 5.2 Criar `.github/workflows/release.yml` com gatilho de tag `v*.*.*` e `workflow_dispatch`, permissões `contents: write` e a matriz de 5 alvos (Linux x86_64 e arm64, Windows x86_64, macOS arm64 e x86_64); verificar com `actionlint` e um run manual que gera os artefatos.
- [ ] 5.3 Passo de verificação tag = versão do `Cargo.toml`, empacotamento (`sqlion-<versão>-<alvo>.tar.gz/.zip` com binário, LICENSE e README) e `SHA256SUMS`; verificar baixando os artefatos de um run e conferindo nomes, conteúdo e checksums.
- [x] 5.3b Adicionar `.gitattributes` com `export-ignore` (`openspec/`, `.github/`, `docs/` se interno) e gerar no job `release` o `sqlion-<versão>-source.tar.gz` e `.zip` com `git archive`, incluídos no `SHA256SUMS`; verificar extraindo o zip, conferindo que não há `target/`, `.git/` nem `.env`, e que `cargo build --release` compila a partir dele.
- [ ] 5.4 Job `release` que cria o GitHub Release com `--generate-notes` somente se todos os builds passarem (5 alvos) e não roda em `workflow_dispatch`; verificar empurrando uma tag de teste (`v0.0.0-test`) num fork ou apagando release e tag depois.
- [ ] 5.5 Adicionar `#![windows_subsystem = "windows"]` condicional ao build release em `src/main.rs`; verificar que o executável Windows abre sem console e que o Linux/macOS não mudam.
- [x] 5.6 Documentar no README o processo de lançamento (atualizar `Cargo.toml`, tag, push), os pacotes gerados, como compilar a partir do código-fonte (Rust, dependências de sistema por plataforma) e o aviso do Gatekeeper no macOS; verificar relendo o passo a passo contra os workflows.

## 6. Integração

- [ ] 6.1 Rodar `cargo test` e `cargo build --release` e percorrer manualmente os cenários das specs (menu de projeto, Commit, grid minúsculo, paginação com medição de RAM); anotar o que ficou de fora.
