//! The admin module's door (U6): Tauri commands over the admin calls of `crates/client`. The
//! control plane checks the role on every call; the UI only hides what it cannot use.

use crate::App;
use api_types::{
    Channel, CreateInviteRequest, EventEntry, InviteCode, InviteLink, Member, PublicConfig, Role,
    UpdateChannelRequest, VersionPolicy,
};
use client::{ApiClient, ApiFailure};
use tauri::State;

type Reply<T> = Result<T, ApiFailure>;

/// How many lines of the event log the admin sees.
const EVENT_LINES: u32 = 200;

fn admin(app: &State<'_, App>) -> Reply<ApiClient> {
    app.client.admin()
}

#[tauri::command]
pub async fn admin_members(app: State<'_, App>) -> Reply<Vec<Member>> {
    admin(&app)?.members().await
}

#[tauri::command]
pub async fn admin_set_role(app: State<'_, App>, user_id: String, role: Role) -> Reply<()> {
    admin(&app)?.set_role(&user_id, role).await
}

#[tauri::command]
pub async fn admin_kick(app: State<'_, App>, user_id: String) -> Reply<()> {
    admin(&app)?.kick(&user_id).await
}

#[tauri::command]
pub async fn admin_invites(app: State<'_, App>) -> Reply<Vec<InviteCode>> {
    admin(&app)?.invites().await
}

#[tauri::command]
pub async fn admin_create_invite(app: State<'_, App>) -> Reply<InviteCode> {
    let request = CreateInviteRequest {
        role: None,
        days: None,
    };
    admin(&app)?.create_invite(&request).await
}

/// The clickable link for a code: the code plus this server's API address.
#[tauri::command]
pub fn admin_invite_link(app: State<'_, App>, code: String) -> Reply<String> {
    let server = admin(&app)?.server().to_string();
    Ok(InviteLink { server, code }.to_link())
}

#[tauri::command]
pub async fn admin_revoke_invite(app: State<'_, App>, code: String) -> Reply<()> {
    admin(&app)?.revoke_invite(&code).await
}

#[tauri::command]
pub async fn admin_create_channel(app: State<'_, App>, name: String) -> Reply<Channel> {
    admin(&app)?.create_channel(&name).await
}

#[tauri::command]
pub async fn admin_rename_channel(app: State<'_, App>, id: String, name: String) -> Reply<()> {
    let request = UpdateChannelRequest {
        name: Some(name),
        position: None,
    };
    admin(&app)?.update_channel(&id, &request).await.map(drop)
}

#[tauri::command]
pub async fn admin_move_channel(app: State<'_, App>, id: String, position: u32) -> Reply<()> {
    let request = UpdateChannelRequest {
        name: None,
        position: Some(position),
    };
    admin(&app)?.update_channel(&id, &request).await.map(drop)
}

#[tauri::command]
pub async fn admin_delete_channel(app: State<'_, App>, id: String) -> Reply<()> {
    admin(&app)?.delete_channel(&id).await
}

#[tauri::command]
pub async fn admin_events(app: State<'_, App>) -> Reply<Vec<EventEntry>> {
    admin(&app)?.events(EVENT_LINES).await
}

#[tauri::command]
pub async fn admin_set_policy(app: State<'_, App>, policy: VersionPolicy) -> Reply<PublicConfig> {
    admin(&app)?.set_policy(&policy).await
}
