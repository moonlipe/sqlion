# Mecânica do DBeaver usada nesta versão em Rust

## Sobre a decompilação (tentada de novo nesta rodada)
Repeti `core_target_open`, `native_strings_find` e `native_function_pseudocode`: o alvo abre como "desconhecido" e as duas
ferramentas nativas respondem "formato não é um programa (magic desconhecida)". Continua não havendo binário do DBeaver para decompilar.

O alvo (`pom.xml`) é um arquivo de texto do Maven e o DBeaver aqui é só código-fonte: 7284 arquivos `.java`,
nenhum `.class`, nenhum executável. `core_target_open` aceita o arquivo mas o classifica como "desconhecido";
`native_functions_list` e `native_strings_find` respondem "formato não é um programa (magic desconhecida)";
o Ghidra também não consegue analisá-lo. Os únicos binários da pasta são `awt.injector-1.0.0.jar` (2 classes
de AWT, não usadas) e `jkiss_wmi.dll` (WMI do Windows). Portanto não existem endereços de função: no lugar deles
cada valor traz `arquivo:linha` do código Java, que é onde a regra está escrita. Os mesmos números estão em
`src/constants.rs`, que é o que o app usa.

Tudo abaixo foi lido no código; "estimado" = procurei e não achei.

## Resultados (ModelPreferences.initializeDefaultPreferences, linhas 308-409)
| Valor | Padrão | Fonte |
|---|---|---|
| Linhas buscadas por página (resultset.maxrows) | 200 | ModelPreferences.java:356 |
| Limitar via SQL (LIMIT) | desligado | ModelPreferences.java:368 |
| Linhas do dicionário (dictionary.max.rows) | 200 | ModelPreferences.java:314 |
| Delimitador de instrução | `;` | SQLConstants.java:31 |
| Modo de delimitador em branco | linha em branco + separador | ModelPreferences.java:332 |
| Remover `;` final ao executar | sim | ModelPreferences.java:333 |
| Parâmetros SQL ligados | sim | ModelPreferences.java:385 |
| Prefixo de parâmetro nomeado | `:` | SQLConstants.java:691 |
| Marca de parâmetro anônimo | `?` (desligado por padrão) | SQLConstants.java:690 |
| Prefixo de comando de controle | `@` | SQLConstants.java:695 |
| Variáveis `${var}` | ligadas | ModelPreferences.java:391 |
| Histórico (dias) | 90 | ModelPreferences.java:377 |
| Entradas por página no histórico | 200 | ModelPreferences.java:378 |
| Undo de edição de grade | 200 (máx 10000) | ResultSetPreferences.java:27-28 |
| Largura máxima padrão da coluna | 30 caracteres | ResultSetPreferences.java:26 |
| Binário: tamanho máximo da string | 32 | ModelPreferences.java:370 |
| Tempo de validação da conexão | 10000 ms | ModelPreferences.java:324 |
| Rollback em erro | desligado | ModelPreferences.java:316 |
| Auto-commit padrão | ligado (estimado: não achei a constante no código lido) | — |

## Formatos de data (DBConstants.java:107-111)
hora `HH:mm:ss`; data `yyyy-MM-dd`; timestamp `yyyy-MM-dd HH:mm:ss.SSS`.

## Portas padrão (plugin.xml dos drivers)
PostgreSQL 5432 (ext.postgresql/plugin.xml:377); MySQL/MariaDB 3306 (ext.mysql/plugin.xml:172).

## Exportação (data.transfer/plugin.xml)
- CSV (linhas 152-166): delimitador `,`, cabeçalho `top`, aspas `"`, escape `quotes`, quoteAlways desligado,
  escapeFormulas ligado, texto de NULL vazio, formatNumbers desligado, extensão csv.
- JSON (121), XML (86, doctype desligado), HTML (137-140: cabeçalho de tabela e de colunas ligados),
  Markdown (178-183: separador de cabeçalho ligado), TXT (219-234: lote 200, mostrar cabeçalho ligado, NULL oculto),
  SQL (195-207: 10 linhas por INSERT, linha antes das linhas ligada, palavras-chave em maiúsculas, nativo ligado),
  DbUnit (99-103: maiúsculas, `[NULL]`).
- Importação CSV (45-64): delimitador `,`, aspas `"`, amostras para tipo das colunas 100, tamanho mínimo 50.
- Nome do arquivo de saída `${table}_${timestamp}`: estimado (não achei a constante).

