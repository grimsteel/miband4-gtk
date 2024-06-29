use std::{cell::RefCell, collections::{HashMap, HashSet}, time::Duration};

use async_io::Timer;
use async_lock::OnceCell;
use futures::{pin_mut, select, stream::SelectAll, StreamExt};
use gtk::{
    gio::{ActionGroup, ActionMap, ListStore}, glib::{self, clone, object_subclass, spawn_future_local, subclass::InitializingObject, Object}, prelude::*, subclass::prelude::*, template_callbacks, Accessible, AlertDialog, Application, ApplicationWindow, Buildable, Button, CompositeTemplate, ConstraintTarget, Entry, HeaderBar, Label, ListItem, ListView, Native, NoSelection, Root, ShortcutManager, SignalListItemFactory, Stack, Widget, Window
};
use log::{debug, error};
use zbus::zvariant::OwnedObjectPath;

use crate::{band::{self, BandChangeEvent, MiBand}, bluez::{BluezSession, DiscoveredDevice, DiscoveredDeviceEvent}, utils::decode_hex};

use super::{auth_key_dialog::AuthKeyDialog, device_row::DeviceRow, device_row_object::DeviceRowObject};

glib::wrapper! {
    pub struct MiBandWindow(ObjectSubclass<MiBandWindowImpl>)
        // refer to https://docs.gtk.org/gtk4/class.ApplicationWindow.html#hierarchy
        @extends ApplicationWindow, Window, Widget,
        @implements ActionGroup, ActionMap, Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager;
}

#[template_callbacks]
impl MiBandWindow {
    
    pub fn new(app: &Application) -> Self {
        Object::builder().property("application", app).build()
    }

    fn set_page(&self, page: &str) {
        self.imp().main_stack.set_visible_child_name(page);
    }

    fn devices(&self) -> ListStore {
        self.imp().devices.borrow().clone().expect("could not get devices")
    }

    fn set_all_titles(&self, title: &str) {
        self.set_title(Some(&title));
        self.imp().titlebar_label.set_label(title);
    }

