# Proposal

## Why

O grid de resultados e o controle de transações têm bugs que impedem o uso diário: não dá para navegar em tabelas largas, a seleção múltipla não funciona, o painel de valores se comporta mal e o modo de commit manual nunca chega a valer. Isso precisa ser corrigido antes de acrescentar novas funcionalidades.

## What Changes

- Corrigir o painel de valores da célula (`value_panel`), que apresenta comportamento incorreto.
- Adicionar barra de rolagem horizontal ao grid, com rolagem automática até a célula selecionada.
- Seleção múltipla de linhas e células: clique e arraste, Shift+clique e Ctrl+clique.
- Copiar e colar em massa: copiar qualquer seleção como TSV e colar blocos multi-linha/multi-coluna.
- Corrigir o commit manual: com auto-commit desligado, as alterações devem ficar pendentes até Commit/Rollback explícito.
- Novo modo **Smart commit**: consultas de leitura (SELECT etc.) são executadas direto; comandos que alteram dados abrem transação manual.
- Clique no título da coluna seleciona a coluna inteira; a ordenação passa a ser um botão separado no cabeçalho (hoje o clique no título ordena).
- **BREAKING (UX)**: o clique no título deixa de ordenar.
- A configuração de conexão passa de checkbox "Auto-commit" para um modo com três valores: Auto, Manual, Smart.

## Capabilities

### New Capabilities
- `result-grid`: seleção (linhas, células, colunas), rolagem, copiar/colar em massa, ordenação e painel de valores do grid de resultados.
- `transaction-modes`: modos de transação Auto, Manual e Smart por conexão, e semântica de commit/rollback.

### Modified Capabilities

(nenhuma; ainda não há specs em `openspec/specs/`)

## Impact

- `src/grid.rs`: cabeçalho, rolagem, seleção, teclado, copiar/colar, painel de valores.
- `src/db.rs`: `Session` (modo de transação, `begin_if_needed`), `ConnConfig`.
- `src/app.rs`, `src/dialogs.rs`, `src/store.rs`, `src/constants.rs`: toolbar de transação, diálogo de conexão, preferências, tipo de conexão (`ConnType`).
- `src/editing.rs`, `src/tabs.rs`: gravação das edições do grid sob o modo de transação ativo.
- Compatibilidade: conexões salvas com `auto_commit: bool` precisam migrar para o novo modo.
