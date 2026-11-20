# Proposal

## Why

O destaque de sintaxe do editor SQL reconhece só ~130 palavras-chave genéricas: comandos e funções do T-SQL e de outros dialetos (`EXEC`, `DECLARE`, `CONVERT`, `DEALLOCATE`, `@variavel`…) aparecem como texto comum, aspas simples e duplas têm cores que não se distinguem bem, variáveis `${nome}` dentro de strings somem e nomes de tabelas e colunas não se destacam do resto. Também não há como desligar o autocompletar e as outras ajudas de digitação, que hoje são constantes fixas. E um cliente com SQL Server antigo não conecta (`Falha ao conectar: An error occured during the attempt of performing I/O: tls handshake eof`): o tiberius usa rustls, que só fala TLS 1.2/1.3 com suítes ECDHE, e o servidor fecha a conexão no handshake.

## What Changes

- Tokenizador e cores do editor:
  - Lista de palavras-chave ampliada (comandos e controle de fluxo T-SQL, PL/pgSQL e MySQL: `EXEC`, `EXECUTE`, `DECLARE`, `DEALLOCATE`, `PREPARE`, `OPEN`, `CLOSE`, `CURSOR`, `PRINT`, `RAISERROR`, `THROW`, `TRY`, `CATCH`, `WHILE`, `GOTO`, `MERGE`, `OUTPUT`, `NOLOCK`, `TOP`…) e tipos (`NVARCHAR`, `BIGINT`, `DATETIME2`, `UNIQUEIDENTIFIER`…).
  - Nova categoria **função** para funções embutidas (`CONVERT`, `TRY_CONVERT`, `ISNULL`, `GETDATE`, `DATEADD`, `SUBSTRING`, `ROW_NUMBER`, `STRING_AGG`, `NOW`, `TO_CHAR`…), com cor própria.
  - Nova categoria **variável**: `@nome`, `@@ROWCOUNT`, `:nome` (parâmetro) e `${nome}` — esta também dentro de strings, com cor forte e fundo destacado para sobressair do texto da string.
  - Aspas simples (string) e aspas duplas/crase (identificador citado) com cores diferentes; as duplas em laranja-amarelado, e em azul-água quando o nome citado é uma tabela, view ou coluna conhecida.
  - Tabelas/views e colunas conhecidas da conexão (metadados já em cache) ganham cores próprias, diferentes do texto comum.
  - Cores personalizáveis: seção "Cores do editor" nas Preferências, com um seletor por categoria, valores separados para tema claro e escuro, amostra ao vivo e "Restaurar padrão".
- Preferências novas para as ajudas do editor: abrir o autocompletar sozinho, inserir sozinha a única sugestão, fechar aspas/parênteses automaticamente e colorir tabelas/colunas pelos metadados. As constantes `AUTOCOMPLETE_AUTO_ACTIVATION`, `AUTOCOMPLETE_INSERT_SINGLE` e `AUTO_CLOSE_PAIRS` passam a ser só os valores padrão.
- SQL Server com TLS antigo:
  - Backend TLS alternativo com OpenSSL embutido (estático, vendored), que aceita TLS 1.0/1.1 e suítes antigas (`@SECLEVEL=0`), usado só quando preciso.
  - Opção por conexão **Criptografia** para SQL Server: *Automático* (padrão: rustls e, se o handshake falhar, nova tentativa com o modo legado), *Moderno*, *Legado (TLS 1.0+)* e *Sem criptografia* (só para servidores que não exigem).
  - Mensagem de erro de handshake traduzida e com a sugestão do que mudar.

## Capabilities

### New Capabilities
- `sql-highlighting`: categorias e cores do destaque de sintaxe do editor SQL (palavras-chave, funções, variáveis, strings, identificadores citados, tabelas e colunas) e a personalização dessas cores por tema.
- `editor-assists`: preferências que ligam e desligam o autocompletar e as demais ajudas de digitação.
- `sqlserver-tls`: negociação TLS com SQL Server, incluindo servidores que só suportam TLS antigo, e a opção de criptografia por conexão.

### Modified Capabilities
_(nenhuma — não há spec existente para o editor nem para a conexão)_

## Impact

- `src/sqltext.rs`: `Tok` ganha `Function`, `Variable`, `QuotedIdent`, `Table`, `Column`; listas de palavras-chave e funções; `${}` dentro de strings vira sub-token.
- `src/tabs.rs`: `highlight()` recebe paleta e conjunto de nomes de tabelas/colunas; o layouter usa um cache por texto + `meta_gen`; checagens das preferências no lugar das constantes.
- `src/theme.rs`: paleta do editor com padrões por tema e sobreposições do usuário.
- `src/store.rs` (`Prefs`, com `#[serde(default)]` para arquivos antigos, incluindo as cores personalizadas de cada tema) e `src/dialogs.rs` (Preferências, "Cores do editor" e diálogo de conexão).
- `src/db.rs` (`ConnConfig.mssql_encryption`) e `src/drivers.rs` (`Ms::connect` com tentativa e fallback).
- `vendor/tiberius`: segundo backend TLS (OpenSSL) compilável junto com rustls e escolhido em tempo de execução; atualizar `VENDORED.md`.
- `Cargo.toml`: dependência `openssl` com `vendored` (via tiberius). Build passa a compilar OpenSSL: exige Perl e `make`/`nmake` nos runners do release (presentes nos runners do GitHub) e aumenta o tempo de build e o binário (~2–3 MB).