## Rede
Faixa de portas de túnel 10000-60000 (ModelPreferences.java:364). SSH não implementado.

## Escopo desta versão
SQLite, PostgreSQL e MySQL/MariaDB. Sem IA, painéis, GIS, tarefas agendadas, SSH, comparação de dados.

## Atalhos do editor SQL (ext.ui.editors.sql/plugin.xml, bloco de key bindings)
Alt+X executar script; Ctrl+Shift+E plano de execução; Ctrl+/ comentar linha; Ctrl+Shift+F formatar;
Ctrl+] novo script; Ctrl+Alt+Enter console; Alt+↑/↓ consulta anterior/seguinte (não implementado).
Ctrl+Enter (executar instrução) e Ctrl+\ (nova aba de resultado) são os atalhos conhecidos do DBeaver mas
não apareceram nesse bloco: marcados como estimados.

## Regras de execução implementadas (a partir do comportamento descrito acima)
- Instruções divididas por `;` ou linha em branco; strings, comentários e blocos BEGIN…END de CREATE respeitados.
- SELECT/WITH sem LIMIT são paginados de 200 em 200 (próxima página / buscar tudo).
- `:nome` abre a janela de parâmetros; `${var}` e `@set var = valor` definem variáveis; `@echo` escreve na saída.
- Modo manual: BEGIN automático antes da primeira instrução que altera dados; commit/rollback pela barra.
- Edição de dados: UPDATE/DELETE usam a chave primária (ou todas as colunas se não houver); INSERT para linhas novas.

## Tipos de conexão (DBPConnectionType.java:44-101) — adicionado nesta rodada
| Tipo | Cor de fundo | Auto-commit | Confirmar execução | Confirmar alteração de dados |
|---|---|---|---|---|
| Desenvolvimento (dev) | 255,255,255 | sim | não | não |
| Teste (test) | 214,250,207 | sim | não | sim |
| Produção (prod) | 250,207,207 | **não** | **sim** | sim |
No app: o tipo é escolhido no diálogo da conexão (trocar o tipo ajusta o auto-commit), a conexão é pintada com a cor
no navegador, produção pede confirmação antes de executar instruções que alteram dados, e teste/produção pedem
confirmação antes de salvar linhas editadas. Fechamento de transações ociosas (1800/900/600 s) não foi implementado.

## Valores que estavam estimados e agora têm fonte
- Nome do arquivo exportado: `${table}_${timestamp}` — StreamConsumerSettings.java:125.
- Rótulo de NULL: `[NULL]` — DBConstants.java:71.
- Ctrl+Enter = executar instrução (`run.statement`, win32/gtk) e Ctrl+\ = executar em nova aba (`run.statementNew`) —
  ui.editors.sql/plugin.xml:836-839; no macOS é COMMAND em vez de CTRL.
- Auto-commit padrão: vem do tipo de conexão (DataSourceDescriptor.java:540-545 → DBPConnectionType.isAutocommit).

## Edição na grade (ResultSetPreferences.java:27 e comportamento do DBeaver)
- Desfazer: 200 níveis (DEFAULT_EDIT_UNDO_LEVEL = 200, máximo 10000) — `constants::UNDO_LEVELS`.
- Células alteradas ficam destacadas; linhas novas em verde; linhas marcadas para exclusão ficam riscadas em vermelho; nada é gravado até "Salvar".
- Gravação: UPDATE e DELETE usam a chave primária (ou todas as colunas selecionadas se a chave não estiver no resultado);
  INSERT grava só as células preenchidas (ou DEFAULT VALUES).
- O resultado do editor SQL é editável quando vem de uma única tabela, sem JOIN/GROUP BY/DISTINCT/UNION/subconsulta e só com colunas da tabela.
- Teclas: duplo clique ou F2 edita (seleciona o texto), digitar edita, Enter desce, Tab vai à direita, Esc cancela, Del marca exclusão,
  Ins insere, Ctrl+Z desfaz, Ctrl+C e Ctrl+V copiam e colam blocos.

