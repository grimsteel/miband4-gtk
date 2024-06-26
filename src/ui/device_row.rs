use std::cell::RefCell;

use gtk::{glib::{self, subclass::InitializingObject, Binding, Object, Properties}, prelude::*, subclass::prelude::*, Accessible, Box as GtkBox, Buildable, CompositeTemplate, ConstraintTarget, Label, Orientable, Widget};

glib::wrapper! {
    pub struct DeviceRowObject(ObjectSubclass<DeviceRowObjectImpl>);
}

glib::wrapper! {
    pub struct DeviceRow(ObjectSubclass<DeviceRowImpl>)
        @extends GtkBox, Widget,
        @implements Accessible, Buildable, ConstraintTarget, Orientable;
}

impl DeviceRowObject {
    pub fn new(address: String, connected: bool, rssi: Option<i32>) -> Self {
        Object::builder()
            .property("address", address)
            .property("connected", connected)
            // todo: don't defaul to 0
            .property("rssi", rssi.unwrap_or(0))
            .build()
    }
}

impl DeviceRow {
    pub fn new() -> Self {
        Object::builder().build()
    }

    pub fn bind(&self, obj: &DeviceRowObject) {
        let mut bindings = self.imp().bindings.borrow_mut();

        let address_label = self.imp().address_label.get();

        // bind the address
        let address_binding = obj
            .bind_property("address", &address_label, "label")
            .sync_create()
            .build();

        bindings.push(address_binding);
    }

    pub fn unbind(&self) {
        // unbind all and remove from the vector
        for binding in self.imp().bindings.borrow_mut().drain(..) {
            binding.unbind();
        }
    }
}

#[derive(Default)]
pub struct DeviceRowData {
    pub address: String,
    pub connected: bool,
    pub rssi: i32
}

// Implementation:

#[derive(Properties, Default)]
#[properties(wrapper_type = DeviceRowObject)]
pub struct DeviceRowObjectImpl {
    #[property(name = "address", get, set, type = String, member = address)]
    #[property(name = "connected", get, set, type = bool, member = connected)]
    #[property(name = "rssi", get, set, type = i32, member = rssi)]
    pub data: RefCell<DeviceRowData>
}

#[glib::object_subclass]
impl ObjectSubclass for DeviceRowObjectImpl {
    const NAME: &'static str = "MiBand4DeviceRowObject";
    type Type = DeviceRowObject;
}

#[glib::derived_properties]
impl ObjectImpl for DeviceRowObjectImpl {}

#[derive(Default, CompositeTemplate)]
#[template(resource = "/me/grimsteel/miband4-gtk/device_row.ui")]
pub struct DeviceRowImpl {
    #[template_child]
    pub address_label: TemplateChild<Label>,
    pub bindings: RefCell<Vec<Binding>>
}

#[glib::object_subclass]
impl ObjectSubclass for DeviceRowImpl {
    const NAME: &'static str = "MiBand4DeviceRow";
    type Type = DeviceRow;
    type ParentType = GtkBox;

    fn class_init(klass: &mut Self::Class) {
        klass.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for DeviceRowImpl {}
impl WidgetImpl for DeviceRowImpl {}
impl BoxImpl for DeviceRowImpl {}
