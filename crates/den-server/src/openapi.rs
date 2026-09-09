use crate::*;
use utoipa::OpenApi;
#[derive(OpenApi)]
#[openapi(
    info(title="Den API",version="0.1.0",description="M1 REST API. Cookie-authenticated writes require Origin and X-CSRF-Token. CLI and agents use bearer authentication. All IDs are ULIDs."),
    paths(health,auth::bootstrap,auth::register,auth::login,auth::logout,auth::me,auth::users,auth::invite,auth::revoke_invite,auth::tokens,auth::create_token,auth::revoke_token,auth::bot,chat::categories,chat::create_category,chat::update_category,chat::delete_category,chat::channels,chat::channel,chat::create_channel,chat::update_channel,chat::delete_channel,chat::dm,chat::messages,chat::send,chat::edit,chat::remove,uploads::begin,uploads::status,uploads::chunk,uploads::complete,uploads::file,ws::connect),
    components(schemas(Event,ApiError,Health,User,Role,Channel,ChannelKind,Message,Upload,Session,Login,Register,Bootstrap,CreateInvite,Invite,CreateToken,Token,TokenSecret,CreateBot,BotCreated,Category,SaveCategory,SaveChannel,CreateDm,CreateMessage,EditMessage,MessageQuery,BeginUpload))
)]
struct Api;
pub(crate) async fn serve()->Json<utoipa::openapi::OpenApi> { Json(Api::openapi()) }
