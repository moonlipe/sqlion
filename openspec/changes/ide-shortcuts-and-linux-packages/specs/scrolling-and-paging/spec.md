# Spec Delta

## Purpose

Define a velocidade da rolagem com a roda do mouse em todo o app e a navegação por página (Page Up/Page Down) no editor SQL.

## ADDED Requirements

### Requirement: Velocidade de rolagem
A rolagem pela roda do mouse (e por touchpads que enviam passos de linha) SHALL avançar, por padrão, o dobro da distância usada até a versão 0.2.0 em todas as áreas roláveis: editor, grade de resultados, navegador, listas e diálogos. As Preferências SHALL ter um ajuste "Velocidade de rolagem" de 0,5× a 5× (padrão 2×), aplicado na hora e gravado com as demais preferências. Rolagem por pixels de touchpad de alta precisão SHALL manter a velocidade nativa do sistema.

#### Scenario: Padrão mais rápido
- **WHEN** o usuário gira a roda do mouse um passo sobre o editor, com a preferência no padrão
- **THEN** o conteúdo rola o dobro da distância que rolava na versão 0.2.0

#### Scenario: Ajuste nas preferências
- **WHEN** o usuário muda a velocidade para 3× nas Preferências
- **THEN** a rolagem passa a usar a nova velocidade imediatamente, sem reiniciar, e o valor é mantido na próxima sessão

#### Scenario: Preferências antigas
- **WHEN** o arquivo de preferências não tem o ajuste de rolagem
- **THEN** o padrão 2× é usado

### Requirement: Page Up e Page Down no editor
No editor SQL, Page Down e Page Up SHALL mover o cursor uma página visível (a altura da área do editor, em linhas) para baixo ou para cima, mantendo a coluna quando a linha de destino for longa o bastante, e SHALL rolar o editor para manter o cursor visível. Com Shift, SHALL estender a seleção até a nova posição. Na primeira página, Page Up SHALL levar o cursor ao início do texto; na última, Page Down SHALL levá-lo ao fim. Com a lista de autocompletar aberta, Page Up e Page Down SHALL mover a seleção da lista uma página.

#### Scenario: Descer uma página
- **WHEN** o editor mostra 30 linhas, o cursor está na linha 10, coluna 5, e o usuário pressiona Page Down
- **THEN** o cursor vai para a linha 40, coluna 5 (ou o fim da linha, se for mais curta), e a linha 40 fica visível

#### Scenario: Selecionar com Shift
- **WHEN** o cursor está na linha 10 e o usuário pressiona Shift+Page Down
- **THEN** o texto da posição original até a nova posição fica selecionado

#### Scenario: Limites do texto
- **WHEN** o cursor está a menos de uma página do fim e o usuário pressiona Page Down
- **THEN** o cursor vai para o fim do texto

#### Scenario: Lista de autocompletar aberta
- **WHEN** a lista de sugestões está aberta e o usuário pressiona Page Down
- **THEN** a seleção da lista desce uma página e o cursor do texto não se move
