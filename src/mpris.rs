use std::collections::HashMap;

use zbus::{proxy, Connection, zvariant::Value};
use futures::{channel::mpsc::Sender, pin_mut, select, stream::StreamExt, SinkExt};

use log::debug;

#[proxy(default_path = "/org/mpris/MediaPlayer2", interface = "org.mpris.MediaPlayer2.Player", gen_blocking = false, default_service = "org.mpris.MediaPlayer2.playerctld")]
trait MediaPlayer {
    fn play_pause(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, Value>>;
    #[zbus(property)]
    fn volume(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn set_volume(&self, new_volume: f64) -> zbus::Result<()>;
    #[zbus(property)]
    fn position(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
}

#[proxy(default_path = "/org/mpris/MediaPlayer2", interface = "com.github.altdesktop.playerctld", gen_blocking = false, default_service = "org.mpris.MediaPlayer2.playerctld")]
trait PlayerCtlD {
    #[zbus(property)]
    fn player_names(&self) -> zbus::Result<Vec<String>>;
}

pub struct MprisController<'a> {
    conn: Connection,
    player_proxy: MediaPlayerProxy<'a>,
    playerctl_proxy: PlayerCtlDProxy<'a>
}

#[derive(Debug)]
pub enum MediaState {
    Playing,
    Paused,
    Stopped
}

#[derive(Debug)]
pub struct MediaInfo {
    pub track: String,
    pub volume: f64,
    pub position: i64,
    pub duration: i64,
    pub state: MediaState
}

impl<'a> MprisController<'a> {
    pub async fn init() -> zbus::Result<Self> {
        let conn = Connection::session().await?;
        let player_proxy = MediaPlayerProxy::new(&conn).await?;
        let playerctl_proxy = PlayerCtlDProxy::new(&conn).await?;
        Ok(Self {
            conn,
            player_proxy,
            playerctl_proxy
        })
    }

    pub async fn watch_changes(&self, mut tx: Sender<Option<MediaInfo>>) -> zbus::Result<()> {
        // update the current player
        //*self.current_player.write().await = self.get_first_player().await?;
        let mut players_stream = self.playerctl_proxy.receive_player_names_changed().await.fuse();

        while let Some(players) = players_stream.next().await {
            let players = players.get().await?;
            if players.len() == 0 {
                let _ = tx.send(None).await;
            } else {
                let metadata = self.player_proxy.metadata().await?;
                let title = metadata.get("xesam:title").and_then(|s| s.downcast_ref().ok()).unwrap_or("Unknown title");
                let duration_micros: i64 = metadata.get("mpris:length").and_then(|s| s.downcast_ref().ok()).unwrap_or_default();
                // optional
                let position_micros = self.player_proxy.position().await.unwrap_or_default();
                // optional
                let volume = self.player_proxy.volume().await.unwrap_or_default();
                let state = self.player_proxy.playback_status().await?;
                let state = match state.as_str() {
                    "Playing" => MediaState::Playing,
                    "Paused" => MediaState::Paused,
                    _ => MediaState::Stopped
                };
                let item = MediaInfo { track: title.into(), volume, position: position_micros, duration: duration_micros, state };
                let _ = tx.send(Some(item)).await;
            }
        }

        Ok(())
    }
}

#[test]
fn test_asd() {
    env_logger::init();
    let context = gtk::glib::MainContext::new();
    context.block_on(async {
        let (tx, rx) = futures::channel::mpsc::channel(1);
        context.spawn_local(async move {
            let controller = MprisController::init().await.unwrap();
            controller.watch_changes(tx).await.unwrap();
        });
        rx.for_each(|item| async move {
            println!("item : {item:?}");
        }).await;
    });
}
