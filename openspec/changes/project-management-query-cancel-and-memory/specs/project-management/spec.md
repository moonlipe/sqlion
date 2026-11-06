## ADDED Requirements

### Requirement: Criar projeto
O sistema SHALL permitir criar um projeto pela interface informando nome e pasta, criando a pasta (e `scripts/`) e o arquivo do projeto, e exibindo o projeto na árvore.

#### Scenario: Novo projeto em pasta nova
- **WHEN** o usuário informa nome e uma pasta que não existe
- **THEN** a pasta é criada, o projeto aparece na árvore e fica pronto para receber conexões e scripts

#### Scenario: Pasta já é projeto
- **WHEN** a pasta informada já contém um projeto
- **THEN** o diálogo recusa criar e oferece abrir o existente

#### Scenario: Pasta inválida
- **WHEN** o caminho está vazio, não pode ser criado ou não tem permissão de escrita
- **THEN** o diálogo mostra o motivo e nada é criado

### Requirement: Abrir projeto existente
O sistema SHALL permitir abrir como projeto uma pasta existente ou clonada, rejeitando pastas sem arquivo de projeto e arquivos de versão desconhecida, sem alterar nada em caso de erro.

#### Scenario: Pasta clonada
- **WHEN** o usuário abre uma pasta de projeto clonada do git
- **THEN** as conexões (sem senha) e os scripts aparecem e a senha é pedida no primeiro uso

#### Scenario: Pasta que não é projeto
- **WHEN** a pasta não tem arquivo de projeto
- **THEN** o sistema informa o erro e oferece criar um projeto nela

### Requirement: Projeto padrão visível e convertível
O projeto padrão (dados anteriores ao uso de projetos) SHALL aparecer na árvore como o nó "Projeto padrão" e SHALL poder ser convertido em projeto de pasta, movendo conexões e scripts para a pasta escolhida.

#### Scenario: Converter
- **WHEN** o usuário converte o projeto padrão em uma pasta vazia e confirma
- **THEN** as conexões e os scripts passam a viver na pasta como um projeto, o nó "Projeto padrão" fica vazio e os scripts originais só são removidos depois que a cópia foi gravada com sucesso

#### Scenario: Falha na conversão
- **WHEN** a gravação na pasta falha no meio do processo
- **THEN** o projeto padrão permanece intacto e o erro é exibido

### Requirement: Mover conexões entre projetos
O sistema SHALL permitir mover uma conexão entre projetos (inclusive o padrão) pelo menu da conexão, preservando o id, as senhas locais e o vínculo dos scripts.

#### Scenario: Mover conexão
- **WHEN** o usuário move uma conexão para outro projeto
- **THEN** ela passa a ser gravada no arquivo do novo projeto e sai do anterior

### Requirement: Pastas de organização das conexões
O sistema SHALL permitir criar, renomear e excluir pastas de organização dentro de um projeto pela árvore, e mover conexões para elas. Excluir uma pasta SHALL mover suas conexões para a raiz do projeto, nunca apagá-las.

#### Scenario: Criar pasta
- **WHEN** o usuário escolhe Nova pasta no menu de um projeto e informa um nome
- **THEN** a pasta aparece na árvore e aceita conexões

#### Scenario: Excluir pasta com conexões
- **WHEN** o usuário exclui uma pasta que tem conexões
- **THEN** as conexões vão para a raiz do projeto e continuam disponíveis

### Requirement: Versionamento do projeto
O sistema SHALL oferecer, no menu do projeto, status e commit git da pasta do projeto, e `git init` quando a pasta não é um repositório.

#### Scenario: Commit do projeto
- **WHEN** o usuário aciona Versionamento e confirma o commit
- **THEN** as alterações da pasta do projeto são gravadas no git, sem senhas nem histórico

### Requirement: Arrastar e soltar na árvore
Os itens da árvore do navegador SHALL poder ser arrastados com o mouse: uma conexão pode ser solta sobre um projeto (vai para a raiz dele) ou sobre uma pasta de organização; um script pode ser solto sobre um projeto ou sobre "Scripts soltos". O destino SHALL ser destacado durante o arraste, e as mesmas regras do menu "Mover para…" valem (recusar nome de script em conflito, preservar id e vínculos).

#### Scenario: Arrastar conexão para outro projeto
- **WHEN** o usuário arrasta uma conexão e a solta sobre outro projeto
- **THEN** a conexão passa a pertencer a esse projeto, na raiz dele

#### Scenario: Arrastar conexão para uma pasta
- **WHEN** a conexão é solta sobre uma pasta de organização
- **THEN** ela vai para o projeto da pasta e para dentro da pasta

#### Scenario: Arrastar script
- **WHEN** um script é solto sobre outro projeto ou sobre "Scripts soltos"
- **THEN** o arquivo é movido, a aba aberta segue o novo caminho e a conexão vinculada é mantida

#### Scenario: Soltar em lugar inválido
- **WHEN** o item é solto fora de um destino válido ou sobre o próprio dono
- **THEN** nada muda
