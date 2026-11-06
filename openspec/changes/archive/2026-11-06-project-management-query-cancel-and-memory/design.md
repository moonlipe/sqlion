# Design

## Context

Veja proposal.md (Why). Estado observado no código:

- **Projetos**: `store.rs` já tem `Project`, `create_project`, `open_project`, `close_project`, `write_project`, `rename_project`, `delete_project_folder`, e a árvore mostra nós de projeto (mudança `project-menu-grid-stability-and-release-pipeline`), mas **nenhuma tela chama** `create_project`/`open_project`. O projeto padrão (`ConnConfig.project == ""`) não é um nó. A "pasta" de organização é só `ConnConfig.folder`, editada como texto no diálogo da conexão (`dialogs.rs`).
- **Scripts**: `next_script_path` sempre usa a pasta global `scripts_dir`; scripts de projeto só são abertos se já existirem em `<projeto>/scripts`. A chave de vínculo script→conexão é `script_key` (prefixo `id-do-projeto/` fora do padrão).
- **Última aba**: `App::update` já mostra `welcome(ui)` quando `tabs` está vazio e `close_tab` não envia `ViewportCommand::Close`. Pelo código, fechar a última aba **não deveria** encerrar o app; o relato indica um panic (ou fechamento por outro caminho, ex.: `Ctrl+W` com `active` fora do intervalo, ou o `Drop` ao fechar). Precisa de reprodução antes de qualquer correção.
- **Execução**: `spawn_job` cria **uma thread nova por execução**, que trava `Mutex<Session>` por todo o job; `Job` só tem `rx`, `total`, `done`, `started`. Não existe cancelamento. `Session.conn` é `Sqlite | Pg | My | Ms | Mongo`.
- **Memória**: cada `SqlTab` guarda `results: Vec<GridState>` (cada um com o `QueryResult` inteiro, `row_state`, `undo`, `cell_rects`), `output`, `stats`. Fechar a aba remove o `Tab`, então a retenção lógica parece pequena; o relato "continua com a mesma memória depois de fechar o script" aponta para memória que o alocador (glibc) não devolve ao sistema, agravada por threads de job com arenas próprias, e/ou caches (`Shared.tables`, `tmeta`, galleys do egui, texturas). Nada disso está medido. A mudança anterior já cortou o texto das células (300 caracteres) antes do layout.
- **Troca de modo**: `set_tx_mode` (`app.rs`) aplica direto; `cycle_tx_mode` e o diálogo da conexão também mudam o modo sem aviso. `set_mode(Auto)` já faz commit das pendências.

## Goals / Non-Goals

**Goals:**
- Criar/abrir/editar projetos e pastas pela interface; converter o projeto padrão sem risco de perda.
- Script sempre com um dono claro (projeto ou solto) e movível.
- App nunca fecha por fechar a última aba.
- Cancelar consultas de verdade (no servidor) nos 5 bancos, com tempo limite opcional.
- Memória que volta ao patamar após fechar/substituir resultados, comprovada por medição.
- Confirmação ao ir para Auto commit.

**Non-Goals:**
- Seletor de arquivos nativo (o caminho é digitado/colado; um botão "Procurar" pode vir depois com `rfd`).
- Sincronização com servidor git (push/pull); continua só init/status/commit.
- Paginação em disco ou streaming de resultados.
- Cancelar operações que não são consultas (exportação, importação).

## Decisions

