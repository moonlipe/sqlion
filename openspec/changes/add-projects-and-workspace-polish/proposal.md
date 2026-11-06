# Proposal

## Why

O dia a dia no Sqlion expôs lacunas de uso: janelas que não se movem nem se redimensionam, a página de boas-vindas com texto saindo da tela, nenhum jeito visível de desconectar ou reconectar, abas perdidas ao fechar o app, conexões todas com a mesma aparência e scripts sem organização por projeto. Também não há como levar a configuração de um computador para outro.

## What Changes

- Botões de preferências e de tema alinhados ao canto direito da barra de ferramentas.
- Todas as janelas e diálogos passam a poder ser arrastados e redimensionados (hoje ficam ancorados ao centro e fixos).
- Página de boas-vindas passa a caber na largura disponível, quebrando linhas em vez de ultrapassar a página.
- Botões **Desconectar** e **Reconectar** (reconectar invalida a sessão e as caches de metadados) visíveis na barra de ferramentas, ao lado de "Nova conexão", e no menu de contexto da conexão.
- As abas abertas são salvas ao fechar e reabertas na próxima sessão como estavam; existe uma preferência para desligar isso.
- Ícone e cor de cada conexão podem ser alterados pelo usuário.
- **Projetos**: um projeto é uma pasta no disco com as conexões (sem senha) e os scripts, pronta para ser versionada com git e compartilhada por um repositório. O que existe hoje vai para um projeto padrão.
- Senhas, histórico e estado das abas ficam apenas na máquina do usuário; o projeto não os carrega. A senha é pedida no primeiro uso de uma conexão vinda de um clone.
- Criar projeto em uma pasta e abrir uma pasta existente ou clonada como projeto, com status e commit git da pasta.
- Exportar e importar projetos em um arquivo `.sqlion` para backup e transporte pontual, com seleção do que vai (conexões, senhas ofuscadas, scripts, preferências, aparência, histórico).
- Tela "Sobre" atualizada: versão, licença GPL-3.0, créditos ao DBeaver, bancos suportados atuais, atalhos úteis.
- **BREAKING (dados)**: o formato de `workspace.json` ganha projetos e `project` em cada conexão; a leitura continua aceitando o formato antigo e migra para o projeto padrão.

## Capabilities

### New Capabilities
- `window-behavior`: janelas móveis/redimensionáveis, boas-vindas sem estouro, botões de preferências/tema à direita e tela Sobre.
- `connection-lifecycle`: desconectar e reconectar conexões de forma visível, com invalidação de sessão e caches.
- `connection-appearance`: ícone e cor personalizáveis por conexão.
- `session-restore`: persistência e reabertura das abas entre sessões, configurável.
- `projects`: projetos em pasta (conexões sem senha e scripts), versionáveis com git, e exportação e importação seletivas em `.sqlion`.

### Modified Capabilities

(nenhuma)

## Impact

- `src/store.rs`: modelo (`Project`, `session`, preferências `restore_tabs`), migração do formato antigo, armazenamento local de senhas por id de conexão, leitura e gravação da pasta do projeto.
- `src/git.rs`: reaproveitado para status e commit da pasta do projeto.
- `src/app.rs`: barra de ferramentas, boas-vindas, navegador (árvore por projeto, menu de conexão), salvar/restaurar abas ao sair.
- `src/dialogs.rs`: janelas padronizadas (`Window` com arrastar/redimensionar), diálogo de conexão (ícone e cor), exportar/importar projeto, Sobre, Preferências.
- `src/db.rs`, `src/shared.rs`: campos de aparência e projeto em `ConnConfig`, reconexão.
- `src/tabs.rs`: serialização do estado das abas.
- Sem novas dependências obrigatórias (JSON via `serde_json`).
