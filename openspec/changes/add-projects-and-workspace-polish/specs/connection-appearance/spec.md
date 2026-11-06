# Spec Delta

## Purpose

Permite ao usuário distinguir conexões por ícone e cor.

## ADDED Requirements

### Requirement: Cor e ícone por conexão
Cada conexão SHALL ter cor e ícone configuráveis no diálogo de conexão. Sem personalização, SHALL valer o ícone e a cor padrão do tipo de banco. A personalização SHALL aparecer no navegador, na barra de ferramentas, nas abas e na boas-vindas onde a conexão é exibida.

#### Scenario: Mudar a cor
- **WHEN** o usuário escolhe uma cor para a conexão e salva
- **THEN** o navegador e as abas dessa conexão passam a usá-la

#### Scenario: Mudar o ícone
- **WHEN** o usuário escolhe um ícone entre os disponíveis ou define um rótulo curto de até 3 caracteres
- **THEN** o ícone aparece no lugar do padrão do tipo de banco

#### Scenario: Restaurar o padrão
- **WHEN** o usuário aciona "Restaurar padrão"
- **THEN** a conexão volta à cor e ao ícone do tipo de banco

#### Scenario: Legibilidade
- **WHEN** a cor escolhida é clara ou escura demais para o tema atual
- **THEN** o texto do rótulo continua legível, escolhendo claro ou escuro conforme o contraste
