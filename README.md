# DoxIA

<p align="center">
  <a href="docs/media/doxia-intro.mp4"><img src="docs/media/doxia-intro.gif" alt="Vídeo de apresentação do DoxIA" width="640"></a>
  <br><sub><a href="docs/media/doxia-intro.mp4">Assista ao vídeo com som</a></sub>
</p>

DoxIA é um desktop Hyprland + Quickshell para **Fedora**, todo em **português do Brasil**,
pensado para quem desenvolve: Node.js e Java já vêm gerenciados, a barra tem um painel
para subir os serviços do dia a dia e há uma ISO própria de instalação. Ele pode ser
instalado ao lado de um desktop que já existe, como o KDE Plasma, sem mexer nele.

O nome é Dox + IA, e o ∞ do logo é um 8 deitado.

Mantido por [@alexandresebrao](https://github.com/alexandresebrao). Problemas e sugestões
vão nas issues deste repositório.

### Recursos

- Hyprland, Quickshell, uwsm e gpu-screen-recorder vindos do COPR
- Menus, painéis, notificações e mensagens de OSD em pt-BR
- Identidade DoxIA: sessão no login, tela de login, logos do Sobre e da proteção de tela,
  saudação do terminal e MOTD
- Pacotes com `dnf` (helpers `omarchy-pkg-*` e scripts de atualização) e seletores do
  Flathub (`omarchy-pkg-flatpak-install` / `-remove`) para instalar e remover apps
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
- ONLYOFFICE do Flathub, em pt-BR
- `tuned-ppd` com um pequeno shim de `powerprofilesctl` para o menu de energia
- Tela de bloqueio com PAM ajustado para o Fedora

### Instalação

```bash
git clone https://github.com/alexandresebrao/DoxIA.git ~/.local/share/omarchy
sudo bash ~/.local/share/omarchy/fedora/install-system.sh
bash ~/.local/share/omarchy/fedora/install-user.sh
```

Depois saia da sessão e escolha **DoxIA (Hyprland uwsm)** na tela de login.

O script de usuário faz backup de toda configuração que substitui (`*.bak-omarchy-<data>`)
e não mexe no autostart, nas variáveis de ambiente nem nas configurações do Chromium do KDE.

Para atualizar uma instalação (git pull, depois tema, identidade visual, fonte de ícones,
menu, MOTD e tela de login) sem mexer no layout da barra nem nos plugins dela, rode
`bash ~/.local/share/omarchy/fedora/update.sh`.

`Super + K` mostra os atalhos e `Super + Espaço` abre o menu.

### ISO de instalação

`fedora/iso/build.sh` gera um instalador DoxIA: o netinstall do Fedora (Anaconda)
reconstruído com lorax com o nome DoxIA, os logos genéricos no lugar dos do Fedora e um
tema do instalador nas cores da tela de login. O kickstart define pt-BR, teclado brasileiro
e horário de São Paulo, deixa disco, rede e conta de usuário para o instalador e clona a
versão mais recente da branch `fedora` deste repositório para rodar os scripts de instalação
acima. Em seguida atualiza todos os pacotes, então o primeiro boot já sai atualizado.
A instalação precisa de internet.

```bash
sudo dnf install lorax lorax-templates-generic
sudo bash ~/.local/share/omarchy/fedora/iso/build.sh ~/doxia-iso
```

O GitHub Actions só gera a ISO quando uma tag é enviada (`git tag v1.2 && git push origin v1.2`)
ou quando o workflow **DoxIA ISO** é iniciado manualmente; pushes na `fedora` não geram ISO.

Crie uma conta de usuário no instalador: o desktop é configurado para esse usuário.
`fedora/iso/make-installer-art.py` gera de novo as imagens do instalador.

### Créditos

Baseado no [Omarchy](https://github.com/basecamp/omarchy) (MIT, © DHH). Não afiliado à
Basecamp. Os nomes `omarchy-*` (comandos, `~/.local/share/omarchy`, `~/.config/omarchy`)
foram mantidos para facilitar o merge com o upstream. O manual original, em inglês,
está em [`manual/`](manual/).

### Licença

[MIT](LICENSE).
