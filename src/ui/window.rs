use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::StreamExt;
use gtk::{
    gio::{ActionGroup, ActionMap, ListStore}, glib::{self, clone, object_subclass, spawn_future_local, subclass::InitializingObject, Object}, prelude::*, subclass::prelude::*, Accessible, Application, ApplicationWindow, Buildable, Button, CompositeTemplate, ConstraintTarget, ListItem, ListView, Native, NoSelection, Root, ShortcutManager, SignalListItemFactory, Stack, Widget, Window
};

use crate::{band::{self, MiBand}, bluez::{BluezSession, DiscoveredDeviceEvent}};

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


            // initialize devices list
            let model = ListStore::new::<DeviceRowObject>();

            // get currently known devices
            let devices = MiBand::get_known_bands(session.clone()).await?;
            let mut shown_devices = HashMap::new();
            for device in devices.into_iter() {
                let path = device.path.clone();
                let obj: DeviceRowObject = device.into();
                model.append(&obj);
                // add it to our devices list
                shown_devices.insert(path, obj);
            }

            self.imp().devices.replace(Some(model));
            self.imp().list_devices.set_model(Some(&NoSelection::new(Some(self.devices()))));

            let shown_devices = Rc::new(RefCell::new(shown_devices));
                                                                  
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

            // now continually stream changes
            MiBand::stream_band_changes(&session).await?.for_each(|e| {
                let shown_devices = shown_devices.clone();
                async move {
                    match e {
                        DiscoveredDeviceEvent::DeviceAdded(device) => {
                            let path = device.path.clone();
                            let obj: DeviceRowObject = device.into();
                            self.devices().append(&obj);
                            shown_devices.borrow_mut().insert(path, obj);
                        },
                        DiscoveredDeviceEvent::DeviceRemoved(path) => {
                            if let Some(existing_device) = shown_devices.borrow_mut().remove(&path) {
                                // find this device in the list and remove it
                                let devices = self.devices();
                                if let Some(idx) = devices.find(&existing_device) {
                                    devices.remove(idx);
                                }
                            }
                        }
                    };
                }
            }).await;
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
