# Design

## Context

- **Destaque**: `sqltext::tokenize` devolve `(Tok, início, fim)` com 8 categorias; `tabs::highlight` mapeia cada uma para uma cor fixa (tuplas de cores dentro da função) e roda no `layouter` do `TextEdit` a cada quadro. `"…"` e `` `…` `` já são `Tok::Ident` (cor de texto comum); `@` e `${` viram `Tok::Op`. A lista `KEYWORDS` é linear (`iter().any`).
- **Metadados**: `Shared.tables[(conn, schema)] -> Vec<(nome, é_view)>` e `Shared.tmeta[(conn, schema, tabela)] -> TableMeta { columns }`, carregados em segundo plano por `want_tables`/`want_tmeta`, com `meta_gen` mudando quando algo chega. O autocompletar (`tabs::complete`) já usa esse padrão e `sqltext::referenced_tables` para achar as tabelas citadas.
- **Ajudas**: `constants::AUTOCOMPLETE_AUTO_ACTIVATION`, `AUTOCOMPLETE_INSERT_SINGLE` e `AUTO_CLOSE_PAIRS` são `const bool`; Ctrl+Espaço já existe (`SqlCmd::Complete`). `Prefs` é serializado em `workspace.json`.
- **TLS**: `Cargo.toml` usa tiberius (vendored em `vendor/tiberius`) com a feature `rustls`. No tiberius as features `rustls`, `native-tls` e `vendored-openssl` são mutuamente exclusivas por `cfg` (cada uma define `TlsStream` e `create_tls_stream`). rustls só implementa TLS 1.2/1.3 com ECDHE e não tem renegociação legada; o servidor antigo fecha a conexão → `tls handshake eof`. O backend `vendored-openssl` (crate `opentls`) não expõe versão mínima, lista de cifras nem opções do OpenSSL.
- Release compila para Linux x86_64/aarch64, Windows MSVC e macOS x86_64/aarch64 (ver spec `release-pipeline`).

## Goals / Non-Goals

**Goals:**
- Conectar no SQL Server do cliente sem mexer na configuração do sistema operacional dele ou do usuário.
- Manter o comportamento atual (rustls) idêntico para servidores modernos.
- Destaque mais rico sem custo perceptível ao digitar, inclusive com milhares de tabelas.

**Non-Goals:**
- Validação do certificado do servidor (continua `trust_cert`).
- TLS legado para PostgreSQL, MySQL ou MongoDB.
- Personalizar as cores do resto da interface (só as do destaque do editor).
- Temas de cores nomeados, importação/exportação de paletas avulsas (as cores vão junto das Preferências no bundle, como os demais campos de `Prefs`).
- Analisador SQL completo: o reconhecimento continua léxico, com heurísticas de vizinhança (`(` depois, `.` antes).

## Decisions

### 1. Dois backends TLS no tiberius, escolhidos em tempo de execução
Alterar o vendored tiberius para compilar **rustls e OpenSSL juntos**: `TlsStream` vira um `enum { Rustls(..), OpenSsl(..) }` que implementa `AsyncRead`/`AsyncWrite` repassando para a variante, e `create_tls_stream` escolhe pela nova opção `Config::legacy_tls(bool)`. O backend legado usa o crate `openssl` com feature `vendored` (OpenSSL 3 estático) via `SslConnector` com:
- `set_min_proto_version(Some(SslVersion::TLS1))`;
- `set_cipher_list("ALL:@SECLEVEL=0")` (libera SHA-1, troca de chave RSA e as suítes que o OpenSSL ainda compila; `ALL` em vez de `DEFAULT` porque o servidor do cliente é desconhecido e o modo só é usado quando o moderno falhou);
- `SSL_OP_LEGACY_SERVER_CONNECT` (bits brutos `0x4`, que o crate `openssl` não expõe) e `ALLOW_UNSAFE_LEGACY_RENEGOTIATION` (servidores Windows antigos sem RFC 5746, que o OpenSSL 3 recusa por padrão);
- `SslVerifyMode::NONE` e sem SNI, espelhando `TrustAll`.
O stream assíncrono vem de `tokio-openssl` sobre o `Compat` já usado pelo backend rustls. A feature nova `legacy-openssl` implica `rustls`; com ela o módulo `openssl_legacy_stream` fornece o `TlsStream` (enum) e o `create_tls_stream`.

*Alternativas*: trocar tudo para `native-tls` — depende do SO (Schannel no Windows bloqueia TLS 1.0 por política, OpenSSL do Linux recusa SECLEVEL < 1 e renegociação legada) e piora servidores modernos; usar a feature `vendored-openssl`/`opentls` existente — exclusiva com rustls e sem acesso a cifras/versão; OpenSSL do sistema — varia por distro e não existe no Windows/macOS. OpenSSL estático é o único caminho que se comporta igual em todas as plataformas do release.

