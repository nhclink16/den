use crate::*;
use utoipa::OpenApi;
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Den API",
        version = "0.1.0",
        description = "M1 REST API. Cookie-authenticated writes require Origin and X-CSRF-Token. CLI and agents use bearer authentication. All IDs are ULIDs."
    ),
    paths(
        health,
        auth::bootstrap,
        auth::register,
        auth::login,
        auth::logout,
        auth::me,
        auth::users,
        credentials::invite,
        credentials::revoke_invite,
        credentials::tokens,
        credentials::create_token,
        credentials::revoke_token,
        credentials::bot,
        chat::categories,
        chat::create_category,
        chat::update_category,
        chat::delete_category,
        chat::channels,
        chat::channel,
        chat::create_channel,
        chat::update_channel,
        chat::delete_channel,
        chat::dm,
        chat::messages,
        chat::send,
        chat::edit,
        chat::remove,
        uploads::begin,
        uploads::status,
        uploads::chunk,
        uploads::complete,
        uploads::file,
        ws::connect
    ),
    components(schemas(
        Event,
        ApiError,
        Health,
        User,
        Role,
        Channel,
        ChannelKind,
        Message,
        Upload,
        Session,
        Login,
        Register,
        Bootstrap,
        CreateInvite,
        Invite,
        CreateToken,
        Token,
        TokenSecret,
        CreateBot,
        BotCreated,
        Category,
        SaveCategory,
        SaveChannel,
        CreateDm,
        CreateMessage,
        EditMessage,
        MessageQuery,
        BeginUpload
    ))
)]
struct Api;
pub(crate) async fn serve() -> Json<utoipa::openapi::OpenApi> {
    use utoipa::openapi::{
        security::{
            ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityRequirement, SecurityScheme,
        },
        Content, Ref, ResponseBuilder,
    };
    let mut api = Api::openapi();
    let components = api.components.as_mut().unwrap();
    components.add_security_scheme(
        "bearerAuth",
        SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
    );
    components.add_security_scheme(
        "sessionCookie",
        SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("den_session"))),
    );
    api.security = Some(vec![
        SecurityRequirement::new("bearerAuth", Vec::<String>::new()),
        SecurityRequirement::new("sessionCookie", Vec::<String>::new()),
    ]);
    for (path, item) in &mut api.paths.paths {
        let public = matches!(
            path.as_str(),
            "/health" | "/auth/init" | "/auth/login" | "/auth/register"
        );
        for op in [
            &mut item.get,
            &mut item.post,
            &mut item.put,
            &mut item.patch,
            &mut item.delete,
        ]
        .into_iter()
        .flatten()
        {
            if public {
                op.security = Some(Vec::new());
            }
            op.responses.responses.insert("default".into(), ResponseBuilder::new().description("API error; invalid input, authentication, permissions, conflict, throttling or storage failure").content("application/json",Content::new(Some(Ref::from_schema_name("ApiError")))).build().into());
        }
        if path == "/uploads/{id}/file" {
            item.head = item.get.clone();
            if let Some(op) = &mut item.head {
                op.operation_id = Some("file_head".into());
            }
        }
    }
    Json(api)
}