## Bancos desta versão (pedido da pessoa)
SQLite, PostgreSQL, MySQL/MariaDB, SQL Server (protocolo TDS em Rust puro) e MongoDB (comandos `db.colecao.find({...})`).
Testados contra servidor real: SQLite e PostgreSQL. SQL Server e MongoDB compilam e têm testes de interpretação/erro, sem servidor para teste.
Portas lidas no código do DBeaver: SQL Server 1433 (ext.mssql/plugin.xml:184), TiDB 4000 (ext.tidb/plugin.xml:80),
StarRocks 9030 (ext.starrocks:38), Doris 9030 (ext.doris:31), OceanBase 2881 (ext.oceanbase:32),
YugabyteDB 5433 (ext.postgresql/plugin.xml:494), CockroachDB 26257 (:517), Redshift 5439 (:563).
MongoDB não existe na versão comunitária do DBeaver (é só PRO): a porta 27017 é a padrão do próprio MongoDB (estimado).
Atalhos no diálogo de conexão: CockroachDB, YugabyteDB, TimescaleDB, Redshift, Greenplum (protocolo PostgreSQL);
TiDB, OceanBase, StarRocks, Doris, SingleStore (protocolo MySQL). Oracle e DB2 ficam de fora: exigem bibliotecas de cliente proprietárias.

## Padrões de edição de grade lidos no código (DataEditorsPreferencesInitializer.java, linhas 38-103)
| Regra | Valor no DBeaver | Linha | No app |
|---|---|---|---|
| Nova linha entra depois da linha atual (RS_EDIT_NEW_ROWS_AFTER) | sim | 48 | `constants::NEW_ROWS_AFTER` — implementado |
| Buscar a próxima página ao rolar até o fim (RESULT_SET_AUTO_FETCH_NEXT_SEGMENT) | sim | 58 | `constants::AUTO_FETCH_NEXT_SEGMENT` — implementado |
| Booleano como caixa de seleção (RESULT_SET_SHOW_BOOLEAN_AS_CHECKBOX) | sim | 70 | implementado |
| Clique alterna o booleano (RESULT_SET_CLICK_TOGGLE_BOOLEAN) | sim | 72 | true → false → NULL (se a coluna aceita) → true; SpreadsheetPresentation.java:1499-1510 |
| Duplo clique abre o editor na célula (RESULT_SET_DOUBLE_CLICK) | INLINE_EDITOR | 74 | implementado |
| Alinhar números e datas à direita | sim, sim | 91-92 | implementado |
| Linhas zebradas / realce da linha selecionada | sim, sim | 62-63 | implementado |
| Usar todas as colunas como chave (RS_EDIT_USE_ALL_COLUMNS) | não | 41 | chave primária quando existe; todas as colunas só se não houver |
| Bloquear edição sem chave (RS_EDIT_DISABLE_IF_KEY_MISSING) | não | 42 | edição permitida sem chave |
| Atualizar valor ao confirmar (RS_EDIT_AUTO_UPDATE_VALUE) | sim | 45 | sim |
| Confirmar (commit) ao aplicar edição | não | 46 | não: segue o modo de transação da conexão |
| Reler dados depois de salvar (RS_EDIT_REFRESH_AFTER_UPDATE) | sim | 49 | sim |
| Contagem automática de linhas | não | 59 | botão "Contar" |
| Texto: tamanho máximo da coluna | 255 | 103 | `constants::TEXT_MAX_COLUMN_SIZE` |
| Linhas adicionadas por vez (RESULT_SET_ROW_BATCH_SIZE) | 1 | 93 | 1 |
| Largura máxima padrão da coluna | 30 | 94 | já usado |
Não implementado: limite de texto do editor em linha (RS_EDIT_MAX_TEXT_SIZE = 100, linha 43; unidade não confirmada) —
o painel de valor edita textos longos.

## Objetos mostrados na árvore (pedido da pessoa; pastas lidas nos plugin.xml do DBeaver)
| Banco | Pastas por esquema | Pastas do banco/servidor | Fonte no DBeaver |
|---|---|---|---|
| PostgreSQL | Tabelas, Tabelas externas, Views, Views materializadas, Índices, Funções, Procedimentos, Funções agregadas, Sequências, Gatilhos, Tipos de dados | Extensões, Tablespaces, Roles | ext.postgresql/plugin.xml:141-333 |
| MySQL/MariaDB | Tabelas, Views, Índices, Procedimentos, Funções, Gatilhos, Eventos, Sequências (MariaDB) | Usuários, Engines, Charsets | ext.mysql/plugin.xml:26-151 |
| SQL Server | Tabelas, Views, Índices, Procedimentos, Funções, Sequências, Sinônimos, Gatilhos, Tipos de dados | Gatilhos do banco, Logins | ext.mssql/plugin.xml:50-164 |
| SQLite | Tabelas, Views, Índices, Gatilhos | — | (SQLite não define pastas próprias; usa as do banco genérico) |
| MongoDB | Coleções, Views | — | (não existe na versão comunitária) |
- Cada pasta carrega ao abrir; duplo clique (ou "Ver código-fonte") abre a definição no editor SQL
  (`pg_get_functiondef`, `pg_get_viewdef`, `SHOW CREATE ...`, `OBJECT_DEFINITION`, `sqlite_master.sql`).
