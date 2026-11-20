# Tasks

## 1. Tokenizador e listas de nomes (`src/sqltext.rs`)

- [x] 1.1 Ampliar `KEYWORDS` (comandos/controle de fluxo T-SQL, PL/pgSQL, MySQL e tipos listados na spec `sql-highlighting`) e criar `FUNCTIONS` com as funções embutidas; trocar as buscas por `HashSet` estático (`OnceLock`); verificar com teste que `exec`, `deallocate`, `nvarchar` são palavra-chave e `execucao_id` não
- [x] 1.2 Adicionar `Tok::Function`, `Tok::Variable`, `Tok::QuotedIdent`; `@nome`, `@@nome` e `${…}` fora de string viram `Variable` (`:nome` continua `Param`, pintado como variável); `"…"`/`` `…` `` viram `QuotedIdent`; nome nas duas listas é `Function` só se seguido de `(`; verificar com testes de `tokenize` cobrindo `convert(varchar(10), getdate())`, `left join … left(x)`, `@@ROWCOUNT` e e-mail dentro de string
- [x] 1.3 Criar `str_var_spans(str_token) -> Vec<(Tok, a, b)>` que divide uma string em segmentos `Str`/`Variable` para `${nome}`; verificar com testes para `'arquivo_${data}.csv'` e `'{"a": 1}'`, e rodar `cargo test` confirmando que `split_statements`, `find_params` e `find_vars` não mudaram

## 2. Paleta e destaque (`src/theme.rs`, `src/tabs.rs`)

- [x] 2.1 Criar `theme::SqlCat` (12 categorias com nome estável e rótulo em português), `SqlPalette` e `default_sql_palette(dark)` com as cores da tabela do design; verificar com teste que as 12 cores padrão são distintas em cada tema
- [x] 2.2 Reescrever `tabs::highlight` para usar `SqlPalette`, segmentar strings com `str_var_spans` (fundo em `TextFormat.background`) e receber `Option<&HlNames>` para classificar `Ident` como tabela/coluna (regra do `.` do design) e `QuotedIdent` como `QuotedKnown`/`Quoted`; verificar com testes que montam o `LayoutJob` de `select c.nome from clientes c` (cores de `clientes`, `nome`, `c`) e de `select "Nome", "Outro" from "Clientes"` (azul-água para `"Nome"`/`"Clientes"`, laranja-amarelado para `"Outro"` e para todos com `names = None`)
- [x] 2.3 Implementar `HlNames` e o cache em `SqlTab` (chave: conexão, `meta_gen`, hash das tabelas referenciadas), usando `tables` do schema padrão e `tmeta` das tabelas citadas, com `want_tmeta` para as que faltam, sem bloquear; verificar com teste em `Shared` de memória (SQLite) que as colunas aparecem após o carregamento e que o layouter não chama o banco
- [x] 2.5 Colorir alias de colunas (`sqltext::column_aliases` + `highlight`) e suavizar as cores padrão de tabela e coluna; verificar com os testes `column_aliases_of_select_lists` e `highlight_tables_columns_and_quoted_names`
- [x] 2.4 Verificar manualmente no app (`cargo run`) nos temas claro e escuro: script T-SQL com `declare/exec/convert/@var`, string com `${var}`, aspas duplas com nome conhecido e desconhecido, tabela e coluna de uma conexão SQLite de exemplo

## 3. Preferências das ajudas (`src/store.rs`, `src/tabs.rs`, `src/dialogs.rs`)

