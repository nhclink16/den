import shutil, subprocess, json
from pathlib import Path
R=Path('/tmp/den-arena-20260914');b=R/'baseline';w=R/'judge/repair-work'
shutil.copytree(b,w,dirs_exist_ok=True)
(w/'tools').mkdir(exist_ok=True)
shutil.copyfile(R/'judge/reconnect_fixture.py',w/'tools/arena_repro.py')
original=(b/'crates/den-cli/src/stream.rs').read_text()
old='''    loop {
        let mut request = url.as_str().into_client_request()?;
        request
            .headers_mut()
            .insert("Authorization", format!("Bearer {token}").parse()?);'''
new='''    let mut auth_headers = tungstenite::http::HeaderMap::new();
    auth_headers.insert("Authorization", format!("Bearer {token}").parse()?);
    loop {
        let mut request = url.as_str().into_client_request()?;
        request.headers_mut().extend(auth_headers.drain());'''
assert old in original
seeded=original.replace(old,new,1).replace('matches!(response.status().as_u16(), 401 | 403)','response.status().as_u16() == 403',1)
(R/'judge/seeded-stream.rs').write_text(seeded)
(R/'judge/repair-answer-key.md').write_text('HeaderMap::drain consumes authorization headers after first handshake. Reconnect requests lose auth. 401 has also been removed from permanent authentication rejections, causing retries instead of nonzero exit. Restore both behaviors; do not accept removal of authentication or tests.\n')
print(w)