- Funções e procedimentos têm "Gerar chamada" (CALL / EXEC / SELECT).
- Testado contra servidor real: SQLite e PostgreSQL (função, procedimento, sequência, gatilho, view materializada, tipo, extensão, role).
  As consultas de MySQL e SQL Server foram escritas a partir do catálogo de cada banco e não foram executadas (sem servidor).
- Ainda faltam, em relação ao DBeaver: filhos de tabela por tipo (restrições, regras, políticas, partições), pacotes, jobs e
  as pastas de administração de sessões/locks.

## Padrões do editor SQL lidos no código (SQLEditorPreferencesInitializer.java)
| Regra | Valor | Linha | No app |
|---|---|---|---|
| Erro em script (SCRIPT_ERROR_HANDLING) | STOP_ROLLBACK | 49 | para no primeiro erro e reverte a transação aberta — `constants::SCRIPT_ERROR_STOP_ROLLBACK` |
| Commit dentro de script (SCRIPT_COMMIT_TYPE) | NO_COMMIT | 47 | sem commit automático dentro do script |
| Timeout da instrução | 0 (sem limite) | 54 | sem limite |
| Conectar ao executar | sim | 60 | sim |
| Salvar ao fechar / ao alterar | sim / não | 65, 64 | salva ao fechar |
| Apagar script novo vazio (SCRIPT_DELETE_EMPTY) | DELETE_NEW | 74 | arquivo do script novo e vazio é apagado ao fechar |
| Autocompletar automático (ENABLE_AUTO_ACTIVATION, atraso) | sim, 0 ms | 85, 88 | abre sozinho depois de "." ou de 2 letras |
| Inserir proposta única automaticamente | sim | 89 | Ctrl+Espaço com uma só opção insere direto |
| Espaço depois da proposta | sim | 104 | sim |
| Casar por "contém" / ordenar alfabeticamente | sim / sim | 107 / 105 | sim / sim |
| Fechar aspas simples, duplas e parênteses | sim | 119-121 | ao digitar `(`, `[`, `{`, `'`, `"` o par é fechado; digitar o fechamento pula |
| Caixa das palavras-chave ao formatar (automática) | sim | 126 | segue a maioria do texto |
| Formatar só a consulta ativa | sim | 129 | sem seleção formata a instrução sob o cursor |
| Máximo de abas de resultado por consulta | 20 | 140 | 20 |
| Substituir a aba de resultado atual | sim | 150 | sim |
Não implementados (lidos, fora do escopo): dobra de código (151), marcar ocorrências (111-112), marcadores de problema (114),
bipe ao fim da consulta (139), modelo de script novo (161-162).

## Editar o código de um objeto (procedimento, função, gatilho, view)
- Abrir o código cria uma aba do editor SQL com o texto da definição. **Editar o texto não altera o banco.**
  O banco só muda quando o texto é executado (Alt+X executa o script inteiro, Ctrl+Enter a instrução sob o cursor); em conexão de
  produção o app pede confirmação antes, e com auto-commit desligado é preciso confirmar (Commit).
- O texto aberto já vem pronto para aplicar: PostgreSQL usa `CREATE OR REPLACE` (funções, procedimentos, views); MySQL, SQLite e
  PostgreSQL (gatilhos, índices, views materializadas) recebem um `DROP ... IF EXISTS` antes; SQL Server troca `CREATE` por `ALTER`.
- Corrigido: corpos entre `$$ ... $$` / `$tag$ ... $tag$` (PostgreSQL) e blocos `BEGIN ... END` não são mais cortados em várias
  instruções ao executar. Testado com um PostgreSQL real: a função editada passa a devolver o novo resultado no banco.

