use std::cell::RefCell;

use gtk::{
    gio::{ActionGroup, ActionMap}, glib::{self, clone, object_subclass, spawn_future_local, subclass::InitializingObject, Object}, prelude::*, subclass::prelude::*, Accessible, Application, ApplicationWindow, Buildable, Button, CompositeTemplate, ConstraintTarget, Label, ListBox, Native, Root, ShortcutManager, Stack, Widget, Window
};

use crate::{band::{self, MiBand}, bluez::BluezSession};

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

    async fn initialize(&self) -> band::Result<()> {
        // initialize bluez connection
        let session = BluezSession::new().await?;

        // make sure bluetooth is on
        if session.adapter.powered().await? {
            self.set_page("device-list");

            // get currently known devices
            let devices = MiBand::get_known_bands(session.clone()).await?;
            for device in devices.iter() {
                let label = Label::new(Some(&device.address));
                self.imp().list_devices.append(&label);
            }
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
    list_devices: TemplateChild<ListBox>,

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
