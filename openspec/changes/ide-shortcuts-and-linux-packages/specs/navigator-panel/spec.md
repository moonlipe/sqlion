# Spec Delta

## Purpose

Define o comportamento do painel lateral do navegador de bancos: o menu de contexto da sua área vazia e o recolhimento do painel para dar espaço ao editor.

## ADDED Requirements

### Requirement: Menu da área vazia do navegador
Ao clicar com o botão direito na área do navegador que não contém nenhum item da árvore (abaixo dos projetos ou entre o filtro e a árvore), o sistema SHALL abrir um menu com: Nova conexão, Novo projeto, Abrir projeto, Novo script, Nova pasta de conexões e Atualizar. Novo script e Nova pasta SHALL usar o projeto padrão. Um clique direito sobre um item da árvore SHALL continuar abrindo o menu daquele item.

#### Scenario: Clique na parte vazia
- **WHEN** o usuário clica com o botão direito abaixo do último projeto da árvore
- **THEN** o menu com as ações de criação e Atualizar aparece junto ao ponteiro

#### Scenario: Clique em um item
- **WHEN** o usuário clica com o botão direito na linha de uma conexão
- **THEN** aparece o menu da conexão, e não o da área vazia

#### Scenario: Navegador sem conexões
- **WHEN** não há conexões nem projetos e o usuário clica com o botão direito no painel
- **THEN** o mesmo menu aparece, e Nova conexão abre o diálogo de conexão

### Requirement: Recolher o navegador
O painel do navegador SHALL poder ser recolhido e expandido por um botão no seu cabeçalho, pelo item "Mostrar navegador" do menu Janela e pelo atalho Ctrl+B (Cmd+B no macOS). Recolhido, o painel SHALL virar uma faixa estreita com o botão de expandir, e o editor SHALL ocupar o espaço liberado. O estado recolhido e a largura do painel SHALL ser lembrados entre sessões.

#### Scenario: Recolher pelo botão
- **WHEN** o usuário clica no botão de recolher no cabeçalho do navegador
- **THEN** o painel vira uma faixa estreita e a área das abas se alarga

#### Scenario: Expandir com a largura anterior
- **WHEN** o painel recolhido é expandido
- **THEN** ele volta com a largura que tinha antes de ser recolhido

#### Scenario: Atalho com o foco no editor
- **WHEN** o foco está no editor SQL e o usuário pressiona Ctrl+B
- **THEN** o painel alterna entre recolhido e expandido e o texto do editor não muda

#### Scenario: Reabrir o app
- **WHEN** o app é fechado com o navegador recolhido e aberto de novo
- **THEN** o navegador começa recolhido