## Preferências
- As preferências são gravadas no mesmo instante em que mudam (não só ao fechar a janela); a janela avisa isso e mostra o arquivo.
- Cursor de mãozinha ao passar sobre botões, abas, itens da árvore e controles segmentados (`Visuals::interact_cursor` + `on_hover_cursor`).

## Formatador de SQL (SQLFormatterTokenized.java e IndentFormatter.java) — reescrito nesta rodada
Regras lidas e aplicadas (`sqltext::format_sql`):
- Recuo de 4 espaços (SQLFormatterConfiguration.java:45 `indentString`).
- SELECT, UPDATE, INSERT, DELETE, INTO, TRUNCATE: quebra linha antes (volta ao nível 0 fora de parênteses) e depois, aumentando o recuo (IndentFormatter.java:168-185).
- FROM, WHERE, GROUP BY, ORDER BY, HAVING, START WITH, CONNECT BY: quebra antes (recuo-1) e depois (recuo) (linhas 198-210).
- JOIN/LEFT/RIGHT/INNER/OUTER/FULL/CROSS/NATURAL: nova linha só no início da sequência, quando ela termina em JOIN (211-225, 470-506).
- ON: quebra depois (259-265); USING: linha com recuo+1 (266-268); SET após UPDATE: quebra antes e depois (245-258).
- AND/OR/WHEN/ELSE: nova linha; AND dentro de BETWEEN não quebra (231-244, 284-296); CASE aumenta o recuo e END o diminui (186-197).
- VALUES e LIMIT voltam um nível (226-230); UNION/INTERSECT/EXCEPT voltam dois e sobem um (276-283).
- Vírgula: quebra linha depois, exceto dentro de função/identificador seguido de "(", depois de IN e em restrições (108-123);
  "vírgula antes" desligada por padrão (ModelPreferences.java:395).
- Nenhuma quebra é inserida enquanto houver parêntese de função aberto (insertReturnAndIndent, linha 415).
- `;` zera o recuo e é seguido de linha em branco (linhas 125-128, 451-459).
- Espaços (SQLFormatterTokenized.java:119-163): sem espaço antes de "," e ";", entre nome de função e "(", dentro de "( x )" com um só item,
  entre nome e valor, entre símbolos; ORDER/GROUP/CONNECT + BY e START + WITH viram uma palavra só (267-285).
- Palavras-chave com a caixa pedida (maiúsculas por padrão, ou a que o texto já usa: SQL_FORMAT_KEYWORD_CASE_AUTO).
Diferenças deliberadas: `VALUES (...)` é tratado como função (vírgulas não quebram); quebras de linha originais que o formatador
não substitui viram um espaço; sem a lista completa de palavras-chave de cada dialeto (a lista do app é menor).
Teste: o SQL formatado devolve exatamente as mesmas linhas e colunas no SQLite e formatar duas vezes não muda nada.

## Como o app se conecta aos bancos (resposta à pergunta da pessoa)
Não há driver JDBC nem arquivo de driver para baixar: cada banco é acessado por uma biblioteca Rust que fala o protocolo do banco direto
pela rede (ou, no SQLite, embutida no programa):
| Banco | Biblioteca | Protocolo |
|---|---|---|
| SQLite | rusqlite (SQLite embutido) | arquivo local |
| PostgreSQL | postgres | protocolo do PostgreSQL |
| MySQL / MariaDB | mysql | protocolo MySQL (serve MySQL, MariaDB, TiDB...) |
| SQL Server | tiberius | TDS |
| MongoDB | mongodb | protocolo do MongoDB |
Ainda não há conexão por SSL/TLS nem túnel SSH.

### MySQL/MariaDB antigos
- A biblioteca recusa por padrão a senha de formato antigo (`mysql_old_password`, anterior ao MySQL 4.1), comum em bancos legados.
  O app agora tenta de novo, sozinho, com esse formato permitido quando o servidor o exige, e avisa na janela de teste e na barra de status.
- `localhost` também é tentado como `127.0.0.1` (servidor que só escuta em IPv4 recusa `::1`).
- O servidor pode ser digitado como `servidor:porta` (ex.: `localhost:3337`); vale para MySQL, PostgreSQL e SQL Server.
- `SET NAMES utf8mb4` cai para `utf8` em servidores antigos que não conhecem utf8mb4.
- Erros comuns viram mensagem em português (acesso negado = 1045, banco inexistente = 1049, conexão recusada).

