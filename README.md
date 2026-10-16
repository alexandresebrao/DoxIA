# DoxIA

<p align="center">
  <a href="docs/media/doxia-intro.mp4"><img src="docs/media/doxia-intro.gif" alt="Vídeo de apresentação do DoxIA" width="640"></a>
  <br><sub><a href="docs/media/doxia-intro.mp4">Assista ao vídeo com som</a></sub>
</p>

DoxIA é um desktop para **Fedora** sobre o compositor [mango](https://github.com/mangowm/mango),
com uma casca própria escrita em **Rust + GTK 4**: barra, painéis, notificações, menus,
tela de bloqueio e os apps do dia a dia são binários nativos, sem QML nem JavaScript no
caminho. Tudo em **português do Brasil** e pensado para quem desenvolve: Node.js e Java já
vêm gerenciados, a barra tem um painel para subir os serviços do dia a dia e há uma ISO
própria de instalação. Ele pode ser instalado ao lado de um desktop que já existe, como o
KDE Plasma, sem mexer nele.

O nome é Dox + IA, e o ∞ do logo é um 8 deitado.

Mantido por [@alexandresebrao](https://github.com/alexandresebrao). Problemas e sugestões
vão nas issues deste repositório.

### Feito em Rust

O workspace Cargo em [`fedora/doxia/rs/`](fedora/doxia/rs/) tem os apps do DoxIA. Cada um
roda no seu próprio processo, então atualizar ou reiniciar a casca não fecha nenhum deles.

| Binário | O que é |
| --- | --- |
| `doxia-shell` | A casca: barra, painéis (bateria, tela, som, rede…), notificações, OSD, menu, Iniciar, tela de bloqueio com PAM, agente polkit e os seletores (área de transferência, emojis, imagens) |
| `doxia-atualizar` | A janela **Atualizar DoxIA** |
| `doxia-painel` | O **Painel Rápido** |
| `doxia-theme` | Biblioteca do visual: lê o `colors.toml` e o `shell.toml` do tema e gera o CSS do GTK |

Os binários saem prontos nos [releases](https://github.com/alexandresebrao/DoxIA/releases)
(`doxia-rs-<versão>`), conferidos por sha256 na instalação e na atualização. Para compilar:

```bash
sudo dnf install cargo gtk4-devel gtk4-layer-shell-devel pam-devel
cd fedora/doxia/rs
cargo test --release && cargo build --release
```

Detalhes de cada app, o tema e como publicar uma versão estão no
[README do workspace](fedora/doxia/rs/README.md).

### Recursos

- mango como compositor (wlroots), numa build do DoxIA publicada nos releases deste
  repositório com uma correção para máquinas sem GPU (como VMs do Hyper-V); sessão
  gerenciada pelo uwsm
- Casca em Rust + GTK 4 com vidro translúcido e blur do mango atrás da barra, dos painéis
  e das janelas dos apps
- Menus, painéis, notificações e mensagens de OSD em pt-BR
- Identidade DoxIA: sessão no login, tela de login, logos do Sobre e da proteção de tela,
  saudação do terminal e MOTD
- Pacotes com `dnf` e seletores do Flathub para instalar e remover apps
- Node.js pelo nvm e Java pelo SDKMAN! (uma LTS de cada por padrão), com **Versões do Node
  e do Java** no lançador para escolher a versão padrão, instalar e remover versões e
  instalar um JDK a partir de um arquivo (zip, tar.gz, rpm… como os da Oracle) ou de uma pasta
- **Painel de serviços** na barra (ícone de servidor entre os agentes e a rede) para
  iniciar e parar serviços de desenvolvimento Node.js e Java, trocar a branch do git
  (rodando os passos de build configurados no checkout), escolher a versão do Node.js (nvm)
  ou do Java (SDKMAN!) e abrir o log. Os serviços são adicionados com a skill `/servicos`
  do Claude Code ou em `~/.config/omarchy/services.json`; o comando `svc` controla tudo
  pelo terminal
- zsh como shell de login, com Oh My Zsh, zsh-autosuggestions, zsh-syntax-highlighting e o
  tema DoxIA (pasta, branch do git e as versões do Node.js e do Java em uso); um `~/.zshrc`
  que já exista mantém o tema e os plugins dele
- Janela **Atualizar DoxIA** (menu Atualizar e ícone na barra): mostra o que vai mudar,
  pede a senha uma vez e instala os pacotes na hora ou no próximo reinício; apps Flatpak
  atualizam sem senha. Roda com prioridade baixa para o computador continuar usável
- **Painel Rápido** (`Super + Alt + P`): formulários de chamado do Jira Service Management,
  listas de tarefas do Jira, disparo de pipelines do GitLab, links e comandos, configurados
  com a skill `/painel` do Claude Code ou em `~/.config/omarchy/painel.json`
- Temas LiquidIA (padrão), LiquidOceanIA e DoxIA, com ícones Lucide na barra e nos menus
- Ditado por voz com o Voxtype
- Google Chrome e Claude Code instalados; Firefox, LibreWolf, Evolution (cliente de e-mail
  padrão) e ONLYOFFICE (em pt-BR, no lugar do LibreOffice) vêm do Flathub
- `tuned-ppd` com um pequeno shim de `powerprofilesctl` para o menu de energia

### Instalação

```bash
git clone https://github.com/alexandresebrao/DoxIA.git ~/.local/share/omarchy
sudo bash ~/.local/share/omarchy/fedora/install-system.sh
bash ~/.local/share/omarchy/fedora/install-user.sh
```

Depois saia da sessão e escolha **DoxIA** na tela de login.

O script de usuário faz backup de toda configuração que substitui (`*.bak-omarchy-<data>`)
e não mexe no autostart, nas variáveis de ambiente nem nas configurações do Chromium do KDE.

Para atualizar, use **Atualizar** no menu: ele atualiza o DoxIA (git pull e os apps em Rust), os pacotes do
sistema, os apps Flatpak e as ferramentas do mise, e depois roda o `fedora/update.sh`, que
reaplica tema, identidade visual, fonte de ícones, menu, MOTD e tela de login sem mexer no
layout da barra nem nos plugins dela. O `fedora/update.sh` também pode ser rodado sozinho:
`bash ~/.local/share/omarchy/fedora/update.sh`.

`Super + K` mostra os atalhos e `Super + Espaço` abre o menu.

### ISO de instalação

`fedora/iso/build.sh` gera um instalador DoxIA: o netinstall do Fedora (Anaconda)
reconstruído com lorax com o nome DoxIA e os logos genéricos no lugar dos do Fedora. Na
frente do Anaconda roda um assistente próprio, no estilo do Windows 98, que pergunta
idioma, rede, disco (o disco inteiro ou partições feitas à mão no blivet-gui), conta de
usuário e nome do computador, e depois mostra o progresso da instalação. O kickstart usa
teclado brasileiro e horário de São Paulo e clona a versão mais recente da branch `fedora`
deste repositório para rodar os scripts de instalação acima. Em seguida atualiza todos os pacotes, então o primeiro boot já sai atualizado.
A instalação precisa de internet.

```bash
sudo dnf install lorax lorax-templates-generic
sudo bash ~/.local/share/omarchy/fedora/iso/build.sh ~/doxia-iso
```

O GitHub Actions só gera a ISO quando uma tag é enviada (`git tag v1.2 && git push origin v1.2`)
ou quando o workflow **DoxIA ISO** é iniciado manualmente; pushes na `fedora` não geram ISO.

O desktop é configurado para a conta de usuário criada no assistente.
`fedora/iso/make-installer-art.py` gera de novo as imagens do instalador.

### Créditos

Nasceu como um fork do [Omarchy](https://github.com/basecamp/omarchy) (MIT, © DHH), não
afiliado à Basecamp. Os nomes `omarchy-*` dos comandos e das pastas
(`~/.local/share/omarchy`, `~/.config/omarchy`) ficaram por compatibilidade. O manual, em
inglês, está em [`manual/`](manual/).

### Licença

[MIT](LICENSE).
