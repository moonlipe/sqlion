# Spec Delta

## Purpose

Define como o editor SQL colore o texto: quais categorias de token existem, como são reconhecidas e que cada uma tem cor própria e legível nos temas claro e escuro.

## ADDED Requirements

### Requirement: Palavras-chave dos dialetos suportados
O editor SHALL destacar como palavra-chave, sem diferenciar maiúsculas de minúsculas, os comandos, cláusulas, controle de fluxo e tipos de dados do SQL padrão e dos dialetos suportados (SQLite, PostgreSQL, MySQL, SQL Server), incluindo no mínimo `EXEC`, `EXECUTE`, `DECLARE`, `DEALLOCATE`, `PREPARE`, `OPEN`, `CLOSE`, `CURSOR`, `PRINT`, `RAISERROR`, `THROW`, `TRY`, `CATCH`, `WHILE`, `RETURN`, `GO`, `MERGE`, `OUTPUT`, `TOP`, `NOLOCK`, `NVARCHAR`, `BIGINT`, `DATETIME`, `DATETIME2`, `BIT` e `UNIQUEIDENTIFIER`.

#### Scenario: Comandos T-SQL
- **WHEN** o texto contém `declare @id int; exec sp_who; deallocate cur`
- **THEN** `declare`, `int`, `exec` e `deallocate` aparecem na cor de palavra-chave

#### Scenario: Palavra dentro de identificador
- **WHEN** o texto contém `execucao_id`
- **THEN** nenhuma parte do nome é colorida como palavra-chave

### Requirement: Funções embutidas
O editor SHALL destacar com uma cor de função, diferente da cor de palavra-chave, os nomes de funções embutidas conhecidas (por exemplo `CONVERT`, `TRY_CONVERT`, `CAST`, `ISNULL`, `COALESCE`, `NULLIF`, `GETDATE`, `DATEADD`, `DATEDIFF`, `SUBSTRING`, `LEN`, `UPPER`, `LOWER`, `TRIM`, `ROUND`, `ROW_NUMBER`, `RANK`, `STRING_AGG`, `NOW`, `TO_CHAR`, `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`). Um nome que também é palavra-chave (como `LEFT`, `RIGHT`, `REPLACE`) SHALL usar a cor de função quando seguido de `(` e a de palavra-chave nos demais casos.

#### Scenario: Função de conversão
- **WHEN** o texto contém `select convert(varchar(10), getdate(), 103)`
- **THEN** `convert` e `getdate` aparecem na cor de função e `select` e `varchar` na de palavra-chave

#### Scenario: LEFT como junção e como função
- **WHEN** o texto contém `left join t on left(t.a, 2) = 'x'`
- **THEN** o primeiro `left` usa a cor de palavra-chave e o segundo a de função

### Requirement: Variáveis e parâmetros
O editor SHALL destacar com uma cor de variável, distinta de todas as outras categorias, as variáveis `@nome`, as variáveis de sistema `@@nome`, os parâmetros `:nome` e as variáveis `${nome}`. O `@` de um e-mail dentro de uma string não SHALL ser tratado como variável.

#### Scenario: Variável T-SQL
- **WHEN** o texto contém `set @total = @@ROWCOUNT`
- **THEN** `@total` e `@@ROWCOUNT` aparecem na cor de variável

#### Scenario: Variável de template
- **WHEN** o texto contém `where id = ${id}`
- **THEN** `${id}`, inclusive `$`, `{` e `}`, aparece na cor de variável

### Requirement: Variáveis `${}` dentro de strings
Uma variável `${nome}` dentro de uma string entre aspas simples ou duplas SHALL aparecer na cor de variável com um fundo de realce, enquanto o restante da string mantém a cor da string. A string continua sendo uma única string para todo o resto (divisão de instruções, comentários, parâmetros).

#### Scenario: Variável no meio da string
- **WHEN** o texto contém `'arquivo_${data}.csv'`
- **THEN** `'arquivo_` e `.csv'` aparecem na cor de string e `${data}` na cor de variável com fundo realçado

#### Scenario: Chaves sem cifrão
- **WHEN** o texto contém `'{"a": 1}'`
- **THEN** toda a string aparece na cor de string

### Requirement: Aspas simples e duplas com cores diferentes
Texto entre aspas simples (literal de string, incluindo `''` escapado e dollar-quoting do PostgreSQL) SHALL usar a cor de string. Texto entre aspas duplas ou crases (identificador citado) SHALL usar:
- a cor de **nome citado conhecido** (azul-água por padrão) quando o conteúdo entre as aspas, sem diferenciar maiúsculas de minúsculas, é uma tabela, view ou coluna conhecida pela regra de "Tabelas, views e colunas";
- a cor de **aspas duplas** (laranja-amarelado por padrão) nos demais casos, inclusive sem conexão ou com a preferência de cores por metadados desligada.

As três cores (string, nome citado conhecido, aspas duplas) SHALL ser distintas entre si.

