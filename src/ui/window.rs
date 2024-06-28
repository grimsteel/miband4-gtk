use std::{cell::RefCell, collections::{HashMap, HashSet}, time::Duration};

use async_io::Timer;
use async_lock::OnceCell;
use futures::{pin_mut, select, stream::SelectAll, StreamExt};
use gtk::{
    gio::{ActionGroup, ActionMap, ListStore}, glib::{self, clone, object_subclass, spawn_future_local, subclass::InitializingObject, Object}, prelude::*, subclass::prelude::*, Accessible, Application, ApplicationWindow, Buildable, Button, CompositeTemplate, ConstraintTarget, Label, ListItem, ListView, Native, NoSelection, Root, ShortcutManager, SignalListItemFactory, Stack, Widget, Window
};
use zbus::zvariant::OwnedObjectPath;

use crate::{band::{self, BandChangeEvent, MiBand}, bluez::{BluezSession, DiscoveredDevice, DiscoveredDeviceEvent}};

use super::{device_row::DeviceRow, device_row_object::DeviceRowObject};

glib::wrapper! {
    pub struct MiBandWindow(ObjectSubclass<MiBandWindowImpl>)
        // refer to https://docs.gtk.org/gtk4/class.ApplicationWindow.html#hierarchy
        @extends ApplicationWindow, Window, Widget,
        @implements ActionGroup, ActionMap, Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager;
}

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
            let model = list_view.model().expect("the model must not be None at this point");
            
            let device: DiscoveredDevice = model
                .item(idx)
                .and_downcast::<DeviceRowObject>()
                .expect("the item must exist and be a DeviceRowObject")
                .into();

            let focused = list_view.focus_child().unwrap();

            spawn_future_local(async move {
                focused.set_sensitive(false);
                if let Err(err) = win.set_new_band(device).await {
                    println!("Error while connecting band: {err:?}");
                }
                focused.set_sensitive(true);
            });
        }));
    }

    async fn show_current_device(&self) -> band::Result<()> {
        if let Some(device) = &*self.imp().current_device.borrow() {
            self.imp().address_label.set_label(&device.address);
            // set everything to loading first
            self.imp().battery_level_label.set_label("Loading...");
            
            let battery_status = device.get_battery().await?;
            self.imp().battery_level_label.set_label(&format!("{}%", battery_status.battery_level));
        }

        Ok(())
    }

    /// connect to, initialize, and show a new band
    /// disconnects from the old connected band
    async fn set_new_band(&self, device: DiscoveredDevice) -> band::Result<()> {
        {
            let mut current_band = self.imp().current_device.borrow_mut();
            // disconnect the current band if it's connected
            if let Some(band) = current_band.as_mut() {
                if band.is_connected().await {
                    band.disconnect().await?;
                }
            }
            // connect to the band and store it
            let mut band = MiBand::from_discovered_device(self.get_session().await?.clone(), device).await?;
            
            band.initialize().await?;
            current_band.replace(band);
        }

        // show the device detail page
        self.imp().main_stack.set_visible_child_name("device-detail");
        self.show_current_device().await?;
        
        Ok(())
    }

    async fn get_session(&self) -> band::Result<&BluezSession<'static>> {
        Ok(self.imp().session.get_or_try_init(|| async {
            BluezSession::new().await
        }).await?)
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

        // scan button
        self.imp().btn_start_scan.connect_clicked({
            let session = session.clone();
            move |_| {
                let session = session.clone();
                spawn_future_local(async move {
                    // start the scan
                    if let Err(err) = MiBand::start_filtered_discovery(session.clone()).await {
                        println!("An error occurred while starting discovery: {err:?}");
                        return;
                    }
                    // wait for 10 seconds
                    Timer::after(Duration::from_secs(10)).await;
                    // stop the scan
                    if let Err(err) = session.adapter.stop_discovery().await {
                        println!("An error occurred while stopping discovery: {err:?}");
                        return;
                    }
                });
            }
        });

        // now continually stream changes
        spawn_future_local(clone!(@weak self as win => async move {
            if let Err(err) = win.watch_device_changes(shown_devices).await {
                println!("Error while watching device changes: {err:?}");
            }
        }));

        Ok(())
    }
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/me/grimsteel/miband4-gtk/window.ui")]
pub struct MiBandWindowImpl {
    #[template_child]
    main_stack: TemplateChild<Stack>,

    // device list page
    #[template_child]
    btn_start_scan: TemplateChild<Button>,
    #[template_child]
    list_devices: TemplateChild<ListView>,

    // device detail page
    #[template_child]
    address_label: TemplateChild<Label>,
    #[template_child]
    battery_level_label: TemplateChild<Label>,
    
    devices: RefCell<Option<ListStore>>,
    current_device: RefCell<Option<MiBand<'static>>>,
    session: OnceCell<BluezSession<'static>>
}

#[object_subclass]
impl ObjectSubclass for MiBandWindowImpl {
    const NAME: &'static str = "MiBand4Window";
    type Type = MiBandWindow;
    type ParentType = ApplicationWindow;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
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
