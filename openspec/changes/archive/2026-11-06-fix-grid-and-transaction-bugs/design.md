# Design

## Context

Veja `proposal.md` para a motivação. Estado atual (ler antes de implementar):

- O grid usa `egui_extras::TableBuilder` em `grid.rs::show_grid`. O `TableBuilder` só rola na vertical; para rolar na horizontal ele precisa estar dentro de uma `ScrollArea::horizontal`. A coluna de número de linha é uma coluna normal, então não fica fixa.
- A seleção é `sel: Option<(row, col)>` + `multi_sel: HashSet<row>` + `anchor`; `select()` já trata Shift/Ctrl a partir de `clicked` e `row_clicked`, mas as respostas vêm de `row.col()` com `Sense::click()`, sem arraste, e só linhas inteiras entram em `multi_sel`.
- Copiar usa `Event::Copy`, mas só quando nenhum widget quer teclado (`egui_wants_keyboard_input`), e só com `multi_sel.len() > 1`; colar usa `GridState::paste` a partir de `sel`.
- O cabeçalho trata `resp.clicked()` como ordenação (`header_click`).
- A transação vive em `Session` (`auto_commit`, `tx_open`, `tx_count`). `begin_if_needed` só abre transação quando `!auto_commit` e a instrução não é de leitura. O diálogo de conexão guarda `auto_commit: bool` em `ConnConfig`, e `ConnType::auto_commit()` define o padrão.
- Causa do commit manual "sempre em auto": o teste `editing::tests::grid_edits_stay_pending_in_manual_mode` mostra que `Session` e `editing::apply` funcionam em modo manual. O diálogo de conexão já desconecta ao salvar, então o valor salvo vale na reconexão. Restam como causas prováveis (não verificáveis sem a interface): o botão da barra só alterava a sessão, sem persistir na `ConnConfig`, e o padrão `Dev` (auto) sobrescreve o campo ao trocar o tipo de conexão no diálogo. A change torna o modo explícito (`TxMode`), persiste a troca feita na barra e mostra o modo na barra e na barra de status; validar manualmente (tarefa 2.4).
- O painel de valores: causa visual confirmada por captura de tela (`DBR_VP=1 sqlion --screenshot`): o grid não rola na horizontal e ultrapassa a própria área, então passa por baixo do `Panel::right`. É a mesma causa do item 2. Correção aplicada: a área é dividida à mão (grid à esquerda, painel à direita com alça de arraste) e a tabela fica em `ScrollArea::horizontal`. Ainda pendente: `vp_buf` só é recarregado quando `vp_for` muda (não reflete desfazer/edição direta).

## Goals / Non-Goals

**Goals:**
- Cada bug reproduzido por um teste (unitário no `GridState`/`Session`, ou captura de tela para o que é puramente visual) antes da correção.
- Modelo de seleção que cubra células, linhas, colunas e retângulos, e que o copiar/colar use sempre.
- Um enum de modo de transação no lugar do booleano.

**Non-Goals:**
- Novas funcionalidades além das seis pedidas (filtros, edição em lote por SQL, etc.).
- Mudar o formato de exportação.
- Colar com criação automática de linhas novas além do fim do resultado (ficam para uma change futura; ver Decisions).

## Decisions

1. **Rolagem horizontal**: envolver o `TableBuilder` em `ScrollArea::horizontal().auto_shrink([false,false])` com largura mínima igual à soma das colunas, mantendo a rolagem vertical do `TableBuilder`. Alternativa: desenhar o grid todo à mão com `ScrollArea::both` e virtualização manual; descartada por custo e risco. Para rolar até a célula selecionada, usar `ui.scroll_to_rect` na célula quando `scroll_to_col` for pedido.
2. **Modelo de seleção** (implementado de forma mais simples: `sel` + `multi_sel` de linhas + `rect_sel` de blocos + `sel_cols` de colunas, com `selection_matrix()` como fonte única para copiar/colar; a coluna de número de linha não fica fixa na rolagem). Plano original: trocar `sel` + `multi_sel` por `Selection { cursor: (row,col), anchor: (row,col), cells: Rect | Set }`, com variantes de linha inteira e coluna inteira. Arraste usa `Sense::click_and_drag()` nas células e `pointer.hover_pos` para descobrir a célula sob o ponteiro (as respostas de `row.col()` não cobrem arraste entre células). Alternativa: manter `multi_sel` e acrescentar um retângulo; descartada por gerar dois modelos para copiar/colar.
3. **Copiar/colar**: sempre derivar o TSV da seleção (retângulo, linhas ou colunas); em colar, aplicar a partir do canto superior esquerdo da seleção, uma única entrada de desfazer. Se a seleção for maior que o bloco colado e o bloco for 1×1, repetir o valor em toda a seleção. O gancho de teclado deve funcionar mesmo quando o grid tem foco por clique (ver `egui_wants_keyboard_input`).
4. **Cabeçalho**: título seleciona a coluna; um botão pequeno (seta de ordenação) à direita faz o ciclo crescente → decrescente → nenhum, reutilizando `g.sort` e `GridAction::SortBy`.
5. **Painel de valores**: derivar o buffer de `(célula, versão do valor)`; recarregar o buffer quando o valor da célula mudar por fora e o usuário não tiver editado o buffer. Dimensionar a `ScrollArea` do painel com o espaço restante, não com constante.
6. **Modos de transação**: `enum TxMode { Auto, Manual, Smart }` em `ConnConfig` (com `#[serde(default)]` e migração de `auto_commit: bool`). `Session` guarda `mode`. `begin_if_needed` abre transação em `Manual` para qualquer instrução e em `Smart` só se `!is_read_only_sql(sql)`; `auto_commit()` vira função derivada. `ConnType` passa a devolver o modo padrão. A barra mostra os três modos e o contador de pendentes.
7. **Gravação do grid**: `editing::apply` passa a rodar sob o modo da sessão (sem `BEGIN/COMMIT` próprios), para que em Manual/Smart as edições fiquem pendentes. Alternativa: sempre confirmar edições do grid; descartada por contradizer o pedido.

## Risks / Trade-offs

- [Mudar o modelo de seleção atinge muitos pontos de `grid.rs` (1200 linhas)] → migrar por etapas, mantendo os testes existentes verdes a cada passo.
- [`is_read_only_sql` classifica `SET`, `BEGIN`, `PRAGMA` (inclusive gravações) como leitura] → revisar a lista para o modo Smart; PRAGMA com `=` e `SET` que mudam estado de sessão não abrem transação, comportamento aceitável, mas documentar.
- [CTEs com escrita (`WITH ... INSERT`) já têm tratamento parcial] → cobrir com teste.
- [Bugs que não reproduzo (painel de valores, commit manual) podem ter causa diferente da hipótese] → as tarefas começam por reprodução; se a hipótese falhar, ajustar a change com `/opsx:update` antes de seguir.
- [Mongo não tem transação neste app] → modos Manual/Smart não se aplicam; a UI deve desabilitá-los.

## Migration Plan

Conexões salvas: `auto_commit: true` → `Auto`; `false` → `Manual`. Campo antigo mantido na leitura (`serde(default)`), gravação já no novo formato. Sem rollback automático; reverter a change restaura o booleano porque a leitura continua aceitando-o.

## Open Questions

- Colar além do fim do resultado deve criar linhas novas? Assumido que não nesta change.
