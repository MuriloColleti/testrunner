# TestRunner

Aplicativo desktop para executar, monitorar e agendar suites de testes automatizados. Construído com **Tauri + Vue 3** para Windows.

---

## Sumário

1. [O que é o TestRunner](#o-que-é-o-testrunner)
2. [Instalação](#instalação)
3. [Como adicionar um projeto](#como-adicionar-um-projeto)
4. [Estrutura de projeto suportada](#estrutura-de-projeto-suportada)
   - [Playwright — layout simples](#playwright--layout-simples)
   - [Playwright — layout multi-módulo (Znap)](#playwright--layout-multi-módulo-znap)
   - [Vitest](#vitest)
   - [Scripts npm personalizados](#scripts-npm-personalizados)
5. [Detecção automática de suites](#detecção-automática-de-suites)
6. [Executando testes](#executando-testes)
7. [Dashboard](#dashboard)
8. [Agendamentos](#agendamentos)
9. [Requisitos do ambiente](#requisitos-do-ambiente)
10. [Banco de dados](#banco-de-dados)

---

## O que é o TestRunner

O TestRunner centraliza todos os seus projetos de teste em um único lugar. Com ele você pode:

- **Executar** qualquer suite com um clique, ver o output em tempo real e acompanhar o progresso (passou / falhou / restantes)
- **Parar** uma execução a qualquer momento
- **Visualizar o histórico** de execuções no Dashboard com métricas de taxa de aprovação, cobertura e tendência
- **Exportar relatórios** em PDF ou Excel
- **Agendar** execuções automáticas por data/hora, de forma recorrente (diário ou semanal)

---

## Instalação

1. Baixe o instalador `TestRunner_x.x.x_x64-setup.exe` em `src-tauri/target/release/bundle/nsis/`
2. Execute o instalador e siga as instruções
3. Na primeira abertura, clique em **+** na sidebar para adicionar seu primeiro projeto

---

## Como adicionar um projeto

1. Clique no botão **+** no canto superior da sidebar
2. Preencha o **Nome** do projeto (ex: `Nissin`, `Venturus`)
3. Informe ou navegue até o **Caminho** da pasta raiz do projeto
4. Clique em **Verificar Caminho** — o sistema varre a pasta e detecta automaticamente as suites
5. Se encontrar suites, o botão **Salvar** é habilitado
6. Clique em **Salvar**

> Para editar ou remover um projeto, passe o mouse sobre o nome dele na sidebar e clique no ícone de lápis.

---

## Estrutura de projeto suportada

O sistema varre a pasta raiz do projeto procurando arquivos de configuração conhecidos. A profundidade máxima de varredura é **4 níveis**.

As seguintes pastas são **ignoradas** automaticamente:
`node_modules`, `target`, `dist`, `.git`, `.next`, `build`, `coverage`, `.cache`, `out`, `.turbo`, `.svelte-kit`

---

### Playwright — layout simples

Qualquer projeto com um dos arquivos abaixo na raiz (ou em subpastas) é detectado como suite E2E:

```
playwright.config.ts
playwright.config.js
playwright.config.mjs
```

**Exemplo de estrutura:**
```
meu-projeto/
├── playwright.config.ts   ← detectado aqui
├── tests/
│   └── login.spec.ts
└── package.json
```

O sistema executa automaticamente:
```bash
npx playwright test --reporter=list
```

A suite aparece na sidebar com a tag **E2E**.

---

### Playwright — layout multi-módulo (Znap)

Se o projeto Playwright tiver uma pasta `modules/` na raiz, o sistema detecta **cada módulo separadamente**, criando uma suite por módulo para Frontend e/ou Backend:

```
projeto/
├── playwright.config.ts
├── modules/
│   ├── nissin/
│   │   ├── frontend/          ← gera suite E2E "Nissin · Frontend"
│   │   │   └── *.spec.ts
│   │   └── backend/           ← gera suite API "Nissin · Backend API"
│   │       └── *.spec.ts
│   ├── venturus/
│   │   └── frontend/          ← gera suite E2E "Venturus · Frontend"
│   │       └── *.spec.ts
│   └── shared/                ← ignorado
└── package.json
```

**Comandos gerados automaticamente:**

| Pasta encontrada | Tag | Comando executado |
|---|---|---|
| `modules/nissin/frontend/` | E2E | `npx playwright test modules/nissin/frontend --project=chromium --reporter=list` |
| `modules/nissin/backend/` | API | `npx playwright test modules/nissin/backend --project=api --reporter=list` |

> A pasta `shared` dentro de `modules/` é ignorada automaticamente.

**Requisito:** o `playwright.config.ts` deve ter os projects `chromium` (para frontend) e `api` (para backend) configurados:

```ts
// playwright.config.ts
import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'api',
      use: { /* sem browser */ },
    },
  ],
})
```

---

### Vitest

Projetos com qualquer um dos arquivos abaixo são detectados como suite Unit:

```
vitest.config.ts
vitest.config.js
vitest.config.mjs
vitest.config.cjs
```

**Exemplo de estrutura:**
```
meu-projeto/
├── vitest.config.ts      ← detectado aqui
├── src/
│   └── utils.test.ts
└── package.json
```

O sistema executa:
```bash
npx vitest run --reporter=verbose
```

**Cobertura de código (opcional):** se o `vitest.config.ts` contiver a palavra `coverage:`, o sistema adiciona `--coverage` automaticamente:

```ts
// vitest.config.ts
export default defineConfig({
  test: {
    coverage: {           // ← presença desta chave ativa --coverage
      provider: 'v8',
      reporter: ['text', 'json-summary'],
    },
  },
})
```

A suite aparece na sidebar com a tag **Unit**. A cobertura lida do arquivo `coverage/coverage-summary.json` é exibida no Dashboard.

---

### Scripts npm personalizados

Se o projeto não tiver `playwright.config.*` nem `vitest.config.*`, mas tiver scripts de teste no `package.json`, o sistema os detecta automaticamente.

**Regras de detecção:**
- Somente scripts com nome `test` ou prefixo `test:` são capturados
- Scripts que começam com `echo` (stubs) são ignorados
- Scripts que apenas chamam playwright/vitest (já detectados) são ignorados

**Exemplo:**
```json
{
  "scripts": {
    "test":          "jest",
    "test:e2e":      "cypress run",
    "test:unit":     "jest --unit",
    "test:api":      "newman run collection.json",
    "build":         "vite build"    ← ignorado (não é test:*)
  }
}
```

**Tags atribuídas automaticamente por nome do script:**

| Contém no nome | Tag |
|---|---|
| `e2e` | E2E |
| `unit` | Unit |
| `api` | API |
| outro | Unit |

---

## Detecção automática de suites

Resumo da lógica de varredura:

```
pasta raiz do projeto
├── tem playwright.config.*?
│   ├── tem subpasta modules/?
│   │   └── cria suite por módulo (frontend/backend)
│   └── não tem modules/ → cria 1 suite E2E geral
├── tem vitest.config.*?
│   └── cria 1 suite Unit (com coverage se configurado)
├── tem package.json com scripts test:*?
│   └── cria 1 suite por script (ignora duplicatas de playwright/vitest)
└── nenhum encontrado aqui? → varre subpastas (até profundidade 4)
```

> **Importante:** quando um arquivo de configuração é encontrado em um diretório, o sistema **não varre subpastas** daquele diretório — a configuração encontrada delimita o escopo de teste daquela pasta.

---

## Executando testes

- Clique no **botão ▶ Play** ao lado de uma suite na sidebar para iniciar
- O sistema abre automaticamente uma aba de terminal com o output em tempo real
- O contador de **Passou / Falhou / Restantes** atualiza linha a linha
- Para **parar**, clique no botão de pausa (fica visível durante a execução)
- Várias suites podem rodar **em paralelo** ao mesmo tempo

**Abas de terminal:**
- Cada suite em execução ou executada abre uma aba própria
- Clique no **×** para fechar uma aba (interrompe a execução se estiver rodando)
- As cores do output identificam o tipo de linha:

| Cor | Significado |
|---|---|
| Verde | Teste passou (`ok N` / `✓`) |
| Vermelho | Teste falhou (`x N` / `✗`) |
| Amarelo | Teste em andamento |
| Cinza | Informação / stack trace |

---

## Dashboard

Acesse pela sidebar clicando em **Dashboard**.

Mostra o histórico de todas as execuções salvas:

- **Total Executados** — quantidade de execuções no período
- **Taxa de Aprovação** — média de testes passando
- **Cobertura Média** — média de cobertura de código (quando disponível)
- **Melhor Suite** — suite com maior taxa de aprovação

**Filtros disponíveis:**
- Por projeto
- Por período (últimos 7 dias, 30 dias, 90 dias, ou tudo)

**Tabela de comparação** mostra cada suite com: último status, total de execuções, média de passou/falhou, tendência (subindo ↑ / caindo ↓ / estável), duração média e cobertura.

**Exportar:** botão **Exportar Excel** gera um `.xlsx` com todos os dados do período filtrado.

---

## Agendamentos

Acesse pela sidebar clicando em **Agendamentos**.

Permite configurar execuções automáticas de qualquer suite em data e hora específicas.

### Criando um agendamento

Clique em **+ Novo Agendamento** e preencha:

| Campo | Descrição |
|---|---|
| **Nome** | Identificação do agendamento (ex: "Nightly E2E Nissin") |
| **Projeto** | Projeto cadastrado no sistema |
| **Suite** | Suite daquele projeto a ser executada |
| **Recorrência** | Uma vez / Diário / Semanal |

**Campos de data/hora variam conforme a recorrência:**

| Recorrência | Campos exibidos | Comportamento |
|---|---|---|
| **Uma vez** | Data + Hora | Executa exatamente na data e hora informadas, depois desativa |
| **Diário** | Somente Hora | Executa todos os dias no horário informado. Se o horário já passou hoje, agenda para amanhã |
| **Semanal** | Dia da semana + Hora | Executa toda semana no dia e horário escolhidos. Calcula automaticamente a próxima ocorrência |

### Gerenciando agendamentos

- **Toggle on/off** — desativa temporariamente sem excluir
- **Lápis** — edita o agendamento
- **Lixeira** — remove o agendamento
- **Filtro por dia** — clique em um dia no calendário para ver só os agendamentos daquele dia

### Como funciona por baixo

O sistema verifica a cada **30 segundos** se há algum agendamento com horário vencido. Quando encontra:

1. Atualiza o `last_run_at` no banco
2. Avança a data para a próxima ocorrência (diário/semanal) ou desativa (uma vez)
3. Dispara a execução automaticamente — exatamente igual a clicar Play manualmente
4. O resultado aparece nas abas de terminal e é salvo no histórico do Dashboard

> O app precisa estar **aberto** para os agendamentos dispararem.

---

## Requisitos do ambiente

Para que as suites sejam executadas corretamente, o ambiente precisa ter:

| Ferramenta | Necessário para |
|---|---|
| **Node.js** | Todos os projetos |
| **npm** | Todos os projetos |
| **npx** | Playwright e Vitest |
| **@playwright/test** | Suites Playwright (instalado no projeto) |
| **vitest** | Suites Vitest (instalado no projeto) |
| **Browsers Playwright** | `npx playwright install` no projeto |

O comando é sempre executado a partir da **pasta do projeto** (ou subpasta configurada em `cwd`), então as dependências precisam estar instaladas com `npm install` no projeto.

---

## Banco de dados

Os dados são armazenados localmente em SQLite, sem nenhuma dependência externa:

**Localização:** `%APPDATA%\com.testrunner.app\testrunner.db`

**Tabelas:**

| Tabela | Conteúdo |
|---|---|
| `projects` | Projetos cadastrados (nome, caminho, suites em JSON) |
| `test_runs` | Histórico de execuções (status, duração, passou/falhou, output) |
| `schedules` | Agendamentos (projeto, suite, horário, recorrência) |

Todos os dados ficam na máquina local — nada é enviado para a nuvem.
