## Why

Projetos e pastas são a principal forma de versionar e fazer backup de conexões e scripts, mas o app não tem como criar um projeto ou uma pasta, nem editar os existentes, e os dados antigos (projeto padrão) ficam fora de qualquer menu. Além disso, três problemas do uso diário atrapalham: fechar a última aba encerra o app, uma consulta demorada não pode ser cancelada, e a memória continua alta depois de fechar scripts e fazer outras consultas. Por fim, trocar para Auto commit sem aviso pode gravar alterações sem o usuário perceber.

## What Changes

- **Projetos e pastas na interface**: *Arquivo → Novo projeto…* (nome e pasta) e *Abrir projeto…* (pasta existente ou clonada); o projeto padrão passa a aparecer na árvore e pode ser **convertido em projeto de pasta** (move conexões e scripts para a pasta escolhida); conexões podem ser movidas entre projetos; nas conexões, criar, renomear e excluir **pastas de organização** pela árvore (hoje só existe o campo de texto no diálogo da conexão).
- **Scripts vinculados ou soltos**: todo script pertence a um projeto ou é "solto" (pasta global de scripts). Novo script pergunta/usa o contexto atual (projeto selecionado, ou solto) e um script pode ser movido entre projeto e soltos pelo menu.
- **Tela vazia**: fechar a última aba não encerra o app; mostra uma tela em branco com atalhos (novo script, nova conexão, abrir projeto). Se a causa for um erro (panic) e não um fechamento, ela é reproduzida e corrigida.
- **Cancelar consulta**: botão Cancelar (e Esc) enquanto a consulta roda, para SQLite, PostgreSQL, MySQL, SQL Server e MongoDB, mais um tempo limite opcional configurável por preferência.
- **Memória liberada**: resultados de consultas anteriores, abas fechadas e caches deixam de ser retidos; a causa (retenção lógica e/ou memória que o alocador não devolve ao sistema, agravada por uma thread nova a cada execução) é medida antes de corrigir.
- **Aviso ao ir para Auto commit**: confirmação ao trocar de Manual/Smart para Auto (explicando que cada instrução passa a ser confirmada na hora e que pendências serão confirmadas), com opção "não perguntar de novo".

## Capabilities

### New Capabilities
- `project-management`: criar, abrir, renomear, mover conexões, converter o projeto padrão e gerenciar pastas de organização pela interface.
- `script-placement`: scripts vinculados a um projeto ou soltos, e mover entre eles.
- `empty-workspace`: tela vazia ao fechar a última aba, sem encerrar o app.
- `query-cancellation`: cancelamento de consultas em execução e tempo limite opcional.

### Modified Capabilities
- `result-grid`: liberação de memória de resultados e caches ao fechar ou substituir consultas.
- `transaction-modes`: aviso de confirmação ao mudar para Auto commit.

## Impact

- Código: `src/app.rs` (menus, árvore, tela vazia, aviso), `src/dialogs.rs` (novo projeto, abrir projeto, converter, pastas, mover script, aviso), `src/store.rs` (mover conexões/scripts, converter projeto padrão, pasta de organização), `src/db.rs` e `src/drivers.rs` (alça de cancelamento por sessão, tempo limite), `src/shared.rs` (`spawn_job`, `Job`), `src/tabs.rs` (botão Cancelar, liberação de resultados), `src/main.rs` (alocador).
- Possível nova dependência para devolver memória ao sistema (alocador) — decisão em `design.md`.
- **Sobreposição**: absorve as tarefas 4.4, 4.5 e 4.6 de `add-projects-and-workspace-polish` (criar/abrir projeto, árvore por projeto, git do projeto) e usa o menu de projeto de `project-menu-grid-stability-and-release-pipeline`. Aplicar depois dessa última (que já está no `main`).
- Dados: converter o projeto padrão move arquivos do usuário; exige confirmação e não apaga o original antes de gravar a cópia.
