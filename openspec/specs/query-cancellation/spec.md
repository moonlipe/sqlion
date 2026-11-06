# query-cancellation Specification

## Purpose
TBD - created by archiving change project-management-query-cancel-and-memory. Update Purpose after archive.

## Requirements

### Requirement: Cancelar consulta em execução
Enquanto uma consulta ou script roda, o sistema SHALL exibir o botão Cancelar (também atalho Esc no editor) que interrompe a instrução em andamento no servidor, descarta as instruções restantes do script e libera a conexão para novas consultas, em SQLite, PostgreSQL, MySQL, SQL Server e MongoDB.

#### Scenario: Cancelar consulta longa
- **WHEN** o usuário aciona Cancelar durante uma consulta demorada
- **THEN** a consulta é interrompida, a aba mostra "Consulta cancelada" e a mesma conexão aceita uma nova consulta

#### Scenario: Cancelar no meio de um script
- **WHEN** o usuário cancela com instruções ainda por executar
- **THEN** as restantes não são executadas e os resultados já obtidos permanecem

#### Scenario: Transação aberta
- **WHEN** o cancelamento ocorre em modo Manual ou Smart com transação aberta
- **THEN** a transação continua aberta, com o contador de pendências coerente com o que foi executado antes do cancelamento, e o usuário decide Commit ou Rollback

#### Scenario: Falha ao cancelar
- **WHEN** o servidor não consegue interromper a consulta
- **THEN** o sistema informa o erro e mantém o botão Cancelar disponível

### Requirement: Tempo limite opcional
O sistema SHALL ter uma preferência de tempo limite de execução em segundos (0 = sem limite); ao estourar, a consulta SHALL ser cancelada como no cancelamento manual, com mensagem que cita o limite.

#### Scenario: Estouro do limite
- **WHEN** o limite é 30 s e a consulta passa disso
- **THEN** a consulta é cancelada e a mensagem informa que o limite de 30 s foi atingido

### Requirement: Indicação de progresso
Durante a execução o sistema SHALL mostrar o tempo decorrido ao lado do botão Cancelar.

#### Scenario: Tempo decorrido
- **WHEN** uma consulta está em execução
- **THEN** o tempo decorrido é atualizado na aba
