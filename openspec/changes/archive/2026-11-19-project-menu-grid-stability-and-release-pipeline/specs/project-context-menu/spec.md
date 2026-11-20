## ADDED Requirements

### Requirement: Menu de contexto do projeto
Ao clicar com o botão direito em um projeto na árvore do navegador, o sistema SHALL abrir um menu de gerenciamento com as ações: Renomear, Remover da lista, Excluir a pasta, Abrir a pasta, Copiar o caminho, Novo script e Nova conexão neste projeto, e Versionamento (git).

#### Scenario: Abrir o menu
- **WHEN** o usuário clica com o botão direito na linha de um projeto
- **THEN** o menu com todas as ações aparece junto ao cursor

#### Scenario: Projeto padrão
- **WHEN** o usuário olha a árvore
- **THEN** o projeto padrão (conexões e scripts do workspace) não aparece como nó de projeto e, portanto, não tem este menu nem pode ser removido ou excluído

### Requirement: Renomear projeto
O sistema SHALL permitir renomear um projeto por diálogo, rejeitando nome vazio ou já usado por outro projeto aberto, sem alterar a pasta nem os ids das conexões.

#### Scenario: Renomear com sucesso
- **WHEN** o usuário informa um nome novo e válido
- **THEN** o projeto aparece com o novo nome na árvore e o nome é gravado no arquivo do projeto

#### Scenario: Nome repetido
- **WHEN** o nome informado já pertence a outro projeto
- **THEN** o diálogo mostra o erro e nada é alterado

### Requirement: Remover e excluir projeto
Remover da lista SHALL fechar o projeto mantendo a pasta e os arquivos no disco. Excluir a pasta SHALL pedir confirmação com o caminho e o número de conexões e scripts afetados, e só então apagar a pasta do projeto.

#### Scenario: Remover da lista
- **WHEN** o usuário escolhe Remover da lista
- **THEN** o projeto some da árvore, a pasta continua no disco e pode ser aberta de novo

#### Scenario: Excluir a pasta confirmado
- **WHEN** o usuário confirma a exclusão
- **THEN** a pasta é apagada, as conexões e os scripts do projeto saem da árvore e as abas que dependiam deles são fechadas

#### Scenario: Excluir cancelado ou falha
- **WHEN** o usuário cancela ou a exclusão falha por permissão
- **THEN** nada muda e, em caso de falha, o motivo é exibido

#### Scenario: Abas com alterações não gravadas
- **WHEN** há abas do projeto com texto não gravado ou alterações pendentes de transação
- **THEN** o diálogo de exclusão avisa antes de confirmar
