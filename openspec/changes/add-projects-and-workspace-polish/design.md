# Design

## Context

Veja `proposal.md` para a motivação. Estado atual (lido no código):

- `dialogs.rs` cria cada `egui::Window` com `.anchor(center, [0,0])` e `.resizable(false)`: a âncora impede arrastar e o `resizable(false)` impede redimensionar. Há 14 janelas.
- `store.rs`: `Store` (`workspace.json`) guarda `connections`, `history`, `scripts` (legado), `script_conns`, `prefs`. Scripts são arquivos `.sql` em uma única pasta (`Store::scripts_dir`). `ConnConfig` tem `folder` (agrupamento visual), `conn_type`, `tx_mode`; ícone/cor vêm de `Kind::badge()` (rótulo + cor fixos).
- `app.rs`: `Tab { id, kind: Welcome | Sql | Table | Er }`; nada é salvo das abas. `toolbar()` usa `horizontal_wrapped` e põe preferências e tema logo depois dos demais botões. `welcome()` centraliza com larguras fixas (520 e 620 px) e um texto de ajuda longo sem quebra. `Dialog::About` tem 4 linhas de texto.
- Desconectar existe só no menu Banco de dados e no menu de contexto (`Shared::disconnect`); não há reconectar.
- O navegador lista conexões por `folder`; scripts ficam em um nó "Scripts" com a pasta única.

## Goals / Non-Goals

**Goals:**
- Um único ponto para criar janelas (arrastáveis e redimensionáveis) e migrar todas.
- Projetos como unidade de organização e de exportação, sem quebrar dados antigos.
- Restauração de abas sem abrir conexões nem depender da rede.

**Non-Goals:**
- Criptografia de senhas (permanece a ofuscação atual; o diálogo avisa).
- Sincronização na nuvem, colaboração ou controle de acesso por projeto.
- Restaurar resultados de consultas ou posições de rolagem.
- Redesenhar o navegador além de agrupar por projeto.
- Implementar push, pull, merge ou resolução de conflitos git dentro do aplicativo (fica com o git do sistema).

## Decisions

