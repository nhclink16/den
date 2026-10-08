// GIF search through KLIPY. Its terms require every search and every GIF load
// to go from the person's own app straight to KLIPY, with no copies kept. So
// Den hands out the key, and a GIF picked as a picture is saved as KLIPY's
// links: Den's picture URL for it redirects there and never fetches the GIF.
use crate::{auth::Auth, *};
use axum::http::header;

const HOSTS: [&str; 3] = [
    "https://static.klipy.com/",
    "https://static1.klipy.com/",
    "https://static2.klipy.com/",
];

fn media(url: &str) -> bool {
    url.len() <= 1024
        && HOSTS.iter().any(|host| url.starts_with(host))
        && url
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\')
}
/// Refuses anything but KLIPY's own media links, so a picture can only ever
/// redirect to KLIPY.
pub(crate) fn check(picture: &KlipyPicture) -> Result<()> {
    if !media(&picture.url) || !media(&picture.still_url) {
        return Err(Error::bad("Pick a GIF from KLIPY search"));
    }
    Ok(())
}
pub(crate) fn save(picture: &KlipyPicture) -> Vec<u8> {
    serde_json::to_vec(picture).expect("plain struct serializes")
}
/// A stored file is either an image or the JSON of a picked GIF. No image
/// format starts with `{`, so the two never mix.
pub(crate) fn load(bytes: &[u8]) -> Option<KlipyPicture> {
    if bytes.first() != Some(&b'{') {
        return None;
    }
    serde_json::from_slice(bytes)
        .ok()
        .filter(|p| check(p).is_ok())
}
pub(crate) fn redirect(url: &str, max_age: u32) -> Response {
    let mut response = StatusCode::FOUND.into_response();
    let headers = response.headers_mut();
    headers.insert(header::LOCATION, url.parse().expect("checked when saved"));
    headers.insert(
        header::CACHE_CONTROL,
        format!("private, max-age={max_age}").parse().unwrap(),
    );
    headers.insert(header::VARY, "Authorization, Cookie".parse().unwrap());
    response
}

#[utoipa::path(get,path="/klipy",responses((status=200,body=Klipy),(status=401,body=ApiError)))]
pub(crate) async fn config(_a: Auth) -> Json<Klipy> {
    Json(Klipy {
        app_key: std::env::var("DEN_KLIPY_KEY")
            .ok()
            .filter(|key| !key.is_empty()),
    })
}
