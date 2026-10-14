// "Idle for N seconds" from the compositor (ext-idle-notify-v1, which
// Quickshell's IdleMonitor uses): a connection of its own, read on the main
// loop when its socket has data. The notification respects idle inhibitors
// (a video playing), as Quickshell's respectInhibitors does.

use gtk::glib;
use std::cell::RefCell;
use std::os::fd::AsRawFd;
use std::rc::Rc;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::ext::idle_notify::v1::client::{ext_idle_notification_v1, ext_idle_notifier_v1};

struct State {
  on_change: Rc<dyn Fn(bool)>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
  fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
  fn event(_: &mut Self, _: &wl_seat::WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ext_idle_notifier_v1::ExtIdleNotifierV1, ()> for State {
  fn event(_: &mut Self, _: &ext_idle_notifier_v1::ExtIdleNotifierV1, _: ext_idle_notifier_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for State {
  fn event(
    state: &mut Self,
    _: &ext_idle_notification_v1::ExtIdleNotificationV1,
    event: ext_idle_notification_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    // After the dispatch: the callback may change the timeout, which needs
    // the queue this runs inside.
    let idle = match event {
      ext_idle_notification_v1::Event::Idled => true,
      ext_idle_notification_v1::Event::Resumed => false,
      _ => return,
    };
    let f = state.on_change.clone();
    glib::idle_add_local_once(move || f(idle));
  }
}

struct Inner {
  conn: Connection,
  queue: EventQueue<State>,
  state: State,
  notifier: ext_idle_notifier_v1::ExtIdleNotifierV1,
  seat: wl_seat::WlSeat,
  notification: Option<ext_idle_notification_v1::ExtIdleNotificationV1>,
  source: Option<glib::SourceId>,
}

pub struct IdleMonitor {
  inner: Rc<RefCell<Inner>>,
}

impl IdleMonitor {
  // `on_change(true)` once idle for the timeout, `(false)` on activity.
  pub fn new(on_change: impl Fn(bool) + 'static) -> Option<IdleMonitor> {
    let conn = Connection::connect_to_env().ok()?;
    let (globals, queue) = registry_queue_init::<State>(&conn).ok()?;
    let qh = queue.handle();
    let notifier: ext_idle_notifier_v1::ExtIdleNotifierV1 = globals.bind(&qh, 1..=1, ()).ok()?;
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=7, ()).ok()?;
    let inner = Rc::new(RefCell::new(Inner {
      conn: conn.clone(),
      queue,
      state: State { on_change: Rc::new(on_change) },
      notifier,
      seat,
      notification: None,
      source: None,
    }));
    let fd = conn.backend().poll_fd().as_raw_fd();
    let weak = Rc::downgrade(&inner);
    let source = glib_unix::unix_fd_add_local(fd, glib::IOCondition::IN, move |_, _| {
      let Some(inner) = weak.upgrade() else { return glib::ControlFlow::Break };
      let mut i = inner.borrow_mut();
      if let Some(guard) = i.queue.prepare_read() {
        if guard.read().is_err() {
          return glib::ControlFlow::Break;
        }
      }
      let Inner { queue, state, .. } = &mut *i;
      let _ = queue.dispatch_pending(state);
      let _ = i.conn.flush();
      glib::ControlFlow::Continue
    });
    inner.borrow_mut().source = Some(source);
    Some(IdleMonitor { inner })
  }

  // Watch for `seconds` of idleness (None: stop watching).
  pub fn set_timeout(&self, seconds: Option<u32>) {
    let mut i = self.inner.borrow_mut();
    if let Some(n) = i.notification.take() {
      n.destroy();
    }
    if let Some(s) = seconds {
      let qh = i.queue.handle();
      let n = i.notifier.get_idle_notification(s.saturating_mul(1000), &i.seat, &qh, ());
      i.notification = Some(n);
    }
    let _ = i.conn.flush();
    // Events already read for the old notification are dropped with it.
    let Inner { queue, state, .. } = &mut *i;
    let _ = queue.dispatch_pending(state);
  }
}

impl Drop for IdleMonitor {
  fn drop(&mut self) {
    if let Some(s) = self.inner.borrow_mut().source.take() {
      s.remove();
    }
  }
}
