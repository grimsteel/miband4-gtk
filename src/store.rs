use std::{collections::HashMap, fmt::{self, Formatter, Display}, io::{self, ErrorKind}, path::{Path, PathBuf}};
use async_fs::{create_dir_all, read, write};
use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::utils::APP_ID;

// custom error wrapper type
#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    SerdeError(serde_json::Error)
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::SerdeError(value)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(err) => write!(f, "I/O error: {}", err),
            Self::SerdeError(err) => write!(f, "Serialization error: {}", err),
        }
    }
}
impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Serialize, Deserialize, Clone)]
pub struct ActivityGoal {
    pub notifications: bool,
    pub steps: u16
}

impl Default for ActivityGoal {
    fn default() -> Self {
        Self { notifications: true, steps: 10000 }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BandLock {
    pub pin: String,
    pub enabled: bool
}

impl Default for BandLock {
    fn default() -> Self {
        Self { pin: "1234".into(), enabled: false }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct BandConf {
    pub auth_key: Option<String>,

    pub activity_goal: Option<ActivityGoal>,
    pub band_lock: Option<BandLock>,
    pub alias: Option<String>
}

pub struct Store {
    data_dir: PathBuf,
    bands: HashMap<String, BandConf>
}

impl Store {
    pub async fn init() -> Result<Self> {
        // create the data dir
        let mut data_dir = glib::user_data_dir();
        data_dir.push(APP_ID);
        create_dir_all(&data_dir).await?;

        // load existing config
        let bands = Store::load_band_conf(&data_dir).await?;
        
        Ok(Self {
            data_dir,
            bands
        })
    }
    async fn load_band_conf(data_dir: &Path) -> Result<HashMap<String, BandConf>> {
        // read the band conf
        match read(data_dir.join("bands.json")).await {
            Ok(data) => {
                Ok(serde_json::from_slice(&data)?)
            },
            Err(err) => {
                // if we couldn't fine the band conf file, just return an empty map
                if err.kind() == ErrorKind::NotFound {
                    Ok(HashMap::new())
                } else {
                    // otherwise propagate the error
                    Err(err.into())
                }
            }
        }
    }
    pub fn get_band(&mut self, band_mac: String) -> &mut BandConf {
        self.bands.entry(band_mac).or_default()
    }
    /// returns the band alias, or the mac address if there was no alias
    pub fn get_band_alias<'a>(&'a self, band_mac: &'a str) -> &'a str {
        self.bands.get(band_mac).and_then(|b| b.alias.as_ref()).map(|s| s.as_str()).unwrap_or(band_mac)
    }

    pub async fn save(&self) -> Result<()> {
        let band_config = serde_json::to_vec(&self.bands)?;
        // write it to the bands file
        Ok(write(self.data_dir.join("bands.json"), band_config).await?)
    }
}