    async fn get_session(&self) -> band::Result<&BluezSession<'static>> {
        Ok(self.imp().session.get_or_try_init(|| async {
            BluezSession::new().await
        }).await?)
    }

    fn show_error(&self, message: &str)  {
        let dialog = AlertDialog::builder()
            .message("An error occurred")
            .detail(message)
            .modal(true)
            .build();

        error!("{}",message);

        dialog.show(Some(self));
    }

    fn show_home(&self) {
        // show the device list page
        self.imp().main_stack.set_visible_child_name("device-list");
        // hide the back button
        self.imp().btn_back.set_visible(false);
    }

    #[template_callback]
    fn handle_start_scan_clicked(&self, _button: &Button) {
        spawn_future_local(clone!(@weak self as win => async move {
            if let Err(err) = win.run_scan().await {
                win.show_error(&format!("An error occurred while running the scan: {err:?}"));
            }
        }));
    }
    #[template_callback]
    fn handle_back_clicked(&self) {
        self.show_home();
    }
    #[template_callback]
    fn handle_auth_key_clicked(&self) {
        // show the auth key modal
        self.imp().auth_key_dialog.present();
        // clear the input
        self.imp().auth_key_dialog.set_auth_key("");
    }

    fn setup_device_list(&self, initial_model: ListStore) {
        // setup the factory
        let device_list_factory = SignalListItemFactory::new();
        device_list_factory.connect_setup(move |_, list_item| {
            let row = DeviceRow::new();
            let list_item = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem");
            
            list_item.set_child(Some(&row));

            // bind list_item->item to row->device
            list_item.property_expression("item").bind(&row, "device", Widget::NONE);
        });

        self.imp().list_devices.set_factory(Some(&device_list_factory));

        // setup the model
        self.imp().devices.replace(Some(initial_model));
        self.imp().list_devices.set_model(Some(&NoSelection::new(Some(self.devices()))));

        self.imp().list_devices.connect_activate(clone!(@weak self as win => move |list_view, idx| {
            // get the DiscoveredDevice they clicked
            let model = list_view.model().expect("the model must not be None at this point");
            
            let device: DiscoveredDevice = model
                .item(idx)
                .and_downcast::<DeviceRowObject>()
                .expect("the item must exist and be a DeviceRowObject")
                .into();

            let focused = list_view.focus_child().unwrap();

            // load the band and display it
            spawn_future_local(async move {
                focused.set_sensitive(false);
                if let Err(err) = win.set_new_band(device).await {
                    win.show_error(&format!("Error while connecting band: {err:?}"));
                }
                focused.set_sensitive(true);
            });
        }));
    }

    async fn show_current_device(&self) -> band::Result<()> {
        if let Some(device) = &*self.imp().current_device.borrow() {
            // display the band address
            self.imp().address_label.set_label(&device.address);
            self.set_all_titles(&format!("{} - Mi Band 4", device.address));
            
            // set everything to loading first
            self.imp().battery_level_label.set_label("Loading...");
            self.imp().last_charged_label.set_label("Loading...");
            self.imp().charging_label.set_visible(false);
            self.imp().current_time_label.set_label("Loading...");

            // Battery
            let battery_status = device.get_battery().await?;
            self.imp().battery_level_label.set_label(&format!("{}%", battery_status.battery_level));
            self.imp().last_charged_label.set_label(&format!("{}", battery_status.last_charge.format("%m/%d/%y %I:%M %p")));
            self.imp().charging_label.set_visible(battery_status.charging);

            // Time
            let current_time = device.get_band_time().await?;
            self.imp().current_time_label.set_label(&format!("{}", current_time.format("%m/%d/%y %I:%M %p")));
        }

        Ok(())
    }

    /// connect to, initialize, and show a new band
    /// disconnects from the old connected band
    async fn set_new_band(&self, device: DiscoveredDevice) -> band::Result<()> {
        // connect to the band and store it
        let mut band = MiBand::from_discovered_device(self.get_session().await?.clone(), device).await?;
        
        band.initialize().await?;
        self.imp().current_device.replace(Some(band));

        // show the device detail page
        self.imp().main_stack.set_visible_child_name("device-detail");
        // show the back button
        self.imp().btn_back.set_visible(true);
        self.show_current_device().await?;
        
        Ok(())
    }

    async fn watch_device_changes(&self, mut shown_devices: HashMap<OwnedObjectPath, DeviceRowObject>) -> band::Result<()> {
        let session = self.get_session().await?;
        
        let device_stream = MiBand::stream_known_bands(session).await?.fuse();
        let scanning_stream = session.adapter.receive_discovering_changed().await.fuse();

        let mut changes = SelectAll::new();
        let mut watched_bands = HashSet::new();

        // watch the initial shown devices
        for path in shown_devices.keys() {
            // start watching this band
            if let Ok(stream) = MiBand::stream_band_events(session, path.clone()).await.map(|s| s.fuse()) {
                changes.push(Box::pin(stream));
                watched_bands.insert(path.clone());
            }
        }

        pin_mut!(device_stream);
        pin_mut!(scanning_stream);
        loop {
            select! {
                e = device_stream.next() => {
                    match e {
                        Some(DiscoveredDeviceEvent::DeviceAdded(device)) => {
                            // if we already have this device, skip the event
                            if shown_devices.contains_key(&device.path) { continue; }
                            
                            let obj: DeviceRowObject = device.clone().into();
                            // add it to the device list
                            self.devices().append(&obj);
                            // add it to our map
                            shown_devices.insert(device.path.clone(), obj);

                            // if we haven't already started watching this one, start
                            if !watched_bands.contains(&device.path) {
                                if let Ok(stream) = MiBand::stream_band_events(&session, device.path.clone()).await.map(|s| s.fuse()) {
                                    changes.push(Box::pin(stream));
                                    watched_bands.insert(device.path);
                                }
                            }
                        },
                        Some(DiscoveredDeviceEvent::DeviceRemoved(path)) => {
                            if let Some(existing_device) = shown_devices.remove(&path) {
                                // find this device in the list and remove it
                                let devices = self.devices();
                                if let Some(idx) = devices.find(&existing_device) {
                                    devices.remove(idx);
                                }
                            }

                            // there's no point trying to stop the `stream_band_events` for this band;
                            // dbus should resume sending us updates when it's found again
                        },
                        None => break
                    };
                },
                e = changes.next() => {
                    match e {
                        Some((path, BandChangeEvent::RSSI(rssi))) => {
                            if let Some(device) = shown_devices.get(&path) {
                                device.set_rssi(rssi.map(|r| r as i32).unwrap_or(0));
                            }
                        },
                        Some((path, BandChangeEvent::Connected(connected))) => {
                            if let Some(device) = shown_devices.get(&path) {
                                device.set_connected(connected);
                            }
                        },
                        // don't break on None
                        None => {}
                    }
                },
                e = scanning_stream.next() => {
                    match e {
                        Some(prop) => {
                            // disable the button when we're scanning
                            let scanning = prop.get().await.unwrap_or(false);
                            self.imp().btn_start_scan.set_sensitive(!scanning);
                        },
                        None => break
                    }
                }
            };
        }
        band::Result::Ok(())
    }

    async fn initialize(&self) -> band::Result<()> {
        let session = self.get_session().await?;
        
        // make sure bluetooth is on
        if !session.adapter.powered().await? { return Ok(()) }

        self.set_page("device-list");

        // initialize devices list
        let model = ListStore::new::<DeviceRowObject>();

        // get currently known devices
        let devices = MiBand::get_known_bands(&session).await?;
        let mut shown_devices = HashMap::new();
        for device in devices.into_iter() {
            let obj: DeviceRowObject = device.clone().into();
            model.append(&obj);
            shown_devices.insert(device.path, obj);
        }

        self.setup_device_list(model);

        // now continually stream changes
        spawn_future_local(clone!(@weak self as win => async move {
            if let Err(err) = win.watch_device_changes(shown_devices).await {
                win.show_error(&format!("Error while watching device changes: {err:?}"));
            }
        }));

        Ok(())
    }

    async fn run_scan(&self) -> band::Result<()> {
        let session = self.get_session().await?;
        // start the scan
        MiBand::start_filtered_discovery(session.clone()).await?;
        // wait for 10 seconds
        Timer::after(Duration::from_secs(10)).await;
        // stop the scan
        session.adapter.stop_discovery().await?;
        Ok(())
    }
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/me/grimsteel/miband4-gtk/window.ui")]
pub struct MiBandWindowImpl {
    #[template_child]
    main_stack: TemplateChild<Stack>,
    #[template_child]
    titlebar_label: TemplateChild<Label>,
    #[template_child]
    btn_back: TemplateChild<Button>,

