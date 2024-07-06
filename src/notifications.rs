use std::{error::Error, fmt::Display};

use log::warn;
use zbus::{fdo::MonitoringProxy, message, zvariant::Structure, Connection, MatchRule, Message, MessageStream};
use futures::{Stream, StreamExt};

// notification error type

#[derive(Debug)]
pub enum NotificationParseError {
    DBusError(zbus::Error),
    WrongSignature
}

impl From<zbus::Error> for NotificationParseError {
    fn from(value: zbus::Error) -> Self {
        Self::DBusError(value)
    }
}

impl Display for NotificationParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DBusError(e) => write!(f, "D-Bus error: {e}"),
            Self::WrongSignature => write!(f, "Wrong notification signature")
        }
    }
}

impl Error for NotificationParseError {}

// notification struct

#[derive(Debug, Clone)]
pub struct Notification {
    pub app: String,
    pub summary: String,
    pub body: String
}

impl TryFrom<Message> for Notification {
    type Error = NotificationParseError;

    fn try_from(msg: Message) -> Result<Self, Self::Error> {
        let body = msg.body();

        // validate the signature
        // See https://specifications.freedesktop.org/notification-spec/notification-spec-latest.html#protocol
        if !body.signature().map(|s| s == "susssasa{sv}i").unwrap_or(false) {
            return Err(Self::Error::WrongSignature);
        }

        let body: Structure = body.deserialize()?;
        let fields = body.fields();
        let app_name: &str = fields[0].downcast_ref().expect("first argument is string");
        let summary: &str = fields[3].downcast_ref().expect("fourth argument is string");
        let body: &str = fields[4].downcast_ref().expect("fifth argument is string");
        
        Ok(Notification {
            app: app_name.to_string(),
            summary: summary.to_string(),
            body: body.to_string()
        })
    }
    
}

// methods

pub async fn stream_notifications() -> zbus::Result<impl Stream<Item = Notification>> {
    let conn = Connection::session().await?;
    let proxy = MonitoringProxy::new(&conn).await?;
    
    // match all calls to Notify on fdo.Notifications
    let rule = MatchRule::builder()
        .path("/org/freedesktop/Notifications")?
        .interface("org.freedesktop.Notifications")?
        .member("Notify")?
        .msg_type(message::Type::MethodCall)
        .build();
    proxy.become_monitor(&[rule.clone()], 0).await?;
    
    // start streaming messages
    let stream: MessageStream = conn.into();
    Ok(stream.filter_map(move |item| {
        let rule = rule.clone();
        async move {
            let item = item.ok()?;
            // make sure it matches
            if !rule.matches(&item).unwrap_or(false) { return None }

            match item.try_into() {
                Ok(notif) => Some(notif),
                Err(err) => {
                    warn!("An error occurred while parsing a notification: {err}");
                    None
                }
            }
        }
    }))
}
