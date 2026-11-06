## 1. Última aba e tela vazia

- [x] 1.1 Reproduzir o fechamento do app ao fechar a última aba (botão da aba, clique do meio, Ctrl+W, "Fechar as outras", aba Bem-vindo) com teste sobre `App` e `RUST_BACKTRACE`; registrar a causa em `design.md`. Verificar com o teste falhando antes da correção, ou documentando que não reproduz e o caminho real do relato.
- [x] 1.2 Corrigir a causa e criar o estado de tela vazia com atalhos (novo script, nova conexão, abrir projeto), sem depender de `tabs[active]` fora dos limites; verificar com os testes de 1.1 passando para todos os caminhos de fechamento e por captura de tela.

## 2. Aviso de Auto commit

- [x] 2.1 Criar `request_tx_mode` único e `Dialog::ConfirmAutoCommit` (modo, pendências em instruções e registros, "não perguntar de novo"), preferência `confirm_auto_commit`; usar nos botões, no atalho de alternância e no diálogo da conexão; verificar com testes de: sem pendências, com pendências, cancelar mantém modo e pendências, "não perguntar" só dispensa o caso sem pendências.
- [x] 2.2 Aviso fixo ao escolher Auto no diálogo da conexão e na preferência de modo padrão; verificar por captura de tela.

## 3. Cancelar consulta

- [x] 3.1 Criar `CancelHandle` por sessão (SQLite `interrupt_handle`, PostgreSQL `cancel_token`), `cancel: Arc<AtomicBool>` no `Job` e `Job::cancel()` com parada entre instruções; verificar com testes de cancelar uma consulta longa (ex.: CTE recursiva no SQLite) e de que a sessão aceita nova consulta depois.
- [x] 3.2 Cancelamento em MySQL (`KILL QUERY` por conexão auxiliar) e SQL Server (`KILL` do `@@SPID`); verificar com testes contra o servidor quando disponível (ignorados sem servidor) e teste unitário da montagem dos comandos.
- [x] 3.3 Cancelamento em MongoDB (`maxTimeMS` + `killOp` ou reabrir a sessão, conforme Decisão 8) e o estado de transação após o cancelamento (Postgres aborta); verificar com teste do fallback de reabrir a sessão e do contador de pendências.
- [x] 3.4 Botão Cancelar, Esc no editor e tempo decorrido na aba, mensagem "Consulta cancelada" e preferência `query_timeout_secs` com cancelamento por limite; verificar com teste do temporizador (limite curto) e captura de tela.

## 4. Memória de consultas anteriores

- [x] 4.1 Medir RSS ao executar 20 consultas grandes seguidas, fechar a aba e executar outra, com e sem painel de valor (benchmark `#[ignore]`), e registrar no `design.md` onde a memória fica retida (lógica vs alocador). Verificar com os números antes/depois.
- [x] 4.2 Liberar explicitamente resultados, `undo` e buffers ao substituir, fechar aba ou projeto, e eliminar duplicatas de `QueryResult` (log, estatísticas, histórico); verificar com teste de que os dados da aba fechada são soltos (contagem de `Drop`/`Weak`) e que 20 execuções não acumulam.
- [x] 4.3 Aplicar a medida de alocador escolhida pela medição (`malloc_trim` em Linux/glibc ao fechar aba, substituir ou remover resultados; a thread por sessão foi dispensada porque a medição mostrou que o trim basta); verificar rodando o benchmark de 4.1 e comparando o RSS depois de fechar a aba com o inicial.

## 5. Projetos e pastas

- [ ] 5.1 Diálogos Novo projeto e Abrir projeto (Arquivo, botão `+` do navegador, tela vazia), com validação de pasta; verificar com testes sobre pastas temporárias (nova, existente, já projeto, sem permissão, sem arquivo de projeto) e captura de tela.
- [ ] 5.2 Nó "Projeto padrão" na árvore com o menu de projeto sem as ações destrutivas; verificar com teste de que Remover/Excluir/Renomear ficam desabilitados e captura de tela.
- [x] 5.3 Converter o projeto padrão em projeto de pasta (cópia, troca de vínculos, remoção só no fim); verificar com testes de sucesso, de falha no meio (padrão intacto) e do vínculo script→conexão.
- [x] 5.4 Mover conexão entre projetos (menu da conexão), preservando id, senha local e vínculo de scripts; verificar com teste de ida e volta e de que os dois arquivos de projeto ficam corretos.
- [x] 5.5 Pastas de organização: campo `folders` no projeto (e no padrão), Nova pasta, Renomear, Excluir (conexões vão para a raiz) e mover conexão para pasta; verificar com testes de formato antigo sem `folders` e de excluir com conexões.
- [x] 5.6 Versionamento do projeto no menu (status, commit, `git init`); verificar com teste em repositório temporário e que abas com texto não gravado sobrevivem à recarga.

## 6. Scripts vinculados ou soltos

- [x] 6.1 `next_script_path` com destino (projeto selecionado → projeto da conexão → solto), menu "Novo script em…" e título com o vínculo; verificar com testes de cada origem de contexto.
- [ ] 6.2 Árvore com scripts sob o projeto e nó "Scripts soltos"; verificar por captura de tela e teste de listagem por projeto.
- [ ] 6.3 Mover script entre projeto e soltos pelo menu, atualizando aba aberta e `script_conns`, e recusando nome em conflito; verificar com testes de mover, conflito e aba aberta.

## 7. Arrastar e soltar

- [x] 7.1 Linhas da árvore arrastáveis (conexões e scripts), destinos destacados (projeto, pasta, scripts soltos) e movimento pelas mesmas regras do menu; verificar com teste do gesto de arrastar entre duas linhas, com teste de `place_connection` (projeto, pasta, raiz, padrão, inexistente) e pelos testes de `move_script`.

## 8. Integração

- [ ] 8.1 Rodar `cargo test` e `cargo build --release` e percorrer manualmente os cenários das specs (criar/abrir/converter projeto, fechar última aba, cancelar consulta longa em cada banco disponível, memória após fechar scripts, aviso de Auto); anotar o que ficou de fora e marcar 4.4–4.6 de `add-projects-and-workspace-polish` como absorvidas.
