// Read the file directly from the source path

## Modos de transação

Cada conexão tem um modo, escolhido no diálogo de conexão e na barra de ferramentas:

- **Auto**: cada instrução é confirmada na hora.
- **Manual**: a primeira instrução (inclusive um SELECT) abre uma transação, que fica aberta até Commit ou Rollback.
- **Smart**: só instruções que alteram dados ou estrutura abrem transação; consultas de leitura rodam direto e enxergam as alterações pendentes.

Conexões salvas por versões anteriores migram sozinhas (auto-commit ligado → Auto, desligado → Manual).

## Grid

Clique e arraste, Shift+clique e Ctrl+clique selecionam células e linhas; clicar no título seleciona a coluna (a seta ao lado ordena). Ctrl+C copia a seleção como TSV e Ctrl+V cola blocos (uma única ação de desfazer).

## Licença e créditos

Sqlion é software livre sob a GPL-3.0-or-later (veja `LICENSE`).

Inspirado no comportamento do [DBeaver Community](https://github.com/dbeaver/dbeaver) (Apache-2.0, © DBeaver Corp). Valores padrão e regras de comportamento foram lidos do código-fonte do DBeaver e reimplementados em Rust; as referências `Arquivo.java:linha` estão em `src/constants.rs` e `docs/MECHANICS.md`. Sqlion não é afiliado ao DBeaver nem endossado por seus autores. "DBeaver" é marca registrada de seus titulares.
