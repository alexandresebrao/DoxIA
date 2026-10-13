// The text rules of shell/plugins/polkit/PolkitModel.js.

// pam_fprintd anywhere in the auth stack (a clamshell gate may come first).
pub fn fingerprint_configured(pam_config: &str) -> bool {
  pam_config.lines().map(str::trim).filter(|l| !l.starts_with('#')).any(|l| {
    l.split_whitespace().next() == Some("auth") && l.contains("pam_fprintd.so")
  })
}

pub fn looks_fingerprint(prompt: &str) -> bool {
  let s = prompt.to_lowercase();
  ["finger", "fprint", "swipe", "dedo"].iter().any(|w| s.contains(w))
}

// pkexec's "Authentication is needed to run `x' as ..." becomes
// "Autorizar a execução de 'x'"; any other message stays as polkit wrote it.
pub fn authorization_label(message: &str) -> String {
  let prefixes = ["Authentication is needed to run ", "Authentication is required to run ", "A autenticação é necessária para executar "];
  for prefix in prefixes {
    let lower_ok = message.len() >= prefix.len() && message[..prefix.len()].eq_ignore_ascii_case(prefix);
    let Some(rest) = lower_ok.then(|| &message[prefix.len()..]) else { continue };
    let mut chars = rest.chars();
    if !matches!(chars.next(), Some('`' | '\'')) {
      continue;
    }
    let body = &rest[1..];
    if let Some(end) = body.find(['`', '\'']) {
      let after = &body[end + 1..];
      if after.starts_with(" as ") || after.starts_with(" como ") {
        return format!("Autorizar a execução de '{}'", &body[..end]);
      }
    }
  }
  message.to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn labels() {
    assert_eq!(
      authorization_label("Authentication is needed to run `/usr/bin/true' as the super user"),
      "Autorizar a execução de '/usr/bin/true'"
    );
    assert_eq!(
      authorization_label("A autenticação é necessária para executar '/usr/bin/dnf' como o super usuário"),
      "Autorizar a execução de '/usr/bin/dnf'"
    );
    assert_eq!(authorization_label("Authentication is required to change the hostname"), "Authentication is required to change the hostname");
  }

  #[test]
  fn fingerprint() {
    assert!(fingerprint_configured("#%PAM-1.0\nauth [success=1 default=ignore] pam_exec.so quiet x\nauth sufficient pam_fprintd.so\n"));
    assert!(!fingerprint_configured("auth include system-auth\n# auth sufficient pam_fprintd.so\n"));
    assert!(looks_fingerprint("Place your finger on the reader"));
    assert!(!looks_fingerprint("Password: "));
  }
}