    // device list page
    #[template_child]
    list_devices: TemplateChild<ListView>,
    #[template_child]
    btn_start_scan: TemplateChild<Button>,

    // device detail page
    #[template_child]
    btn_auth_key: TemplateChild<Button>,

    // battery
    #[template_child]
    address_label: TemplateChild<Label>,
    #[template_child]
    battery_level_label: TemplateChild<Label>,
    #[template_child]
    last_charged_label: TemplateChild<Label>,
    #[template_child]
    charging_label: TemplateChild<Label>,

    // time
    #[template_child]
    current_time_label: TemplateChild<Label>,
    #[template_child]
    btn_sync_time: TemplateChild<Button>,

    // auth key
    #[template_child]
    auth_key_dialog: TemplateChild<AuthKeyDialog>,
    
    devices: RefCell<Option<ListStore>>,
    current_device: RefCell<Option<MiBand<'static>>>,
    session: OnceCell<BluezSession<'static>>
}

#[object_subclass]
impl ObjectSubclass for MiBandWindowImpl {
    const NAME: &'static str = "MiBand4Window";
    type Type = MiBandWindow;
    type ParentType = ApplicationWindow;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
        class.bind_template_instance_callbacks()
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template()
    }
}

impl ObjectImpl for MiBandWindowImpl {
    fn constructed(&self) {
        self.parent_constructed();
        self.main_stack.set_visible_child_name("bluetooth-off");
        
        spawn_future_local(clone!(@weak self as win => async move {
            if let Err(err) = win.obj().initialize().await {
                // TODO: show err
                println!("Uncaught error in window initialization: {err:?}");
                win.obj().close();
            }
        }));
    }
}
impl WidgetImpl for MiBandWindowImpl {}
impl WindowImpl for MiBandWindowImpl {}
impl ApplicationWindowImpl for MiBandWindowImpl {}
