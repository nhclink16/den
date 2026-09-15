use crate::{
    auth::{self, Auth},
    *,
};

/// Ephemeral credentials do not survive a restart. Only SHA-256 digests are retained.
#[derive(Default)]
pub(crate) struct Tickets(HashMap<String, (Auth, Instant)>);
impl Tickets {
    fn issue(&mut self, auth: Auth, at: Instant) -> String {
        // A client needs one pending upgrade. Reissuing also bounds storage per session.
        self.0
            .retain(|_, (a, expiry)| *expiry > at && a.credential != auth.credential);
        let ticket = auth::secret();
        self.0
            .insert(auth::hash(&ticket), (auth, at + Duration::from_secs(30)));
        ticket
    }
    pub(crate) fn take(&mut self, ticket: &str, at: Instant) -> Option<Auth> {
        self.0
            .remove(&auth::hash(ticket))
            .filter(|(_, expiry)| *expiry > at)
            .map(|(a, _)| a)
    }
}

#[utoipa::path(post,path="/auth/ws-ticket",responses((status=200,body=WsTicket)))]
pub(crate) async fn issue(State(s): State<AppState>, a: Auth) -> impl IntoResponse {
    let ticket = s.tickets.lock().await.issue(a, Instant::now());
    (
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(WsTicket {
            ticket,
            expires_in: 30,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_tickets_are_consumed_and_never_return_credentials() {
        let mut tickets = Tickets::default();
        let at = Instant::now();
        let auth = Auth {
            user: User {
                id: "a".into(),
                username: "a".into(),
                display_name: "a".into(),
                avatar_url: None,
                banner_url: None,
                bio: None,
                accent: None,
                status: None,
                bot: false,
                role: Role::Member,
            },
            credential: "session".into(),
            session: true,
        };
        let ticket = tickets.issue(auth.clone(), at);
        assert!(!tickets.0.contains_key(&ticket));
        assert!(tickets
            .take(&ticket, at + Duration::from_secs(30))
            .is_none());
        assert!(tickets.take(&ticket, at).is_none());
        let ticket = tickets.issue(auth, at);
        assert!(tickets
            .take(&ticket, at + Duration::from_secs(29))
            .is_some());
        assert!(tickets.take(&ticket, at).is_none());
    }
}