### 2. Modo de criptografia por conexão com fallback
`ConnConfig.mssql_encryption: MsEncryption { Auto, Modern, Legacy, Off }` com `#[serde(default)]` = `Auto`. Em `Ms::connect`:
- `Modern`: como hoje.
- `Legacy`: `legacy_tls(true)`.
- `Off`: `EncryptionLevel::NotSupported` (nenhum TLS; o `Off` do TDS ainda negocia TLS no login, por isso não serve).
- `Auto`: tenta `Modern`; se o erro for de handshake (`tiberius::Error::Tls` ou `Error::Io` ocorrido antes do fim do login cujo texto contém `tls`/`handshake`/`eof`), abre **nova conexão TCP** e tenta `Legacy`. Erros de login, rede ou timeout não disparam a segunda tentativa.
`Ms::connect` devolve também o modo efetivamente usado, para o teste de conexão informar (spec `sqlserver-tls`). A mensagem de erro final é montada em `drivers.rs` com o(s) modo(s) tentado(s) e a sugestão; o texto original vai como detalhe.

*Alternativa*: sempre usar OpenSSL — descartada para não mudar o comportamento de quem já conecta e manter o caminho testado.

### 3. Tokenizador com categorias novas
`Tok` ganha `Function`, `Variable`, `QuotedIdent`. `Param` (`:nome`) continua um token próprio, porque `find_params`/`substitute_params` dependem dele, e é pintado com a cor de variável (exceto `::tipo`). Regras:
- `@nome`/`@@nome` → `Variable`; `${…}` fora de string → `Variable`.
- `"…"` e `` `…` `` → `QuotedIdent` (só cor; a semântica de identificador é preservada no resto do código, que não usa o tipo do token para isso).
- Palavras: `KEYWORDS` e `FUNCTIONS` viram buscas em `HashSet` estático (`OnceLock`); nome que está nas duas listas é `Function` se o próximo caractere não-espaço for `(`, senão `Keyword`. Função sem `(` (ex.: `GETDATE` sem parênteses) continua `Function`.
- Strings: `tokenize` continua emitindo um único `Tok::Str` (usado por `split_statements`, `find_params` etc.). A divisão de `${…}` dentro de strings acontece **só em `highlight`**, que varre o trecho da string e emite segmentos `Str`/`Variable` — assim nenhuma regra de divisão/parâmetros muda.
- Tabela/coluna são decididas em `highlight` (como `SqlCat`), não em `tokenize`, porque dependem de metadados; por isso não há variantes `Tok::Table`/`Tok::Column`.
- Consumidores existentes mantêm o comportamento: no formatador `Function` que também é palavra-chave conta como palavra-chave (maiúsculas), os demais tipos novos como nome; `split_statements` trata um `@nome` no início como comando de controle (`@set`), como antes.

### 4. Cores de tabelas/colunas com cache de nomes
`highlight(text, pal, size, wrap, names: Option<&HlNames>)` onde `HlNames { tables: HashSet<String>, columns: HashSet<String> }` em minúsculas. Para cada `Ident`, regra simples e previsível: se precedido de `.`, testa coluna primeiro e depois tabela (`schema.tabela`); senão, tabela primeiro e depois coluna. Sem correspondência → texto comum. Um `QuotedIdent` tem as aspas removidas (e `""` interno desfeito) e é procurado em `tables` e `columns`: se estiver em qualquer um, usa a cor `quoted_known` (azul-água); senão, `quoted`. Com `names = None` é sempre `quoted`.
`SqlTab` guarda `hl_cache: (conn, meta_gen, hash das tabelas referenciadas) -> Arc<HlNames>`; só reconstrói quando a chave muda. Tabelas: todas do schema padrão em cache (`tables` de `cached_default_schema`). Colunas: das tabelas citadas no texto (`referenced_tables` sobre o texto todo), com `want_tmeta` para as que faltam — nunca bloqueia, igual ao autocompletar. Preferência desligada ou sem conexão → `names = None`.

### 4b. Alias de colunas
`sqltext::column_aliases(texto, tokens)` percorre os tokens com uma pilha de profundidades de SELECT (subconsultas e CTEs) e, dentro de cada lista de SELECT, marca como alias o identificador depois de `AS`/`END`, o que segue diretamente o fim de uma expressão (`expr nome`) e o `nome =` no início de um item (T-SQL), sem `.` ou `(` depois. O `highlight` usa o mesmo vetor de tokens e pinta essas palavras com a cor de coluna em qualquer ponto do texto (fora de `x.nome`); é puramente sintático, então funciona sem conexão e com as cores por metadados desligadas.

### 5. Paleta padrão e personalização por tema
`theme::SqlCat` enumera as 12 categorias (`Keyword, Function, Variable, VariableBg, Str, Quoted, QuotedKnown, Number, Comment, Table, Column, Text`) com nome em português para a UI. `theme::SqlPalette([Color32; 12])` indexada por `SqlCat`; `default_sql_palette(dark)` devolve os padrões abaixo e `sql_palette(prefs, dark)` aplica por cima as sobreposições do tema.