1. **Janelas**: criar `fn win(title) -> egui::Window` em `dialogs.rs` com `.default_pos(centro)`, `.resizable(true)`, `.collapsible(false)` e `.constrain(true)` (fica dentro da tela), e uma `ScrollArea` interna nos diálogos longos. Trocar as 14 chamadas por ela. Alternativa: manter `anchor` só nos diálogos pequenos; descartada porque o usuário pediu "toda janela".
2. **Barra de ferramentas**: reutilizar o padrão do `editing::bar` (coluna fixa à direita quando a largura passa de um limite; senão, linha própria) para preferências e tema.
3. **Boas-vindas**: remover as larguras fixas; usar `horizontal_wrapped` centralizado, `Label::wrap` no texto de ajuda e `ScrollArea::vertical` em volta de tudo.
4. **Desconectar/Reconectar**: `Shared::reconnect(id)` = descartar sessão, `snaps` e caches (`schemas`, `tables`, `tmeta`, `objects`) e chamar `connect`. Com transação pendente (`tx_info`), um diálogo `ConfirmDisconnect` oferece Commit/Rollback/Cancelar. Botões na barra, ao lado de "Nova conexão", com ícones já existentes (`Plug`, `Refresh`); desabilitados sem conexão atual. Abas continuam abertas e usam `Shared::connect` na próxima execução (já é o comportamento de `start_confirmed`).
5. **Aparência**: `ConnConfig` ganha `color: Option<[u8;3]>` e `icon: Option<String>` (um nome de `Icon` ou um rótulo de até 3 caracteres), ambos `#[serde(default)]`. `Kind::badge()` passa a ser o padrão; uma função `ConnConfig::badge()` aplica a personalização e escolhe texto claro/escuro pela luminância. O diálogo de conexão ganha seletor de cor (`color_edit_button_srgb` mais paleta de 12 cores) e de ícone, com "Restaurar padrão".
6. **Projetos em pasta**: `Store.projects: Vec<Project { id, name, path }>`; o projeto padrão aponta para a pasta de configuração e é o único que o `workspace.json` guarda por inteiro. Para os demais, o `workspace.json` guarda só `{ id, path }` (e a lista de projetos abertos); o conteúdo vem da pasta: `sqlion-project.json` (`format_version`, nome, conexões sem senha, pastas, aparência, vínculos script→conexão) e `scripts/*.sql`. `ConnConfig.project` aponta o projeto; vazio = padrão. O padrão continua usando a raiz atual de `scripts_dir`, então nada é movido. Alternativa: um arquivo de projeto fora da pasta de scripts; descartada porque o repositório precisa conter tudo.
7. **Segredos locais**: senhas saem de `ConnConfig` na gravação do projeto e ficam em `secrets.json` (permissão `0600`) na pasta de configuração, indexadas pelo id da conexão; a ofuscação atual é mantida. O id da conexão é gerado uma vez e gravado no projeto (hoje `new_id()` já é um valor estável por conexão). Ao conectar sem senha local, `Dialog::Password` pede a senha e oferece guardá-la. Histórico e abas continuam fora do projeto (`workspace.json` e `session.json`). Alternativa: guardar senhas cifradas no repositório; descartada (risco e gestão de chaves) e fora do escopo.
8. **Git do projeto**: reaproveitar `git.rs` (`is_repo`, `changed_files`, `commit_all`) com a pasta do projeto no lugar de `scripts_dir`; adicionar `init` e a sugestão de `.gitignore` (`*.local`, `secrets.json`). Push e pull ficam com o git do sistema; o app detecta mudança por data de modificação ao receber o foco e recarrega o arquivo do projeto, mantendo abas com texto não gravado.
9. **Exportar/importar `.sqlion`**: formato JSON com `format_version: 1`, `app`, `exported_at` e seções opcionais `connections` (senhas ofuscadas, opcionais), `scripts` (nome + conteúdo), `prefs`, `appearance`, `history`. O diálogo de exportação lista cada seção e cada item com caixas; o de importação lê o arquivo, mostra o conteúdo selecionável, resolve conflitos por item e só então grava, em um projeto novo (pasta escolhida) ou existente. Ids de conexão colidentes são regenerados e as referências (`script_conns`) remapeadas. Alternativa: zip com `.sql` separados; descartada para manter um único arquivo e nenhuma dependência nova.
10. **Restauração de abas**: `session.json` na pasta de configuração, com um enum serializável `SavedTab` (Welcome, Sql { conn, file, text, cursor, dirty }, Table { conn, schema, table, view, sub, sort, mode }, Er { conn, schema, focus }) e `active`. Salvo em `App::on_exit` e a cada alteração relevante, com limite de uma gravação por 2 segundos. Na abertura, com `prefs.restore_tabs` ligada, recriar as abas sem conectar; `TableTab` e `ErTab` carregam ao serem ativadas (já fazem `reload` sob demanda). Preferência `restore_tabs: bool` (padrão `true`). Arquivo ilegível é descartado.
11. **Sobre**: texto construído a partir de `env!("CARGO_PKG_VERSION")` e `CARGO_PKG_LICENSE`; lista de bancos derivada de `PRESETS`/`Kind`, para não ficar desatualizada.

## Risks / Trade-offs

- [Migrar para projetos pode bagunçar scripts existentes] → projeto padrão usa a pasta raiz atual; teste de migração com o `workspace.json` antigo.
- [Texto de scripts em `session.json` pode conter dados sensíveis] → arquivo em `~/.config/sqlion` com permissão `0600` no Linux; a preferência desligada apaga o arquivo.
- [Senhas ofuscadas em arquivos `.sqlion` compartilhados] → aviso explícito no diálogo; senhas desmarcadas por padrão. No fluxo por repositório as senhas nunca entram no projeto.
- [Conflito de merge em `sqlion-project.json` entre pessoas] → JSON gravado com chaves e conexões em ordem estável (por id), uma conexão por bloco, para diffs e merges legíveis.
- [Dois projetos com a mesma conexão (mesmo id)] → ao abrir, o aplicativo avisa e oferece gerar novo id para a cópia.
- [`secrets.json` entrar no repositório por engano] → ele mora na pasta de configuração, fora do projeto; o `.gitignore` sugerido cobre cópias locais.
- [Janelas redimensionáveis com layouts de largura fixa] → revisar cada diálogo com `desired_width`/`Grid`; o teste visual por captura de tela cobre os principais.
- [Reconectar com transação pendente] → diálogo de confirmação antes de descartar a sessão.
- [Restaurar uma aba de tabela cuja tabela foi removida] → a aba mostra o erro de carga e pode ser fechada.

## Migration Plan

`Store` ganha campos com `#[serde(default)]`; arquivos antigos carregam sem alteração e a primeira gravação já escreve o novo formato. `ConnConfig.project` vazio significa o projeto padrão. Sem passos manuais. Reverter a change deixa os campos novos ignorados pela versão anterior (o serde ignora campos desconhecidos).

## Open Questions

- O limite de largura da barra para o alinhamento à direita (560 px no grid) pode precisar de ajuste fino após testes no monitor do usuário.
