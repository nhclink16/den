use crate::*;
use den_core::Object as LiveObject;
use utoipa::OpenApi;
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Den API",
        version = "0.1.0",
        description = "M3 REST API. Cookie-authenticated writes require Origin and X-CSRF-Token. CLI and agents use bearer authentication. All IDs are ULIDs."
    ),
    paths(
        health,
        hosts::list,
        hosts::enroll,
        hosts::login,
        hosts::remove,
        hosts::connect,
        hosts::direct_check,
        access::request,
        access::decide,
        access::grants,
        access::revoke,
        access::log,
        terminal::open,
        terminal::controller,
        terminal::request_control,
        terminal::share,
        terminal::write,
        terminal::close,
        terminal::direct_token,
        objects::settings,
        objects::save_settings,
        objects::create,
        objects::get,
        objects::summary,
        objects::patch,
        objects::update,
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
        messages::messages,
        messages::send,
        messages::edit,
        messages::remove,
        uploads::begin,
        uploads::status,
        uploads::chunk,
        uploads::complete,
        uploads::file,
        activity::message,
        activity::react,
        activity::unreact,
        activity::search,
        inbox::get_preferences,
        inbox::put_preferences,
        inbox::read_states,
        inbox::mark_read,
        ws::presence,
        calls::token,
        calls::list,
        calls::webhook,
        thumbnails::serve,
        ws::connect
    ),
    components(schemas(
        Host,
        HostEnrollment,
        HostCredential,
        HostLogin,
        HostFrame,
        TerminalFrame,
        Capability,
        Grant,
        RequestAccess,
        AccessRequest,
        AccessDecision,
        AccessLog,
        OpenTerminal,
        TerminalState,
        SetController,
        ShareTerminal,
        TerminalWrite,
        DirectToken,
        DirectCheck,
        DirectPermission,
        Event,
        LiveObject,
        ObjectSummary,
        CreateObject,
        ObjectPatch,
        UpdateObject,
        ObjectVersion,
        ObjectPresence,
        Settings,
        UpdateSettings,
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
        BeginUpload,
        Reaction,
        SetReaction,
        MarkRead,
        ChannelReadState,
        NotificationPreferences,
        NotificationReason,
        SearchMessages,
        ClientEvent,
        PresenceState,
        CallToken,
        CallState
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
            "/hosts/login"
                | "/livekit/webhook"
                | "/health"
                | "/auth/init"
                | "/auth/login"
                | "/auth/register"
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
        if path == "/uploads/{id}/file" || path == "/uploads/{id}/thumbnail" {
            item.head = item.get.clone();
            if let Some(op) = &mut item.head {
                op.operation_id = Some(
                    if path.ends_with("thumbnail") {
                        "thumbnail_head"
                    } else {
                        "file_head"
                    }
                    .into(),
                );
            }
        }
    }
    Json(api)
}
