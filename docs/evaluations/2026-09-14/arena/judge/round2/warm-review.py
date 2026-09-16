from pathlib import Path
import subprocess,json,os,shutil,signal,time,hashlib
root=Path('/tmp/den-arena-20260914');out=root/'judge/round2'
receipts={}
for name in ['review-muse','review-opus']:
 source=root/name;target=root/('target-'+name)
 assert shutil.disk_usage(root).free>=8*1024**3,'Storage below 8 GiB'
 env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(target),CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='3')
 log_path=out/(name+'-warm.log')
 with log_path.open('w') as log:
  p=subprocess.Popen(['cargo','test','-p','den-server','--test','api','--no-run','--locked','--offline','--message-format=json'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  while p.poll() is None:
   if shutil.disk_usage(root).free<8*1024**3:
    os.killpg(p.pid,signal.SIGTERM);p.wait();raise RuntimeError('Stopped build below 8 GiB')
   time.sleep(1)
 assert p.returncode==0,str(log_path)
 local=[];binary=None
 for line in log_path.read_text().splitlines():
  try:i=json.loads(line)
  except ValueError:continue
  if i.get('reason')!='compiler-artifact':continue
  manifest=Path(i['manifest_path'])
  if i['target']['name'] in ['den_core','den_server','den-server','api']:
   assert manifest.resolve().is_relative_to(source.resolve()),str(manifest)
   local.append({'target':i['target']['name'],'manifest_path':str(manifest),'fresh':i['fresh']})
  if i['target']['name']=='api' and i.get('executable'):binary=Path(i['executable'])
 assert binary and binary.resolve().is_relative_to(target.resolve())
 listing=subprocess.check_output([str(binary),'--list'],text=True)
 assert listing==(out/'clean-target-tests.txt').read_text()
 assert b'arena_review::' not in binary.read_bytes()
 assert subprocess.check_output(['git','status','--porcelain'],cwd=source,text=True)==''
 receipts[name]={'source':str(source.resolve()),'target':str(target.resolve()),'commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=source,text=True).strip(),'local_artifacts':local,'api_binary':str(binary),'api_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'api_binary_inode':binary.stat().st_ino,'tests':listing.splitlines()[-1],'free_bytes':shutil.disk_usage(root).free}
 (out/(name+'-tests.txt')).write_text(listing)
 (out/'review-warm-receipt.json').write_text(json.dumps(receipts,indent=2)+'\n')
 print(name,'warm; own source/target verified; no hidden tests',flush=True)
assert receipts['review-muse']['api_binary_inode']!=receipts['review-opus']['api_binary_inode']
subprocess.run(['diff','-qr','--exclude=.git',str(root/'review-muse'),str(root/'review-opus')],check=True)
print('Both patched snapshots match; targets are independent',flush=True)
