use std::sync::RwLock;

use zbus::{fdo::DBusProxy, names::OwnedBusName, Connection, proxy};

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
        let current_player = Self::get_current_player(&conn, &dbus_proxy).await?;
        Ok(Self {
            conn,
            dbus: dbus_proxy,
            current_player: RwLock::new(current_player)
        })
    }

    async fn get_current_player<'b, 'c>(conn: &Connection, proxy: &DBusProxy<'b>) -> zbus::Result<Option<MediaPlayerProxy<'c>>> {
        let bus_names: Vec<OwnedBusName> = proxy.list_activatable_names().await?;

        // For right now, just use the first one
        if let Some(mpris_bus_name) = bus_names
            .into_iter()
            .filter(|name| name.starts_with("org.mpris.MediaPlayer2"))
            .next()
        {
            Ok(Some(MediaPlayerProxy::new(conn, mpris_bus_name).await?))
        } else {
            Ok(None)
        }
    }

    
}
