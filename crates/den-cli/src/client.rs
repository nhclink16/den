use anyhow::{bail, Context};
use den_core::Session;
use reqwest::{blocking::Client as Http, Method, Url};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub struct Client {
    pub url: String,
    pub token: Option<String>,
    http: Http,
    pub config: PathBuf,
}
impl Client {
    pub fn new(
        url: String,
        token: Option<String>,
        config: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        let parsed = Url::parse(&url)?;
        anyhow::ensure!(
            parsed.scheme() == "https"
                || (parsed.scheme() == "http"
                    && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))),
            "Use HTTPS except on loopback"
        );
        anyhow::ensure!(
            parsed.path() == "/"
                && parsed.query().is_none()
                && parsed.fragment().is_none()
                && parsed.username().is_empty()
                && parsed.password().is_none(),
            "Server URL must be an origin"
        );
        let url = url.trim_end_matches('/').to_string();
        let config = config.unwrap_or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()))
                        .join(".config")
                })
                .join("den/credentials.json")
        });
        let saved = Self::read(&config)?;
        let token = token.or_else(|| saved.get(&url).map(|s| s.token.clone()));
        Ok(Self {
            url,
            token,
            config,
            http: Http::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(60))
                .build()?,
        })
    }
    fn read(path: &PathBuf) -> anyhow::Result<BTreeMap<String, Session>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).context("Invalid credential file")?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&mut self, session: Session) -> anyhow::Result<()> {
        let mut saved = Self::read(&self.config)?;
        self.token = Some(session.token.clone());
        saved.insert(self.url.clone(), session);
        self.write(&saved)
    }
    pub fn forget(&self) -> anyhow::Result<()> {
        let mut saved = Self::read(&self.config)?;
        saved.remove(&self.url);
        self.write(&saved)
    }
    fn write(&self, saved: &BTreeMap<String, Session>) -> anyhow::Result<()> {
        use std::io::Write;
        if let Some(parent) = self.config.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = self
            .config
            .with_extension(format!("{}.tmp", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(&serde_json::to_vec_pretty(saved)?)?;
        file.sync_all()?;
        std::fs::rename(&temp, &self.config)?;
        Ok(())
    }
    pub fn request(&self, method: Method, path: &str) -> reqwest::blocking::RequestBuilder {
        let request = self.http.request(method, format!("{}{path}", self.url));
        if let Some(token) = &self.token {
            request.bearer_auth(token)
        } else {
            request
        }
    }
    pub fn decode<T: DeserializeOwned>(response: reqwest::blocking::Response) -> anyhow::Result<T> {
        let status = response.status();
        if !status.is_success() {
            let message = response
                .json::<den_core::ApiError>()
                .map(|v| v.message)
                .unwrap_or_else(|_| status.to_string());
            bail!("{status}: {message}");
        }
        response.json().context("Invalid API response")
    }
    pub fn get<T: DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        Self::decode(self.request(Method::GET, path).send()?)
    }
    pub fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &impl Serialize,
    ) -> anyhow::Result<T> {
        Self::decode(self.request(method, path).json(body).send()?)
    }
    pub fn empty(&self, method: Method, path: &str) -> anyhow::Result<()> {
        let response = self.request(method, path).send()?;
        if !response.status().is_success() {
            let _: serde_json::Value = Self::decode(response)?;
        }
        Ok(())
    }
}
pub fn print(value: &impl Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}
