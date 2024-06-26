use gtk::{
    gio::{ActionGroup, ActionMap},
    glib::{self, object_subclass, subclass::InitializingObject, Object},
    prelude::*,
    subclass::prelude::*,
    Accessible, Application, ApplicationWindow, Buildable, Button, CompositeTemplate,
    ConstraintTarget, Native, Root, ShortcutManager, Widget, Window,
};

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
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/me/grimsteel/miband4-gtk/window.ui")]
pub struct MiBandWindowImpl {
    #[template_child]
    pub btn_start_scan: TemplateChild<Button>,
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
        self.btn_start_scan.connect_clicked(move |button| {
            button.set_label("hi");
        });
    }
}
impl WidgetImpl for MiBandWindowImpl {}
impl WindowImpl for MiBandWindowImpl {}
impl ApplicationWindowImpl for MiBandWindowImpl {}
