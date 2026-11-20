## ADDED Requirements

### Requirement: Estabilidade em áreas pequenas
O grid e o painel de valor SHALL funcionar sem falhar em qualquer tamanho de área, inclusive menor que o mínimo útil, sem usar dimensões negativas ou inválidas.

#### Scenario: Redimensionar para muito pequeno
- **WHEN** o usuário reduz a janela, o painel dividido ou o grid a uma largura ou altura quase nula
- **THEN** o aplicativo continua em execução e o conteúdo é cortado ou substituído por um aviso

#### Scenario: Painel de valor aberto
- **WHEN** o painel de valor está aberto e a área fica menor que a soma dos mínimos do grid e do painel
- **THEN** o painel é reduzido ou escondido e o grid continua utilizável

### Requirement: Memória proporcional aos dados
O consumo de memória do grid SHALL crescer de forma proporcional ao tamanho dos dados carregados, sem cópias do resultado inteiro por quadro nem caches que cresçam a cada rolagem ou busca de página.

#### Scenario: Buscar a página seguinte
- **WHEN** o usuário rola até o fim e a página seguinte de 200 linhas é carregada
- **THEN** a memória aumenta aproximadamente pelo tamanho das novas linhas, e não por múltiplos do resultado já carregado

#### Scenario: Rolar sem buscar
- **WHEN** o usuário rola de cima a baixo um resultado já carregado
- **THEN** a memória permanece estável
