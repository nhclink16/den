use den_core::*;
use den_server::{router,AppState};
use reqwest::{Client,Method,StatusCode};
use serde_json::{json,Value};
use std::{net::SocketAddr,path::PathBuf,time::Duration};
use futures_util::{SinkExt,StreamExt};
use tokio_tungstenite::{connect_async,tungstenite::{client::IntoClientRequest,Message as Frame}};

struct Test {url:String,dir:PathBuf,state:AppState,task:tokio::task::JoinHandle<()>,http:Client,admin:Session}
impl Drop for Test {fn drop(&mut self){self.task.abort();let _=std::fs::remove_dir_all(&self.dir);}}
impl Test {
    async fn new()->Self {
        let dir=std::env::temp_dir().join(format!("den-test-{}",ulid::Ulid::new()));
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let url=format!("http://{}",listener.local_addr().unwrap());
        let state=AppState::open(dir.join("den.db"),dir.join("uploads"),dir.join("bootstrap.key"),url.clone(),1024*1024).await.unwrap();
        let app=router(state.clone());let task=tokio::spawn(async move{axum::serve(listener,app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();});
        let http=Client::new();let key=std::fs::read_to_string(dir.join("bootstrap.key")).unwrap();
        let response=http.post(format!("{url}/auth/init")).json(&Bootstrap{username:"admin".into(),password:"test-password-123".into(),bootstrap_token:key}).send().await.unwrap();assert_eq!(response.status(),200);
        let admin=response.json().await.unwrap();Self{url,dir,state,task,http,admin}
    }
    fn req(&self,method:Method,path:&str,token:&str)->reqwest::RequestBuilder {self.http.request(method,format!("{}{path}",self.url)).bearer_auth(token)}
    async fn post(&self,path:&str,token:&str,body:Value)->Value {let r=self.req(Method::POST,path,token).json(&body).send().await.unwrap();assert_eq!(r.status(),200,"{path}");r.json().await.unwrap()}
    async fn member(&self,name:&str)->Session {let invite=self.post("/invites",&self.admin.token,json!({"uses":1,"expires_in_hours":1})).await;serde_json::from_value(self.post("/auth/register","",json!({"username":name,"password":"test-password-123","invite":invite["code"]})).await).unwrap()}
    async fn general(&self)->String {let v:Vec<Channel>=self.req(Method::GET,"/channels",&self.admin.token).send().await.unwrap().json().await.unwrap();v[0].id.clone()}
}
#[tokio::test]
async fn auth_consumes_invites_protects_cookies_and_revokes_credentials() {
    let t=Test::new().await;let a=&t.admin.token;
    assert!(!t.dir.join("bootstrap.key").exists());
    assert_eq!(t.http.get(format!("{}/users",t.url)).send().await.unwrap().status(),401);
    let invite=t.post("/invites",a,json!({"uses":1,"expires_in_hours":1})).await;
    let body=json!({"username":"alice","password":"test-password-123","invite":invite["code"]});
    let alice:Session=serde_json::from_value(t.post("/auth/register","",body).await).unwrap();
    let again=t.req(Method::POST,"/auth/register","").json(&json!({"username":"carol","password":"test-password-123","invite":invite["code"]})).send().await.unwrap();assert_eq!(again.status(),400);
    assert_eq!(t.req(Method::POST,"/invites",&alice.token).json(&json!({"uses":1,"expires_in_hours":1})).send().await.unwrap().status(),403);
    assert_eq!(t.req(Method::POST,"/auth/login","").json(&json!({"username":"alice","password":"wrong"})).send().await.unwrap().status(),401);
    let login=t.req(Method::POST,"/auth/login","").json(&json!({"username":"alice","password":"test-password-123"})).send().await.unwrap();
    assert!(login.headers()["set-cookie"].to_str().unwrap().contains("HttpOnly; SameSite=Strict"));let login:Session=login.json().await.unwrap();
    let cookie=format!("den_session={}",login.token);
    let write=||t.http.post(format!("{}/tokens",t.url)).header("Cookie",&cookie).json(&CreateToken{name:"cookie".into()});
    assert_eq!(write().send().await.unwrap().status(),403);
    assert_eq!(write().header("Origin","https://evil.example").header("X-CSRF-Token",&login.csrf_token).send().await.unwrap().status(),403);
    assert_eq!(write().header("Origin",&t.url).header("X-CSRF-Token",&login.csrf_token).send().await.unwrap().status(),200);
    let token=t.post("/tokens",&alice.token,json!({"name":"agent"})).await;let secret=token["token"].as_str().unwrap();
    assert_eq!(t.req(Method::GET,"/users/me",secret).send().await.unwrap().status(),200);
    assert_eq!(t.req(Method::DELETE,&format!("/tokens/{}",token["credential"]["id"].as_str().unwrap()),&alice.token).send().await.unwrap().status(),204);
    assert_eq!(t.req(Method::GET,"/users/me",secret).send().await.unwrap().status(),401);
    assert_eq!(t.req(Method::POST,"/auth/logout",&alice.token).send().await.unwrap().status(),204);
    assert_eq!(t.req(Method::GET,"/users/me",&alice.token).send().await.unwrap().status(),401);
    let hashes:Vec<String>=sqlx::query_scalar("SELECT secret_hash FROM sessions UNION SELECT secret_hash FROM tokens").fetch_all(&t.state.db).await.unwrap();
    assert!(hashes.iter().all(|h|h!=a && h!=&login.token && h!=secret));
}
#[tokio::test]
async fn channel_crud_dm_privacy_messages_and_cursor_pagination() {
    let t=Test::new().await;let alice=t.member("alice").await;let bob=t.member("bob").await;let a=&t.admin.token;
    let category=t.post("/categories",a,json!({"name":"Games","position":1})).await;
    let channel=t.post("/channels",a,json!({"name":"gaming","category_id":category["id"],"position":1})).await;let cid=channel["id"].as_str().unwrap();
    assert_eq!(t.req(Method::POST,"/channels",&alice.token).json(&json!({"name":"nope","position":0})).send().await.unwrap().status(),403);
    assert_eq!(t.req(Method::PUT,&format!("/channels/{cid}"),a).json(&json!({"name":"games","position":2})).send().await.unwrap().status(),200);
    let private=t.post("/dms",&alice.token,json!({"member_ids":[bob.user.id]})).await;let dm=private["id"].as_str().unwrap();
    let same=t.post("/dms",&bob.token,json!({"member_ids":[alice.user.id]})).await;assert_eq!(private["id"],same["id"]);
    assert_eq!(t.req(Method::GET,&format!("/channels/{dm}/messages"),a).send().await.unwrap().status(),404);
    let visible:Vec<Channel>=t.req(Method::GET,"/channels",a).send().await.unwrap().json().await.unwrap();assert!(visible.iter().all(|c|c.id!=dm));
    let path=format!("/channels/{cid}/messages");let mut ids=Vec::new();
    for text in ["one","two","three"] {ids.push(t.post(&path,&alice.token,json!({"content":text})).await["id"].as_str().unwrap().to_string());}
    assert!(ids.windows(2).all(|w|w[0]<w[1]));
    let latest:Vec<Message>=t.req(Method::GET,&format!("{path}?limit=2"),a).send().await.unwrap().json().await.unwrap();assert_eq!(latest.iter().map(|m|m.content.as_str()).collect::<Vec<_>>(),["two","three"]);
    let before:Vec<Message>=t.req(Method::GET,&format!("{path}?before={}&limit=2",ids[1]),a).send().await.unwrap().json().await.unwrap();assert_eq!(before.len(),1);assert_eq!(before[0].content,"one");
    let after:Vec<Message>=t.req(Method::GET,&format!("{path}?after={}&limit=1",ids[0]),a).send().await.unwrap().json().await.unwrap();assert_eq!(after[0].content,"two");
    let target=format!("/messages/{}",ids[0]);assert_eq!(t.req(Method::PATCH,&target,&bob.token).json(&json!({"content":"stolen"})).send().await.unwrap().status(),403);
    let edited:Message=t.req(Method::PATCH,&target,&alice.token).json(&json!({"content":"edited"})).send().await.unwrap().json().await.unwrap();assert!(edited.edited_at.is_some());
    assert_eq!(t.req(Method::POST,&format!("/channels/{dm}/messages"),&alice.token).json(&json!({"content":"cross-channel reply","reply_to":ids[0]})).send().await.unwrap().status(),400);
    assert_eq!(t.req(Method::DELETE,&target,&bob.token).send().await.unwrap().status(),403);
    assert_eq!(t.req(Method::DELETE,&target,a).send().await.unwrap().status(),204);
    assert_eq!(t.req(Method::DELETE,&format!("/categories/{}",category["id"].as_str().unwrap()),a).send().await.unwrap().status(),204);
    assert_eq!(t.req(Method::DELETE,&format!("/channels/{cid}"),a).send().await.unwrap().status(),204);
}
#[tokio::test]
async fn upload_resume_survives_uncommitted_bytes_and_serves_authenticated_ranges() {
    let t=Test::new().await;let alice=t.member("alice").await;let bob=t.member("bob").await;
    let dm=t.post("/dms",&alice.token,json!({"member_ids":[bob.user.id]})).await;let cid=dm["id"].as_str().unwrap();
    let upload=t.post("/uploads",&alice.token,json!({"channel_id":cid,"filename":"clip.mp4","content_type":"video/mp4","size":10})).await;let uid=upload["id"].as_str().unwrap();let path=format!("/uploads/{uid}");
    assert_eq!(t.req(Method::POST,&format!("{path}/complete"),&alice.token).send().await.unwrap().status(),409);
    assert_eq!(t.req(Method::PATCH,&path,&bob.token).header("Upload-Offset",0).body("01234").send().await.unwrap().status(),403);
    assert_eq!(t.req(Method::PATCH,&path,&alice.token).header("Upload-Offset",0).body("01234").send().await.unwrap().status(),200);
    // Simulate a process dying after file write but before SQLite offset commit.
    {use std::io::Write;let mut f=std::fs::OpenOptions::new().append(true).open(t.dir.join("uploads").join(format!("{uid}.part"))).unwrap();f.write_all(b"crash").unwrap();}
    let status:Upload=t.req(Method::GET,&path,&alice.token).send().await.unwrap().json().await.unwrap();assert_eq!(status.offset,5);
    assert_eq!(t.req(Method::PATCH,&path,&alice.token).header("Upload-Offset",0).body("01234").send().await.unwrap().status(),409);
    assert_eq!(t.req(Method::PATCH,&path,&alice.token).header("Upload-Offset",5).body("567890").send().await.unwrap().status(),400);
    assert_eq!(t.req(Method::PATCH,&path,&alice.token).header("Upload-Offset",5).body("56789").send().await.unwrap().status(),200);
    // Recover finalization after rename but before the complete flag commits.
    std::fs::rename(t.dir.join("uploads").join(format!("{uid}.part")),t.dir.join("uploads").join(uid)).unwrap();
    let completed=t.post(&format!("{path}/complete"),&alice.token,json!({})).await;assert_eq!(completed["complete"],true);
    assert_eq!(t.post(&format!("{path}/complete"),&alice.token,json!({})).await["complete"],true);
    let file=format!("{path}/file");assert_eq!(t.req(Method::GET,&file,&t.admin.token).send().await.unwrap().status(),404);
    assert_eq!(t.http.get(format!("{}{file}",t.url)).send().await.unwrap().status(),401);
    let head=t.req(Method::HEAD,&file,&bob.token).send().await.unwrap();assert_eq!(head.status(),200);assert_eq!(head.headers()["content-length"],"10");assert!(head.bytes().await.unwrap().is_empty());
    for (range,expected,content_range) in [("bytes=2-5","2345","bytes 2-5/10"),("bytes=-3","789","bytes 7-9/10"),("bytes=7-","789","bytes 7-9/10")] {
        let r=t.req(Method::GET,&file,&bob.token).header("Range",range).send().await.unwrap();assert_eq!(r.status(),206);assert_eq!(r.headers()["content-range"],content_range);assert_eq!(r.headers()["content-type"],"video/mp4");assert_eq!(r.text().await.unwrap(),expected);
    }
    assert_eq!(t.req(Method::GET,&file,&bob.token).header("Range","bytes=20-30").send().await.unwrap().status(),416);
    let msg=t.post(&format!("/channels/{cid}/messages"),&alice.token,json!({"content":"clip","upload_ids":[uid]})).await;assert_eq!(msg["attachments"][0]["id"],uid);
    assert_eq!(t.req(Method::POST,&format!("/channels/{cid}/messages"),&alice.token).json(&json!({"content":"reuse","upload_ids":[uid]})).send().await.unwrap().status(),400);
    let stale=t.post("/uploads",&alice.token,json!({"channel_id":cid,"filename":"stale","content_type":"text/plain","size":10})).await;
    let stale=stale["id"].as_str().unwrap();sqlx::query("UPDATE uploads SET touched_at=0 WHERE id=?").bind(stale).execute(&t.state.db).await.unwrap();t.state.cleanup().await.unwrap();assert!(!t.dir.join("uploads").join(format!("{stale}.part")).exists());
}

type Socket=tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
async fn socket(t:&Test,token:&str)->Socket {let mut request=format!("{}/ws",t.url.replace("http:","ws:")).into_client_request().unwrap();request.headers_mut().insert("Authorization",format!("Bearer {token}").parse().unwrap());connect_async(request).await.unwrap().0}
async fn event(socket:&mut Socket)->Event {loop{match socket.next().await.unwrap().unwrap(){Frame::Text(v)=>return serde_json::from_str(&v).unwrap(),Frame::Ping(v)=>socket.send(Frame::Pong(v)).await.unwrap(),_=>{}}}}
#[tokio::test]
async fn websocket_filters_dms_marks_reconnect_and_closes_revoked_bot_tokens() {
    let t=Test::new().await;let alice=t.member("alice").await;
    let bot:BotCreated=serde_json::from_value(t.post("/bots",&alice.token,json!({"username":"clanker","display_name":"Clanker"})).await).unwrap();
    let mut peer=socket(&t,&bot.credential.token).await;let mut outsider=socket(&t,&t.admin.token).await;
    assert!(matches!(event(&mut peer).await,Event::Resync{..}));assert!(matches!(event(&mut outsider).await,Event::Resync{..}));
    let dm=t.post("/dms",&alice.token,json!({"member_ids":[bot.user.id]})).await;
    let sent=t.post(&format!("/channels/{}/messages",dm["id"].as_str().unwrap()),&bot.credential.token,json!({"content":"private bot message"})).await;
    match tokio::time::timeout(Duration::from_secs(2),event(&mut peer)).await.unwrap(){Event::MessageCreated(m)=>{assert_eq!(m.author_id,bot.user.id);assert_eq!(m.id,sent["id"]);},_=>panic!("expected message")}
    assert!(tokio::time::timeout(Duration::from_millis(200),event(&mut outsider)).await.is_err());
    peer.close(None).await.unwrap();let mut peer=socket(&t,&bot.credential.token).await;assert!(matches!(event(&mut peer).await,Event::Resync{..}));
    assert_eq!(t.req(Method::DELETE,&format!("/tokens/{}",bot.credential.credential.id),&alice.token).send().await.unwrap().status(),204);
    let general=t.general().await;t.post(&format!("/channels/{general}/messages"),&t.admin.token,json!({"content":"trigger revocation check"})).await;
    tokio::time::timeout(Duration::from_secs(7),async {loop {match peer.next().await {None|Some(Ok(Frame::Close(_)))=>break,Some(Ok(Frame::Ping(v)))=>{let _=peer.send(Frame::Pong(v)).await;},Some(Err(_))=>break,Some(Ok(Frame::Text(_)))=>panic!("revoked token received event"),_=>{}}}}).await.unwrap();
    let mut request=format!("{}/ws",t.url.replace("http:","ws:")).into_client_request().unwrap();request.headers_mut().insert("Cookie",format!("den_session={}",alice.token).parse().unwrap());request.headers_mut().insert("Origin","https://evil.example".parse().unwrap());assert!(connect_async(request).await.is_err());
}
#[tokio::test]
async fn login_throttles_and_openapi_contains_the_shared_contract() {
    let t=Test::new().await;
    // Invalid bootstrap attempts consume the same bounded throttle without hashing passwords.
    for _ in 0..9 {assert_eq!(t.req(Method::POST,"/auth/init","").json(&json!({"username":"other","password":"test-password-123","bootstrap_token":"wrong"})).send().await.unwrap().status(),409);}
    assert_eq!(t.req(Method::POST,"/auth/login","").json(&json!({"username":"admin","password":"test-password-123"})).send().await.unwrap().status(),StatusCode::TOO_MANY_REQUESTS);
    let spec:Value=t.http.get(format!("{}/openapi.json",t.url)).send().await.unwrap().json().await.unwrap();
    assert!(spec["components"]["schemas"]["Event"].is_object());assert!(spec["paths"]["/uploads/{id}"]["patch"].is_object());assert!(spec["paths"]["/channels/{id}/messages"]["post"]["requestBody"].is_object());
}
