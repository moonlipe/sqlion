# Spec Delta

## Purpose

Define os atalhos de teclado no estilo de IDE do Sqlion para abrir scripts e objetos do banco sem passar pelos menus.

## ADDED Requirements

### Requirement: Ctrl+T abre um script vazio
Ao pressionar Ctrl+T (Cmd+T no macOS) em qualquer lugar da janela principal, o sistema SHALL abrir uma nova aba de script SQL vazia e ativá-la, ligada à conexão atual e gravada no mesmo lugar em que Ctrl+] cria novos scripts. O foco SHALL ir para o editor da nova aba.

#### Scenario: Novo script com conexão atual
- **WHEN** a aba ativa é um script da conexão "vendas" e o usuário pressiona Ctrl+T
- **THEN** uma nova aba de script vazia, ligada a "vendas", é aberta e fica ativa, com o cursor no editor

#### Scenario: Sem abas abertas
- **WHEN** não há abas abertas e o usuário pressiona Ctrl+T
- **THEN** uma nova aba de script vazia é aberta, ligada à conexão selecionada no navegador ou, sem seleção, à última usada

#### Scenario: Script vazio fechado
- **WHEN** o usuário fecha a aba aberta por Ctrl+T sem digitar nada
- **THEN** nenhum arquivo de script fica no disco

### Requirement: F4 abre o objeto sob o cursor
Ao pressionar F4 no editor SQL, o sistema SHALL identificar o nome do objeto selecionado ou, sem seleção, o identificador sob o cursor (aceitando a forma `schema.nome` e nomes entre aspas, colchetes ou crases) e procurá-lo nos metadados da conexão da aba. Uma tabela SHALL abrir a aba da tabela. Uma view, procedure, função, trigger ou outro objeto com código SHALL abrir uma aba com a sua definição. Sem schema no nome, a busca SHALL usar primeiro o schema padrão da conexão e depois os demais schemas carregados.

#### Scenario: Tabela sob o cursor
- **WHEN** o cursor está em `clientes` em `SELECT * FROM clientes` e o usuário pressiona F4
- **THEN** a aba da tabela `clientes` é aberta (ou ativada, se já estiver aberta)

#### Scenario: View com schema
- **WHEN** o texto selecionado é `relatorio.vw_vendas`, uma view, e o usuário pressiona F4
- **THEN** uma aba com a definição da view é aberta

#### Scenario: Procedure
- **WHEN** o cursor está no nome de uma procedure em `EXEC atualiza_saldo 10` e o usuário pressiona F4
- **THEN** uma aba com o código da procedure é aberta

#### Scenario: Objeto desconhecido
- **WHEN** o identificador sob o cursor não corresponde a nenhum objeto conhecido, ou a aba não tem conexão aberta
- **THEN** nada é aberto e a barra de status informa que o objeto não foi encontrado ou que a conexão está fechada

#### Scenario: F4 no navegador
- **WHEN** uma tabela ou view está selecionada no navegador, o foco não está no editor, e o usuário pressiona F4
- **THEN** a tabela abre a aba da tabela e a view abre a sua definição

### Requirement: F3 escolhe um script do projeto
Ao pressionar F3, o sistema SHALL abrir uma janela com a lista dos scripts do projeto da aba ativa (ou, sem aba de script, do projeto da conexão atual; sem projeto, dos scripts soltos), ordenados pela última modificação, com um campo de filtro por nome já focado. Enter ou clique duplo SHALL abrir o script escolhido (ou ativar a aba se já estiver aberto); Esc SHALL fechar a janela sem abrir nada.

#### Scenario: Abrir pela lista
- **WHEN** a aba ativa pertence ao projeto "financeiro" e o usuário pressiona F3, digita "fecha" e Enter
- **THEN** o primeiro script de "financeiro" cujo nome contém "fecha" é aberto em uma aba

#### Scenario: Script já aberto
- **WHEN** o script escolhido na lista já está aberto em outra aba
- **THEN** essa aba é ativada, sem abrir uma segunda

#### Scenario: Projeto sem scripts
- **WHEN** o projeto não tem scripts
- **THEN** a janela mostra que não há scripts e oferece criar um novo

#### Scenario: Navegação por teclado
- **WHEN** a janela está aberta e o usuário usa as setas para cima e para baixo
- **THEN** a seleção percorre a lista filtrada e Enter abre o item selecionado
