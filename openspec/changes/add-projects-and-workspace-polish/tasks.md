# Tasks

## 1. Janelas e barra de ferramentas

- [x] 1.1 Criar o helper `win()` em `dialogs.rs` e migrar as 14 janelas para ele (sem `anchor`, redimensionáveis, `constrain`); verificar com teste de que nenhuma chamada `Window::new` sobra fora do helper (`grep`) e com captura de tela de Preferências e Conexão
- [x] 1.2 Mover preferências e tema para o canto direito da barra, com recuo para linha própria em janela estreita; verificar por captura de tela em 1280 e 800 px de largura
- [x] 1.3 Reescrever `welcome()` sem larguras fixas, com quebra de linha e rolagem; verificar por captura de tela em 800 px sem texto cortado
- [x] 1.4 Atualizar a tela Sobre (versão do `Cargo.toml`, bancos, licença, créditos); verificar que o texto lista SQL Server e MongoDB e a versão atual, e abrir o diálogo por captura de tela

## 2. Desconectar e reconectar

- [x] 2.1 Implementar `Shared::reconnect` e o diálogo de pendências (Commit/Rollback/Cancelar); verificar com teste que a sessão nova substitui a antiga, as caches são limpas e uma falha deixa a conexão desconectada
- [x] 2.2 Adicionar os botões Desconectar e Reconectar ao lado de "Nova conexão" e entradas no menu de contexto; verificar por captura de tela e que ficam desabilitados sem conexão
- [x] 2.3 Indicar na aba que a conexão está fechada e reabrir ao executar; verificar com teste de executar após desconectar

## 3. Aparência das conexões

- [x] 3.1 Adicionar `color` e `icon` a `ConnConfig` e `ConnConfig::badge()` com contraste automático; verificar com testes de padrão, personalização e contraste
- [x] 3.2 Seletor de cor e ícone no diálogo de conexão, com "Restaurar padrão", e uso nos locais de exibição (navegador, barra, abas, boas-vindas); verificar por captura de tela

## 4. Projetos em pasta

- [x] 4.1 Modelo `Project` com pasta, arquivo `sqlion-project.json` (leitura e gravação estáveis, `format_version`) e `ConnConfig.project`; verificar com testes de ida e volta e de ordem estável do JSON
- [x] 4.2 Migrar o formato antigo para o projeto padrão sem mover arquivos; verificar com teste de carga do `workspace.json` antigo e dos scripts existentes
- [x] 4.3 Segredos locais: tirar senhas do arquivo do projeto, guardar em `secrets.json` por id da conexão e pedir senha ao conectar sem senha local; verificar com testes de que o arquivo do projeto não contém senha e de que a senha pedida é guardada
- [x] 4.4 Criar projeto em uma pasta, abrir pasta existente ou clonada, fechar, renomear e mover conexões e scripts; verificar com testes sobre pastas temporárias, incluindo vínculo script→conexão pelo id
- [x] 4.5 Navegador em árvore por projeto com o menu de projeto; verificar por captura de tela
- [x] 4.6 Git do projeto: status, commit, `init` e `.gitignore` sugerido sobre a pasta do projeto, e recarga ao receber o foco; verificar com testes em repositório temporário e que abas com texto não gravado sobrevivem à recarga
- [x] 4.7 Formato `.sqlion` e exportação seletiva (conexões, senhas, scripts, preferências, aparência, histórico); verificar com testes de cada seleção, inclusive "sem senhas"
- [x] 4.8 Importação com prévia, resolução de conflitos, remapeamento de ids e diálogos de exportar e importar no menu Arquivo; verificar com testes de conflito (renomear, substituir, ignorar), arquivo inválido e versão desconhecida, e a ida e volta exportar→importar em workspace vazio
- [x] 4.9 Excluir a pasta do projeto só fecha as abas depois que `delete_project_folder` der certo (hoje `src/dialogs.rs` chama `close_tabs_of_project` antes); em caso de recusa pelas checagens de segurança ou de falha de permissão, as abas e o texto não gravado ficam como estavam (spec `project-context-menu`, "Excluir cancelado ou falha"); verificar com teste em pasta temporária sem `sqlion-project.json` e com pasta sem permissão de escrita, conferindo que as abas do projeto continuam abertas
- [x] 4.10 Remover da lista e Excluir a pasta encerram as sessões das conexões do projeto (`sh.disconnect` para cada conexão, hoje `store.close_project` só tira as conexões da lista e a sessão fica órfã com a transação aberta); o diálogo de exclusão e o Remover da lista avisam quando alguma conexão do projeto tem transação pendente (`has_pending`) e a pendência é desfeita (rollback) ao confirmar; verificar com teste de que, após remover um projeto com conexão SQLite em modo Manual com alteração pendente, não há sessão nem transação aberta e o arquivo do banco não tem a alteração

## 5. Restaurar abas

- [x] 5.1 Serializar o estado das abas (`SavedTab`) e gravar `session.json` ao sair e com limite de frequência; verificar com teste de ida e volta de cada tipo de aba
- [x] 5.2 Restaurar as abas ao abrir, sem conectar, ignorando conexões inexistentes e arquivo corrompido; verificar com testes desses casos
- [x] 5.3 Preferência "Reabrir as abas da última sessão" (padrão ligada) em Preferências, apagando o arquivo quando desligada; verificar com teste e por captura de tela

## 7. Correções do teste manual

- [x] 7.1 Abas restauradas voltam com o cursor, a rolagem e o foco do editor (o widget do editor começava vazio e o cursor ia para o início); verificar com o teste de ida e volta da sessão
- [x] 7.2 Autocomplete sem esperar o banco: tabelas, esquemas e colunas carregam em segundo plano e a lista se atualiza quando chegam; nada de consultar metadados do nome parcial digitado; sem a deduplicação quadrática; nomes que começam com o texto primeiro; verificar com testes de que a primeira chamada responde na hora e de que 30 000 tabelas respondem em menos de 400 ms
- [x] 7.3 Fechamento automático de parênteses e aspas: o cursor ia para frente antes de o editor inserir o par, que caía depois do caractere seguinte; agora volta para o meio depois da inserção, e diante de uma palavra só o caractere digitado entra; verificar com teste de digitação no editor
- [x] 7.4 Variáveis `${nome}` não definidas por `@set` são pedidas no diálogo junto com os `:parâmetros` e entram como texto; os valores digitados só preenchem o diálogo da próxima vez; verificar com teste
- [ ] 7.5 Colunas `varchar`/`text` com collation de code page DOS (CP850, sort ID 40-61; CP437, 30-35) no SQL Server: cópia do `tiberius` 0.12.3 em `vendor/tiberius` com decodificação própria (`[patch.crates-io]`); verificar com as tabelas de conversão (geradas dos codecs do Python) e com uma consulta real num banco CP850

## 6. Integração

- [ ] 6.1 Rodar `cargo test`, `cargo build --release` e percorrer manualmente os cenários das specs; anotar o que ficou de fora
