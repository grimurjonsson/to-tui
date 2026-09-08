"""Verify CLI folder routing, remote persistence and no fallback using a real server."""
import sys
sys.dont_write_bytecode = True

from test_multitenancy import BINARY, Gateway, free_port
from http.server import ThreadingHTTPServer
from pathlib import Path
import json, os, sqlite3, subprocess, tempfile, threading, time, urllib.request, uuid, datetime


def main():
    gateway=ThreadingHTTPServer(('127.0.0.1',0),Gateway)
    threading.Thread(target=gateway.serve_forever,daemon=True).start()
    process=None
    try:
        with tempfile.TemporaryDirectory(prefix='totui-cli-routing-') as directory:
            root=Path(directory).resolve(); client_root=root/'client';client_root.mkdir()
            folder=root/'repository';folder.mkdir();(folder/'.git').mkdir()
            child=folder/'src';child.mkdir()
            port=free_port();origin=f'http://127.0.0.1:{port}'
            env=dict(os.environ,TOTUI_DATA_DIR=str(client_root))
            server_env=dict(os.environ,TOTUI_DATA_DIR=str(root/'server'),TOTUI_BIND='127.0.0.1',TOTUI_AUTH_URL=f'http://127.0.0.1:{gateway.server_port}/auth')
            server_env.pop('NOTIFY_SOCKET',None)
            with (root/'server.log').open('w') as log:
                process=subprocess.Popen([str(BINARY),'web','--auth','--port',str(port)],env=server_env,stdout=log,stderr=log)
                for _ in range(100):
                    try:
                        urllib.request.urlopen(origin+'/api/ready',timeout=1).close();break
                    except OSError:time.sleep(.05)
                else:raise AssertionError((root/'server.log').read_text())
                def cli(*args,input=None,success=True):
                    result=subprocess.run([str(BINARY),*args],cwd=child,env=env,input=input,text=True,capture_output=True,timeout=20)
                    assert (result.returncode==0)==success,(args,result.stderr)
                    return json.loads(result.stdout) if result.stdout.strip().startswith(('{','[')) else result
                cli('--local','todo','create','--content','Local sentinel')
                config=client_root/'config.toml'
                config.write_text('[folder_projects]\n'+json.dumps(str(folder))+'="work"\n')
                cli('remote','add','test',origin)
                cli('remote','login','test','--cookie-stdin',input='session=alice')
                cli('remote','use','test')
                me=json.load(urllib.request.urlopen(urllib.request.Request(origin+'/api/me',headers={'Cookie':'session=alice'})))
                req=urllib.request.Request(origin+'/api/remote/v1',headers={'Cookie':'session=alice','Content-Type':'application/json','x-totui-expected-user':me['id']},data=json.dumps({'operation':'create_project','project':{'id':str(uuid.uuid4()),'name':'work','created_at':datetime.datetime.now(datetime.timezone.utc).isoformat()}}).encode())
                urllib.request.urlopen(req).close()
                destination=cli('todo','context')
                assert destination['backend']=='remote' and destination['project']=='work'
                assert destination['remote']=='test' and destination['folder']==str(folder)
                created=cli('todo','create','--content','Remote parent')
                nested=cli('--remote','test','todo','create','--parent-id',created['id'],'--content','Remote child')
                assert nested['parent_id']==created['id']
                cli('todo','update',nested['id'],'--state','x')
                items=cli('todo','list');assert len(items)==2 and items[1]['state']=='x'
                assert cli('todo','list','--project','default')==[]
                assert cli('todo','context','--project','default')['project']=='default'
                cli('todo','create','--project','missing','--content','Must fail',success=False)
                local=cli('--local','todo','list','--project','default')
                assert [t['content'] for t in local]==['Local sentinel']
                conn=sqlite3.connect(client_root/'todos.db')
                assert conn.execute('select count(*) from todos where content like "Remote%"').fetchone()[0]==0
                cli('todo','delete',created['id'])
                assert cli('todo','list')==[]
                process.terminate();process.wait(timeout=10);process=None
                cli('todo','create','--content','Offline must fail',success=False)
                assert conn.execute('select count(*) from todos').fetchone()[0]==1
                conn.close()
                print('PASS: remote folder routing, explicit overrides, nested CRUD, and no local fallback')
    finally:
        if process is not None:process.terminate();process.wait(timeout=10)
        gateway.shutdown();gateway.server_close()

if __name__=='__main__':main()
