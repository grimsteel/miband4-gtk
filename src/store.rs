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

#[derive(Serialize, Deserialize, Default)]
pub struct BandConf {
    pub auth_key: Option<String>,
    //pub alias: Option<String>
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

    pub async fn save(&self) -> Result<()> {
        let band_config = serde_json::to_vec(&self.bands)?;
        // write it to the bands file
        Ok(write(self.data_dir.join("bands.json"), band_config).await?)
    }
}
