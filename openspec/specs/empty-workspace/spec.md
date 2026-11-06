# empty-workspace Specification

## Purpose
TBD - created by archiving change project-management-query-cancel-and-memory. Update Purpose after archive.

## Requirements

### Requirement: Fechar a última aba não encerra o app
O aplicativo SHALL permanecer aberto quando a última aba é fechada, por qualquer meio (botão da aba, clique do meio, menu, Ctrl+W, "Fechar as outras"), e SHALL exibir uma tela vazia com atalhos para novo script, nova conexão e abrir projeto.

#### Scenario: Fechar a única aba
- **WHEN** o usuário fecha a única aba aberta
- **THEN** o app continua em execução e mostra a tela vazia

#### Scenario: Fechar a última aba com script não gravado
- **WHEN** a aba tem texto não gravado
- **THEN** o texto é gravado como hoje e o app mostra a tela vazia

#### Scenario: Reabrir
- **WHEN** o usuário cria um script a partir da tela vazia
- **THEN** a barra de abas volta com a nova aba

#### Scenario: Sair continua possível
- **WHEN** o usuário escolhe Arquivo → Sair ou fecha a janela
- **THEN** o app encerra normalmente, gravando o que for necessário
