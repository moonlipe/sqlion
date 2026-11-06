# result-grid Specification

## Purpose

Define como o grid de resultados permite navegar, selecionar, copiar, colar, ordenar e inspecionar valores de células, inclusive em resultados com muitas colunas ou linhas.

## Requirements

### Requirement: Rolagem horizontal
O grid SHALL exibir uma barra de rolagem horizontal quando a largura total das colunas exceder a área visível, e o cabeçalho SHALL rolar junto, alinhado às colunas.

#### Scenario: Tabela com muitas colunas
- **WHEN** o resultado tem colunas cuja largura somada excede a largura do grid
- **THEN** uma barra horizontal aparece e o usuário alcança a última coluna

#### Scenario: Navegação por teclado
- **WHEN** o usuário move a seleção com as setas para uma coluna fora da área visível
- **THEN** o grid rola horizontalmente até que a célula fique visível

### Requirement: Seleção múltipla
O grid SHALL permitir selecionar várias linhas ou células por clique e arraste, Shift+clique (intervalo a partir da âncora) e Ctrl+clique (alternar item), tanto no corpo quanto no número da linha.

#### Scenario: Arrastar
- **WHEN** o usuário pressiona o botão em uma célula e arrasta até outra
- **THEN** todas as células/linhas do retângulo ficam selecionadas e destacadas

#### Scenario: Shift+clique
- **WHEN** há uma célula âncora e o usuário clica com Shift em outra
- **THEN** o intervalo entre as duas é selecionado

#### Scenario: Ctrl+clique
- **WHEN** o usuário clica com Ctrl em uma linha ou célula
- **THEN** o item é adicionado à seleção, ou removido se já estava nela, sem perder os demais

### Requirement: Copiar e colar em massa
O grid SHALL copiar a seleção atual (células, linhas ou colunas) como texto TSV e SHALL colar texto TSV de várias linhas e colunas a partir da célula atual, aplicando-o como edições pendentes.

#### Scenario: Copiar seleção
- **WHEN** o usuário seleciona um bloco e pressiona Ctrl+C
- **THEN** a área de transferência contém os valores separados por tabulação e quebra de linha

#### Scenario: Colar bloco
- **WHEN** o usuário cola um TSV de 3 linhas por 2 colunas em uma célula editável
- **THEN** as células correspondentes são alteradas como uma única operação desfazível

#### Scenario: Grid somente leitura
- **WHEN** o grid não é editável e o usuário cola
- **THEN** nada é alterado

### Requirement: Seleção e ordenação por coluna
Clicar no título de uma coluna SHALL selecionar a coluna inteira. A ordenação SHALL ser acionada por um botão dedicado no cabeçalho, alternando crescente, decrescente e sem ordenação, e indicando o estado atual.

#### Scenario: Selecionar coluna
- **WHEN** o usuário clica no título da coluna
- **THEN** todas as células da coluna ficam selecionadas e a ordenação não muda

#### Scenario: Ordenar
- **WHEN** o usuário clica no botão de ordenação da coluna
- **THEN** o resultado é ordenado por ela e o botão mostra a direção

#### Scenario: Copiar coluna
- **WHEN** a coluna está selecionada e o usuário copia
- **THEN** os valores de todas as linhas da coluna são copiados

### Requirement: Painel de valores
O painel de valores SHALL mostrar e permitir editar o valor completo da célula atual, refletindo mudanças feitas fora do painel (edição na célula, desfazer, recarga) e preservando o texto digitado enquanto a célula atual não mudar.

#### Scenario: Troca de célula
- **WHEN** o usuário seleciona outra célula
- **THEN** o painel mostra o valor e o tipo da nova célula

#### Scenario: Valor alterado por desfazer
- **WHEN** a célula atual é alterada por desfazer ou edição direta
- **THEN** o painel mostra o novo valor

#### Scenario: Painel não cobre o grid
- **WHEN** o painel está aberto
- **THEN** o grid ocupa o espaço restante, com rolagem própria, e o painel pode ser redimensionado

#### Scenario: Conteúdo acompanha a largura
- **WHEN** o usuário redimensiona o painel
- **THEN** a caixa de texto, a alça de altura e a tabela de estatísticas usam a nova largura

#### Scenario: Caixa de valor redimensionável
- **WHEN** o usuário arrasta a alça abaixo da caixa de valor
- **THEN** a caixa muda de altura, e o padrão é compacto para que as estatísticas fiquem visíveis

### Requirement: Estatísticas da seleção
O painel de valores SHALL exibir, em uma seção aberta por padrão, as estatísticas da seleção atual: número de células, Count (não nulos), Distinct, Nulos, Sum, Avg, Min e Max. Sum e Avg SHALL considerar apenas valores numéricos e ficar vazios quando não houver nenhum. Min e Max SHALL ser numéricos quando todos os valores forem números e textuais caso contrário. O usuário SHALL poder alternar entre a seleção e a coluna inteira e copiar as estatísticas.

#### Scenario: Uma célula
- **WHEN** o usuário seleciona uma única célula de valor 2
- **THEN** o painel mostra Células 1, Count 1, Distinct 1, Sum 2, Avg 2

#### Scenario: Bloco ou coluna
- **WHEN** o usuário seleciona um bloco ou o título de uma coluna
- **THEN** as estatísticas cobrem todas as células selecionadas

#### Scenario: Coluna toda
- **WHEN** o usuário escolhe "Coluna toda"
- **THEN** as estatísticas cobrem todas as linhas visíveis da coluna da célula atual

#### Scenario: Valores nulos e repetidos
- **WHEN** a seleção tem 6 células, 1 nula e 1 repetida
- **THEN** Count é 5, Nulos é 1 e Distinct é 4

### Requirement: Liberação de memória de consultas anteriores
Ao fechar uma aba, substituir os resultados por uma nova execução ou fechar um projeto, o sistema SHALL liberar os dados dos resultados anteriores (linhas, estado das linhas, desfazer, texto do painel de valor), e o uso de memória do processo SHALL voltar a um patamar próximo do inicial, sem crescer a cada nova consulta.

#### Scenario: Fechar a aba de script
- **WHEN** o usuário executa uma consulta grande e fecha a aba
- **THEN** os resultados são descartados e a memória do processo cai de volta, em vez de permanecer no pico

#### Scenario: Várias consultas em sequência
- **WHEN** o usuário executa 20 consultas grandes seguidas na mesma aba, cada uma substituindo a anterior
- **THEN** a memória estabiliza em torno do tamanho de um resultado e não cresce de forma cumulativa

#### Scenario: Resultado antigo ao recarregar tabela
- **WHEN** o usuário recarrega os dados de uma tabela
- **THEN** a grade anterior é descartada antes de a nova ser mantida