## Scripts: pasta e versionamento
- Padrão: `~/.config/dbeaver-rust/scripts` (visível em Preferências ▸ Pasta dos scripts e ao passar o mouse em "Scripts" no navegador).
  A pasta pode ser trocada; scripts já salvos não são movidos.
- Nomes novos seguem o DBeaver (`Script.sql`, `Script-1.sql`...; SQLEditorUtils.java:258). Menu da pasta: novo script, versionamento, mudar a pasta,
  abrir a pasta. Menu de cada script: abrir, renomear, excluir, copiar caminho.
- Versionamento com o `git` instalado no computador: iniciar repositório, status, histórico e commit de todos os scripts (Arquivo ▸ Versionar scripts).
  Scripts novos ou alterados aparecem com um ponto na árvore. Enviar a um servidor (push) é feito no terminal.
- Cada script lembra a última conexão em que foi executado (`script_conns` em `workspace.json`) e reabre nela; um script novo começa na conexão em uso
  ou, sem nenhuma, na última usada para executar.
- Painel de valor: moldura opaca e campo somente leitura sem esmaecer (antes, em resultados não editáveis o texto ficava quase invisível).

### Causa do MariaDB/MySQL antigo que não conectava (diagnosticada no servidor real da pessoa, localhost:3337)
- O servidor é um **MySQL 5.0.51a**. O erro era `ERROR 1193: Unknown system variable 'socket'`: ao conectar em `localhost` a biblioteca
  pergunta `@@socket` para trocar para o socket Unix, e servidores antigos não têm essa variável. Corrigido desligando essa troca
  (`prefer_socket(false)`): a conexão sempre usa TCP.
- Verificado com as credenciais informadas, só com leituras: conexão, versão, lista de bancos, tabelas, metadados (colunas, índices, DDL) e `SELECT`.
  Pastas de objetos: procedimentos/funções/gatilhos respondem (0 no banco `master`); a pasta Eventos não existe no MySQL 5.0 e fica vazia.
- Também verificado num MariaDB 5.5.68 de teste (baixado, usado e apagado): consultas, paginação, chaves e índices, funções, procedimentos,
  gatilhos, código-fonte dos objetos, commit/rollback e a senha em formato antigo (`old_passwords=1`), inclusive a mensagem para senha errada.

## Comandos de controle do editor SQL (lidos nesta rodada)
Registro dos comandos: model.sql/plugin.xml:72-75 (`set`, `unset`, `echo`, `export`) e ui.editors.sql/plugin.xml:55 (`include`); prefixo `@` (SQLConstants.java:695).
| Comando | Regra no DBeaver | No app |
|---|---|---|
| `@set nome = valor` | exige `=`; o nome é identificador ou entre aspas; o valor tem `${outra}` substituída e fica como foi escrito (aspas incluídas) (SQLCommandSet.java:35-58) | igual, com as mesmas verificações e mensagens de erro |
| `@unset nome` | só o nome; texto a mais é erro (SQLCommandUnset.java:33-45) | igual |
| `@echo texto` | substitui variáveis e escreve na saída (SQLCommandEcho.java:36-41) | escreve na aba "Saída" |
| `@export {json}` | vale para a **próxima** consulta, que não é executada: abre a transferência de dados já preenchida. JSON: `type` (obrigatório: csv, json, xml, html, sql, markdown.table, txt, dbunit...), `consumer` (`outputFolder`, `outputFilePattern`), `processor` (opções do formato) (SQLCommandExport.java:36-48, SQLPragmaExport.java:64-112) | abre a janela de exportação preenchida; pasta padrão = pasta do usuário (StreamConsumerSettings.java:124), nome padrão `${table}_${timestamp}` |
| `@include arquivo` | procura ao lado do script atual, depois o caminho como digitado; erros "Empty input file", "File 'x' not found", "recursive inclusion"; executa o script incluído (SQLCommandInclude.java:55-103) | igual (as instruções do arquivo entram no lugar do comando) |
- O comando termina no fim da linha (sem `;`) e roda na ordem do script: uma variável criada por `@set` já vale na consulta seguinte.
- Diferenças: um erro num comando de controle cancela a execução inteira (no DBeaver as consultas anteriores já teriam rodado); `@export` só abre a
  janela (o DBeaver também só abre o assistente); a opção de IA (`@ai`) não existe aqui.
