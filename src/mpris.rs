use std::sync::RwLock;

use zbus::{fdo::DBusProxy, names::{BusName, OwnedBusName, WellKnownName}, proxy, Connection};
use futures::{stream::StreamExt, select, pin_mut};

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

struct MprisController<'a> {
    conn: Connection,
    dbus: DBusProxy<'a>,
    current_player: RwLock<Option<MediaPlayerProxy<'a>>>,
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

    async fn refresh_current_player<'b, 'c>(&self) -> zbus::Result<()> {
        let bus_names: Vec<OwnedBusName> = self.dbus.list_names().await?;

        let mut player = self.current_player.write().expect("can write to current player");
        
        // For right now, just use the first one
        *player = if let Some(mpris_bus_name) = bus_names
            .into_iter()
            .filter(|name| name.starts_with("org.mpris.MediaPlayer2"))
            .next()
        {
            Some(MediaPlayerProxy::new(&self.conn, mpris_bus_name).await?)
        } else {
            None
        };

        Ok(())
    }

    async fn watch_changes(&self) -> zbus::Result<()> {
        self.refresh_current_player().await?;
        
        loop {
            let current_player = self.current_player.read().expect("can read player");
            if let Some(player) = current_player.as_ref() {
                // wait for this player to disappear from the bus
                let player_name = player.0.destination().as_str();


                // TODO: actually wait
                debug!("Player {:?} disappeared", player_name);

                // drop early so refresh_current_player can write
                drop(current_player);
                self.refresh_current_player().await?;
            } else {
                // wait for a name to be acquired on the bus before continuing
                let new_player_stream = self.dbus.receive_name_owner_changed().await?
                    .filter_map(|e| async move {
                        let args = e.args().ok()?;
                        // wait for a new MPRIS player
                        if let BusName::WellKnown(name) = args.name {
                            if name.starts_with("org.mpris.MediaPlayer2") {
                                return Some(name.into_owned());
                            }
                        }
                        None
                    });
                pin_mut!(new_player_stream);
                let new_player: WellKnownName = new_player_stream.next().await.expect("stream will never end");

                debug!("Player {:?} found", new_player);
                let proxy = MediaPlayerProxy::new(&self.conn, new_player).await?;

                // drop it early so we can write
                drop(current_player);
                self.current_player.write()
                    .expect("can write to current player")
                    .replace(proxy);
            }
        }
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
