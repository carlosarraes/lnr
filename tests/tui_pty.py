"""Exercise the real terminal lifecycle with disposable credentials and a local API."""
import os, pty, fcntl, termios, struct, subprocess, select, time, tempfile, threading, json
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from pathlib import Path
BIN = os.environ.get('LNR_TEST_BINARY', str(Path('target/debug/lnr').resolve()))
def page(nodes): return dict(nodes=nodes, pageInfo=dict(hasNextPage=False, endCursor=None))
class API(BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def do_POST(self):
        req=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        op=req.get('operationName')
        if op=='AuthStatus': data=dict(viewer=dict(id='u',name='Carlos'),organization=dict(urlKey='fixture'))
        elif op=='Teams': data=dict(teams=page([dict(id='t',name='Platform')]))
        elif op=='OverviewStatuses': data=dict(projectStatuses=page([dict(id='s',name='In progress')]))
        elif op=='OverviewProjects': data=dict(projects=page([dict(id='00000000-0000-0000-0000-000000000001',name='Workspace permissions',description='Project description',progress=.68,lead=dict(id='u',name='Carlos',isMe=True),status=dict(id='s',name='In progress'),teams=page([dict(id='t',name='Platform')]),members=dict(nodes=[]),issues=dict(nodes=[]))]))
        elif op=='Issues': data=dict(issues=page([dict(id='00000000-0000-0000-0000-000000000002',identifier='ENG-1',title='Access policy')]))
        elif op=='IssueView': data=dict(issue=dict(id='00000000-0000-0000-0000-000000000002',title='Access policy',description='Issue description'))
        elif op=='Comments': data=dict(issue=dict(comments=page([dict(id='c',body='Discussion text',user=None)])))
        else: data={}
        body=json.dumps(dict(data=data)).encode();self.send_response(200);self.send_header('Content-Length',str(len(body)));self.end_headers()
        try: self.wfile.write(body)
        except BrokenPipeError: pass
api=ThreadingHTTPServer(('127.0.0.1',0),API)
threading.Thread(target=api.serve_forever,daemon=True).start()
def run(keys, auth=True, navigate=False):
    with tempfile.TemporaryDirectory() as home:
        master,slave=pty.openpty();before=termios.tcgetattr(slave)
        fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',36,120,0,0))
        env=dict(os.environ,TERM='xterm-256color',XDG_CONFIG_HOME=home,HOME=home,LNR_CREDENTIAL_STORE='file',LNR_API_URL=f'http://127.0.0.1:{api.server_port}')
        env.pop('LINEAR_API_KEY',None)
        if auth: env['LINEAR_API_KEY']='fixture'
        def setup():
            os.setsid()
        proc=subprocess.Popen([BIN],stdin=slave,stdout=slave,stderr=slave,env=env,preexec_fn=setup)
        output=bytearray()
        def read_until(needle, timeout=5):
            deadline=time.monotonic()+timeout
            while needle not in output and time.monotonic()<deadline:
                if select.select([master],[],[],.1)[0]:
                    try: output.extend(os.read(master,65536))
                    except OSError: break
                if proc.poll() is not None: break
            assert needle in output, f'missing {needle!r}: {bytes(output)[-1000:]!r}'
        try:
            if auth:
                read_until(b'Workspace permissions')
                if navigate:
                    os.write(master,b'\r');read_until(b'Access policy')
                    os.write(master,b'\r');read_until(b'text')
                    def pump(seconds=.3):
                        deadline=time.monotonic()+seconds
                        while time.monotonic()<deadline:
                            if select.select([master],[],[],.02)[0]: output.extend(os.read(master,65536))
                    os.write(master,b'\x1b');pump();os.write(master,b'\x1b');pump()
                    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',10,40,0,0));os.kill(proc.pid,28)
                    read_until(b'resize')
                    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',36,120,0,0));os.kill(proc.pid,28)
                if navigate: pump()
                os.write(master,keys)
            deadline=time.monotonic()+4
            while proc.poll() is None and time.monotonic()<deadline:
                if select.select([master],[],[],.05)[0]: output.extend(os.read(master,65536))
            assert proc.poll() is not None, f'exit timeout: {bytes(output)[-500:]!r}' 
            while select.select([master],[],[],.05)[0]:
                output.extend(os.read(master,65536))
            assert termios.tcgetattr(slave)==before, 'terminal attributes were not restored'
            if auth:
                assert proc.returncode==0
                assert b'\x1b[?1049l' in output, 'alternate screen not restored'
            else: assert proc.returncode!=0 and b'\x1b[?1049h' not in output
        finally:
            if proc.poll() is None: proc.kill();proc.wait()
            os.close(master);os.close(slave)
run(b'q',navigate=True)
run(b'\x03')
run(b'',auth=False)
api.shutdown()
print('PTY: navigation, resize, quit, Ctrl-C, missing auth and restoration passed')
