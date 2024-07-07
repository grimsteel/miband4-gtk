use std::{collections::HashMap, time::Duration};

use async_io::Timer;
use zbus::{proxy, Connection, zvariant::Value};
use futures::{channel::mpsc::{Receiver, Sender}, pin_mut, select, stream::StreamExt, SinkExt};

use crate::band::MusicEvent;

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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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
    pub track: Option<String>,
    pub volume: Option<u8>, // 0 to 100
    pub position: Option<u64>,
    pub duration: Option<u64>,
    pub state: MediaState
}

const STREAM_THROTTLE: Duration = Duration::from_secs(1);

pub async fn watch_mpris(mut tx: Sender<Option<MediaInfo>>, mut controller_rx: Receiver<MusicEvent>) -> zbus::Result<()> {
    let conn = Connection::session().await?;
    let player_proxy = MediaPlayerProxy::new(&conn).await?;
    let playerctl_proxy = PlayerCtlDProxy::new(&conn).await?;
    
    // setup all of the streams
    let players_stream = playerctl_proxy.receive_player_names_changed().await
        .then(|e| async move {
            let players =  e.get().await;
            // return true if there's at least 1 player
            players.map(|p| p.len() > 0).unwrap_or(false)
        }).fuse();
    pin_mut!(players_stream);

    let _ = tx.send(None).await;

    // wait for at least one player to start before proceeding
    while let Some(false) = players_stream.next().await {}
    
    let mut metadata_stream = player_proxy.receive_metadata_changed().await.fuse();
    let mut playback_status_stream = player_proxy.receive_playback_status_changed().await.fuse();
    let mut volume_stream = player_proxy.receive_volume_changed().await.fuse();
    let mut position_stream = player_proxy.receive_position_changed().await.fuse();

    let mut current_media_info = MediaInfo::default();
    let mut players_exist = true;

    let mut need_send = false;

    let mut debounce_timer = Timer::after(STREAM_THROTTLE).fuse();

    loop {
        select! {
            new_players_exist = players_stream.next() => {
                players_exist = new_players_exist.unwrap_or_default();
                need_send = true;
            },
            metadata = metadata_stream.next() => {
                if let Some(metadata) = metadata {
                    if let Ok(metadata) = metadata.get().await {
                        let title = metadata.get("xesam:title").and_then(|s| s.downcast_ref().ok()).unwrap_or("Unknown title");
                        let duration_micros: Option<u64> = metadata
                            .get("mpris:length")
                            .and_then(|s| s.downcast_ref::<i64>().ok())
                            .and_then(|s| s.try_into().ok());
                        current_media_info.track = Some(title.into());
                        current_media_info.duration = duration_micros;
                    } else {
                        // set default values
                        current_media_info.track = None;
                        current_media_info.duration = None;
                    }
                    need_send = true;
                }
            },
            pos = position_stream.next() => {
                if let Some(pos) = pos {
                    current_media_info.position = pos.get().await.ok().and_then(|p| p.try_into().ok());
                    need_send = true;
                }
            },
            volume = volume_stream.next() => {
                if let Some(volume) = volume {
                    let volume = volume.get().await.map(|v| 
                                                        // add bounds to the volume and cast to u8
                                                        (v * 100f64).max(0f64).min(100f64) as u8).ok();
                    current_media_info.volume = volume;
                    need_send = true;
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
                    need_send = true;
                }
            },
            event = controller_rx.next() => {
                // the position doesn't refresh automatically
                current_media_info.position = player_proxy.position().await.ok().and_then(|p| p.try_into().ok());

                match event {
                    Some(MusicEvent::Open) => {
                        // immediately send an update
                        if tx.send(
                            if players_exist {
                                Some(current_media_info.clone())
                            } else {
                                None
                            }).await.is_err()
                        {
                            break
                        } else {
                            need_send = false;
                        }
                    },
                    _ => {}
                }
            },
            // once second has passed since the last update
            _ = debounce_timer.next() => {
                if need_send && tx.send(
                    if players_exist {
                        Some(current_media_info.clone())
                    } else {
                        None
                    }).await.is_err()
                {
                    break
                } else {
                    need_send = false;
                }
            }
        };

        // Reset the debounce timer
        if need_send {
            debounce_timer.get_mut().set_after(STREAM_THROTTLE);
        }
    }

    Ok(())
}
