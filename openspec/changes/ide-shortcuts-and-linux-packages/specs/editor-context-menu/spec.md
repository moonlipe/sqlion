# Spec Delta

## Purpose

Define o menu de contexto do editor SQL, com as ações de edição e execução e, quando o cursor ou a seleção estiver sobre um objeto do banco, as ações desse objeto.

## ADDED Requirements

### Requirement: Menu de contexto do editor
Ao clicar com o botão direito dentro do editor SQL, o sistema SHALL abrir junto ao cursor do mouse um menu com: Recortar, Copiar, Colar, Selecionar tudo, Executar instrução, Executar script, Formatar, Comentar/descomentar e Abrir objeto (F4), com os atalhos de cada ação mostrados ao lado. Recortar e Copiar SHALL ficar desabilitados sem seleção. As ações SHALL ter o mesmo efeito dos seus atalhos.

#### Scenario: Abrir o menu
- **WHEN** o usuário clica com o botão direito no texto do editor
- **THEN** o menu aparece junto ao ponteiro com as ações e os atalhos

#### Scenario: Sem seleção
- **WHEN** não há texto selecionado
- **THEN** Recortar e Copiar aparecem desabilitados e Colar continua disponível

#### Scenario: Executar pelo menu
- **WHEN** o usuário escolhe Executar instrução no menu
- **THEN** a instrução sob o cursor é executada como em Ctrl+Enter

### Requirement: Clique direito preserva a seleção
O clique com o botão direito SHALL manter a seleção atual quando o ponteiro estiver dentro dela; fora da seleção, ou sem seleção, SHALL mover o cursor para a posição clicada antes de abrir o menu.

#### Scenario: Clique dentro da seleção
- **WHEN** há uma seleção e o usuário clica com o botão direito sobre ela
- **THEN** a seleção continua a mesma e o menu age sobre ela

#### Scenario: Clique fora da seleção
- **WHEN** o usuário clica com o botão direito em outra palavra
- **THEN** o cursor vai para a palavra clicada e o menu considera essa palavra

### Requirement: Ações do objeto sob o cursor
Quando o texto selecionado ou o identificador na posição do clique corresponder a uma tabela, view ou outro objeto conhecido da conexão da aba (pela mesma regra de busca de F4), o menu SHALL ganhar uma seção com o nome e o tipo do objeto e as ações que o navegador oferece para ele: para tabela e view, abrir a tabela (dados), novo script SELECT, diagrama ER, gerar SQL, exportar dados, copiar nome; para view e objetos com código, ver código-fonte; para procedure e função, gerar chamada.

#### Scenario: Tabela sob o clique
- **WHEN** o usuário clica com o botão direito em `pedidos`, uma tabela da conexão
- **THEN** o menu mostra a seção "pedidos (tabela)" com Abrir tabela, Novo script SELECT, Diagrama ER, Gerar SQL, Exportar dados e Copiar nome

#### Scenario: Procedure sob o clique
- **WHEN** o identificador clicado é uma procedure
- **THEN** a seção mostra Ver código-fonte, Gerar chamada e Copiar nome

#### Scenario: Palavra que não é objeto
- **WHEN** a palavra clicada é uma palavra-chave, um literal ou um nome desconhecido
- **THEN** o menu mostra só as ações de edição, sem a seção de objeto

#### Scenario: Metadados ainda não carregados
- **WHEN** os metadados da conexão ainda estão sendo carregados em segundo plano
- **THEN** o menu abre sem esperar o banco e mostra só as ações de edição
