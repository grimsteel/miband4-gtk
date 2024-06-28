use std::{cell::RefCell, collections::{HashMap, HashSet}, rc::Rc};

use futures::{pin_mut, select, stream::SelectAll, StreamExt};
use gtk::{
    gio::{ActionGroup, ActionMap, ListStore}, glib::{self, clone, object_subclass, spawn_future_local, subclass::InitializingObject, Object}, prelude::*, subclass::prelude::*, Accessible, Application, ApplicationWindow, Buildable, Button, CompositeTemplate, ConstraintTarget, ListItem, ListView, Native, NoSelection, Root, ShortcutManager, SignalListItemFactory, Stack, Widget, Window
};

use crate::{band::{self, BandChangeEvent, MiBand}, bluez::{BluezSession, DiscoveredDeviceEvent}};

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

    async fn initialize(&self) -> band::Result<()> {
        // initialize bluez connection
        let session = BluezSession::new().await?;

        // make sure bluetooth is on
        if session.adapter.powered().await? {
            self.set_page("device-list");

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

            // initialize devices list
            let model = ListStore::new::<DeviceRowObject>();

            // get currently known devices
            let devices = MiBand::get_known_bands(session.clone()).await?;
            let mut shown_devices = HashMap::new();
            // stream of changes for all bands we've seen
            let mut changes = SelectAll::new();
            // all band paths that are in the SelectAll above
            let mut watched_bands = HashSet::new();
            for device in devices.into_iter() {
                let obj: DeviceRowObject = device.clone().into();
                model.append(&obj);
                // add it to our devices list
                shown_devices.insert(device.path.clone(), obj);
                // start watching this band
                if let Ok(stream) = MiBand::stream_band_events(&session, &device).await.map(|s| s.fuse()) {
                    changes.push(Box::pin(stream));
                    watched_bands.insert(device.path);
                }
            }

            self.imp().devices.replace(Some(model));
            self.imp().list_devices.set_model(Some(&NoSelection::new(Some(self.devices()))));

            let shown_devices = Rc::new(RefCell::new(shown_devices));

            // now continually stream changes
            spawn_future_local(clone!(@weak self as win, @strong session => async move {
                if let Ok(stream) = MiBand::stream_known_bands(&session).await.map(|s| s.fuse()) {
                    
                    pin_mut!(stream);
                    loop {
                        select! {
                            e = stream.next() => {
                                match e {
                                    Some(DiscoveredDeviceEvent::DeviceAdded(device)) => {
                                        // if we already have this device, skip the event
                                        if shown_devices.borrow().contains_key(&device.path) { return; }
                                        
                                        let obj: DeviceRowObject = device.clone().into();
                                        // add it to the device list
                                        win.devices().append(&obj);
                                        // add it to our map
                                        shown_devices.borrow_mut().insert(device.path.clone(), obj);

                                        // if we haven't already started watching this one, start
                                        if !watched_bands.contains(&device.path) {
                                            if let Ok(stream) = MiBand::stream_band_events(&session, &device).await.map(|s| s.fuse()) {
                                                changes.push(Box::pin(stream));
                                                watched_bands.insert(device.path);
                                            }
                                        }
                                    },
                                    Some(DiscoveredDeviceEvent::DeviceRemoved(path)) => {
                                        if let Some(existing_device) = shown_devices.borrow_mut().remove(&path) {
                                            // find this device in the list and remove it
                                            let devices = win.devices();
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
                                        if let Some(device) = shown_devices.borrow().get(&path) {
                                            device.set_rssi(rssi.map(|r| r as i32).unwrap_or(0));
                                        }
                                    },
                                    Some((path, BandChangeEvent::Connected(connected))) => {
                                        if let Some(device) = shown_devices.borrow().get(&path) {
                                            device.set_connected(connected);
                                        }
                                    },
                                    // don't break on None
                                    None => {}
                                }
                            }
                        };
                    }
                }
            }));
        }

        Ok(())
    }
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/me/grimsteel/miband4-gtk/window.ui")]
pub struct MiBandWindowImpl {
    #[template_child]
    btn_start_scan: TemplateChild<Button>,
    #[template_child]
    main_stack: TemplateChild<Stack>,
    #[template_child]
    list_devices: TemplateChild<ListView>,
    devices: RefCell<Option<ListStore>>

    //bluez_session: RefCell<Option<BluezSession<'static>>>
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
