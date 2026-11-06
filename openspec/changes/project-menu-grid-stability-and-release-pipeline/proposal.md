## Why

O uso real do Sqlion trouxe quatro problemas e duas lacunas de distribuição: o projeto na árvore não tem como ser gerenciado (excluir, renomear), o botão Commit do modo Manual não mostra o número de transações pendentes de forma clara, reduzir muito o grid derruba o app, e rolar resultados grandes faz o consumo de RAM explodir (≈140 MB com 200 linhas para ≈1 GB ao buscar as 200 seguintes). Além disso, não há pipeline para compilar e publicar versões para Linux, Windows e macOS.

## What Changes

- Clique direito em um projeto na árvore do navegador abre o menu de gerenciamento: renomear, remover da lista (mantém a pasta), excluir a pasta (com confirmação), abrir a pasta, copiar o caminho, novo script e novo diretório/conexão no projeto, e git do projeto.
- O botão Commit do modo Manual/Smart passa a mostrar o número de instruções (transações) pendentes e o de registros afetados no próprio rótulo, e não só no tooltip.
- O grid (e o painel de valor ao lado dele) deixa de causar panic quando a área disponível fica muito pequena; abaixo do mínimo útil mostra um aviso ou se reduz a um tamanho seguro.
- O consumo de memória ao paginar resultados passa a crescer de forma proporcional aos dados: o grid não recria cópias do resultado inteiro nem acumula caches por quadro; busca da causa por medição antes de corrigir.
- Novo workflow de GitHub Actions que, ao empurrar uma tag `vX.Y.Z`, compila o Sqlion para Linux (x86_64 e arm64), Windows (x86_64) e macOS (arm64 e x86_64), empacota, gera o código-fonte zipado, gera checksums e cria o GitHub Release automaticamente com notas geradas. Um workflow de CI roda `cargo test` e `cargo build` em PRs e em `main`.

## Capabilities

### New Capabilities
- `project-context-menu`: menu de contexto de gerenciamento de projeto na árvore do navegador.
- `release-pipeline`: CI e publicação automática de releases multiplataforma por tag.

### Modified Capabilities
- `transaction-modes`: rótulo do botão Commit exibe instruções e registros pendentes.
- `result-grid`: estabilidade em áreas pequenas e consumo de memória limitado ao paginar.

## Impact

- Código: `src/app.rs` (árvore, barra de ferramentas), `src/dialogs.rs` (diálogos de renomear/excluir projeto), `src/store.rs` (renomear/excluir projeto), `src/grid.rs` (layout do painel de valor, reindex, caches), `src/tabs.rs` (paginação).
- Novos arquivos: `.github/workflows/ci.yml` e `.github/workflows/release.yml`.
- Dependência da mudança em andamento `add-projects-and-workspace-polish` (tarefas 4.4 e 4.5, projetos e árvore por projeto): o menu daqui assume o modelo `Project` e a árvore por projeto dessa mudança; aplicar depois delas ou junto.
- Sem alteração de formato de dados. O primeiro release exige criar o repositório com permissão `contents: write` para o `GITHUB_TOKEN`.
