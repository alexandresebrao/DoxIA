---
name: servicos
description: Adiciona, edita ou remove serviços de desenvolvimento (node ou java) do painel de serviços da barra do Omarchy (ícone de servidor entre a IA e a rede). Use quando o usuário pedir para colocar um projeto no painel, configurar como um serviço inicia, o que fazer ao trocar de branch (compilar, instalar deps, rodar script), ou mexer em ~/.config/omarchy/services.json.
---

# Painel de serviços

O painel é o plugin `alexandre.services` do Quickshell em
`~/.config/omarchy/plugins/alexandre.services/`. Ele só lê
`~/.config/omarchy/services.json` (relido a cada poll, sem reiniciar nada) e
chama o script `svc` (também em `~/.local/bin/svc`).

Cada serviço vira uma unit transitória do systemd do usuário `svc-<id>.service`;
troca de branch e build rodam em `svc-<id>-task.service`. Toda saída vai para
`~/.local/state/omarchy-services/<id>.log` (o botão de log abre um terminal com
`tail -F` dele).

## Formato da config

```json
{
  "services": [
    {
      "id": "api",                      // a-z 0-9 - _ ; vira o nome da unit
      "name": "API de pedidos",         // nome no painel
      "type": "java",                   // "node" ou "java"
      "dir": "~/dev/pedidos-api",       // repositório git do serviço
      "start": "./mvnw spring-boot:run -Dspring-boot.run.profiles=local",
      "env": { "SERVER_PORT": "8080" }, // opcional
      "port": 8080,                     // opcional, só exibido
      "url": "http://localhost:8080",   // opcional
      "java": "21.0.5-tem",             // opcional (java): identificador do sdkman
      "node": "v20.18.0",               // opcional (node): versão/alias do nvm
      "pullOnCheckout": false,          // opcional: git pull --ff-only depois do checkout
      "restartOnBranchChange": "auto",  // "auto" = religa só se estava rodando; "always" = sempre liga
      "onBranchChange": [               // passos em ordem, no dir, com o ambiente do serviço
        { "name": "Instalar deps", "run": "npm ci", "ifChanged": ["package.json", "package-lock.json"] },
        { "name": "Compilar", "run": "./mvnw -q -DskipTests package" }
      ]
    }
  ]
}
```

Comentários `//` acima são só explicação: o arquivo real é JSON puro.

Regras de execução que importam para escrever a config:

- `start` roda com `bash -c` dentro de `dir`, como processo principal da unit.
  Ele precisa ficar em primeiro plano (nada de `&`, `nohup`, `docker compose up -d`).
- Ambiente: `~/.local/bin` no PATH, `env` exportado, depois a versão do runtime:
  1. a escolhida na config: `node` (nvm, em `~/.config/nvm`) ou `java`
     (sdkman: `~/.sdkman/candidates/java/<id>` vira `JAVA_HOME`; `javaHome`
     com caminho absoluto ainda funciona para JDKs fora do sdkman);
  2. sem escolha (“auto”): `mise.toml`/`.tool-versions` via `mise env`, senão
     `.nvmrc` / `.sdkmanrc`, senão o `default` do nvm / o `current` do sdkman.
  O log mostra no start qual node/java foi usado. Não rode `.zshrc`; se algo
  depende dele, coloque no `env` ou num script do projeto.
- A versão também é escolhida no painel (lista ao lado da branch, com as
  versões instaladas) ou por `svc runtime <id> <versão>` (sem versão = auto);
  isso grava `node`/`java` na config e religa o serviço se estiver rodando.
  `svc runtimes` lista o que está instalado. Se o projeto precisa de uma
  versão que não está instalada, sugira `nvm install <v>` ou
  `sdk install java <id>` (`sdk list java` mostra os ids) — não instale sem
  o usuário pedir.
- Ao trocar de branch pelo painel: para o serviço (se estiver rodando),
  `git checkout <branch>` (com fetch se for só remota), roda os passos de
  `onBranchChange` e religa. Passo com `ifChanged` só roda se algum arquivo
  alterado entre o commit antigo e o novo casa com um dos globs (`*` casa `/`
  também, então `"**/pom.xml"` e `"*pom.xml"` funcionam). O botão de build (martelo)
  roda todos os passos sem trocar de branch, ignorando `ifChanged`.
- Se um passo falha, a tarefa para, o serviço fica parado, chega uma
  notificação e o painel mostra "build falhou" até o próximo start/tarefa.

## Como adicionar um serviço

1. Pergunte ou descubra o diretório do projeto. Leia o que existe antes de
   decidir comandos:
   - **node**: `package.json` (scripts `dev`/`start`/`serve`, `engines`),
     `engines.node`/`.nvmrc` (compare com `svc runtimes` e fixe `node` se o
     default do nvm não servir), lockfile para escolher `npm ci` / `pnpm install --frozen-lockfile` /
     `yarn install --frozen-lockfile`, `.nvmrc`, monorepo (workspaces, turbo, nx).
   - **java**: `mvnw`/`pom.xml` ou `gradlew`/`build.gradle(.kts)`; Spring Boot
     (`spring-boot:run` / `bootRun`), versão do Java (`<java.version>`,
     `<maven.compiler.release>`, `toolchain`, `.sdkmanrc`) — compare com
     `svc runtimes` e fixe `java` se o `current` do sdkman não servir —, porta em `application*.yml|properties`,
     profiles locais. Prefira o wrapper (`./mvnw`, `./gradlew`) ao global.
   - Se o projeto tem README/Makefile/scripts com o jeito "oficial" de subir
     local, siga eles.
2. Proponha `start` e `onBranchChange` ao usuário em poucas linhas e pergunte
   só o que não dá para deduzir (profile, porta, se quer build a cada troca).
   Sugestões padrão:
   - node: `{"name":"Instalar deps","run":"npm ci","ifChanged":["package.json","package-lock.json"]}`
     (+ build só se o `start` precisa de artefato compilado).
   - maven: `{"name":"Compilar","run":"./mvnw -q -DskipTests package","ifChanged":["*pom.xml","*src/*"]}`
     — ou sem `ifChanged` se o usuário quer sempre.
   - gradle: `{"name":"Compilar","run":"./gradlew -q assemble"}`.
   - Instruções que o usuário der ("ao trocar, rode scripts/migrate.sh") viram
     passos na ordem dita.
3. Edite `~/.config/omarchy/services.json` preservando os outros serviços (crie
   com `{"services":[]}` se não existir). Use `jq` ou Edit; mantenha 2 espaços.
4. Valide: `svc validate`.
5. Teste se o usuário concordar: `svc start <id>`, espere alguns segundos,
   `svc status | jq '.services[] | select(.id=="<id>")'` e
   `tail -n 40 ~/.local/state/omarchy-services/<id>.log`. Se caiu, corrija a
   config e repita. Pare com `svc stop <id>` se o usuário não pediu para
   deixar rodando.

## Outros comandos

`svc status | branches <id> | runtimes | runtime <id> [versão] | start | stop | restart <id> | checkout <id> <branch> | build <id> | log <id> | validate`

Para remover um serviço: `svc stop <id>` e tire a entrada do JSON.
Para depurar a unit: `systemctl --user status svc-<id>.service`.
O painel em si (QML) está em `Panel.qml` no diretório do plugin; o Quickshell
recarrega ao salvar.