1. **Diálogos de projeto**: `Dialog::NewProject { name, dir, msg }` (sugere `~/Sqlion/<nome>`), `Dialog::OpenProject { dir, msg }`, `Dialog::ConvertDefault { dir, msg }`. Entradas em *Arquivo*, no botão `+` do navegador e na tela vazia. Reaproveitam `Store::create_project/open_project`. Alternativa: um único diálogo "Projetos…" com lista; descartada por mais código sem ganho.
2. **Projeto padrão como nó** "Projeto padrão" (id `""`), com o mesmo menu de projeto, mas sem Renomear/Remover/Excluir, mais **Converter em projeto de pasta…**. Alternativa: migrar automaticamente no primeiro uso; descartada porque move arquivos do usuário sem pedir.
3. **Conversão em duas fases**: (a) criar o projeto na pasta e gravar o arquivo com as conexões; (b) **copiar** os `.sql` do diretório global; (c) só após tudo gravado, trocar `c.project` das conexões, remapear `script_conns`, e remover os `.sql` originais. Qualquer falha antes de (c) desfaz a cópia e deixa o padrão intacto. Teste com falha injetada (pasta sem permissão).
4. **Mover conexão** = trocar `ConnConfig.project` e regravar os dois arquivos de projeto; a senha local fica em `secrets.json` por id e não se move. **Mover script** = `rename` (ou cópia+remoção entre volumes) para `<destino>/scripts`, atualizando `script_conns` (`script_key`) e `SqlTab.file`.
5. **Pastas de organização**: continuam sendo `ConnConfig.folder` (sem objeto novo); criar pasta vazia exige existir sem conexões, então `Project` ganha `folders: Vec<String>` (no arquivo do projeto, `#[serde(default)]`, `format_version` 1 compatível) e, para o padrão, uma lista no workspace. Excluir pasta zera `folder` das conexões. Alternativa: pastas só implícitas (existem enquanto houver conexão); descartada porque o usuário quer "simplesmente criar uma pasta".
6. **Novo script**: `next_script_path` passa a receber o destino: projeto do `nav_sel` (projeto, conexão ou script selecionado) → projeto da conexão atual → solto. Menu "Novo script em…" lista projetos e "Solto". O título mostra `[projeto]` quando vinculado.
7. **Última aba**: primeiro reproduzir com teste que fecha todas as abas de várias formas (inclusive Welcome, `Ctrl+W`, "Fechar as outras") sobre o `App`, e com `RUST_BACKTRACE`; corrigir a causa real. Em paralelo, a tela vazia vira um estado explícito (`empty_workspace`) com atalhos, em vez de reaproveitar `welcome`, e `close_tab` nunca depende de `tabs[active]` sem checar limites.
8. **Cancelamento**: cada `Session` guarda um `CancelHandle` criado na conexão e **copiável sem travar a sessão** (como `TxSnap`). Por driver: SQLite `Connection::get_interrupt_handle()`; PostgreSQL `Client::cancel_token()` + `cancel_query`; MySQL guarda `connection_id()` e abre uma conexão auxiliar curta para `KILL QUERY <id>`; SQL Server guarda `@@SPID` e usa conexão auxiliar com `KILL <spid>`; MongoDB não tem cancelamento síncrono simples, então usa `maxTimeMS` derivado do tempo limite e, no cancelamento manual, `killOp` via `currentOp` filtrado por comentário da operação (alternativa: abandonar a thread e reconectar a sessão). `Job` ganha `cancel: Arc<AtomicBool>` checado entre instruções e um `CancelHandle`; `Job::cancel()` seta a flag e aciona o handle. Erro de interrupção vira `Cancelada` (não erro de SQL). O fallback de último recurso é descartar e reabrir a sessão, com a transação aberta perdida, e avisar.
9. **Tempo limite**: preferência `query_timeout_secs` (0 = sem limite); um temporizador no UI chama `Job::cancel()` e marca o motivo "limite de N s". Sem usar `statement_timeout` do servidor, para valer igual em todos os bancos.
10. **Memória, em duas frentes, após medir**: (a) *lógica*: ao substituir resultados (`results.clear()`), fechar aba ou projeto, soltar explicitamente `GridState`, `undo` e buffers, e não guardar `QueryResult` duplicado no log/histórico; (b) *alocador*: no Linux, chamar `malloc_trim(0)` depois de fechar aba/substituir resultados grandes e limitar arenas (`M_ARENA_MAX`) ou trocar o alocador global por `mimalloc`; e usar **uma thread de execução por sessão** (reaproveitada) em vez de uma thread nova por job, para não criar arenas novas. A escolha entre `malloc_trim`/`mimalloc` sai da medição (RSS depois de 20 consultas grandes + fechar a aba). Alternativa: não mexer no alocador e só tratar a retenção lógica; só se a medição mostrar que basta.
11. **Aviso de Auto commit**: função única `request_tx_mode(conn, mode)` usada pelos botões, pelo atalho de alternância e pelo diálogo da conexão; só quando `mode == Auto` e o modo atual não é Auto abre `Dialog::ConfirmAutoCommit { conn, pending_stmts, pending_rows, dont_ask }`. Preferência `confirm_auto_commit` (padrão ligada). O diálogo da conexão e as preferências mostram um aviso fixo ao escolher Auto.

## Risks / Trade-offs

