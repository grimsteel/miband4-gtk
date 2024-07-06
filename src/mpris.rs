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

#[derive(Debug, Copy, Clone)]
pub enum MediaState {
    Playing,
    Paused,
    Stopped
}

impl Default for MediaState {
    fn default() -> Self {
        Self::Stopped
    }
}

#[derive(Debug, Default, Clone)]
pub struct MediaInfo {
    pub track: String,
    pub volume: u8, // 0 to 100
    pub position: u64,
    pub duration: u64,
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
        // setup all of the streams
        let players_stream = self.playerctl_proxy.receive_player_names_changed().await
            .then(|e| async move {
                let players =  e.get().await;
                // return true if there's at least 1 player
                players.map(|p| p.len() > 0).unwrap_or(false)
            }).fuse();
        pin_mut!(players_stream);

        // wait for at least one player to start before proceeding
        while let Some(false) = players_stream.next().await {}
        
        let mut metadata_stream = self.player_proxy.receive_metadata_changed().await.fuse();
        let mut playback_status_stream = self.player_proxy.receive_playback_status_changed().await.fuse();
        let mut volume_stream = self.player_proxy.receive_volume_changed().await.fuse();
        let mut position_stream = self.player_proxy.receive_position_changed().await.fuse();

        let mut current_media_info = MediaInfo::default();
        let mut players_exist = true;

        loop {
            select! {
                new_players_exist = players_stream.next() => {
                    players_exist = new_players_exist.unwrap_or_default();
                },
                metadata = metadata_stream.next() => {
                    if let Some(metadata) = metadata {
                        if let Ok(metadata) = metadata.get().await {
                            let title = metadata.get("xesam:title").and_then(|s| s.downcast_ref().ok()).unwrap_or("Unknown title");
                            let duration_micros: i64 = metadata.get("mpris:length").and_then(|s| s.downcast_ref().ok()).unwrap_or_default();
                            current_media_info.track = title.into();
                            current_media_info.duration = duration_micros.try_into().unwrap_or_default();
                        } else {
                            // set default values
                            current_media_info.track = "".into();
                            current_media_info.duration = 0;
                        }
                    }
                },
                pos = position_stream.next() => {
                    if let Some(pos) = pos {
                        let pos = pos.get().await.unwrap_or_default();
                        current_media_info.position = pos.try_into().unwrap_or_default();
                    }
                },
                volume = volume_stream.next() => {
                    if let Some(volume) = volume {
                        let volume = volume.get().await.unwrap_or_default();
                        // add bounds to the volume and cast to u8
                        let volume = (volume * 100f64).max(0f64).min(100f64) as u8;
                        current_media_info.volume = volume;
                    }
                },
                status = playback_status_stream.next() => {
                    if let Some(status) = status {
                        let status = status.get().await
                            // transform  the status string to our enum
                            .map(|s| match s.as_str() {
                                "Playing" => MediaState::Playing,
                                "Paused" => MediaState::Paused,
                                _ => MediaState::Stopped
                            }).unwrap_or_default();
                        current_media_info.state = status;
                    }
                }
            };

            if tx.send(
                if players_exist {
                    Some(current_media_info.clone())
                } else {
                    None
                }).await.is_err()
            {
                break
            };
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
