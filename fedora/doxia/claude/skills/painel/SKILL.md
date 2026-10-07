---
name: painel
description: Cria, edita ou remove ações do Painel Rápido do DoxIA (o raio na barra, SUPER+ALT+P, comando doxia-painel) — formulários de chamado no Jira Service Management, listas de tarefas do Jira por JQL, disparo de pipelines no GitLab com variáveis, links e comandos. Use quando o usuário pedir para colocar um atalho/ação/formulário no painel, mudar campos, trocar a ação favorita, ajustar o prompt do "Melhorar com Claude" ou mexer em ~/.config/omarchy/painel.json.
---

# Painel Rápido

O app é `doxia-painel` (fonte em `$OMARCHY_PATH/fedora/doxia/painel`, GTK4 +
libadwaita, app id `org.doxia.Painel`). Ele é genérico: **todas as ações vêm de
`~/.config/omarchy/painel.json`**, que é pessoal e não vai para o repositório.
Seu trabalho nesta skill é quase sempre editar esse arquivo; só mexa no código
do app se o usuário pedir um tipo de ação que não existe.

Abrir: clique no raio da barra (plugin `doxia.painel`), `SUPER+ALT+P` (abre a
ação `favorite`) ou `doxia-painel [<id> | --favorito]`. O app relê o arquivo ao
abrir uma janela nova; com ele aberto, o botão ↻ da tela inicial (Ctrl+R)
recarrega. Na tela inicial, Alt+1…9 abre as nove primeiras ações.

## Formato

```json
{
  "favorite": "sre",                       // opcional: ação do SUPER+ALT+P e do clique direito no raio
  "jira": { "site": "https://empresa.atlassian.net" },  // site padrão das ações do Jira
  "actions": [ … ]                         // na ordem em que aparecem na tela inicial
}
```

Campos comuns a toda ação: `id` (a-z 0-9 - _, único; não use `home` nem
`credenciais`), `type`, `title`, e opcionais `subtitle` e `icon` (nome de ícone
simbólico do tema, ex. `dialog-warning-symbolic`, `view-list-symbolic`,
`media-playback-start-symbolic`, `web-browser-symbolic`,
`utilities-terminal-symbolic`, `mail-send-symbolic`, `system-run-symbolic`).
Comentários `//` aqui são só explicação: o arquivo real é JSON puro.

### `jira-request` — chamado no Jira Service Management

```json
{
  "id": "sre", "type": "jira-request", "title": "Abrir chamado SRE",
  "subtitle": "Service Desk · portal 217",
  "serviceDesk": "217", "requestType": "1235",
  "site": "https://outra.atlassian.net",       // opcional, senão jira.site
  "portalUrl": "https://…/portal/217/group/525/create/1235",  // opcional: botão "abrir no portal"
  "formTitle": "SRE · Solicitação", "formDescription": "Todos os campos são obrigatórios.",
  "submitLabel": "Abrir chamado",              // opcional
  "fields": [
    { "key": "summary", "label": "Resumo" },
    { "key": "customfield_10151", "label": "Aplicação" },
    { "key": "description", "label": "Descrição", "multiline": true, "hint": "texto de ajuda" },
    { "key": "customfield_10200", "label": "Ticket relacionado", "required": false }
  ],
  "claude": {                                  // opcional: botão "Melhorar com Claude" (Ctrl+E)
    "field": "description",                    // campo que o Claude reescreve
    "promptFile": "~/.config/omarchy/painel-prompts/sre.txt",  // ou "prompt": "texto"
    "label": "Melhorar com Claude", "tooltip": "…", "model": "sonnet"
  }
}
```

- Os valores vão como `requestFieldValues` em `POST /rest/servicedeskapi/request`;
  campos vazios e não obrigatórios não são enviados. `required` é `true` por padrão.
- Só campos de texto (linha única ou `multiline`). Campos de seleção/usuário do
  Jira precisam de outro formato de valor: avise o usuário em vez de fingir que
  funciona.
