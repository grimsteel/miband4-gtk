use std::{collections::HashMap, io, path::{Path, PathBuf}};
use async_fs::{create_dir_all, read};
use gtk::glib;
use serde::{Deserialize, Serialize};

use crate::utils::APP_ID;

enum Error {
    IoError(io::Error),
    SerdeError(serde_json::Error)
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Serialize, Deserialize)]
struct BandConf {
    pub auth_key: Option<String>,
    pub alias: Option<String>
}

pub struct Store {
    data_dir: PathBuf,
    bands: HashMap<String, BandConf>
}

impl Store {
    pub async fn init() -> Result<Self> {
        let mut data_dir = glib::user_data_dir();
        data_dir.push(APP_ID);
        create_dir_all(&data_dir).await?;
        let bands = load_band_conf(&data_dir).await?;
        Ok(Self {
            data_dir,
            bands
        })
    }
    async fn load_band_conf(data_dir: &Path) -> Result<HashMap<String, BandConf>> {
        // read the band conf
        let data = read(data_dir.join("bands.json")).await?;
        let bands = serde_json::from_slice(&data);
    }
}
