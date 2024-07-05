use zbus::{fdo::MonitoringProxy, Connection, MatchRule, MessageStream, message};
use futures::{Stream, StreamExt};

#[derive(Debug, Clone)]
pub struct Notification {
    app: String,
    summary: String,
    body: String
}

#[derive(Debug)]
pub struct NotificationWatcher {
    stream: MessageStream
}

impl NotificationWatcher {
    pub async fn init() -> zbus::Result<Self> {
        let conn = Connection::session().await?;
        let proxy = MonitoringProxy::new(&conn).await?;
        // match all calls to Notify on fdo.Notifications
        let rule = MatchRule::builder()
            .arg0ns("org.freedesktop.Notifications")?
            .path("/org/freedesktop/Notifications")?
            .interface("org.freedesktop.Notifications")?
            .member("Notify")?
            .msg_type(message::Type::MethodCall)
            .build();
        proxy.become_monitor(&[rule], 0).await?;
        Ok(Self {
            stream: conn.into()
        })
    }

    pub fn stream_notifications(self) -> impl Stream<Item = Notification> {
        self.stream.filter_map(|item| async {
            let item = item.ok()?;
            eprintln!("got item: {item:?}");
            Some(Notification {
                app: "".into(),
                summary: "".into(),
                body: "".into()
            })
        })
    }
}


#[test]
fn random_test() {
    let ctx = gtk::glib::MainContext::new();
    ctx.block_on(async {
        NotificationWatcher::init().await.unwrap().stream_notifications().for_each(|item| async {}).await;
    });
}
