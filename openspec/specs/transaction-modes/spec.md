# transaction-modes Specification

## Purpose

Define os modos de transação de uma conexão (Auto, Manual e Smart) e como alterações ficam pendentes até serem confirmadas ou revertidas.

## Requirements

### Requirement: Modo Auto
No modo Auto o sistema SHALL confirmar cada instrução imediatamente.

#### Scenario: UPDATE em modo Auto
- **WHEN** o usuário executa um UPDATE
- **THEN** a alteração é confirmada e nenhuma transação fica pendente

### Requirement: Modo Manual
No modo Manual o sistema SHALL abrir uma transação na primeira instrução executada, inclusive de leitura, e mantê-la aberta até Commit ou Rollback explícito, inclusive para edições gravadas pelo grid. O número de registros afetados pelas instruções de escrita pendentes SHALL ser exibido no botão Commit, junto com o número de instruções, e SHALL permanecer atualizado enquanto um script está em execução.

#### Scenario: Alteração fica pendente
- **WHEN** o modo é Manual e o usuário executa INSERT ou grava edições do grid
- **THEN** a alteração é visível na própria sessão, não é confirmada e o contador de pendentes aumenta

#### Scenario: Contador soma registros
- **WHEN** duas instruções pendentes afetam 3 e 2 registros
- **THEN** o botão Commit mostra 5 e informa 2 instruções

#### Scenario: Contador durante a execução
- **WHEN** um script está em execução e já alterou registros
- **THEN** o contador reflete as alterações sem esperar o fim do script

#### Scenario: Leitura abre transação
- **WHEN** o modo é Manual e o usuário executa um SELECT
- **THEN** uma transação é aberta e permanece até Commit ou Rollback, com zero alterações pendentes

#### Scenario: Rollback
- **WHEN** há alterações pendentes e o usuário aciona Rollback
- **THEN** as alterações são descartadas e o contador volta a zero

#### Scenario: Commit
- **WHEN** há alterações pendentes e o usuário aciona Commit
- **THEN** as alterações são confirmadas e o contador volta a zero

#### Scenario: Troca para Auto com pendências
- **WHEN** o modo muda de Manual para Auto com transação aberta
- **THEN** o sistema confirma as pendências antes de trocar

### Requirement: Modo Smart
No modo Smart o sistema SHALL executar instruções de leitura (SELECT, WITH sem escrita, SHOW, EXPLAIN, DESCRIBE, PRAGMA de leitura) sem abrir transação, e SHALL tratar instruções que alteram dados ou estrutura como no modo Manual.

#### Scenario: SELECT em modo Smart
- **WHEN** o usuário executa um SELECT
- **THEN** a consulta roda sem transação pendente

#### Scenario: DML em modo Smart
- **WHEN** o usuário executa DELETE
- **THEN** uma transação é aberta e fica pendente até Commit ou Rollback

#### Scenario: Leitura durante pendências
- **WHEN** há alterações pendentes e o usuário executa um SELECT
- **THEN** o SELECT roda na mesma sessão, vendo as alterações pendentes, sem confirmá-las

### Requirement: Seleção e persistência do modo
O modo SHALL ser escolhido por conexão no diálogo de conexão e alternável na barra de ferramentas. O padrão para novas conexões SHALL seguir o tipo de conexão (Produção: Manual; demais: Auto) e a preferência global. Conexões salvas com auto-commit ligado SHALL abrir em Auto e com desligado em Manual.

#### Scenario: Conexão antiga
- **WHEN** uma conexão salva anteriormente com auto-commit desligado é aberta
- **THEN** ela abre em modo Manual

#### Scenario: Alternar na barra
- **WHEN** o usuário escolhe outro modo na barra
- **THEN** o novo modo vale para as próximas instruções e a barra o exibe

### Requirement: Fim de script com erro
Em modos Manual e Smart, um script que falha SHALL parar e reverter a transação aberta, preservando o comportamento atual de parada com rollback.

#### Scenario: Erro no meio do script
- **WHEN** a segunda instrução de um script falha em modo Manual
- **THEN** a execução para e a transação é revertida