Persistência em `Prefs`: `sql_colors_dark` e `sql_colors_light`, cada um `BTreeMap<String, [u8; 4]>` (chave = nome estável da categoria, valor RGBA), `serde(default)` vazio. Só o que o usuário mudou fica gravado, então categorias não tocadas acompanham mudanças de padrão em versões futuras; chave desconhecida é ignorada.

*Alternativa*: guardar a paleta inteira por tema — descartada porque congelaria os padrões e quebraria ao adicionar categorias.

UI: seção "Cores do editor" na janela Preferências (abaixo de "Editor"), título com o tema ativo; uma linha por categoria com `color_edit_button_srgba` (padrão já usado em `dialogs.rs` para a cor de conexão) e um botão "↺" visível só quando a categoria está personalizada; botão "Restaurar padrão do tema"; amostra fixa de SQL (cobrindo todas as categorias) renderizada com `tabs::highlight` e `HlNames` fictícios (`clientes`, `nome`). A paleta efetiva é calculada uma vez por quadro em `show_editor` e passada ao `layouter`.

Padrões (escuro / claro):

| categoria | escuro | claro |
|---|---|---|
| palavra-chave | (86,156,214) azul | (0,0,192) |
| função | (220,220,170) amarelo-claro | (121,94,38) |
| variável | (255,121,198) rosa | (190,0,120) |
| fundo de variável | (255,121,198,40) | (190,0,120,30) |
| string `'…'` | (206,145,120) | (42,120,42) |
| aspas `"…"` | (240,170,60) laranja-amarelado | (190,110,0) |
| nome citado conhecido | (90,190,235) azul-piscina | (0,120,175) |
| número | (181,206,168) | (160,80,0) |
| comentário | (106,153,85) itálico | (110,130,110) itálico |
| tabela | (184,160,214) lavanda suave | (125,90,165) |
| coluna | (80,200,195) verde-água/azul-piscina | (0,135,135) |
| texto | (212,212,212) | (30,30,30) |

Coluna em verde-água puxado para o azul-piscina e nome citado conhecido um pouco mais azul, para continuarem distintos; tabela em lavanda suave (o roxo saturado e o verde-limão ficaram fortes demais na prática). Ajustes finos de tom podem ser feitos na implementação, mantendo as categorias distintas (o teste de 2.1 garante isso para os padrões; cores do usuário não são validadas).

### 6. Preferências
`Prefs` ganha `ac_auto_popup`, `ac_insert_single`, `auto_close_pairs`, `hl_metadata` (todas `true`, com `#[serde(default = "yes")]`). Os pontos que hoje leem as constantes passam a ler `sh.store.prefs`; as constantes ficam como fonte do padrão (comentário DBeaver preservado). Seção "Editor" na grade de Preferências em `dialogs.rs`.

## Risks / Trade-offs

- [Build do OpenSSL vendored exige Perl e `make`/`nmake` e aumenta o tempo de CI] → runners do GitHub já têm Perl (Strawberry no Windows); cache do `target` já existe no release; validar os 5 alvos no primeiro tag.
- [Binário ~2–3 MB maior] → aceitável para app desktop.
- [TLS 1.0/SECLEVEL 0 é criptografia fraca] → só usado em Legado ou após falha em Automático; o teste de conexão mostra qual modo foi usado.
- [Fallback mascarar erro real] → só dispara para erro de handshake; a mensagem final lista os dois modos tentados.
- [`ALL:@SECLEVEL=0` ainda sem a suíte do servidor (ex.: só RC4/export, que o OpenSSL 3 não compila mais)] → o teste unitário cobre TLS 1.0/1.1 com RSA sem ECDHE; validar com o servidor do cliente e, se preciso, habilitar algoritmos legados do `openssl-src`.
- [Muitas tabelas tornando o cache de nomes pesado] → construção só quando `meta_gen`/tabelas citadas mudam; consulta por `HashSet` é O(1) por token.
- [Usuário escolher cores iguais ou ilegíveis no fundo do tema] → é escolha dele; a amostra ao vivo mostra o resultado e "Restaurar padrão" desfaz.
- [Heurística função × palavra-chave errar em casos raros (`REPLACE INTO`)] → só afeta cor.

## Migration Plan

- Campos novos com `serde(default)`: `workspace.json` e conexões antigas abrem com ajudas ligadas e criptografia Automático; nenhuma migração manual.
- Rollback: reverter o commit; arquivos gravados pela versão nova continuam legíveis pela antiga (serde ignora campos desconhecidos — confirmar que `Prefs`/`ConnConfig` não usam `deny_unknown_fields`).
- Atualizar `vendor/tiberius/VENDORED.md` com a nova alteração (backend duplo + `legacy_tls`).

## Open Questions

- Versão exata do SQL Server/Windows do cliente: confirma se TLS 1.0/1.1 com suítes AES/3DES basta (coberto) ou se o servidor só tem RC4; resolvido no teste com o servidor real, sem mudar o desenho.
