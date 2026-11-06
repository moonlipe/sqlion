# Tasks

## 1. Janelas e barra de ferramentas

- [x] 1.1 Criar o helper `win()` em `dialogs.rs` e migrar as 14 janelas para ele (sem `anchor`, redimensionáveis, `constrain`); verificar com teste de que nenhuma chamada `Window::new` sobra fora do helper (`grep`) e com captura de tela de Preferências e Conexão
- [x] 1.2 Mover preferências e tema para o canto direito da barra, com recuo para linha própria em janela estreita; verificar por captura de tela em 1280 e 800 px de largura
- [x] 1.3 Reescrever `welcome()` sem larguras fixas, com quebra de linha e rolagem; verificar por captura de tela em 800 px sem texto cortado
- [x] 1.4 Atualizar a tela Sobre (versão do `Cargo.toml`, bancos, licença, créditos); verificar que o texto lista SQL Server e MongoDB e a versão atual, e abrir o diálogo por captura de tela

## 2. Desconectar e reconectar

- [x] 2.1 Implementar `Shared::reconnect` e o diálogo de pendências (Commit/Rollback/Cancelar); verificar com teste que a sessão nova substitui a antiga, as caches são limpas e uma falha deixa a conexão desconectada
- [x] 2.2 Adicionar os botões Desconectar e Reconectar ao lado de "Nova conexão" e entradas no menu de contexto; verificar por captura de tela e que ficam desabilitados sem conexão
- [x] 2.3 Indicar na aba que a conexão está fechada e reabrir ao executar; verificar com teste de executar após desconectar

## 3. Aparência das conexões

- [x] 3.1 Adicionar `color` e `icon` a `ConnConfig` e `ConnConfig::badge()` com contraste automático; verificar com testes de padrão, personalização e contraste
- [x] 3.2 Seletor de cor e ícone no diálogo de conexão, com "Restaurar padrão", e uso nos locais de exibição (navegador, barra, abas, boas-vindas); verificar por captura de tela

## 4. Projetos em pasta

- [x] 4.1 Modelo `Project` com pasta, arquivo `sqlion-project.json` (leitura e gravação estáveis, `format_version`) e `ConnConfig.project`; verificar com testes de ida e volta e de ordem estável do JSON
- [x] 4.2 Migrar o formato antigo para o projeto padrão sem mover arquivos; verificar com teste de carga do `workspace.json` antigo e dos scripts existentes
- [x] 4.3 Segredos locais: tirar senhas do arquivo do projeto, guardar em `secrets.json` por id da conexão e pedir senha ao conectar sem senha local; verificar com testes de que o arquivo do projeto não contém senha e de que a senha pedida é guardada
- [ ] 4.4 Criar projeto em uma pasta, abrir pasta existente ou clonada, fechar, renomear e mover conexões e scripts; verificar com testes sobre pastas temporárias, incluindo vínculo script→conexão pelo id
- [ ] 4.5 Navegador em árvore por projeto com o menu de projeto; verificar por captura de tela
- [ ] 4.6 Git do projeto: status, commit, `init` e `.gitignore` sugerido sobre a pasta do projeto, e recarga ao receber o foco; verificar com testes em repositório temporário e que abas com texto não gravado sobrevivem à recarga
- [ ] 4.7 Formato `.sqlion` e exportação seletiva (conexões, senhas, scripts, preferências, aparência, histórico); verificar com testes de cada seleção, inclusive "sem senhas"
- [ ] 4.8 Importação com prévia, resolução de conflitos, remapeamento de ids e diálogos de exportar e importar no menu Arquivo; verificar com testes de conflito (renomear, substituir, ignorar), arquivo inválido e versão desconhecida, e a ida e volta exportar→importar em workspace vazio

## 5. Restaurar abas

- [ ] 5.1 Serializar o estado das abas (`SavedTab`) e gravar `session.json` ao sair e com limite de frequência; verificar com teste de ida e volta de cada tipo de aba
- [ ] 5.2 Restaurar as abas ao abrir, sem conectar, ignorando conexões inexistentes e arquivo corrompido; verificar com testes desses casos
- [ ] 5.3 Preferência "Reabrir as abas da última sessão" (padrão ligada) em Preferências, apagando o arquivo quando desligada; verificar com teste e por captura de tela

## 6. Integração

- [ ] 6.1 Rodar `cargo test`, `cargo build --release` e percorrer manualmente os cenários das specs; anotar o que ficou de fora
