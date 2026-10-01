#!/usr/bin/env python3
"""Controle do Tomcat e do JBoss com o JDK Oracle 8 do SDKMAN.

Uso: ctl.py status | start <svc> | stop <svc> [--force]
O status sai como um objeto JSON para o Panel.qml.
"""
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

HOME = Path.home()
JAVA_HOME = HOME / ".sdkman/candidates/java/8.0.192-oracle"
STATE_DIR = Path(os.environ.get("XDG_STATE_HOME", HOME / ".local/state")) / "klab-javaservers"
STOP_TIMEOUT = 40  # segundos até forçar SIGKILL

SERVICES = {
    "tomcat": {
        "nome": "Tomcat 8.5",
        "cwd": "/usr/local/java/tomcat/bin",
        "cmd": ["./catalina.sh", "jpda", "run", "-Djava.security.egd=file:/dev/./urandom"],
        "match": ["org.apache.catalina.startup.Bootstrap", "/usr/local/java/tomcat"],
        "porta": 8081,
        "pronto_log": b"Server startup in",
        "url": "http://localhost:8081",
    },
    "jboss": {
        "nome": "JBoss 4.0.2",
        "cwd": "/usr/local/java",
        "cmd": ["sh", "/usr/local/java/jboss-4.0.2/bin/run.sh", "-c", "postgres",
                f"-Djboss.partition.name={os.environ.get('USER', 'klab')}"],
        "match": ["org.jboss.Main"],
        "porta": 8080,
        "pronto_log": b"] Started in",
        "url": "http://localhost:8080/jmx-console",
    },
}


def log_path(svc):
    return STATE_DIR / f"{svc}.log"


def sid_path(svc):
    return STATE_DIR / f"{svc}.sid"


def find_pid(svc):
    """PID do java do serviço, mesmo se iniciado pelo terminal."""
    patterns = SERVICES[svc]["match"]
    for d in os.listdir("/proc"):
        if not d.isdigit():
            continue
        try:
            cmdline = Path(f"/proc/{d}/cmdline").read_bytes().replace(b"\0", b" ").decode(errors="ignore")
        except OSError:
            continue
        if "java" in cmdline and all(p in cmdline for p in patterns):
            return int(d)
    return None


def port_open(port):
    import socket
    with socket.socket() as s:
        s.settimeout(0.3)
        return s.connect_ex(("127.0.0.1", port)) == 0


def is_ready(svc, pid):
    """Pronto quando o log mostra o fim do startup; a porta abre antes do deploy acabar.
    Se foi iniciado fora do plugin (sem log nosso), usa a porta."""
    cfg = SERVICES[svc]
    try:
        own = sid_path(svc).read_text().strip() == str(os.getsid(pid))
    except (OSError, ValueError):
        own = False
    if not own:
        return port_open(cfg["porta"])
    try:
        return cfg["pronto_log"] in log_path(svc).read_bytes()
    except OSError:
        return False


def proc_info(pid):
    info = {"uptime": 0, "memoriaMb": 0}
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                info["memoriaMb"] = int(line.split()[1]) // 1024
        start_ticks = int(Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19])
        boot = float(Path("/proc/uptime").read_text().split()[0])
        info["uptime"] = int(boot - start_ticks / os.sysconf("SC_CLK_TCK"))
    except OSError:
        pass
    return info


def status():
    out = []
    for svc, cfg in SERVICES.items():
        pid = find_pid(svc)
        item = {"id": svc, "nome": cfg["nome"], "pid": pid or 0, "url": cfg["url"],
                "log": str(log_path(svc)), "temLog": log_path(svc).exists(),
                "estado": "parado" if not pid else ("rodando" if is_ready(svc, pid) else "iniciando")}
        if pid:
            item.update(proc_info(pid))
        out.append(item)
    return {"javaOk": (JAVA_HOME / "bin/java").exists(), "javaHome": str(JAVA_HOME),
            "algumAtivo": any(s["pid"] for s in out), "servicos": out}


def start(svc):
    if find_pid(svc):
        return
    cfg = SERVICES[svc]
    env = os.environ.copy()
    env.update(JAVA_HOME=str(JAVA_HOME), JRE_HOME=str(JAVA_HOME), J2EE_HOME=str(JAVA_HOME / "bin"),
               PATH=f"{JAVA_HOME / 'bin'}:{env.get('PATH', '')}")
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    lp = log_path(svc)
    if lp.exists() and lp.stat().st_size:
        lp.replace(lp.with_suffix(".log.anterior"))
    with open(lp, "ab") as out:
        out.write(f"=== iniciando {cfg['nome']} com {JAVA_HOME} em {time.strftime('%d/%m %H:%M:%S')} ===\n".encode())
        out.flush()
        # Sessão própria: o servidor não morre junto com a barra.
        proc = subprocess.Popen(cfg["cmd"], cwd=cfg["cwd"], env=env, stdout=out, stderr=subprocess.STDOUT,
                                stdin=subprocess.DEVNULL, start_new_session=True)
    sid_path(svc).write_text(str(proc.pid))  # com start_new_session, sid == pid do filho


def stop(svc, force=False):
    pid = find_pid(svc)
    if not pid:
        return
    try:
        pgid = os.getpgid(pid)
        # Grupo inteiro só quando é uma sessão nossa, nunca o shell de um terminal.
        own_group = pgid == os.getsid(pid)
    except OSError:
        return

    def kill(sig):
        try:
            os.killpg(pgid, sig) if own_group else os.kill(pid, sig)
        except ProcessLookupError:
            pass

    kill(signal.SIGKILL if force else signal.SIGTERM)
    deadline = time.time() + STOP_TIMEOUT
    while time.time() < deadline and find_pid(svc):
        time.sleep(0.5)
    if find_pid(svc):
        kill(signal.SIGKILL)


if __name__ == "__main__":
    args = sys.argv[1:]
    if args[:1] == ["status"]:
        print(json.dumps(status()))
    elif len(args) >= 2 and args[0] in ("start", "stop") and args[1] in SERVICES:
        start(args[1]) if args[0] == "start" else stop(args[1], force="--force" in args)
    else:
        sys.exit(__doc__)
