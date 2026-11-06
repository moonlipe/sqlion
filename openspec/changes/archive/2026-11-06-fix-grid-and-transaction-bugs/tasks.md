# Tasks

## 1. Reproduzir os bugs

- [x] 1.1 Escrever um teste de `Session` com modo manual que grava via `editing::apply` e verifica `tx_open`/`tx_count` e que um `rollback` desfaz; verificar que o teste reproduz (ou refuta) o bug de commit manual com `cargo test`
- [x] 1.2 Reproduzir o bug do painel de valores com captura de tela (variável `DBR_VP`, como em `docs/shots`) e registrar a causa em `design.md`; verificar que a causa está anotada
- [x] 1.3 Escrever testes de `GridState` que falham hoje para: Shift+clique, Ctrl+clique, copiar seleção multi-linha e colar bloco; verificar que falham pelo motivo esperado

## 2. Modos de transação (Auto / Manual / Smart)

- [x] 2.1 Criar `TxMode` em `db.rs`, migrar `ConnConfig.auto_commit` (serde com padrão) e ajustar `ConnType`; verificar com teste de leitura de JSON antigo
- [x] 2.2 Implementar Smart em `begin_if_needed` e corrigir a causa do commit manual encontrada em 1.1; verificar com testes de Auto, Manual e Smart (SELECT sem transação, DML com transação, SELECT vê pendências)
- [x] 2.3 Fazer `editing::apply` respeitar o modo da sessão e preservar parar-e-reverter em erro de script; verificar com teste de gravação do grid em Manual e Smart
- [x] 2.4 Atualizar diálogo de conexão, toolbar (três modos, contador de pendentes), preferência global e desabilitar Manual/Smart no Mongo; verificar abrindo o app e alternando os modos, e com captura de tela atualizada em `docs/shots`
- [x] 2.5 Documentar os modos no `README.md`; verificar que o texto descreve os três modos e a migração

## 3. Grid: rolagem e cabeçalho

- [x] 3.1 Envolver a tabela em `ScrollArea::horizontal` e rolar até a célula selecionada; verificar abrindo uma tabela com mais de 30 colunas e navegando com setas
- [x] 3.2 Clique no título seleciona a coluna; criar o botão de ordenação separado com indicador de direção; verificar com teste de `GridState` (seleção de coluna não altera `sort`) e manualmente
- [x] 3.3 Copiar coluna selecionada; verificar com teste que o TSV contém todas as linhas

## 4. Grid: seleção, copiar e colar

- [x] 4.1 Introduzir o novo modelo de seleção (células, linhas, colunas, retângulo) mantendo os testes existentes verdes; verificar com `cargo test`
- [x] 4.2 Implementar clique e arraste, Shift+clique e Ctrl+clique no corpo e no número da linha; verificar com os testes de 1.3 e manualmente
- [x] 4.3 Copiar/colar em massa baseados na seleção, com uma única entrada de desfazer e repetição de valor 1×1; verificar com testes de bloco, seleção maior que o bloco e grid somente leitura

## 5. Painel de valores

- [x] 5.1 Corrigir o painel conforme a causa anotada em 1.2 (buffer reflete mudanças externas, altura baseada no espaço disponível); verificar com teste de desfazer e com captura de tela

## 6. Integração

- [x] 6.1 Rodar `cargo test` e `cargo build --release`, e percorrer manualmente os cenários das specs em SQLite; verificar que todos passam e anotar o que ficou de fora