- [x] 3.1 Adicionar `ac_auto_popup`, `ac_insert_single`, `auto_close_pairs`, `hl_metadata` em `Prefs` com `serde(default = true)`; verificar com teste que um `workspace.json` sem esses campos carrega tudo `true` e que o round-trip preserva `false`
- [x] 3.2 Trocar as leituras de `AUTOCOMPLETE_AUTO_ACTIVATION`, `AUTOCOMPLETE_INSERT_SINGLE` e `AUTO_CLOSE_PAIRS` em `tabs.rs` pelas preferências e passar `names = None` quando `hl_metadata` está desligado; manter Ctrl+Espaço funcionando; verificar com teste de `complete`/inserção única desligada e manualmente que digitar `select * from cl` não abre popup com a opção desligada
- [x] 3.3 Adicionar a seção "Editor" com as quatro caixas na janela Preferências; verificar manualmente que alternar cada caixa muda o comportamento na hora e persiste após reiniciar
- [x] 3.4 Adicionar `sql_colors_dark`/`sql_colors_light` (mapa categoria → RGBA, `serde(default)`) em `Prefs` e `theme::sql_palette(prefs, dark)` aplicando as sobreposições sobre o padrão do tema; usar essa paleta no editor; verificar com testes que sobreposição no escuro não altera o claro, chave desconhecida é ignorada e `workspace.json` antigo dá a paleta padrão
- [x] 3.5 Adicionar a seção "Cores do editor" nas Preferências (seletor por categoria do tema ativo, "↺" por categoria personalizada, "Restaurar padrão do tema", amostra de SQL com `highlight`); verificar manualmente que mudar a cor de tabela no escuro recolore editor e amostra na hora, não muda o tema claro, persiste após reiniciar e que restaurar volta ao padrão só no tema ativo

## 4. Backend TLS duplo no tiberius (`vendor/tiberius`)

- [x] 4.1 Adicionar a feature `legacy-openssl` (deps `openssl` com `vendored` e `tokio-openssl`) compilável junto com `rustls`; ajustar os `cfg` de `tls_stream.rs` para que, com as duas, `TlsStream` seja um enum `Rustls`/`OpenSsl` implementando `AsyncRead`/`AsyncWrite`; verificar com `cargo build` e `cargo test -p tiberius --features tds73,rustls,legacy-openssl --lib`
- [x] 4.2 Implementar o backend OpenSSL (TLS 1.0 mínimo, `DEFAULT:@SECLEVEL=0`, `LEGACY_SERVER_CONNECT`, sem verificação/SNI) e `Config::legacy_tls(bool)` escolhendo o backend em `create_tls_stream`; verificar com teste unitário que o `SslConnector` construído aceita `SslVersion::TLS1` e com `cargo build`
- [x] 4.3 Atualizar `vendor/tiberius/VENDORED.md` descrevendo a alteração e ativar a feature em `Cargo.toml`; verificar com `cargo build --release --locked` local e `cargo tree -i openssl-sys` mostrando `openssl-src`

## 5. Modo de criptografia na conexão (`src/db.rs`, `src/drivers.rs`, `src/dialogs.rs`, `src/shared.rs`)

- [x] 5.1 Adicionar `MsEncryption { Auto, Modern, Legacy, Off }` e `ConnConfig.mssql_encryption` com `serde(default)` = `Auto`; verificar com teste que uma conexão salva antiga carrega `Auto`
- [x] 5.2 Em `Ms::connect`, aplicar o modo (Legacy → `legacy_tls(true)`, Off → `EncryptionLevel::NotSupported`) e, em Auto, repetir com nova conexão TCP em modo legado só quando o erro for de handshake; devolver o modo usado; verificar com teste unitário da função que classifica erros (handshake × login × timeout)
- [x] 5.3 Montar a mensagem de erro em português com modos tentados, sugestão e detalhe original; fazer `Shared::test` informar o modo usado no sucesso; verificar com teste da função de mensagem para falha moderna, falha nos dois modos e sucesso em legado
- [x] 5.4 Adicionar o campo "Criptografia" (só para SQL Server) no diálogo de conexão, com aviso para "Sem criptografia"; incluir o campo na exportação/importação de bundle se `ConnConfig` for serializado inteiro; verificar manualmente que o campo salva e reabre com o valor escolhido

## 6. Release e validação integrada

- [x] 6.1 Confirmar que o workflow de release compila os 5 alvos com o OpenSSL vendored (Perl/nmake no Windows, cross aarch64 Linux); ajustar `.github/workflows/release.yml` e `ci.yml` se faltar algo; verificar com `workflow_dispatch` verde nos 5 alvos
- [x] 6.2 Validar com o servidor do cliente: conexão em Automático conecta (teste informa "TLS legado"), consultas e cancelamento funcionam; validar também um SQL Server moderno em Automático usando TLS moderno sem segunda tentativa
- [x] 6.3 Rodar `cargo test` e `cargo clippy` sem novos avisos e `openspec validate editor-highlight-assists-legacy-tls --strict`
