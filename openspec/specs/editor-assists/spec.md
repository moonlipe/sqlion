# editor-assists Specification

## Purpose
Permite ao usuário ligar e desligar o autocompletar e as demais ajudas automáticas de digitação do editor SQL, que hoje estão sempre ativas.

## Requirements

### Requirement: Preferências das ajudas do editor
A janela Preferências SHALL ter uma seção "Editor" com uma caixa para cada ajuda, todas ligadas por padrão e salvas na hora como as outras preferências:
- "Abrir sugestões automaticamente ao digitar"
- "Inserir sozinha a única sugestão"
- "Fechar aspas e parênteses automaticamente"
- "Colorir tabelas e colunas pelos metadados"

Um arquivo de configuração gravado por uma versão anterior SHALL abrir com todas as ajudas ligadas.

#### Scenario: Configuração antiga
- **WHEN** o app abre um `workspace.json` sem esses campos
- **THEN** as quatro preferências aparecem ligadas e o editor se comporta como antes

#### Scenario: Preferência persistida
- **WHEN** o usuário desliga uma ajuda e reinicia o app
- **THEN** a ajuda continua desligada

### Requirement: Desligar o autocompletar automático
Com "Abrir sugestões automaticamente ao digitar" desligado, a lista de sugestões NÃO SHALL abrir sozinha ao digitar (nem após `.` nem após duas letras); o atalho de autocompletar (Ctrl+Espaço) SHALL continuar abrindo a lista sob demanda.

#### Scenario: Digitação sem popup
- **WHEN** a preferência está desligada e o usuário digita `select * from cl`
- **THEN** nenhuma lista de sugestões aparece

#### Scenario: Pedido manual
- **WHEN** a preferência está desligada e o usuário pressiona Ctrl+Espaço
- **THEN** a lista de sugestões abre para a palavra sob o cursor

### Requirement: Desligar inserção da sugestão única
Com "Inserir sozinha a única sugestão" desligado, um pedido de autocompletar com uma única sugestão SHALL mostrar a lista com essa sugestão em vez de inseri-la direto.

#### Scenario: Uma sugestão só
- **WHEN** a preferência está desligada e o pedido manual encontra uma só sugestão
- **THEN** a lista aparece com ela e nada é inserido até o usuário confirmar

### Requirement: Desligar fechamento automático de pares
Com "Fechar aspas e parênteses automaticamente" desligado, digitar `(`, `[`, `{`, `'` ou `"` SHALL inserir só o caractere digitado, e digitar o caractere de fechamento SHALL sempre inseri-lo (sem pular o existente).

#### Scenario: Parêntese sem par
- **WHEN** a preferência está desligada e o usuário digita `(`
- **THEN** só `(` é inserido

### Requirement: Desligar cores por metadados
Com "Colorir tabelas e colunas pelos metadados" desligado, o editor NÃO SHALL colorir tabelas e colunas nem pedir metadados para isso; as demais cores do destaque continuam.

#### Scenario: Sem cores de metadados
- **WHEN** a preferência está desligada
- **THEN** nomes de tabelas e colunas aparecem na cor de texto comum e palavras-chave, strings e variáveis continuam coloridas
