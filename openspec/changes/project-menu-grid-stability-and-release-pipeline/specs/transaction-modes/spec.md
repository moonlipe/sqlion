## ADDED Requirements

### Requirement: Rótulo do Commit com pendências
Nos modos Manual e Smart o botão Commit SHALL exibir, no próprio rótulo, o número de instruções (transações) pendentes e o de registros afetados, e SHALL manter o contador visível e atualizado durante a execução de scripts e após gravar edições do grid.

#### Scenario: Duas instruções pendentes
- **WHEN** duas instruções pendentes afetam 3 e 2 registros
- **THEN** o rótulo mostra 2 instruções e 5 registros

#### Scenario: Sem pendências
- **WHEN** há transação aberta sem alterações
- **THEN** o rótulo mostra 0 instruções e 0 registros

#### Scenario: Edições do grid
- **WHEN** o usuário grava edições do grid em modo Manual
- **THEN** o rótulo passa a contar essa gravação como pendente sem esperar nova execução