- [Converter o projeto padrão move arquivos do usuário] → cópia primeiro, remoção só no fim, confirmação com contagens, teste com falha injetada.
- [`KILL` no SQL Server/MySQL exige uma segunda conexão e permissão] → falha reportada ao usuário; fallback de reabrir a sessão.
- [Cancelar no meio de transação pode deixar estado inconsistente (PostgreSQL aborta a transação)] → depois do cancelamento, ler o estado da sessão; se a transação foi abortada pelo servidor, mostrar que só Rollback é possível.
- [Cancelamento do Mongo é o menos confiável] → documentar a limitação, usar `maxTimeMS` e o fallback.
- [`mimalloc` aumenta binário e o tempo de build; `malloc_trim` só existe em glibc] → decidir pela medição; usar `cfg(target_os = "linux", target_env = "gnu")` para o que for específico.
- [Duas mudanças tocam `app.rs`/`dialogs.rs`] → aplicar depois de `project-menu-grid-stability-and-release-pipeline` (já em `main`); tarefas 4.4–4.6 da `add-projects-and-workspace-polish` ficam cobertas aqui e devem ser marcadas como absorvidas ao arquivar.
- [`folders` no arquivo do projeto muda o formato] → campo opcional com padrão vazio; versões antigas ignoram.

## Migration Plan

Sem migração automática: o projeto padrão continua funcionando como hoje até o usuário converter. O campo `folders` e as preferências novas têm padrão. Rollback: reverter os commits; projetos criados continuam sendo pastas válidas com arquivo legível por versões antigas.

## Open Questions

- Mongo: aceitar o fallback de reabrir a sessão no cancelamento manual, ou investir no `killOp`? Pode ser decidido na implementação sem mudar as specs.
- Quando "Novo script" é criado sem contexto e existem projetos, perguntar sempre ou usar "solto" por padrão? Assumido: usar o contexto e cair em "solto", com "Novo script em…" para escolher.

## Resultado da investigação da última aba (tarefa 1.1)

- Fechar todas as abas pelo mesmo caminho do botão da aba (`close_tab`) no app real **não** encerra o app: a tela de boas-vindas volta (verificado com `DBR_CLOSE_ALL` e captura de tela).
- Defeito encontrado: "Fechar as outras" alterava `self.tabs` dentro do laço `for i in 0..self.tabs.len()` de `tab_bar`; a iteração seguinte indexava `self.tabs[i]` fora dos limites, o que causa panic e encerra o app. Não foi possível acionar o menu de contexto automaticamente, então a reprodução é por leitura do código; a correção aplica a ação depois do laço (`close_other_tabs`) e passa a salvar scripts alterados, como `close_tab` faz.
- A tela vazia agora é um estado próprio (`empty_workspace`) com Novo script, Nova conexão e Tela inicial.

## Resultado do cancelamento (grupo 3)

- SQLite, PostgreSQL, MySQL e SQL Server foram testados contra servidores reais (podman) com `SLEEP`/`pg_sleep`/`WAITFOR` e recursão infinita: o cancelamento volta em < 1 s e a mesma sessão aceita nova consulta. No SQL Server a requisição é descartada e a sessão é reaberta antes da próxima instrução (uma transação aberta se perde); no MySQL `SLEEP` interrompido devolve resultado normal, não erro.
- MongoDB: não há interrupção no servidor; a aba para de esperar e avisa que a execução pode continuar. Sem teste automático (não havia servidor Mongo).
- Achado fora do escopo: enquanto uma consulta roda, qualquer ação da interface que precise do cadeado da sessão (carregar tabelas do navegador, abrir uma tabela na mesma conexão) congela a interface até a consulta terminar. O botão Cancelar funciona porque não usa o cadeado, mas isso merece uma tarefa própria (usar `try_lock`/carregar em segundo plano).

## Resultado da medição de memória (grupo 4)

Benchmark `shared::tests::measure_memory_across_queries` (20 consultas de ~40 MB, uma thread de job por consulta, 3 resultados mantidos como abas abertas, depois tudo descartado):

| Momento | RSS |
|---|---|
| início | 22 MB |
| durante as consultas (estável, sem crescer) | 178 MB |
| depois de descartar todos os resultados | **88 MB** |
| depois de `malloc_trim(0)` | **25 MB** |

- A retenção **não é lógica**: `GridState` libera tudo ao ser descartado (teste `replacing_results_does_not_accumulate_memory`, com alocador contador por thread: 20 substituições mantêm ~1 resultado e descartar libera o restante). O que fica é a memória que o glibc não devolve ao sistema, que é exatamente o "continua com a mesma memória depois de fechar o script".
- Correção: `shared::release_memory()` (`malloc_trim(0)` em Linux/glibc; sem efeito nas outras plataformas) chamada ao fechar aba, fechar as outras, fechar projeto, remover uma aba de resultado, antes de guardar os resultados de uma nova execução e ao recarregar uma tabela.
- Não foi preciso trocar de alocador nem reaproveitar a thread por sessão (Decisão 10 ficou só com o `malloc_trim`). Se o RSS de uso real ainda ficar alto, o próximo passo é `mimalloc`.
