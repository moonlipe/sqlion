# Spec Delta

## Purpose

Define o comportamento comum das janelas e diálogos do Sqlion, a página de boas-vindas, a tela Sobre e a posição dos botões globais da barra de ferramentas.

## ADDED Requirements

### Requirement: Janelas móveis e redimensionáveis
Toda janela ou diálogo do aplicativo SHALL poder ser arrastada pelo título e redimensionada pelas bordas, com tamanho mínimo que mantém o conteúdo utilizável. A janela SHALL abrir centralizada na primeira vez.

#### Scenario: Arrastar uma janela
- **WHEN** o usuário arrasta o título de qualquer diálogo (preferências, conexão, exportar, sobre, confirmações)
- **THEN** a janela se move e permanece na nova posição enquanto estiver aberta

#### Scenario: Redimensionar
- **WHEN** o usuário arrasta a borda ou o canto de uma janela
- **THEN** a janela muda de tamanho e seu conteúdo se ajusta, com rolagem quando não couber

#### Scenario: Janela maior que a tela
- **WHEN** a janela do aplicativo é pequena
- **THEN** os diálogos continuam acessíveis, com rolagem interna

### Requirement: Botões globais à direita
Os botões de preferências e de alternar tema SHALL ficar alinhados ao canto direito da barra de ferramentas, em qualquer largura de janela em que a barra caiba em uma linha. Em janelas estreitas SHALL passar para a linha seguinte sem sobrepor outros elementos.

#### Scenario: Janela larga
- **WHEN** a janela tem largura suficiente
- **THEN** preferências e tema aparecem juntos na borda direita da barra

#### Scenario: Janela estreita
- **WHEN** a largura não comporta a barra inteira
- **THEN** a barra quebra de linha e nenhum botão fica cortado ou sobreposto

### Requirement: Boas-vindas sem estouro
A página de boas-vindas SHALL caber na largura disponível: botões, lista de bancos suportados e textos de ajuda SHALL quebrar de linha em vez de ultrapassar a página, e a página SHALL rolar na vertical quando necessário.

#### Scenario: Janela estreita
- **WHEN** a janela tem 800 pixels de largura
- **THEN** nenhum texto ou botão da boas-vindas ultrapassa a área visível

### Requirement: Tela Sobre atualizada
A tela Sobre SHALL mostrar nome, versão do aplicativo, descrição, bancos suportados na versão atual, licença GPL-3.0-or-later, o crédito de inspiração ao DBeaver Community (Apache-2.0, sem afiliação) e as principais bibliotecas utilizadas.

#### Scenario: Abrir Sobre
- **WHEN** o usuário abre Ajuda › Sobre
- **THEN** vê a versão do `Cargo.toml`, a lista de bancos (SQLite, PostgreSQL, MySQL/MariaDB, SQL Server, MongoDB), a licença e o aviso de não afiliação