- O Claude roda `claude -p` sem ferramentas; recebe os outros campos como
  contexto (`rótulo: valor`) e o texto do campo alvo. Prompts longos ficam em
  arquivo em `~/.config/omarchy/painel-prompts/`; o prompt deve pedir texto
  puro e só a resposta final.

### `jira-tasks` — lista de tarefas por JQL

```json
{ "id": "tarefas", "type": "jira-tasks", "title": "Minhas tarefas",
  "jql": "assignee = currentUser() AND (statusCategory != Done OR resolved >= -14d) ORDER BY updated DESC" }
```

Agrupa por categoria (Em andamento / A fazer / Concluídas) e status, com filtro,
link e cópia do link. Sem `jql`, usa as tarefas abertas do usuário.

### `gitlab-pipeline` — iniciar pipeline com variáveis

```json
{ "id": "spark", "type": "gitlab-pipeline", "title": "Iniciar pipeline do Spark",
  "project": "75072547",                        // id numérico ou caminho "grupo/projeto"
  "host": "gitlab.com",                         // opcional
  "projectUrl": "https://gitlab.com/grupo/projeto",  // opcional (botão de pipelines)
  "defaultRef": "dev",
  "variables": [
    { "key": "NAMESPACE", "default": "projetos", "hint": "ex.: projetos, gladiator" },
    { "key": "CONTROLE", "label": "Controle", "default": "projetos3", "required": false }
  ] }
```

Usa o token do `glab` para o host (`glab auth login`). As últimas 6 combinações
ficam em `~/.local/state/doxia-painel/<id>.json` e aparecem em "Recentes".

### `link` e `command`

```json
{ "id": "grafana", "type": "link", "title": "Grafana", "url": "https://…" }
{ "id": "vpn-log", "type": "command", "title": "Log da VPN",
  "run": "kitty -e journalctl -fu 'openfortivpn@*'", "dir": "~" }
```

`run` é uma string (roda com `bash -c`) ou lista de argumentos. O painel fecha
depois de disparar.

## Descobrindo IDs

As credenciais do Jira ficam em `~/.config/omarchy/painel-credenciais.json`
(modo 600, `{ "<site>": { "email", "token" } }`), criadas pelo próprio app na
primeira vez. **Nunca copie o token para o painel.json, para a conversa ou para
comandos visíveis**; leia do arquivo dentro do comando:

```bash
site=https://empresa.atlassian.net
auth=$(jq -r --arg s "$site" '.[$s] | "\(.email):\(.token)"' ~/.config/omarchy/painel-credenciais.json)
jira() { curl -sf -u "$auth" -H 'Accept: application/json' "$site$1"; }
jira /rest/servicedeskapi/servicedesk | jq '.values[] | {id, projectKey, projectName}'
jira /rest/servicedeskapi/servicedesk/217/requesttype | jq '.values[] | {id, name}'
jira /rest/servicedeskapi/servicedesk/217/requesttype/1235/field \
  | jq '.requestTypeFields[] | {fieldId, name, required, type: .jiraSchema.type}'
```

Se não houver credencial para o site, peça ao usuário para abrir a ação uma vez
no painel (ele pede e-mail + API token) em vez de pedir o token na conversa.
Um link de portal `…/portal/<serviceDesk>/…/create/<requestType>` já traz os
dois IDs. Para o GitLab: `glab api "projects/$(jq -rn --arg p grupo/projeto '$p|@uri')" | jq .id`.

## Depois de editar

1. Valide: `jq empty ~/.config/omarchy/painel.json`.
2. Abra a ação para conferir: `doxia-painel <id>` (se o painel já estiver
   aberto, use ↻ na tela inicial). Ações malformadas são ignoradas e o motivo
   aparece num aviso na tela inicial.
3. Não envie formulários nem dispare pipelines para testar: isso cria chamados
   e pipelines de verdade. Testar `jira-tasks` (só leitura) é seguro.
