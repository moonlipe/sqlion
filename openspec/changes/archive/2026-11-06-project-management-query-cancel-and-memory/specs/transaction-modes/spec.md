## ADDED Requirements

### Requirement: Aviso ao mudar para Auto commit
Ao mudar uma conexão de Manual ou Smart para Auto, por qualquer caminho (botões de modo, atalho de alternância, diálogo da conexão), o sistema SHALL pedir confirmação explicando que cada instrução passará a ser confirmada imediatamente e, se houver pendências, que elas serão confirmadas agora com o número de instruções e de registros. Cancelar SHALL manter o modo atual e as pendências.

#### Scenario: Troca sem pendências
- **WHEN** o usuário muda de Manual para Auto sem transação pendente
- **THEN** aparece a confirmação sobre o modo e, ao confirmar, o modo muda

#### Scenario: Troca com pendências
- **WHEN** há 2 instruções pendentes afetando 5 registros
- **THEN** a confirmação cita 2 instruções e 5 registros e informa que serão confirmados antes da troca

#### Scenario: Cancelar
- **WHEN** o usuário cancela a confirmação
- **THEN** o modo continua como estava e as pendências não são confirmadas

#### Scenario: Não perguntar de novo
- **WHEN** o usuário marca "Não perguntar de novo" e confirma
- **THEN** as próximas trocas sem pendências não pedem confirmação, mas trocas com pendências continuam pedindo

#### Scenario: Escolha de modo Auto em conexão nova
- **WHEN** o usuário escolhe Auto no diálogo de uma conexão nova ou na preferência de modo padrão
- **THEN** o diálogo mostra um aviso permanente de que as alterações são confirmadas automaticamente
