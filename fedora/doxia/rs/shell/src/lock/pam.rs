// PAM for the lock screen, in a child process (`doxia-shell --pam <service>`),
// as Quickshell does it: a module that hangs (a fingerprint reader) or crashes
// takes only the child with it, and the shell aborts it by killing it. The
// child authenticates $USER against /etc/pam.d/<service>, answers password
// prompts with the line it reads on stdin (once), and exits 0 on success, 1 on
// a refused authentication, 2 when PAM could not run.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::io::BufRead;

const PAM_SUCCESS: c_int = 0;
const PAM_BUF_ERR: c_int = 5;
const PAM_CONV_ERR: c_int = 19;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;

#[repr(C)]
struct PamMessage {
  msg_style: c_int,
  msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
  resp: *mut c_char,
  resp_retcode: c_int,
}

type Conv = extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int;

#[repr(C)]
struct PamConv {
  conv: Conv,
  appdata_ptr: *mut c_void,
}

#[link(name = "pam")]
unsafe extern "C" {
  fn pam_start(service: *const c_char, user: *const c_char, conv: *const PamConv, pamh: *mut *mut c_void) -> c_int;
  fn pam_authenticate(pamh: *mut c_void, flags: c_int) -> c_int;
  fn pam_end(pamh: *mut c_void, status: c_int) -> c_int;
}

unsafe extern "C" {
  fn calloc(n: usize, size: usize) -> *mut c_void;
  fn strdup(s: *const c_char) -> *mut c_char;
}

struct Answers {
  // The password, handed out once: a second prompt in the same run means the
  // first answer was refused.
  password: Option<CString>,
}

extern "C" fn converse(n: c_int, msgs: *mut *const PamMessage, resp: *mut *mut PamResponse, data: *mut c_void) -> c_int {
  if n <= 0 || msgs.is_null() || resp.is_null() || data.is_null() {
    return PAM_CONV_ERR;
  }
  let answers = unsafe { &mut *(data as *mut Answers) };
  let replies = unsafe { calloc(n as usize, std::mem::size_of::<PamResponse>()) } as *mut PamResponse;
  if replies.is_null() {
    return PAM_BUF_ERR;
  }
  for i in 0..n as usize {
    let msg = unsafe { &**msgs.add(i) };
    let reply = unsafe { &mut *replies.add(i) };
    match msg.msg_style {
      PAM_PROMPT_ECHO_OFF | PAM_PROMPT_ECHO_ON => match answers.password.take() {
        Some(p) => reply.resp = unsafe { strdup(p.as_ptr()) },
        // Nothing to say (the fingerprint service never asks for one).
        None => return PAM_CONV_ERR,
      },
      // Fingerprint prompts and errors: the shell shows its own text.
      _ => {
        if !msg.msg.is_null() {
          let text = unsafe { CStr::from_ptr(msg.msg) }.to_string_lossy();
          println!("{text}");
        }
      }
    }
  }
  unsafe { *resp = replies };
  PAM_SUCCESS
}

pub fn run(service: &str) -> i32 {
  let user = std::env::var("USER").or_else(|_| std::env::var("LOGNAME")).unwrap_or_default();
  if user.is_empty() || service.is_empty() || service.contains('/') {
    return 2;
  }
  // The password comes as one line on stdin, if at all.
  let mut line = String::new();
  let password = if service.ends_with("-fingerprint") {
    None
  } else {
    if !std::io::stdin().lock().read_line(&mut line).is_ok_and(|n| n > 0) {
      return 2;
    }
    let p = line.strip_suffix('\n').unwrap_or(&line);
    CString::new(p).ok()
  };
  let mut answers = Box::new(Answers { password });
  let conv = PamConv { conv: converse, appdata_ptr: &mut *answers as *mut Answers as *mut c_void };
  let (Ok(service), Ok(user)) = (CString::new(service), CString::new(user)) else { return 2 };
  let mut handle: *mut c_void = std::ptr::null_mut();
  if unsafe { pam_start(service.as_ptr(), user.as_ptr(), &conv, &mut handle) } != PAM_SUCCESS {
    return 2;
  }
  let status = unsafe { pam_authenticate(handle, 0) };
  unsafe { pam_end(handle, status) };
  if status == PAM_SUCCESS { 0 } else { 1 }
}
