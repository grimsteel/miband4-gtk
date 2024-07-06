use zbus::{fdo::{DBusProxy, PropertiesProxy}, names::{BusName, OwnedBusName, WellKnownName}, proxy, Connection};
use futures::{stream::StreamExt, select, pin_mut};
use async_lock::RwLock;

use log::debug;

#[proxy(default_path = "/org/mpris/MediaPlayer2", interface = "org.mpris.MediaPlayer2.Player", gen_blocking = false)]
trait MediaPlayer {
    fn play_pause(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn volume(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn set_volume(&self, new_volume: f64) -> zbus::Result<()>;
    #[zbus(property)]
    fn position(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
}

pub struct MprisController<'a> {
    conn: Connection,
    dbus: DBusProxy<'a>,
    current_player: RwLock<Option<MediaPlayerProxy<'a>>>,
}

pub struct MediaStatus {
    track: String,
    volume: u8,
    position: u8,
    duration: u8,
    state: u8
}

impl<'a> MprisController<'a> {
    pub async fn init() -> zbus::Result<Self> {
        let conn = Connection::session().await?;
        let dbus_proxy = DBusProxy::new(&conn).await?;
        Ok(Self {
            conn,
            dbus: dbus_proxy,
            current_player: RwLock::new(None)
        })
    }

    async fn get_first_player<'b>(&self) -> zbus::Result<Option<MediaPlayerProxy<'b>>> {
        let bus_names: Vec<OwnedBusName> = self.dbus.list_names().await?;

        //let mut player = self.current_player.write().expect("can write to current player");
        
        // For right now, just use the first one
        Ok(if let Some(mpris_bus_name) = bus_names
            .into_iter()
            .filter(|name| name.starts_with("org.mpris.MediaPlayer2"))
            .next()
        {
            Some(MediaPlayerProxy::new(&self.conn, mpris_bus_name).await?)
        } else {
            None
        })
    }

    pub async fn watch_changes(&self) -> zbus::Result<()> {
        // update the current player
        *self.current_player.write().await = self.get_first_player().await?;
        
        loop {
            let current_player = self.current_player.read().await;
            let new_player = if let Some(player) = current_player.as_ref() {
                let player_name = player.0.destination().to_owned();

                // wait for this player to disappear from the bus
                let mut player_gone = self.dbus
                    .receive_name_owner_changed_with_args(&[
                        (0, player_name.as_str()), // name = our current player
                        (2, "") // new_owner = None
                    ]).await?.fuse();

                let properties_proxy = PropertiesProxy::new(&self.conn, player_name, "/org/mpris/MediaPlayer2")
                    .await?;

                let mut properties_changed = properties_proxy.receive_properties_changed_with_args(&[
                    (0, "org.mpris.MediaPlayer2.Player")
                ]).await?.fuse();

                loop {
                    // but also listen to changes
                    select! {
                        _ = player_gone.next() => {
                            // if the player disappeared, stop
                            debug!("player gone");
                            break;
                        },
                        e = properties_changed.next() => {
                            let args = e.as_ref().unwrap().args();
                            debug!("args {:?}", args);
                        }
                    };
                }
                self.get_first_player().await?
            } else {
                // wait for a name to be acquired on the bus before continuing
                let new_player_stream = self.dbus.receive_name_owner_changed().await?
                    .filter_map(|e| async move {
                        let args = e.args().ok()?;
                        // wait for a new MPRIS player
                        if let BusName::WellKnown(name) = args.name {
                            //                                            make sure there's actually an owner here
                            if name.starts_with("org.mpris.MediaPlayer2") && args.new_owner.is_some() {
                                return Some(name.into_owned());
                            }
                        }
                        None
                    });
                pin_mut!(new_player_stream);
                let new_player: WellKnownName = new_player_stream.next().await.expect("stream never ends");

                let proxy = MediaPlayerProxy::new(&self.conn, new_player).await?;

                Some(proxy)
            };

            // update with the new player
            *self.current_player.write().await = new_player;
        };
    }
}

#[test]
fn test_asd() {
    env_logger::init();
    let context = gtk::glib::MainContext::new();
    context.block_on(async {
        let controller = MprisController::init().await.unwrap();
        controller.watch_changes().await.unwrap();
    });
}