#### Scenario: Os dois tipos de aspas na mesma linha
- **WHEN** o texto contém `select "Nome Completo" from t where a = 'x'` e `Nome Completo` não é tabela nem coluna conhecida
- **THEN** `"Nome Completo"` aparece em laranja-amarelado e `'x'` na cor de string, visivelmente diferentes

#### Scenario: Tabela e coluna entre aspas duplas
- **WHEN** a conexão tem a tabela `Clientes` com a coluna `Nome` e o texto é `select "Nome" from "Clientes"`
- **THEN** `"Nome"` e `"Clientes"`, com as aspas, aparecem em azul-água

#### Scenario: Aspas duplas sem metadados
- **WHEN** a preferência de cores por metadados está desligada e o texto é `select "Nome" from "Clientes"`
- **THEN** `"Nome"` e `"Clientes"` aparecem em laranja-amarelado

### Requirement: Tabelas, views e colunas
Quando o editor está ligado a uma conexão cujos metadados já estão em cache, o editor SHALL colorir os nomes de tabelas e views conhecidos com uma cor de tabela e os nomes de colunas conhecidas das tabelas citadas no texto com uma cor de coluna; ambas diferentes do texto comum e entre si. Nomes qualificados (`schema.tabela`, `alias.coluna`) SHALL ser reconhecidos pela parte final. Colorir nunca SHALL esperar o banco: o que ainda não está em cache é pedido em segundo plano e a cor aparece quando os metadados chegarem. Sem conexão, ou com a preferência de cores por metadados desligada, esses nomes usam a cor de texto comum.

#### Scenario: Tabela e coluna conhecidas
- **WHEN** a conexão tem a tabela `clientes` com a coluna `nome` e o texto é `select c.nome from clientes c`
- **THEN** `clientes` aparece na cor de tabela, `nome` na cor de coluna e o alias `c` na cor de texto comum

#### Scenario: Metadados ainda não carregados
- **WHEN** o usuário digita o nome de uma tabela cujas colunas ainda não estão em cache
- **THEN** a digitação não trava e as colunas passam a ser coloridas assim que os metadados chegam

### Requirement: Alias de colunas
O editor SHALL colorir com a cor de coluna os alias de colunas definidos nas listas de SELECT do texto (`expr AS nome`, `expr nome`, `CASE … END nome` e `nome = expr` do T-SQL, inclusive em subconsultas e CTEs), tanto na definição quanto nos usos posteriores (`ORDER BY nome`), com ou sem conexão. Alias entre aspas duplas usam a cor de nome citado conhecido. Alias de tabelas não são afetados.

#### Scenario: Alias definido e usado
- **WHEN** o texto é `select count(*) as qtd, v "Bruto" from t order by qtd`
- **THEN** as duas ocorrências de `qtd` aparecem na cor de coluna, `"Bruto"` em azul-água e `t` na cor de texto comum

#### Scenario: Nome inexistente
- **WHEN** o texto cita `tabela_que_nao_existe`
- **THEN** o nome aparece na cor de texto comum

### Requirement: Paleta por tema
Cada categoria (palavra-chave, função, variável, fundo de variável em string, string, aspas duplas, nome citado conhecido, número, comentário, tabela, coluna, texto comum) SHALL ter uma cor padrão no tema escuro e outra no tema claro, todas distintas entre si dentro do mesmo tema; comentários continuam em itálico. O editor SHALL usar sempre a paleta do tema ativo.

#### Scenario: Troca de tema
- **WHEN** o usuário alterna entre tema claro e escuro
- **THEN** o editor recolore o texto com a paleta do tema escolhido, incluindo as cores personalizadas desse tema

### Requirement: Cores personalizáveis por tema
A janela Preferências SHALL ter uma seção "Cores do editor" que mostra, para o tema ativo, um seletor de cor por categoria e uma amostra de SQL colorida com a paleta atual. A cor escolhida SHALL valer só para o tema em que foi definida e SHALL ser salva na hora, como as outras preferências. A seção SHALL ter "Restaurar padrão" por categoria e para o tema inteiro. Categorias nunca personalizadas SHALL seguir a cor padrão da versão instalada.

#### Scenario: Personalizar no tema escuro
- **WHEN** o usuário, no tema escuro, muda a cor de tabela para verde
- **THEN** tabelas aparecem em verde no editor e na amostra, e no tema claro continuam com a cor de tabela do tema claro

#### Scenario: Persistência
- **WHEN** o usuário personaliza cores e reinicia o app
- **THEN** as cores personalizadas de cada tema continuam aplicadas

#### Scenario: Restaurar padrão
- **WHEN** o usuário aciona "Restaurar padrão" do tema
- **THEN** todas as categorias desse tema voltam à cor padrão e as do outro tema não mudam

#### Scenario: Configuração antiga
- **WHEN** o app abre um `workspace.json` sem cores personalizadas
- **THEN** o editor usa as cores padrão dos dois temas
