//! Terminal ownership with rollback even if setup fails partway through.
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io::{self, Stdout},
    sync::{
        Once,
        atomic::{AtomicBool, Ordering},
    },
};
static ACTIVE: AtomicBool = AtomicBool::new(false);
static HOOK: Once = Once::new();
fn restore() {
    if ACTIVE.swap(false, Ordering::SeqCst) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    }
}
struct Mode;
impl Drop for Mode {
    fn drop(&mut self) {
        restore();
    }
}
pub struct TerminalSession {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
    _mode: Mode,
}
impl TerminalSession {
    pub fn enter() -> io::Result<Self> {
        HOOK.call_once(|| {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                restore();
                previous(info);
            }));
        });
        enable_raw_mode()?;
        ACTIVE.store(true, Ordering::SeqCst);
        let mode = Mode;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        Ok(Self {
            terminal,
            _mode: mode,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panic_child() {
        if std::env::var_os("LNR_TERMINAL_PANIC_TEST").is_none() {
            return;
        }
        let _session = TerminalSession::enter().unwrap();
        panic!("terminal restoration test");
    }
    #[test]
    fn setup_failure_child() {
        if std::env::var_os("LNR_TERMINAL_SETUP_TEST").is_none() {
            return;
        }
        assert!(TerminalSession::enter().is_err());
        assert!(!ACTIVE.load(Ordering::SeqCst));
    }
    #[test]
    #[cfg(unix)]
    fn restores_on_panic_and_failed_setup() {
        let script = r#"
import os,pty,subprocess,termios,fcntl,struct,sys,select,time
for mode in ['panic','setup']:
 m,s=pty.openpty();before=termios.tcgetattr(s);fcntl.ioctl(s,termios.TIOCSWINSZ,struct.pack('HHHH',24,80,0,0))
 env=dict(os.environ,TERM='xterm-256color');env['LNR_TERMINAL_'+('PANIC' if mode=='panic' else 'SETUP')+'_TEST']='1'
 def setup():
  os.setsid()
 # A read-only descriptor reliably fails writes on both Linux and macOS.
 sink=open('/dev/null','rb') if mode=='setup' else None
 p=subprocess.Popen([sys.argv[1],'--exact','tui::terminal::tests::'+('panic_child' if mode=='panic' else 'setup_failure_child'),'--nocapture'],stdin=s,stdout=s if sink is None else sink,stderr=s,env=env,preexec_fn=setup)
 data=b'';deadline=time.monotonic()+5
 while p.poll() is None and time.monotonic()<deadline:
  if select.select([m],[],[],.05)[0]:data+=os.read(m,65536)
 assert p.poll() is not None,'child hung'
 assert termios.tcgetattr(s)==before,'raw mode leaked'
 if mode=='panic':assert b'\x1b[?1049l' in data,'alternate screen leaked'
 if sink:sink.close()
 os.close(m);os.close(s)
"#;
        let output = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(std::env::current_exe().unwrap())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
