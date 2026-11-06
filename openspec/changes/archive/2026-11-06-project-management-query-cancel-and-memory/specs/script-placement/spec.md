## ADDED Requirements

### Requirement: Script vinculado a projeto ou solto
Todo script SHALL ser ou vinculado a um projeto (arquivo na pasta `scripts/` do projeto) ou solto (pasta global de scripts), e a árvore SHALL mostrar cada script no lugar a que pertence.

#### Scenario: Script de projeto
- **WHEN** um script está na pasta de um projeto
- **THEN** ele aparece sob esse projeto, e entra no git do projeto

#### Scenario: Script solto
- **WHEN** um script está na pasta global
- **THEN** ele aparece no nó de scripts soltos

### Requirement: Onde o novo script é criado
Um novo script SHALL ser criado no projeto selecionado ou da conexão atual, se houver, e solto caso contrário, e o título SHALL indicar o vínculo. O usuário SHALL poder escolher outro destino no menu "Novo script em…".

#### Scenario: Com projeto selecionado
- **WHEN** o usuário cria um script com um projeto selecionado na árvore
- **THEN** o arquivo é criado na pasta `scripts/` desse projeto

#### Scenario: Sem projeto
- **WHEN** nenhum projeto está selecionado e a conexão atual não pertence a projeto
- **THEN** o script é criado solto

### Requirement: Mover script
O sistema SHALL permitir mover um script de um projeto para outro ou para solto (e o inverso) pelo menu do script, atualizando abas abertas e o vínculo com a conexão, e recusando se já existe arquivo com o mesmo nome no destino.

#### Scenario: Mover para projeto
- **WHEN** o usuário move um script solto para um projeto
- **THEN** o arquivo passa para a pasta do projeto, a aba aberta segue apontando para o novo caminho e a conexão vinculada é mantida

#### Scenario: Nome em conflito
- **WHEN** o destino já tem um script com o mesmo nome
- **THEN** a operação é recusada com mensagem e nada é movido
